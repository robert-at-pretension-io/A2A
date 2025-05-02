# LLM Core Module

This module provides a unified interface for working with Large Language Models (LLMs) in the A2A Test Suite. It establishes a common trait-based approach that allows for different LLM implementations while providing consistent functionality.

## Overview

The `llm_core` module consists of:

1. **LlmClient Trait**: Common interface for all LLM implementations
2. **Concrete Implementations**: Claude, Mock clients (extensible to other providers)
3. **Template System**: File-based prompt templates with variable substitution
4. **Testing Utilities**: Tools to facilitate testing LLM-dependent code

## LlmClient Trait

The core of this module is the `LlmClient` trait which defines the interface that all LLM implementations must follow:

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Simple text completion - send a prompt and get a text response
    async fn complete(&self, prompt: &str) -> Result<String>;
    
    /// Structured JSON completion - send a prompt and parse response as JSON
    async fn complete_json<T: DeserializeOwned + Send>(&self, prompt: &str) -> Result<T>;
    
    /// Extract JSON from a text that might contain other content
    fn extract_json(&self, text: &str) -> Result<String>;
}
```

The trait provides:
- Basic text completion (`complete`)
- JSON completion with automatic parsing (`complete_json`)
- JSON extraction from unstructured text (`extract_json`)

## Implementations

### ClaudeClient

The `ClaudeClient` provides integration with Anthropic's Claude models. Usage:

```rust
use crate::bidirectional_agent::llm_core::{ClaudeClient, LlmConfig};

// Create configuration
let config = LlmConfig {
    api_key: "your_api_key".to_string(),
    model: "claude-3-haiku-20240307".to_string(),
    max_tokens: 2048,
    temperature: 0.1,
    timeout_seconds: 30,
};

// Create client
let client = ClaudeClient::new(config)?;

// Use for completion
let response = client.complete("Your prompt here").await?;

// Or with JSON parsing
#[derive(Deserialize)]
struct MyResponse {
    key: String,
    value: i32,
}

let parsed: MyResponse = client.complete_json("Prompt requesting JSON").await?;
```

### MockLlmClient

The `MockLlmClient` is designed for testing and provides deterministic responses based on either exact or partial prompt matching:

```rust
use crate::bidirectional_agent::llm_core::MockLlmClient;

// Create with specific prompt-response pairs
let client = MockLlmClient::new(vec![
    ("keyword to match".to_string(), "response for this keyword".to_string()),
    ("another pattern".to_string(), r#"{"json": "response"}"#.to_string()),
]);

// Or with a default response
let client = MockLlmClient::with_default_response("default response".to_string());

// Use like any other LlmClient
let response = client.complete("Text with keyword to match in it").await?;
assert_eq!(response, "response for this keyword");
```

The mock client does partial matching, so any prompt containing a stored prompt string will return the corresponding response.

## Template System

The template system provides a way to manage and render prompt templates from files with variable substitution:

```rust
use crate::bidirectional_agent::llm_core::TemplateManager;
use std::collections::HashMap;

// Create template manager (default location is src/bidirectional_agent/llm_core/prompts)
let manager = TemplateManager::with_default_dir();

// Create variables for substitution
let mut variables = HashMap::new();
variables.insert("name".to_string(), "Claude".to_string());
variables.insert("task".to_string(), "Analyze this data".to_string());

// Render a template
let rendered = manager.render("template_name", &variables)?;
```

### Template Format

Templates are stored as text files in the `prompts` directory with a `.txt` extension. They use a simple `{{variable_name}}` syntax for placeholders:

```
# Example Template

Hello, {{name}}!

Your task is: {{task}}

Please respond with a detailed analysis.
```

## Integration with Routing

The `llm_core` module integrates with the existing routing system through the `RefactoredLlmTaskRouter` class in `task_router_llm_refactored.rs`. This implementation uses the new LlmClient trait and template system to make routing decisions.

## Error Handling

All operations return `Result` types with descriptive error messages. Common error scenarios:

- API connection failures
- Authentication errors
- JSON parsing errors
- Missing template variables
- Timeout errors

## Testing

The module includes comprehensive testing tools:

- `MockLlmClient` for simulating LLM responses
- Unit tests for template rendering
- Integration tests for end-to-end functionality

## Extending with New Models

To add support for a new LLM provider:

1. Create a new file (e.g., `gpt.rs`) in the `llm_core` module
2. Implement the `LlmClient` trait for your provider
3. Re-export the implementation in `mod.rs`

Example:

```rust
// In gpt.rs
pub struct GptClient {
    // Implementation details
}

#[async_trait]
impl LlmClient for GptClient {
    async fn complete(&self, prompt: &str) -> Result<String> {
        // Implementation
    }
    
    // Other required methods
}

// In mod.rs
pub mod gpt;
pub use gpt::GptClient;
```

## Performance Considerations

- Template caching: Templates are cached after first load for performance
- Properly configure timeouts to prevent hanging requests
- Consider using lightweight models for routing decisions
- Monitor token usage for cost optimization

## Security Considerations

- API keys should be stored securely (e.g., in environment variables)
- Be careful with user-provided content in prompts
- Validate and sanitize LLM outputs before using them in critical operations