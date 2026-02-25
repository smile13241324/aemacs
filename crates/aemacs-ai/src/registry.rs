use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use notify::{Watcher, RecursiveMode, EventKind};
use crate::error::AIError;
use crate::persona::Persona;
use tracing::{info, warn, error};

/// The PersonaRegistry indexes all available agents and watches for changes in the agents directory.
pub struct PersonaRegistry {
    personas: RwLock<HashMap<String, Persona>>,
    agents_dir: PathBuf,
}

impl PersonaRegistry {
    /// Create a new registry and initialize the agents directory.
    pub async fn new() -> Result<Arc<Self>, AIError> {
        let home = dirs::home_dir().ok_or_else(|| AIError::ConfigError("Could not find home directory".into()))?;
        let agents_dir = home.join(".aemacs").join("agents");
        
        if !agents_dir.exists() {
            std::fs::create_dir_all(&agents_dir)?;
            info!("Created agents directory: {:?}", agents_dir);
        }

        let registry = Arc::new(Self {
            personas: RwLock::new(HashMap::new()),
            agents_dir,
        });

        // Initial load
        registry.load_all().await?;
        
        Ok(registry)
    }

    /// Load or reload all personas from the agents directory.
    pub async fn load_all(&self) -> Result<(), AIError> {
        let mut new_personas = HashMap::new();
        let entries = std::fs::read_dir(&self.agents_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            match Persona::from_yaml(&content) {
                                Ok(mut persona) => {
                                    persona.name = name.to_string();
                                    new_personas.insert(name.to_string(), persona);
                                    info!("Loaded persona: {}", name);
                                }
                                Err(e) => warn!("Failed to parse persona YAML at {:?}: {}", path, e),
                            }
                        }
                        Err(e) => warn!("Failed to read persona file at {:?}: {}", path, e),
                    }
                }
            }
        }

        let mut lock = self.personas.write().await;
        *lock = new_personas;
        Ok(())
    }

    /// Retrieve a persona by name.
    pub async fn get_persona(&self, name: &str) -> Option<Persona> {
        let lock = self.personas.read().await;
        lock.get(name).cloned()
    }

    /// List all available personas.
    pub async fn list_personas(&self) -> Vec<String> {
        let lock = self.personas.read().await;
        lock.keys().cloned().collect()
    }

    /// Start watching the agents directory for changes.
    pub fn start_watching(self: Arc<Self>) -> Result<(), AIError> {
        let registry = self.clone();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        let _ = tx.blocking_send(());
                    }
                    _ => {}
                }
            }
        }).map_err(|e| AIError::Persona(format!("Failed to create watcher: {}", e)))?;

        watcher.watch(&self.agents_dir, RecursiveMode::NonRecursive)
            .map_err(|e| AIError::Persona(format!("Failed to watch directory: {}", e)))?;

        // Keep the watcher alive in a background task
        tokio::spawn(async move {
            // Keep a reference to the watcher so it doesn't get dropped
            let _watcher = watcher;
            while let Some(_) = rx.recv().await {
                info!("Agents directory changed, reloading personas...");
                if let Err(e) = registry.load_all().await {
                    error!("Failed to reload personas: {}", e);
                }
            }
        });

        Ok(())
    }
}
