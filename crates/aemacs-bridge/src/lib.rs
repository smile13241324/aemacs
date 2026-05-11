use std::sync::Arc;

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
/// Initializes the component.
///
/// # Errors
/// Returns an error if the initialization fails.
pub fn init() -> Result<()> {
    info!("🌉 [BRIDGE] Connecting to Scripting Engines...");

    Python::attach(|py| -> PyResult<()> {
        let sys = py.import("sys")?;
        let version: String = sys.getattr("version")?.extract()?;

        info!("🐍 [BRIDGE] Python Runtime attached successfully!");

        let short_version = version.split_whitespace().next().unwrap_or("Unknown");
        info!("🐍 [BRIDGE] Version: {short_version}");

        Ok(())
    })
    .map_err(|e| anyhow::anyhow!("Python Init Failed: {e}"))?;

    Ok(())
}

/// Spawns an HTTP bridge that listens for external intents.
/// This server is protected by a randomly generated API key.
/// # Errors
/// Returns an error if the server fails to bind to the port.
pub async fn spawn_intent_bridge(bus: EventBus, port: u16) -> Result<()> {
    let api_key = Uuid::new_v4().to_string();

    // Holy Edict: Print the API key once to the terminal for the Citizen to see.
    println!("\n🌉 [BRIDGE] External Intent Gateway Opening...");
    println!("🔑 [BRIDGE] SECURITY KEY: {api_key}");
    println!("📡 [BRIDGE] Listening on: http://0.0.0.0:{port}/intent\n");

    spawn_intent_bridge_internal(bus, port, api_key).await
}

async fn spawn_intent_bridge_internal(bus: EventBus, port: u16, api_key: String) -> Result<()> {
    let state = Arc::new(BridgeState { bus, api_key: api_key.clone() });

    let app = Router::new().route("/intent", post(handle_intent)).with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            warn!("🌉 [BRIDGE] Intent Gateway collapsed: {e}");
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
        return (StatusCode::UNAUTHORIZED, "Unauthorized: Invalid API Key".to_string());
    }

    // 2. Transduce Signal to the Forge
    let payload = match serde_json::to_string(&intent) {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialization failed: {e}"));
        },
    };

    let event = SystemEvent::Signal {
        source: format!("Bridge:{}", intent.source),
        event_type: intent.intent_type,
        payload,
    };

    if state.bus.tx.send(event).is_err() {
        return (StatusCode::SERVICE_UNAVAILABLE, "Event Bus disconnected".to_string());
    }

    info!("🌉 [BRIDGE] Injected intent from source: {}", intent.source);
    (StatusCode::OK, "Intent accepted".to_string())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use aemacs_core::bus::{EventBus, SystemEvent};
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_webhook_handshake_quest() -> Result<()> {
        let bus = EventBus::new();
        let api_key = "test-sacred-key".to_string();

        // 1. Forge the Bridge state manually to get the assigned port
        let state = Arc::new(BridgeState { bus: bus.clone(), api_key: api_key.clone() });

        let app = Router::new().route("/intent", post(handle_intent)).with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let port = addr.port();

        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{port}/intent");

        let valid_intent = ExternalIntent {
            source: "TestRunner".to_string(),
            intent_type: "ManualTrigger".to_string(),
            data: json!({ "command": "deploy" }),
        };

        // 2. Quest: Unauthorized Joust (No Key)
        let res = client.post(&url).json(&valid_intent).send().await?;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 3. Quest: Unauthorized Joust (Wrong Key)
        let res =
            client.post(&url).header("X-API-KEY", "wrong-key").json(&valid_intent).send().await?;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 4. Quest: Successful Handshake
        let mut rx = bus.subscribe();

        let res =
            client.post(&url).header("X-API-KEY", &api_key).json(&valid_intent).send().await?;

        assert_eq!(res.status(), StatusCode::OK);

        // 5. Verify signal reached the bus
        let event = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await??;

        match event {
            SystemEvent::Signal { source, event_type, payload } => {
                assert_eq!(source, "Bridge:TestRunner");
                assert_eq!(event_type, "ManualTrigger");
                assert!(payload.contains("deploy"));
            },
            _ => {
                return Err(anyhow::anyhow!("Unexpected event type on bus: {event:?}"));
            },
        }

        Ok(())
    }
}
