use crate::error::Result;
use crate::tools::Tool;
use serde_json::{json, Value};

pub struct CreateRoom;

#[async_trait::async_trait]
impl Tool for CreateRoom {
    fn name(&self) -> &'static str {
        "create-room"
    }
    
    fn description(&self) -> &'static str {
        "Create a new Matrix room"
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
