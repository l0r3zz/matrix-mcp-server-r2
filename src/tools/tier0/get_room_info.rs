use crate::error::Result;
use crate::tools::Tool;
use serde_json::{json, Value};

pub struct GetRoomInfo;

#[async_trait::async_trait]
impl Tool for GetRoomInfo {
    fn name(&self) -> &'static str {
        "get-room-info"
    }
    
    fn description(&self) -> &'static str {
        "Get detailed information about a Matrix room including name, topic, settings, member count, and encryption status"
    }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "roomId": {
                    "type": "string",
                    "description": "Matrix room ID (e.g., !roomid:domain.com)"
                }
            },
            "required": ["roomId"],
            "additionalProperties": false
        })
    }
    
    async fn execute(&self, args: Value) -> Result<Value> {
        let room_id = args.get("roomId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AppError::InvalidParameter("roomId is required".to_string()))?;
        
        Ok(json!({
            "roomId": room_id,
            "name": null,
            "topic": null,
            "memberCount": 0,
            "isEncrypted": false,
            "message": "Tool scaffolded - Matrix client integration pending"
        }))
    }
}
