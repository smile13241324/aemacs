use crate::error::AIError;
use crate::persona::Persona;
use notify::{EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// The PersonaRegistry indexes all available agents and watches for changes in the agents directory.
/// It provides a centralized point for retrieving and hot-reloading agent personas.
pub struct PersonaRegistry {
    /// Thread-safe storage for loaded personas, keyed by their internal name.
    personas: RwLock<HashMap<String, Persona>>,
    /// The directory containing system-wide global agent definitions.
    global_agents_dir: Option<PathBuf>,
    /// The directory containing project-specific local agent definitions.
    local_agents_dir: Option<PathBuf>,
    /// A handle to the Tokio runtime for spawning background watch tasks.
    runtime_handle: tokio::runtime::Handle,
}

impl PersonaRegistry {
    /// Create a new registry and initialize the agents directory.
    /// It automatically scans for global and local agent definitions.
    pub async fn new(runtime_handle: tokio::runtime::Handle) -> Result<Arc<Self>, AIError> {
        // Global Directory
        let global_agents_dir = dirs::home_dir().map(|home| home.join(".aemacs").join("agents"));

        if let Some(ref dir) = global_agents_dir {
            if !dir.exists() {
                // Silently ignore failure to create global dir, it's optional
                let _ = std::fs::create_dir_all(dir);
            }
        }

        // Local Directory
        let local_agents_dir = match std::env::current_dir() {
            Ok(cwd) => {
                let dir = cwd.join(".aemacs").join("agents");
                if !dir.exists() {
                    let _ = std::fs::create_dir_all(&dir);
                }
                Some(dir)
            }
            Err(_) => None,
        };

        let registry = Arc::new(Self {
            personas: RwLock::new(HashMap::new()),
            global_agents_dir,
            local_agents_dir,
            runtime_handle,
        });

        // Initial load
        registry.load_all().await?;

        Ok(registry)
    }

    /// Load or reload all personas from the agents directory.
    /// Local personas will overwrite global personas with the same name.
    pub async fn load_all(&self) -> Result<(), AIError> {
        let mut new_personas = HashMap::new();

        // Load Global Personas First (The Foundation)
        if let Some(ref global_dir) = self.global_agents_dir {
            self.load_from_directory(global_dir, &mut new_personas);
        }

        // Load Local Personas Second (The Overwrite)
        if let Some(ref local_dir) = self.local_agents_dir {
            self.load_from_directory(local_dir, &mut new_personas);
        }

        let mut lock = self.personas.write().await;
        *lock = new_personas;
        Ok(())
    }

    /// Internal helper to load all valid YAML persona files from a given directory.
    fn load_from_directory(&self, dir: &PathBuf, new_personas: &mut HashMap<String, Persona>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        match std::fs::read_to_string(&path) {
                            Ok(content) => match Persona::from_yaml(&content) {
                                Ok(mut persona) => {
                                    persona.name = name.to_string();
                                    // This will automatically overwrite any global persona with the same name
                                    new_personas.insert(name.to_string(), persona);
                                    info!("Loaded persona: {} from {:?}", name, dir);
                                }
                                Err(e) => {
                                    warn!("Failed to parse persona YAML at {:?}: {}", path, e)
                                }
                            },
                            Err(e) => warn!("Failed to read persona file at {:?}: {}", path, e),
                        }
                    }
                }
            }
        }
    }

    /// Retrieve a cloned instance of a persona by name.
    pub async fn get_persona(&self, name: &str) -> Option<Persona> {
        let lock = self.personas.read().await;
        lock.get(name).cloned()
    }

    /// Returns a list of all currently loaded persona names.
    pub async fn list_personas(&self) -> Vec<String> {
        let lock = self.personas.read().await;
        lock.keys().cloned().collect()
    }

    /// Start watching the agents directory for changes.
    /// Whenever a YAML file is modified, created, or removed, the registry reloads its content.
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
        })
        .map_err(|e| AIError::Persona(format!("Failed to create watcher: {}", e)))?;

        if let Some(ref dir) = self.global_agents_dir {
            if dir.exists() {
                let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
            }
        }

        if let Some(ref dir) = self.local_agents_dir {
            if dir.exists() {
                let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
            }
        }

        // Keep the watcher alive in a background task
        self.runtime_handle.spawn(async move {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persona::Persona;
    use std::fs;

    #[tokio::test]
    async fn test_persona_registry_global_missing_quest() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let local_agents_dir = temp_dir.path().join("local_agents");
        fs::create_dir_all(&local_agents_dir)?;

        let test_persona = Persona::new("local-only", "Local Tester", "Local prompt", None);
        fs::write(
            local_agents_dir.join("local-only.yaml"),
            serde_yaml::to_string(&test_persona)?,
        )?;

        // Manually instantiate to simulate a missing global directory
        let registry = Arc::new(PersonaRegistry {
            personas: RwLock::new(HashMap::new()),
            global_agents_dir: None,
            local_agents_dir: Some(local_agents_dir),
            runtime_handle: tokio::runtime::Handle::current(),
        });

        registry.load_all().await?;

        let personas = registry.list_personas().await;
        assert!(
            personas.contains(&"local-only".to_string()),
            "Local persona should load even if global directory is missing!"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_persona_registry_override_quest() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let global_agents_dir = temp_dir.path().join("global_agents");
        let local_agents_dir = temp_dir.path().join("local_agents");
        fs::create_dir_all(&global_agents_dir)?;
        fs::create_dir_all(&local_agents_dir)?;

        // The Global Fiend
        let global_persona = Persona::new("test-agent", "Global Tester", "I am global.", None);
        fs::write(
            global_agents_dir.join("test-agent.yaml"),
            serde_yaml::to_string(&global_persona)?,
        )?;

        // The Local Hero
        let local_persona = Persona::new("test-agent", "Local Tester", "I am local.", None);
        fs::write(
            local_agents_dir.join("test-agent.yaml"),
            serde_yaml::to_string(&local_persona)?,
        )?;

        let registry = Arc::new(PersonaRegistry {
            personas: RwLock::new(HashMap::new()),
            global_agents_dir: Some(global_agents_dir),
            local_agents_dir: Some(local_agents_dir),
            runtime_handle: tokio::runtime::Handle::current(),
        });

        registry.load_all().await?;

        let loaded_persona = registry
            .get_persona("test-agent")
            .await
            .expect("Hark! The agent was not loaded!");

        assert_eq!(
            loaded_persona.description, "Local Tester",
            "The Local Shield failed to block the Global Fiend! Override unsuccessful!"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_persona_registry_hot_reload_quest() -> anyhow::Result<()> {
        // QUEST: Verify the 'Hot-Reloading Ritual'.
        let temp_dir = tempfile::tempdir()?;
        let agents_dir = temp_dir.path().join("agents");
        fs::create_dir_all(&agents_dir)?;

        // 1. Initial Inscription
        let persona_path = agents_dir.join("bob.yaml");
        let mut persona = Persona::new("bob", "The Architect", "Build well.", None);
        fs::write(&persona_path, serde_yaml::to_string(&persona)?)?;

        let registry = Arc::new(PersonaRegistry {
            personas: RwLock::new(HashMap::new()),
            global_agents_dir: None,
            local_agents_dir: Some(agents_dir),
            runtime_handle: tokio::runtime::Handle::current(),
        });

        registry.load_all().await?;
        assert_eq!(
            registry.get_persona("bob").await.unwrap().description,
            "The Architect"
        );

        // 2. Start the Vigil
        registry.clone().start_watching()?;

        // 3. The Evolution
        persona.description = "The Overworked Doubter".to_string();
        fs::write(&persona_path, serde_yaml::to_string(&persona)?)?;

        // 4. Validation (Wait for reload with timeout)
        let mut reloaded = false;
        for _ in 0..20 {
            // 2 seconds total
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            if let Some(p) = registry.get_persona("bob").await {
                if p.description == "The Overworked Doubter" {
                    reloaded = true;
                    break;
                }
            }
        }

        assert!(
            reloaded,
            "The 'Watcher's-Echo' failed! Registry did not reload the persona after disk modification."
        );

        Ok(())
    }
}
