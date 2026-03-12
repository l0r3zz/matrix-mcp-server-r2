use crate::error::Result;
use crate::tools::Tool;
use serde_json::{json, Value};

pub struct GetAllUsers;

#[async_trait::async_trait]
impl Tool for GetAllUsers {
    fn name(&self) -> &'static str {
        "get-all-users"
    }
    
    fn description(&self) -> &'static str {
        "Get all known users"
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
