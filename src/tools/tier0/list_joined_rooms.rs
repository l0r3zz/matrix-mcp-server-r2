use crate::error::Result;
use crate::tools::Tool;
use serde_json::{json, Value};

pub struct ListJoinedRooms;

#[async_trait::async_trait]
impl Tool for ListJoinedRooms {
    fn name(&self) -> &'static str {
        "list-joined-rooms"
    }
    
    fn description(&self) -> &'static str {
        "Get a list of all Matrix rooms the user has joined, with room names, IDs, and basic information"
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
        // For now, return empty list - will be integrated with Matrix client
        Ok(json!({
            "rooms": [],
            "count": 0,
            "message": "Tool scaffolded - Matrix client integration pending"
        }))
    }
}
