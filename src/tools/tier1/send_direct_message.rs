use crate::error::Result;
use crate::tools::Tool;
use serde_json::{json, Value};

pub struct SendDirectMessage;

#[async_trait::async_trait]
impl Tool for SendDirectMessage {
    fn name(&self) -> &'static str {
        "send-direct-message"
    }
    
    fn description(&self) -> &'static str {
        "Send a direct message to a Matrix user"
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
