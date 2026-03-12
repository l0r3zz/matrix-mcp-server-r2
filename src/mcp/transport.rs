use super::server::{ServerState, ToolRequest, ToolResponse};
use axum::{
    extract::{Query, State},
    response::sse::{Event as SseEvent, Sse},
};
use serde::Deserialize;
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info};

/// SSE query parameters
#[derive(Debug, Deserialize)]
pub struct SseQuery {
    pub session_id: Option<String>,
}

/// Handle A2A (Agent-to-Agent) endpoint
pub async fn handle_a2a(State(state): State<ServerState>) -> impl axum::response::IntoResponse {
    axum::Json(serde_json::json!({
        "protocol": "A2A-v1",
        "capabilities": {
            "streaming": true,
            "tools": true,
            "multiModal": false,
        },
        "authentication": {
            "required": state.config.mcp_api_key.is_some(),
        },
    }))
}

/// Handle SSE connections for streaming tool calls
pub async fn handle_sse(
    State(_state): State<ServerState>,
    Query(query): Query<SseQuery>,
) -> Sse<ReceiverStream<Result<SseEvent, Infallible>>> {
    let session_id = query.session_id.unwrap_or_else(|| {
        format!("sess-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis())
    });
    
    info!("New SSE connection established: {}", session_id);
    
    let (tx, rx) = mpsc::channel::<Result<SseEvent, Infallible>>(100);
    
    tokio::spawn(async move {
        // Send initial connection event
        let init_event = SseEvent::default()
            .event("connected")
            .data(serde_json::json!({ "session_id": &session_id }).to_string());
        let _ = tx.send(Ok(init_event)).await;
        
        // Keep connection alive with heartbeats
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let heartbeat = SseEvent::default()
                        .event("heartbeat")
                        .data("{}");
                    if tx.send(Ok(heartbeat)).await.is_err() {
                        break;
                    }
                }
            }
        }
        
        debug!("SSE connection closed: {}", session_id);
    });
    
    Sse::new(ReceiverStream::new(rx))
}
