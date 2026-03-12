use crate::error::Result;
use crate::tools::Tool;
use serde_json::{json, Value};

pub struct JoinRoom;

#[async_trait::async_trait]
impl Tool for JoinRoom {
    fn name(&self) -> &'static str {
        "join-room"
    }
    
    fn description(&self) -> &'static str {
        "Join a Matrix room by ID or alias"
    }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }
    
    async fn execute(&self, _args: Value) -> Result<Value> {
        Ok(json!({
            "success": true,
            "message": "Tool scaffolded - Matrix client integration pending"
        }))
    }
}
