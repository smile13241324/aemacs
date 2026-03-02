use aemacs_core::bus::{EventBus, SystemEvent};
use anyhow::Result;
use axum::{
    Router,
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use log::{info, warn};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Represents an external intent received from the outside world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalIntent {
    pub source: String,
    pub intent_type: String,
    pub data: serde_json::Value,
}

#[derive(Clone)]
struct BridgeState {
    bus: EventBus,
    api_key: String,
}

/// Initializes the scripting bridge and verifies the Python connection.
pub fn init() -> Result<()> {
    info!("🌉 [BRIDGE] Connecting to Scripting Engines...");

    Python::attach(|py| -> PyResult<()> {
        let sys = py.import("sys")?;
        let version: String = sys.getattr("version")?.extract()?;

        info!("🐍 [BRIDGE] Python Runtime attached successfully!");

        let short_version = version.split_whitespace().next().unwrap_or("Unknown");
        info!("🐍 [BRIDGE] Version: {}", short_version);

        Ok(())
    })
    .map_err(|e| anyhow::anyhow!("Python Init Failed: {}", e))?;

    Ok(())
}

/// Spawns an HTTP bridge that listens for external intents.
/// This server is protected by a randomly generated API key.
pub async fn spawn_intent_bridge(bus: EventBus, port: u16) -> Result<()> {
    let api_key = Uuid::new_v4().to_string();

    // Holy Edict: Print the API key once to the terminal for the Citizen to see.
    println!("\n🌉 [BRIDGE] External Intent Gateway Opening...");
    println!("🔑 [BRIDGE] SECURITY KEY: {}", api_key);
    println!("📡 [BRIDGE] Listening on: http://0.0.0.0:{}/intent\n", port);

    let state = Arc::new(BridgeState { bus, api_key });

    let app = Router::new()
        .route("/intent", post(handle_intent))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            warn!("🌉 [BRIDGE] Intent Gateway collapsed: {}", e);
        }
    });

    Ok(())
}

async fn handle_intent(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(intent): Json<ExternalIntent>,
) -> (StatusCode, String) {
    // 1. Verify Authentication
    let auth_header = headers.get("X-API-KEY").and_then(|h| h.to_str().ok());

    if auth_header != Some(&state.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            "Unauthorized: Invalid API Key".to_string(),
        );
    }

    // 2. Transduce Signal to the Forge
    let payload = match serde_json::to_string(&intent) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Serialization failed: {}", e),
            );
        }
    };

    let event = SystemEvent::Signal {
        source: format!("Bridge:{}", intent.source),
        event_type: intent.intent_type,
        payload,
    };

    if state.bus.tx.send(event).await.is_err() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Event Bus disconnected".to_string(),
        );
    }

    info!("🌉 [BRIDGE] Injected intent from source: {}", intent.source);
    (StatusCode::OK, "Intent accepted".to_string())
}
