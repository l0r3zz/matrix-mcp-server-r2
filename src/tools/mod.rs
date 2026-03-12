pub mod tier0;
pub mod tier1;

use serde_json::Value;

/// Tool handler trait for implementing MCP tools[async_trait::async_trait]
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, args: Value) -> crate::error::Result<Value>;
}
