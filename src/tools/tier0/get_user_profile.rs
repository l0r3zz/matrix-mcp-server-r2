use crate::error::Result;
use crate::tools::Tool;
use serde_json::{json, Value};

pub struct GetUserProfile;

#[async_trait::async_trait]
impl Tool for GetUserProfile {
    fn name(&self) -> &'static str {
        "get-user-profile"
    }
    
    fn description(&self) -> &'static str {
        "Get profile information for a specific Matrix user including display name, avatar, and presence"
    }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "targetUserId": {
                    "type": "string",
                    "description": "Target user's Matrix ID (e.g., @user:domain.com)"
                }
            },
            "required": ["targetUserId"],
            "additionalProperties": false
        })
    }
    
    async fn execute(&self, args: Value) -> Result<Value> {
        let user_id = args.get("targetUserId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AppError::InvalidParameter("targetUserId is required".to_string()))?;
        
        Ok(json!({
            "userId": user_id,
            "displayName": null,
            "avatarUrl": null,
            "presence": "unknown",
            "message": "Tool scaffolded - Matrix client integration pending"
        }))
    }
}
