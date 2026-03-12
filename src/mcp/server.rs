use crate::config::Config;
use crate::error::{AppError, Result};
use crate::matrix::MatrixClient;
use crate::tools::{Tool, tier0, tier1};
use axum::{
    Router,
    routing::get,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response, Sse},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, warn};

/// MCP Server state shared across handlers
#[derive(Clone)]
pub struct ServerState {
    pub config: Config,
    pub matrix_client: MatrixClient,
    pub tools: Arc<RwLock<HashMap<String, Box<dyn Tool>>>>,
}

/// Matrix MCP Server implementation
pub struct MatrixMcpServer {
    state: ServerState,
}

impl MatrixMcpServer {
    /// Create a new MCP server
    pub async fn new(config: Config) -> Result<Self> {
        // Validate configuration
        config.validate()?;
        
        // Create Matrix client
        let matrix_client = MatrixClient::new(config.clone()).await?;
        
        // Initialize tools registry
        let tools: Arc<RwLock<HashMap<String, Box<dyn Tool>>>> = Arc::new(RwLock::new(HashMap::new()));
        
        // Register all tools
        {
            let mut tools_lock = tools.write().await;
            
            // Tier 0 tools
            tools_lock.insert(
                "list-joined-rooms".to_string(),
                Box::new(tier0::list_joined_rooms::ListJoinedRooms),
            );
            tools_lock.insert(
                "get-room-info".to_string(),
                Box::new(tier0::get_room_info::GetRoomInfo),
            );
            tools_lock.insert(
                "get-room-members".to_string(),
                Box::new(tier0::get_room_members::GetRoomMembers),
            );
            tools_lock.insert(
                "get-room-messages".to_string(),
                Box::new(tier0::get_room_messages::GetRoomMessages),
            );
            tools_lock.insert(
                "get-messages-by-date".to_string(),
                Box::new(tier0::get_messages_by_date::GetMessagesByDate),
            );
            tools_lock.insert(
                "identify-active-users".to_string(),
                Box::new(tier0::identify_active_users::IdentifyActiveUsers),
            );
            tools_lock.insert(
                "get-user-profile".to_string(),
                Box::new(tier0::get_user_profile::GetUserProfile),
            );
            tools_lock.insert(
                "get-my-profile".to_string(),
                Box::new(tier0::get_my_profile::GetMyProfile),
            );
            tools_lock.insert(
                "get-all-users".to_string(),
                Box::new(tier0::get_all_users::GetAllUsers),
            );
            tools_lock.insert(
                "search-public-rooms".to_string(),
                Box::new(tier0::search_public_rooms::SearchPublicRooms),
            );
            tools_lock.insert(
                "get-notification-counts".to_string(),
                Box::new(tier0::get_notification_counts::GetNotificationCounts),
            );
            tools_lock.insert(
                "get-direct-messages".to_string(),
                Box::new(tier0::get_direct_messages::GetDirectMessages),
            );
            
            // Tier 1 tools
            tools_lock.insert(
                "send-message".to_string(),
                Box::new(tier1::send_message::SendMessage),
            );
            tools_lock.insert(
                "send-direct-message".to_string(),
                Box::new(tier1::send_direct_message::SendDirectMessage),
            );
            tools_lock.insert(
                "join-room".to_string(),
                Box::new(tier1::join_room::JoinRoom),
            );
            tools_lock.insert(
                "leave-room".to_string(),
                Box::new(tier1::leave_room::LeaveRoom),
            );
            tools_lock.insert(
                "create-room".to_string(),
                Box::new(tier1::create_room::CreateRoom),
            );
            tools_lock.insert(
                "invite-user".to_string(),
                Box::new(tier1::invite_user::InviteUser),
            );
            tools_lock.insert(
                "set-room-name".to_string(),
                Box::new(tier1::set_room_name::SetRoomName),
            );
            tools_lock.insert(
                "set-room-topic".to_string(),
                Box::new(tier1::set_room_topic::SetRoomTopic),
            );
        }
        
        let state = ServerState {
            config,
            matrix_client,
            tools,
        };
        
        Ok(Self { state })
    }
    
    /// Run the MCP server
    pub async fn run(&self) -> Result<()> {
        let app = Router::new()
            .route("/a2a", get(super::transport::handle_a2a))
            .route("/tools", get(handle_list_tools))
            .route("/sse", get(super::transport::handle_sse))
            .route("/health", get(handle_health))
            .layer(tower_http::trace::TraceLayer::new_for_http())
            .with_state(self.state.clone());
        
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.state.config.mcp_port))
            .await
            .map_err(|e| AppError::Io(e))?;
        
        info!("MCP server listening on port {}", self.state.config.mcp_port);
        
        axum::serve(listener, app).await
            .map_err(|e| AppError::Unknown(format!("Server error: {}", e)))?;
        
        Ok(())
    }
}

/// Health check handler
async fn handle_health(State(state): State<ServerState>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "healthy",
        "version": "0.1.0",
        "user_id": state.config.matrix_user_id,
    }))
}

/// List all available tools
async fn handle_list_tools(State(state): State<ServerState>) -> impl IntoResponse {
    let tools = state.tools.read().await;
    let tool_list: Vec<Value> = tools.values()
        .map(|tool| {
            serde_json::json!({
                "name": tool.name(),
                "description": tool.description(),
                "inputSchema": tool.input_schema(),
            })
        })
        .collect();
    
    axum::Json(serde_json::json!({ "tools": tool_list }))
}

/// Request to execute a tool
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolRequest {
    pub name: String,
    pub arguments: Option<Value>,
}

/// Response from tool execution
#[derive(Debug, Clone, Serialize)]
pub struct ToolResponse {
    pub content: Vec<ToolContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

impl ToolResponse {
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: text.into(),
            }],
            error: None,
        }
    }
    
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![],
            error: Some(message.into()),
        }
    }
}

/// Execute a tool by name
pub async fn execute_tool(
    state: &ServerState,
    name: &str,
    arguments: Value,
) -> Result<ToolResponse> {
    let tools = state.tools.read().await;
    
    let tool = tools.get(name)
        .ok_or_else(|| AppError::InvalidParameter(format!("Unknown tool: {}", name)))?;
    
    debug!("Executing tool: {}", name);
    
    match tool.execute(arguments).await {
        Ok(result) => Ok(ToolResponse::success(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
        )),
        Err(e) => Ok(ToolResponse::error(e.to_string())),
    }
}
