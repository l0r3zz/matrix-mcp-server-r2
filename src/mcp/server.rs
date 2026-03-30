use matrix_sdk::Client;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

use crate::config::Config;

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn text_result(texts: Vec<String>) -> CallToolResult {
    CallToolResult::success(texts.into_iter().map(Content::text).collect::<Vec<_>>())
}

fn single_text(text: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![Content::text(text)])
}

fn err_text(text: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![Content::text(text)])
}

// ---------------------------------------------------------------------------
// Tool input structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RoomIdInput {
    #[schemars(description = "Matrix room ID (e.g., !roomid:domain.com)")]
    #[serde(rename = "roomId")]
    pub room_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetRoomMessagesInput {
    #[schemars(description = "Matrix room ID (e.g., !roomid:domain.com)")]
    #[serde(rename = "roomId")]
    pub room_id: String,
    #[schemars(description = "Maximum number of messages to retrieve (default: 20)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetMessagesByDateInput {
    #[schemars(description = "Matrix room ID (e.g., !roomid:domain.com)")]
    #[serde(rename = "roomId")]
    pub room_id: String,
    #[schemars(description = "Start date in ISO 8601 format")]
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[schemars(description = "End date in ISO 8601 format")]
    #[serde(rename = "endDate")]
    pub end_date: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IdentifyActiveUsersInput {
    #[schemars(description = "Matrix room ID (e.g., !roomid:domain.com)")]
    #[serde(rename = "roomId")]
    pub room_id: String,
    #[schemars(description = "Maximum number of active users to return (default: 10)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TargetUserIdInput {
    #[schemars(description = "Target user's Matrix ID (e.g., @user:domain.com)")]
    #[serde(rename = "targetUserId")]
    pub target_user_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchPublicRoomsInput {
    #[schemars(description = "Search term to filter rooms by name or topic")]
    #[serde(rename = "searchTerm")]
    pub search_term: Option<String>,
    #[schemars(description = "Specific server to search rooms on")]
    pub server: Option<String>,
    #[schemars(description = "Maximum number of rooms to return (default: 20)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetNotificationCountsInput {
    #[schemars(description = "Optional room ID to get counts for specific room only")]
    #[serde(rename = "roomFilter")]
    pub room_filter: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDirectMessagesInput {
    #[schemars(description = "Include DM rooms with no recent messages (default: false)")]
    #[serde(rename = "includeEmpty")]
    pub include_empty: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SendMessageInput {
    #[schemars(description = "Matrix room ID (e.g., !roomid:domain.com)")]
    #[serde(rename = "roomId")]
    pub room_id: String,
    #[schemars(description = "The message content to send")]
    pub message: String,
    #[schemars(description = "Type of message: text, html, or emote")]
    #[serde(rename = "messageType")]
    pub message_type: Option<String>,
    #[schemars(description = "Event ID to reply to (optional)")]
    #[serde(rename = "replyToEventId")]
    pub reply_to_event_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SendDirectMessageInput {
    #[schemars(description = "Target user's Matrix ID (e.g., @user:domain.com)")]
    #[serde(rename = "targetUserId")]
    pub target_user_id: String,
    #[schemars(description = "The message content to send")]
    pub message: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateRoomInput {
    #[schemars(description = "Name for the new room")]
    #[serde(rename = "roomName")]
    pub room_name: String,
    #[schemars(description = "Whether the room should be private (default: false)")]
    #[serde(rename = "isPrivate")]
    pub is_private: Option<bool>,
    #[schemars(description = "Optional topic/description for the room")]
    pub topic: Option<String>,
    #[schemars(description = "Optional array of user IDs to invite")]
    #[serde(rename = "inviteUsers")]
    pub invite_users: Option<Vec<String>>,
    #[schemars(description = "Optional room alias (e.g., 'my-room')")]
    #[serde(rename = "roomAlias")]
    pub room_alias: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct JoinRoomInput {
    #[schemars(description = "Room ID or room alias")]
    #[serde(rename = "roomIdOrAlias")]
    pub room_id_or_alias: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LeaveRoomInput {
    #[schemars(description = "Matrix room ID (e.g., !roomid:domain.com)")]
    #[serde(rename = "roomId")]
    pub room_id: String,
    #[schemars(description = "Optional reason for leaving")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InviteUserInput {
    #[schemars(description = "Matrix room ID (e.g., !roomid:domain.com)")]
    #[serde(rename = "roomId")]
    pub room_id: String,
    #[schemars(description = "Target user's Matrix ID to invite")]
    #[serde(rename = "targetUserId")]
    pub target_user_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetRoomNameInput {
    #[schemars(description = "Matrix room ID (e.g., !roomid:domain.com)")]
    #[serde(rename = "roomId")]
    pub room_id: String,
    #[schemars(description = "New name for the room")]
    #[serde(rename = "roomName")]
    pub room_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetRoomTopicInput {
    #[schemars(description = "Matrix room ID (e.g., !roomid:domain.com)")]
    #[serde(rename = "roomId")]
    pub room_id: String,
    #[schemars(description = "New topic/description for the room")]
    pub topic: String,
}

// ---------------------------------------------------------------------------
// Server struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MatrixMcpServer {
    client: Client,
    config: Arc<Config>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl MatrixMcpServer {
    pub fn new(client: Client, config: Arc<Config>) -> Self {
        let tool_router = Self::tool_router();
        Self { client, config, tool_router }
    }

    fn get_room(&self, room_id: &str) -> std::result::Result<matrix_sdk::Room, CallToolResult> {
        let Ok(owned_id) = ruma::OwnedRoomId::try_from(room_id) else {
            return Err(err_text(format!("Error: Invalid room ID format: {}", room_id)));
        };
        self.client
            .get_room(&owned_id)
            .ok_or_else(|| err_text(format!("Error: Room with ID {} not found. You may not be a member of this room.", room_id)))
    }

    async fn room_name(&self, room: &matrix_sdk::Room) -> String {
        room.display_name()
            .await
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "Unnamed Room".into())
    }

    /// Parse timeline events from room.messages() into (sender, body) pairs.
    fn parse_message_events(chunk: &[matrix_sdk::deserialized_responses::TimelineEvent]) -> Vec<(String, String, i64)> {
        let mut out = Vec::new();
        for event in chunk {
            let json_str = serde_json::to_string(&event.kind).unwrap_or_default();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                // PlainText variant wraps an event object
                let ev = if val.get("event").is_some() {
                    val.get("event").unwrap()
                } else {
                    &val
                };
                if ev.get("type").and_then(|t| t.as_str()) == Some("m.room.message") {
                    let sender = ev.get("sender").and_then(|s| s.as_str()).unwrap_or("unknown").to_string();
                    let body = ev.pointer("/content/body").and_then(|b| b.as_str()).unwrap_or("").to_string();
                    let ts = ev.get("origin_server_ts").and_then(|t| t.as_i64()).unwrap_or(0);
                    if !body.is_empty() {
                        out.push((sender, body, ts));
                    }
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

#[tool_router]
impl MatrixMcpServer {
    #[tool(description = "Get a list of all Matrix rooms the user has joined, including room names, IDs, and basic information")]
    pub async fn list_joined_rooms(&self) -> Result<CallToolResult, McpError> {
        let rooms = self.client.joined_rooms();
        if rooms.is_empty() {
            return Ok(single_text("No joined rooms found"));
        }
        let mut texts = Vec::with_capacity(rooms.len());
        for room in &rooms {
            let name = self.room_name(room).await;
            texts.push(format!("Room: {} ({}) - {} members", name, room.room_id(), room.joined_members_count()));
        }
        Ok(text_result(texts))
    }

    #[tool(description = "Get detailed information about a Matrix room including name, topic, settings, and member count")]
    pub async fn get_room_info(&self, Parameters(input): Parameters<RoomIdInput>) -> Result<CallToolResult, McpError> {
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let name = self.room_name(&room).await;
        let topic = room.topic().unwrap_or_else(|| "No topic set".into());
        let members = room.joined_members_count();
        let is_encrypted = room.is_encrypted().await.unwrap_or(false);
        let alias = room.canonical_alias().map(|a| a.to_string()).unwrap_or_else(|| "No alias".into());
        Ok(single_text(format!(
            "Room Information:\nName: {}\nRoom ID: {}\nAlias: {}\nTopic: {}\nMembers: {}\nEncrypted: {}",
            name, room.room_id(), alias, topic, members, if is_encrypted { "Yes" } else { "No" }
        )))
    }

    #[tool(description = "List all members currently joined to a Matrix room with their display names and user IDs")]
    pub async fn get_room_members(&self, Parameters(input): Parameters<RoomIdInput>) -> Result<CallToolResult, McpError> {
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
            Ok(m) => m,
            Err(e) => return Ok(err_text(format!("Error: Failed to get room members - {}", e))),
        };
        if members.is_empty() {
            return Ok(single_text(format!("No members found in room {}", self.room_name(&room).await)));
        }
        let texts: Vec<String> = members.iter().map(|m| {
            format!("{} ({})", m.display_name().unwrap_or(m.user_id().as_str()), m.user_id())
        }).collect();
        Ok(text_result(texts))
    }

    #[tool(description = "Retrieve recent messages from a specific Matrix room")]
    pub async fn get_room_messages(&self, Parameters(input): Parameters<GetRoomMessagesInput>) -> Result<CallToolResult, McpError> {
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let limit = input.limit.unwrap_or(20).max(1) as usize;
        let opts = matrix_sdk::room::MessagesOptions::backward();
        let messages = match room.messages(opts).await {
            Ok(m) => m,
            Err(e) => return Ok(err_text(format!("Error: Failed to get room messages - {}", e))),
        };
        let parsed = Self::parse_message_events(&messages.chunk);
        if parsed.is_empty() {
            return Ok(single_text(format!("No messages found in room {}", self.room_name(&room).await)));
        }
        let texts: Vec<String> = parsed.iter().take(limit).map(|(s, b, _)| format!("{}: {}", s, b)).collect();
        Ok(text_result(texts))
    }

    #[tool(description = "Retrieve messages from a Matrix room within a specific date range")]
    pub async fn get_messages_by_date(&self, Parameters(input): Parameters<GetMessagesByDateInput>) -> Result<CallToolResult, McpError> {
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let start_ms = chrono::DateTime::parse_from_rfc3339(&input.start_date).map(|d| d.timestamp_millis()).unwrap_or(0);
        let end_ms = chrono::DateTime::parse_from_rfc3339(&input.end_date).map(|d| d.timestamp_millis()).unwrap_or(i64::MAX);
        let opts = matrix_sdk::room::MessagesOptions::backward();
        let messages = match room.messages(opts).await {
            Ok(m) => m,
            Err(e) => return Ok(err_text(format!("Error: Failed to filter messages by date - {}", e))),
        };
        let parsed = Self::parse_message_events(&messages.chunk);
        let filtered: Vec<String> = parsed.iter().filter(|(_, _, ts)| *ts >= start_ms && *ts <= end_ms).map(|(s, b, _)| format!("{}: {}", s, b)).collect();
        if filtered.is_empty() {
            return Ok(single_text(format!("No messages found in room {} between {} and {}", self.room_name(&room).await, input.start_date, input.end_date)));
        }
        Ok(text_result(filtered))
    }

    #[tool(description = "Find the most active users in a Matrix room based on message count")]
    pub async fn identify_active_users(&self, Parameters(input): Parameters<IdentifyActiveUsersInput>) -> Result<CallToolResult, McpError> {
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let limit = input.limit.unwrap_or(10).max(1) as usize;
        let opts = matrix_sdk::room::MessagesOptions::backward();
        let messages = match room.messages(opts).await {
            Ok(m) => m,
            Err(e) => return Ok(err_text(format!("Error: Failed to identify active users - {}", e))),
        };
        let parsed = Self::parse_message_events(&messages.chunk);
        let mut counts: HashMap<String, usize> = HashMap::new();
        for (sender, _, _) in &parsed {
            *counts.entry(sender.clone()).or_default() += 1;
        }
        if counts.is_empty() {
            return Ok(single_text(format!("No message activity found in room {}", self.room_name(&room).await)));
        }
        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(limit);
        Ok(text_result(sorted.iter().map(|(u, c)| format!("{}: {} messages", u, c)).collect()))
    }

    #[tool(description = "Get profile information for a specific Matrix user")]
    pub async fn get_user_profile(&self, Parameters(input): Parameters<TargetUserIdInput>) -> Result<CallToolResult, McpError> {
        let Ok(user_id) = ruma::OwnedUserId::try_from(input.target_user_id.as_str()) else {
            return Ok(err_text(format!("Error: Invalid user ID format: {}", input.target_user_id)));
        };
        let profile = match self.client.account().fetch_user_profile_of(&user_id).await {
            Ok(p) => p,
            Err(e) => return Ok(err_text(format!("Error: Failed to get user profile - {}", e))),
        };
        let display_name = profile.displayname.as_deref().unwrap_or("No display name set");
        let avatar = profile.avatar_url.as_ref().map(|u: &ruma::OwnedMxcUri| u.to_string()).unwrap_or_else(|| "No avatar set".into());

        let mut shared_rooms = Vec::new();
        for room in self.client.joined_rooms().iter().take(100) {
            if let Ok(Some(_)) = room.get_member_no_sync(&user_id).await {
                shared_rooms.push(room.canonical_alias().map(|a| a.to_string()).unwrap_or_else(|| room.room_id().to_string()));
                if shared_rooms.len() >= 5 { break; }
            }
        }
        Ok(single_text(format!(
            "User Profile: {}\nDisplay Name: {}\nAvatar: {}\nShared Rooms (up to 5): {}",
            input.target_user_id, display_name, avatar,
            if shared_rooms.is_empty() { "None visible".into() } else { shared_rooms.join(", ") }
        )))
    }

    #[tool(description = "Get your own profile information including display name, avatar, and device list")]
    pub async fn get_my_profile(&self) -> Result<CallToolResult, McpError> {
        let user_id = self.client.user_id().map(|u| u.to_string()).unwrap_or_default();
        let display_name = self.client.account().get_display_name().await.ok().flatten().unwrap_or_else(|| "No display name set".into());
        let avatar = self.client.account().get_avatar_url().await.ok().flatten().map(|u| u.to_string()).unwrap_or_else(|| "No avatar set".into());
        let rooms = self.client.joined_rooms();
        let room_count = rooms.len();
        let dm_count = rooms.iter().filter(|r| r.joined_members_count() == 2).count();
        Ok(single_text(format!(
            "My Profile: {}\nDisplay Name: {}\nAvatar: {}\nJoined Rooms: {}\nDirect Messages: {}",
            user_id, display_name, avatar, room_count, dm_count
        )))
    }

    #[tool(description = "List all users known to the Matrix client")]
    pub async fn get_all_users(&self) -> Result<CallToolResult, McpError> {
        let mut seen = std::collections::HashSet::new();
        let mut texts = Vec::new();
        for room in self.client.joined_rooms() {
            if let Ok(members) = room.members(matrix_sdk::RoomMemberships::JOIN).await {
                for member in members {
                    let uid = member.user_id().to_string();
                    if seen.insert(uid.clone()) {
                        texts.push(format!("{} ({})", member.display_name().unwrap_or(member.user_id().as_str()), uid));
                    }
                }
            }
        }
        if texts.is_empty() { return Ok(single_text("No users found in the client cache")); }
        Ok(text_result(texts))
    }

    #[tool(description = "Search for public Matrix rooms that you can join")]
    pub async fn search_public_rooms(&self, Parameters(input): Parameters<SearchPublicRoomsInput>) -> Result<CallToolResult, McpError> {
        use ruma::api::client::directory::get_public_rooms_filtered::v3::Request;
        use ruma::directory::Filter;
        let limit = input.limit.unwrap_or(20).max(1) as u32;
        let server = input.server.as_ref().and_then(|s| ruma::OwnedServerName::try_from(s.as_str()).ok());
        let mut request = Request::new();
        request.limit = Some(limit.into());
        if let Some(ref term) = input.search_term {
            let mut filter = Filter::new();
            filter.generic_search_term = Some(term.clone());
            request.filter = filter;
        }
        if let Some(ref srv) = server { request.server = Some(srv.clone()); }
        let response = match self.client.public_rooms_filtered(request).await {
            Ok(r) => r,
            Err(e) => return Ok(err_text(format!("Error: Failed to search public rooms - {}", e))),
        };
        if response.chunk.is_empty() {
            return Ok(single_text(input.search_term.as_ref().map(|t| format!("No public rooms found matching \"{}\"", t)).unwrap_or_else(|| "No public rooms found".into())));
        }
        let mut texts = vec![format!("Found {} public rooms{}:", response.chunk.len(), input.search_term.as_ref().map(|t| format!(" matching \"{}\"", t)).unwrap_or_default())];
        for room in &response.chunk {
            texts.push(format!("{} ({})\nTopic: {}\nMembers: {}\nRoom ID: {}",
                room.name.as_deref().unwrap_or("Unnamed Room"),
                room.canonical_alias.as_ref().map(|a| a.to_string()).unwrap_or_else(|| room.room_id.to_string()),
                room.topic.as_deref().unwrap_or("No topic"),
                room.num_joined_members, room.room_id));
        }
        Ok(text_result(texts))
    }

    #[tool(description = "Get unread message counts and notification status for Matrix rooms")]
    pub async fn get_notification_counts(&self, Parameters(input): Parameters<GetNotificationCountsInput>) -> Result<CallToolResult, McpError> {
        let rooms = self.client.joined_rooms();
        let filtered: Vec<_> = if let Some(ref fid) = input.room_filter {
            rooms.iter().filter(|r| r.room_id().as_str() == fid).collect()
        } else { rooms.iter().collect() };
        if input.room_filter.is_some() && filtered.is_empty() {
            return Ok(err_text(format!("Error: Room with ID {} not found.", input.room_filter.as_deref().unwrap_or(""))));
        }
        let mut total_unread: u64 = 0; let mut total_mentions: u64 = 0;
        let mut room_texts: Vec<String> = Vec::new();
        for room in &filtered {
            let c = room.unread_notification_counts();
            total_unread += c.notification_count;
            total_mentions += c.highlight_count;
            if c.notification_count > 0 || c.highlight_count > 0 || input.room_filter.is_some() {
                room_texts.push(format!("{} ({})\nUnread: {} messages\nMentions: {}",
                    self.room_name(room).await, room.room_id(), c.notification_count, c.highlight_count));
            }
        }
        if input.room_filter.is_some() {
            return Ok(if room_texts.is_empty() { single_text(format!("No notifications in room {}", input.room_filter.as_deref().unwrap_or(""))) } else { text_result(room_texts) });
        }
        if room_texts.is_empty() { return Ok(single_text("No unread notifications across all rooms")); }
        let mut texts = vec![format!("Notification Summary:\nTotal unread messages: {}\nTotal mentions/highlights: {}\nRooms with notifications: {}", total_unread, total_mentions, room_texts.len())];
        texts.extend(room_texts);
        Ok(text_result(texts))
    }

    #[tool(description = "List all direct message conversations with their recent activity")]
    pub async fn get_direct_messages(&self, Parameters(input): Parameters<GetDirectMessagesInput>) -> Result<CallToolResult, McpError> {
        let include_empty = input.include_empty.unwrap_or(false);
        let my_uid = self.client.user_id().map(|u| u.to_string()).unwrap_or_default();
        let dm_rooms: Vec<_> = self.client.joined_rooms().into_iter().filter(|r| r.joined_members_count() == 2).collect();
        if dm_rooms.is_empty() { return Ok(single_text("No direct message conversations found")); }
        let mut dm_texts: Vec<String> = Vec::new();
        for room in &dm_rooms {
            let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await { Ok(m) => m, Err(_) => continue };
            let other = match members.iter().find(|m| m.user_id().as_str() != my_uid) { Some(o) => o, None => continue };
            let c = room.unread_notification_counts();
            if !include_empty && c.notification_count == 0 && c.highlight_count == 0 { continue; }
            dm_texts.push(format!("{} ({})\nRoom ID: {}\nUnread: {} messages\nMentions: {}",
                other.display_name().unwrap_or(other.user_id().as_str()), other.user_id(), room.room_id(), c.notification_count, c.highlight_count));
        }
        if dm_texts.is_empty() {
            return Ok(single_text(if include_empty { "No direct message conversations found" } else { "No direct message conversations with recent activity found" }));
        }
        let mut texts = vec![format!("Found {} direct message conversation{}:", dm_texts.len(), if dm_texts.len() == 1 { "" } else { "s" })];
        texts.extend(dm_texts);
        Ok(text_result(texts))
    }

    // ---- Tier 1 ----

    #[tool(description = "Send a text message to a Matrix room, with support for plain text, HTML, and emotes")]
    pub async fn send_message(&self, Parameters(input): Parameters<SendMessageInput>) -> Result<CallToolResult, McpError> {
        use ruma::events::room::message::RoomMessageEventContent;
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let msg_type = input.message_type.as_deref().unwrap_or("text");
        let content = match msg_type {
            "html" => RoomMessageEventContent::text_html(&input.message, &input.message),
            "emote" => RoomMessageEventContent::emote_plain(&input.message),
            _ => RoomMessageEventContent::text_plain(&input.message),
        };
        let resp = match room.send(content).await {
            Ok(r) => r,
            Err(e) => return Ok(err_text(format!("Error: Failed to send message - {}", e))),
        };
        Ok(single_text(format!("Message sent successfully to {}\nEvent ID: {}\nMessage type: {}{}",
            self.room_name(&room).await, resp.event_id, msg_type,
            input.reply_to_event_id.as_ref().map(|id| format!(" (reply to {})", id)).unwrap_or_default())))
    }

    #[tool(description = "Send a direct message to a Matrix user. Creates a new DM room if one doesn't exist")]
    pub async fn send_direct_message(&self, Parameters(input): Parameters<SendDirectMessageInput>) -> Result<CallToolResult, McpError> {
        use ruma::api::client::room::create_room::v3::Request as CreateRoomRequest;
        use ruma::events::room::message::RoomMessageEventContent;
        let target_uid = match ruma::OwnedUserId::try_from(input.target_user_id.as_str()) {
            Ok(u) => u,
            Err(e) => return Ok(err_text(format!("Error: Invalid user ID '{}': {}", input.target_user_id, e))),
        };
        let mut existing_dm = None;
        for room in self.client.joined_rooms() {
            if room.joined_members_count() != 2 { continue; }
            if let Ok(Some(_)) = room.get_member_no_sync(&target_uid).await {
                existing_dm = Some(room);
                break;
            }
        }
        let (room, created) = if let Some(dm) = existing_dm { (dm, false) } else {
            let mut req = CreateRoomRequest::new();
            req.is_direct = true;
            req.invite = vec![target_uid.clone()];
            req.preset = Some(ruma::api::client::room::create_room::v3::RoomPreset::TrustedPrivateChat);
            let resp = match self.client.create_room(req).await {
                Ok(r) => r,
                Err(e) => return Ok(err_text(format!("Error: Failed to send direct message - {}", e))),
            };
            match self.client.get_room(&resp.room_id()) {
                Some(r) => (r, true),
                None => return Ok(err_text("Error: Created DM room but could not retrieve it")),
            }
        };
        let resp = match room.send(RoomMessageEventContent::text_plain(&input.message)).await {
            Ok(r) => r,
            Err(e) => return Ok(err_text(format!("Error: Failed to send direct message - {}", e))),
        };
        Ok(single_text(format!("Direct message sent successfully to {}\nRoom: {} ({})\nEvent ID: {}\n{}",
            input.target_user_id, self.room_name(&room).await, room.room_id(), resp.event_id,
            if created { "New DM room created" } else { "Used existing DM room" })))
    }

    #[tool(description = "Create a new Matrix room with customizable settings")]
    pub async fn create_room(&self, Parameters(input): Parameters<CreateRoomInput>) -> Result<CallToolResult, McpError> {
        use ruma::api::client::room::create_room::v3::{Request as CreateRoomRequest, RoomPreset};
        let is_private = input.is_private.unwrap_or(false);
        let mut req = CreateRoomRequest::new();
        req.name = Some(input.room_name.clone());
        if let Some(ref t) = input.topic { req.topic = Some(t.clone()); }
        if let Some(ref a) = input.room_alias { req.room_alias_name = Some(a.clone()); }
        req.preset = Some(if is_private { RoomPreset::PrivateChat } else { RoomPreset::PublicChat });
        if let Some(ref invites) = input.invite_users {
            let mut uids = Vec::new();
            for s in invites {
                match ruma::OwnedUserId::try_from(s.as_str()) { Ok(u) => uids.push(u), Err(e) => { warn!("Skipping invalid user ID '{}': {}", s, e); } }
            }
            req.invite = uids;
        }
        let resp = match self.client.create_room(req).await {
            Ok(r) => r,
            Err(e) => { let m = e.to_string(); return Ok(err_text(
                if m.contains("M_ROOM_IN_USE") { format!("Error: Room alias \"{}\" is already in use", input.room_alias.as_deref().unwrap_or("unknown")) }
                else if m.contains("M_FORBIDDEN") { "Error: You don't have permission to create rooms".into() }
                else { format!("Error: Failed to create room \"{}\" - {}", input.room_name, e) }
            )); }
        };
        let domain = self.config.matrix_user_id.split(':').nth(1).unwrap_or("unknown");
        Ok(single_text(format!("Successfully created room: {}\nRoom ID: {}\nAlias: {}\nPrivacy: {}\nTopic: {}\nInvited users: {}",
            input.room_name, resp.room_id(),
            input.room_alias.as_ref().map(|a| format!("#{}:{}", a, domain)).unwrap_or_else(|| "No alias".into()),
            if is_private { "Private" } else { "Public" },
            input.topic.as_deref().unwrap_or("No topic set"),
            input.invite_users.as_ref().filter(|v| !v.is_empty()).map(|v| v.join(", ")).unwrap_or_else(|| "None".into())
        )))
    }

    #[tool(description = "Join a Matrix room by room ID or alias")]
    pub async fn join_room(&self, Parameters(input): Parameters<JoinRoomInput>) -> Result<CallToolResult, McpError> {
        let room_or_alias = match ruma::OwnedRoomOrAliasId::try_from(input.room_id_or_alias.as_str()) {
            Ok(id) => id, Err(e) => return Ok(err_text(format!("Error: Invalid room ID or alias '{}': {}", input.room_id_or_alias, e))),
        };
        let resp = match self.client.join_room_by_id_or_alias(room_or_alias.as_ref(), &[]).await {
            Ok(r) => r,
            Err(e) => { let m = e.to_string(); return Ok(err_text(
                if m.contains("M_NOT_FOUND") { format!("Error: Room {} not found", input.room_id_or_alias) }
                else if m.contains("M_FORBIDDEN") { format!("Error: Access denied to room {}", input.room_id_or_alias) }
                else { format!("Error: Failed to join room {} - {}", input.room_id_or_alias, e) }
            )); }
        };
        Ok(single_text(format!("Successfully joined room\nRoom ID: {}{}", resp.room_id(),
            if input.room_id_or_alias != resp.room_id().as_str() { format!("\nJoined via alias: {}", input.room_id_or_alias) } else { String::new() })))
    }

    #[tool(description = "Leave a Matrix room with an optional reason message")]
    pub async fn leave_room(&self, Parameters(input): Parameters<LeaveRoomInput>) -> Result<CallToolResult, McpError> {
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let name = self.room_name(&room).await;
        if let Err(e) = room.leave().await {
            return Ok(err_text(format!("Error: Failed to leave room {} - {}", input.room_id, e)));
        }
        Ok(single_text(format!("Successfully left room: {}\nRoom ID: {}{}", name, input.room_id,
            input.reason.as_ref().map(|r| format!("\nReason: {}", r)).unwrap_or_default())))
    }

    #[tool(description = "Invite a user to a Matrix room")]
    pub async fn invite_user(&self, Parameters(input): Parameters<InviteUserInput>) -> Result<CallToolResult, McpError> {
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let target_uid = match ruma::OwnedUserId::try_from(input.target_user_id.as_str()) {
            Ok(u) => u, Err(e) => return Ok(err_text(format!("Error: Invalid user ID '{}': {}", input.target_user_id, e))),
        };
        let name = self.room_name(&room).await;
        if let Err(e) = room.invite_user_by_id(&target_uid).await {
            return Ok(err_text(format!("Error: Failed to invite {} to room {} - {}", input.target_user_id, input.room_id, e)));
        }
        Ok(single_text(format!("Successfully invited {} to room {}\nRoom ID: {}\nThe user will receive an invitation and can choose to join the room.",
            input.target_user_id, name, input.room_id)))
    }

    #[tool(description = "Update the display name of a Matrix room")]
    pub async fn set_room_name(&self, Parameters(input): Parameters<SetRoomNameInput>) -> Result<CallToolResult, McpError> {
        use ruma::events::room::name::RoomNameEventContent;
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let current = self.room_name(&room).await;
        if let Err(e) = room.send_state_event(RoomNameEventContent::new(input.room_name.clone())).await {
            return Ok(err_text(format!("Error: Failed to set room name - {}", e)));
        }
        Ok(single_text(format!("Successfully updated room name\nRoom ID: {}\nPrevious name: {}\nNew name: {}", input.room_id, current, input.room_name)))
    }

    #[tool(description = "Update the topic/description of a Matrix room")]
    pub async fn set_room_topic(&self, Parameters(input): Parameters<SetRoomTopicInput>) -> Result<CallToolResult, McpError> {
        use ruma::events::room::topic::RoomTopicEventContent;
        let room = match self.get_room(&input.room_id) { Ok(r) => r, Err(e) => return Ok(e) };
        let name = self.room_name(&room).await;
        let current_topic = room.topic().unwrap_or_else(|| "No topic set".into());
        if let Err(e) = room.send_state_event(RoomTopicEventContent::new(input.topic.clone())).await {
            return Ok(err_text(format!("Error: Failed to set room topic - {}", e)));
        }
        Ok(single_text(format!("Successfully updated room topic for {}\nRoom ID: {}\nPrevious topic: {}\nNew topic: {}", name, input.room_id, current_topic, input.topic)))
    }
}

// ---------------------------------------------------------------------------
// ServerHandler
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for MatrixMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::new("matrix-mcp-server-r2", env!("CARGO_PKG_VERSION")))
        .with_instructions(
            "Matrix MCP Server providing room, message, and user management tools.".to_string(),
        )
    }
}
