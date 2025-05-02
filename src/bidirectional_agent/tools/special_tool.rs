use async_trait::async_trait;
use serde_json::Value;
use crate::bidirectional_agent::tool_executor::ToolError;
use super::Tool;

/// A special tool for testing purposes that echoes back parameters with identifier 1
pub struct SpecialEchoTool1;

#[async_trait]
impl Tool for SpecialEchoTool1 {
    fn name(&self) -> &str {
        "special_echo_1"
    }

    fn description(&self) -> &str {
        "Special echo tool for testing that adds identifier 1 to response"
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let mut result = serde_json::Map::new();
        result.insert("echo".to_string(), params.clone());
        result.insert("identifier".to_string(), Value::String("special_echo_1".to_string()));
        
        Ok(Value::Object(result))
    }

    fn capabilities(&self) -> &[&'static str] {
        &["testing"]
    }
}

/// A special tool for testing purposes that echoes back parameters with identifier 2
pub struct SpecialEchoTool2;

#[async_trait]
impl Tool for SpecialEchoTool2 {
    fn name(&self) -> &str {
        "special_echo_2"
    }

    fn description(&self) -> &str {
        "Special echo tool for testing that adds identifier 2 to response"
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let mut result = serde_json::Map::new();
        result.insert("echo".to_string(), params.clone());
        result.insert("identifier".to_string(), Value::String("special_echo_2".to_string()));
        
        Ok(Value::Object(result))
    }

    fn capabilities(&self) -> &[&'static str] {
        &["testing"]
    }
}