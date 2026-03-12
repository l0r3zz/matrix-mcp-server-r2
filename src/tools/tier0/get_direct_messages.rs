use crate::error::Result;
use crate::tools::Tool;
use serde_json::{json, Value};

pub struct GetDirectMessages;

#[async_trait::async_trait]
impl Tool for GetDirectMessages {
    fn name(&self) -> &'static str {
        "get-direct-messages"
    }
    
    fn description(&self) -> &'static str {
        "Get direct message conversations"
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
            "message": "Tool scaffolded - Matrix client integration pending"
        }))
    }
}
