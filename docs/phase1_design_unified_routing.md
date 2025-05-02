# Phase 1 Design: Unified RoutingAgent

## Overview

This document outlines the detailed design for the unified RoutingAgent, which will merge the existing `LlmTaskRouter` and `RoutingAgent` implementations while leveraging the new LLM Core infrastructure. The goal is to create a more maintainable, extensible routing system that preserves backward compatibility.

## Current Architecture

The current system has two overlapping components:

1. **LlmTaskRouter** (in `task_router_llm.rs`)
   - Implements the `LlmTaskRouterTrait`
   - Created via the `create_llm_task_router` factory
   - Uses the `RoutingAgent` internally for LLM-based decisions

2. **RoutingAgent** (in `llm_routing/mod.rs`)
   - Created and managed by `LlmTaskRouter`
   - Contains the actual LLM-based routing logic
   - Implements the `RoutingAgentTrait`

This separation creates duplicated code and unnecessary complexity.

## Unified Design

The new unified design will:

1. Create a new implementation that combines both components
2. Use the new `LlmClient` trait and template system
3. Respect the existing traits for backward compatibility
4. Provide a clean migration path

### Core Components

#### 1. UnifiedRoutingAgent

```rust
// src/bidirectional_agent/routing/unified_router.rs

pub struct UnifiedRoutingAgent {
    // Dependencies
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
    llm_client: Arc<dyn LlmClient>,
    template_manager: TemplateManager,
    
    // Configuration
    config: RoutingConfig,
}

#[async_trait]
impl LlmTaskRouterTrait for UnifiedRoutingAgent {
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError>;
    
    #[cfg(feature = "bidir-delegate")]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError>;
    
    #[cfg(feature = "bidir-delegate")]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError>;
}
```

#### 2. Routing Configuration

```rust
// src/bidirectional_agent/routing/config.rs

#[derive(Debug, Clone, Deserialize)]
pub struct RoutingConfig {
    // LLM configuration
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    
    // Routing behavior
    pub default_local_tool: String,
    pub fallback_behavior: FallbackBehavior,
    
    // Advanced options
    pub cache_ttl_seconds: u64,
    pub enable_explanation: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum FallbackBehavior {
    UseLocalTool(String),
    UseRemoteAgent(String),
    Reject(String),
}
```

#### 3. Factory Functions

```rust
// src/bidirectional_agent/routing/factory.rs

pub fn create_unified_router(
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
    llm_client: Arc<dyn LlmClient>,
    config: Option<RoutingConfig>
) -> Arc<dyn LlmTaskRouterTrait>;

// Legacy adapter
pub fn create_router_from_legacy_config(
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
    legacy_config: Option<llm_routing::LlmRoutingConfig>
) -> Arc<dyn LlmTaskRouterTrait>;
```

### Implementation Details

#### Decision Making Flow

The decision flow in the `decide` method:

1. Check for explicit routing hints in metadata
2. If no hints, parse the task to extract key information
3. Get available tools and agents from registries
4. Prepare the routing prompt using templates
5. Make the LLM call to get a structured response
6. Validate and process the response
7. Apply fallback logic if needed

```rust
async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
    // Check for explicit routing hints
    if let Some(hint) = self.extract_routing_hint(params) {
        return self.handle_routing_hint(hint);
    }
    
    // Extract task text
    let task_text = self.extract_text_from_params(params);
    if task_text.is_empty() {
        return self.apply_fallback("Empty task text");
    }
    
    // Get available tools and agents
    let tools = self.get_available_tools();
    let agents = self.get_available_agents().await?;
    
    // Create routing prompt with template
    let mut variables = HashMap::new();
    variables.insert("task_description".to_string(), task_text);
    variables.insert("available_tools".to_string(), format_tools(&tools));
    variables.insert("available_agents".to_string(), format_agents(&agents));
    
    let prompt = match self.template_manager.render("routing_decision", &variables) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("Failed to render routing template: {}", e);
            return self.apply_fallback("Template rendering failed");
        }
    };
    
    // Call LLM
    let response = match self.llm_client.complete_json::<RoutingResponse>(&prompt).await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("LLM routing failed: {}", e);
            return self.apply_fallback("LLM call failed");
        }
    };
    
    // Process response
    self.process_routing_response(response, &tools, &agents)
}
```

