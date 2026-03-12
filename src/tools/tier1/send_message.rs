use crate::error::Result;
use crate::tools::Tool;
use serde_json::{json, Value};

pub struct SendMessage;

#[async_trait::async_trait]
impl Tool for SendMessage {
    fn name(&self) -> &'static str {
        "send-message"
    }
    
    fn description(&self) -> &'static str {
        "Send a text message to a Matrix room"
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