#### Caching Layer

To improve performance, we'll add a caching layer for routing decisions:

```rust
pub struct RoutingCache {
    cache: DashMap<String, CachedDecision>,
    ttl: Duration,
}

struct CachedDecision {
    decision: RoutingDecision,
    timestamp: Instant,
}

impl RoutingCache {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            cache: DashMap::new(),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }
    
    pub fn get(&self, key: &str) -> Option<RoutingDecision> {
        // Check if entry exists and is not expired
        if let Some(entry) = self.cache.get(key) {
            if entry.timestamp.elapsed() < self.ttl {
                return Some(entry.decision.clone());
            }
        }
        None
    }
    
    pub fn insert(&self, key: String, decision: RoutingDecision) {
        self.cache.insert(key, CachedDecision {
            decision,
            timestamp: Instant::now(),
        });
    }
}
```

#### Explanation Generation

For improved transparency, the router can generate explanations for its decisions:

```rust
async fn explain_decision(&self, decision: &RoutingDecision, task_text: &str) -> Result<String, AgentError> {
    if !self.config.enable_explanation {
        return Ok("Decision explanation disabled".to_string());
    }
    
    let mut variables = HashMap::new();
    variables.insert("task_description".to_string(), task_text.to_string());
    variables.insert("decision".to_string(), format!("{:?}", decision));
    
    let prompt = self.template_manager.render("explain_decision", &variables)?;
    
    self.llm_client.complete(&prompt).await
}
```

### Task Decomposition

Task decomposition follows a similar pattern using templates and the LLM client:

```rust
#[cfg(feature = "bidir-delegate")]
async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError> {
    // Extract task text
    let task_text = self.extract_text_from_params(params);
    if task_text.is_empty() {
        return Ok(false);
    }
    
    // Create variables for template
    let mut variables = HashMap::new();
    variables.insert("task_description".to_string(), task_text);
    
    // Render template
    let prompt = match self.template_manager.render("should_decompose", &variables) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("Failed to render decomposition template: {}", e);
            return self.fallback_should_decompose(&task_text);
        }
    };
    
    // Call LLM
    match self.llm_client.complete_json::<DecompositionResponse>(&prompt).await {
        Ok(response) => {
            log::info!("💭 Decomposition reasoning: {}", response.reasoning);
            Ok(response.should_decompose)
        },
        Err(e) => {
            log::warn!("LLM decomposition check failed: {}", e);
            self.fallback_should_decompose(&task_text)
        }
    }
}
```

## Migration Strategy

To ensure a smooth transition:

1. The unified router will be introduced alongside the existing implementations
2. A feature flag will control which implementation is used
3. The `create_transitional_llm_router` function will be updated to use the unified router when enabled
4. Comprehensive test coverage will ensure behavior parity

## Testing Plan

Testing will cover:

1. **Unit Tests**
   - Test decision logic in isolation with mock clients
   - Test template rendering
   - Test fallback behaviors

2. **Integration Tests**
   - Test with various task types
   - Test with different configurations
   - Test caching behavior

3. **Compatibility Tests**
   - Ensure behavior matches legacy implementation
   - Test with real LLM APIs (when available)

## Performance Considerations

The unified design brings several performance improvements:

1. **Caching**: Router decisions are cached to reduce LLM calls
2. **Prompt Efficiency**: Templates are optimized for token efficiency
3. **Parallel Processing**: Tools and agents are fetched in parallel
4. **Error Resilience**: Graceful fallbacks reduce latency under errors

## Backward Compatibility

This design is backward compatible with existing code:

1. It implements the same traits as the current routers
2. Factory functions preserve the same signatures
3. Configuration is backward compatible with legacy configs

## Next Steps

The next steps in implementing this design:

1. Create the unified router implementation
2. Set up tests for the new implementation
3. Update the factory functions to use the new router
4. Update documentation to reflect the new design