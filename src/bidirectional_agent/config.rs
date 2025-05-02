/// Configuration structures for the Bidirectional Agent.

#[cfg(feature = "bidir-core")]

use serde::Deserialize;
use std::{collections::HashMap, path::Path};
use anyhow::{Context, Result};
use serde_json::Value;
use tempfile::tempdir; // Import tempdir for tests
use serde::de::DeserializeOwned;

#[cfg(feature = "bidir-local-exec")]
use crate::bidirectional_agent::llm_routing::{LlmClient, LlmClientConfig};

/// Main configuration for the Bidirectional Agent.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct BidirectionalAgentConfig {
    /// A unique identifier for this agent (e.g., its public URL or a unique name).
    pub self_id: String,
    /// The base URL where this agent's server component will listen.
    pub base_url: String,
    /// A list of bootstrap URLs to discover other agents on startup.
    #[serde(default)]
    pub discovery: Vec<String>,
    /// Authentication configuration for this agent.
    #[serde(default)]
    pub auth: AuthConfig,
    /// Network configuration (proxy, TLS).
    #[serde(default)]
    pub network: NetworkConfig,
    /// Tool-specific configurations.
    #[cfg(feature = "bidir-local-exec")]
    #[serde(default)]
    pub tools: ToolConfigs,
    /// LLM configuration for all LLM-powered features.
    #[cfg(feature = "bidir-local-exec")]
    #[serde(default)]
    pub llm: LlmConfig,
    /// Agent directory configuration. Included if 'bidir-core' is enabled.
    #[cfg(feature = "bidir-core")]
    #[serde(default)]
    pub directory: DirectoryConfig,
    /// Interval (in minutes) for discovering tools from other agents.
    /// Only used if 'bidir-delegate' feature is enabled.
    #[cfg(feature = "bidir-delegate")]
    #[serde(default = "default_tool_discovery_interval")]
    pub tool_discovery_interval_minutes: u64,
    // Add fields for routing policy etc. in later slices
}

/// Default interval for tool discovery (e.g., 30 minutes).
#[cfg(feature = "bidir-delegate")]
fn default_tool_discovery_interval() -> u64 {
    30
}

/// Tool configurations. Keyed by tool name.
#[cfg(feature = "bidir-local-exec")]
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ToolConfigs {
    /// Tool-specific configurations
    #[serde(flatten)]
    pub specific_configs: HashMap<String, Value>, // Allows arbitrary tool configs
}

/// Consolidated LLM configuration for all operations.
#[cfg(feature = "bidir-local-exec")]
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LlmConfig {
    /// Whether to use LLM for routing decisions.
    #[serde(default)]
    pub use_for_routing: bool,
    
    /// Whether to use the new LLM core infrastructure.
    #[serde(default)]
    pub use_new_core: bool,
    
    /// LLM provider to use. Currently only "claude" is supported.
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    
    /// API key for LLM service. If not provided, will attempt to use environment variable.
    pub api_key: Option<String>,
    
    /// Environment variable name for API key if not explicitly provided.
    #[serde(default = "default_api_key_env_var")]
    pub api_key_env_var: String,
    
    /// Model to use for LLM operations.
    #[serde(default = "default_llm_model")]
    pub model: String,
    
    /// Base URL for API requests (provider-specific).
    pub api_base_url: Option<String>,
    
    /// Default maximum tokens to generate.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    
    /// Default temperature (0.0 - 1.0).
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    
    /// Request timeout in seconds.
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    
    /// Use case specific configurations that override the defaults.
    #[serde(default)]
    pub use_cases: LlmUseCases,
}

/// LLM configurations for specific use cases.
#[cfg(feature = "bidir-local-exec")]
#[derive(Deserialize, Debug, Clone, Default)]
pub struct LlmUseCases {
    /// Routing-specific configuration.
    #[serde(default)]
    pub routing: LlmRoutingConfig,
    
    /// Decomposition-specific configuration.
    #[serde(default)]
    pub decomposition: LlmDecompositionConfig,
    
    /// Synthesis-specific configuration.
    #[serde(default)]
    pub synthesis: LlmSynthesisConfig,
}

/// Routing-specific LLM configuration.
#[cfg(feature = "bidir-local-exec")]
#[derive(Deserialize, Debug, Clone, Default)]
pub struct LlmRoutingConfig {
    /// Model override for routing operations.
    pub model: Option<String>,
    
    /// Max tokens override for routing operations.
    pub max_tokens: Option<u32>,
    
    /// Temperature override for routing operations.
    pub temperature: Option<f32>,
    
    /// Prompt template for routing decisions.
    #[serde(default = "default_routing_prompt_template")]
    pub prompt_template: String,
}

/// Task decomposition-specific LLM configuration.
#[cfg(feature = "bidir-local-exec")]
#[derive(Deserialize, Debug, Clone, Default)]
pub struct LlmDecompositionConfig {
    /// Model override for decomposition operations.
    pub model: Option<String>,
    
    /// Max tokens override for decomposition operations.
    pub max_tokens: Option<u32>,
    
    /// Temperature override for decomposition operations.
    pub temperature: Option<f32>,
    
    /// Prompt template for decomposition decisions.
    #[serde(default = "default_decomposition_prompt_template")]
    pub prompt_template: String,
}

/// Results synthesis-specific LLM configuration.
#[cfg(feature = "bidir-local-exec")]
#[derive(Deserialize, Debug, Clone, Default)]
pub struct LlmSynthesisConfig {
    /// Model override for synthesis operations.
    pub model: Option<String>,
    
    /// Max tokens override for synthesis operations.
    pub max_tokens: Option<u32>,
    
    /// Temperature override for synthesis operations.
    pub temperature: Option<f32>,
    
    /// Prompt template for synthesis operations.
    #[serde(default = "default_synthesis_prompt_template")]
    pub prompt_template: String,
}

#[cfg(feature = "bidir-local-exec")]
fn default_llm_provider() -> String {
    "claude".to_string()
}

#[cfg(feature = "bidir-local-exec")]
fn default_api_key_env_var() -> String {
    "ANTHROPIC_API_KEY".to_string()
}

#[cfg(feature = "bidir-local-exec")]
fn default_llm_model() -> String {
    "claude-3-haiku-20240307".to_string()
}

#[cfg(feature = "bidir-local-exec")]
fn default_max_tokens() -> u32 {
    2048
}

#[cfg(feature = "bidir-local-exec")]
fn default_temperature() -> f32 {
    0.1
}

#[cfg(feature = "bidir-local-exec")]
fn default_timeout_seconds() -> u64 {
    30
}

#[cfg(feature = "bidir-local-exec")]
fn default_routing_prompt_template() -> String {
    "# Task Routing Decision\n\n\
     You are a task router for a bidirectional A2A agent system. Your job is to determine the best way to handle a task.\n\n\
     ## Task Description\n{task_description}\n\n\
     ## Routing Options\n\
     1. LOCAL: Handle the task locally using one or more tools\n\
     {tools_description}\n\
     2. REMOTE: Delegate the task to another agent\n\
     {agents_description}\n\n\
     ## Instructions\n\
     Analyze the task and decide whether to handle it locally with tools or delegate to another agent.\n\
     If handling locally, specify which tool(s) to use.\n\
     If delegating, specify which agent to delegate to.\n\n\
     ## Response Format Requirements\n\
     Return ONLY a JSON object with no additional text, explanations, or decorations.\n\
     The JSON MUST follow this exact structure:\n\
     \n\
     {{\n  \
         \"decision_type\": \"LOCAL\" or \"REMOTE\",\n  \
         \"reason\": \"Brief explanation of your decision\",\n  \
         \"tools\": [\"tool1\", \"tool2\"] (include only if decision_type is LOCAL),\n  \
         \"agent_id\": \"agent_id\" (include only if decision_type is REMOTE)\n\
     }}\n\
     \n\n\
     DO NOT include any other text, markdown formatting, or explanations outside the JSON.\n\
     DO NOT use non-existent tools or agents - only use the ones listed above.\n\
     If LOCAL decision, you MUST include at least one valid tool from the available tools list.\n\
     If REMOTE decision, you MUST include a valid agent_id from the available agents list.".to_string()
}

#[cfg(feature = "bidir-local-exec")]
fn default_decomposition_prompt_template() -> String {
    "# Task Decomposition\n\n\
     You are an AI assistant that breaks down complex tasks into manageable subtasks.\n\n\
     ## Task Description\n{task_description}\n\n\
     ## Instructions\n\
     Break down this task into 2-5 clear, logical subtasks that collectively achieve the overall goal.\n\
     For each subtask:\n\
     - Provide a clear, specific description of what needs to be done\n\
     - Make sure it's a discrete unit of work\n\
     - Ensure the subtasks collectively cover the entire original task\n\n\
     ## Response Format Requirements\n\
     Return ONLY a JSON object with no additional text, explanations, or decorations.\n\
     The JSON MUST follow this exact structure:\n\
     \n\
     {{\n  \
         \"subtasks\": [\n    \
             {{\n      \
                 \"description\": \"Clear description of the subtask\"\n    \
             }},\n    \
             {{\n      \
                 \"description\": \"Another subtask description\"\n    \
             }}\n  \
         ]\n\
     }}\n\
     \n\n\
     DO NOT include any other text, markdown formatting, prefixes, or notes outside the JSON.\n\
     DO NOT add any other fields or change the structure.\n\
     DO NOT include backticks, JSON labels, or any other text outside of the JSON object itself.\n\
     Your entire response must be valid JSON that can be parsed directly.".to_string()
}

#[cfg(feature = "bidir-local-exec")]
fn default_synthesis_prompt_template() -> String {
    "# Task Result Synthesis\n\n\
     You are an AI assistant that synthesizes results from multiple subtasks into a cohesive, comprehensive response.\n\n\
     ## Subtask Results\n\n{subtask_results}\n\n\
     ## Instructions\n\
     Synthesize the results from all subtasks into a single, unified response:\n\
     - Combine information logically and avoid redundancy\n\
     - Ensure all key insights from each subtask are preserved\n\
     - Structure the response in a clear, coherent manner\n\
     - Make connections between related pieces of information\n\
     - Present a holistic answer that addresses the overall task\n\n\
     ## Response Format Requirements\n\
     - Provide your synthesized response as plain text\n\
     - Use markdown formatting only for basic structure (headings, lists, etc.)\n\
     - Do not include any meta-commentary about your synthesis process\n\
     - Do not include phrases like \"Based on the subtasks\" or \"Here is my synthesis\"\n\
     - Start directly with the synthesized content\n\
     - Do not include the original subtask texts or labels unless they form part of your answer\n\
     - Focus on delivering a coherent, unified response as if it were written as a single piece".to_string()
}

#[cfg(feature = "bidir-local-exec")]
impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            use_for_routing: false,
            use_new_core: false,
            provider: default_llm_provider(),
            api_key: None,
            api_key_env_var: default_api_key_env_var(),
            model: default_llm_model(),
            api_base_url: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            timeout_seconds: default_timeout_seconds(),
            use_cases: LlmUseCases::default(),
        }
    }
}

#[cfg(feature = "bidir-local-exec")]
impl LlmConfig {
    /// Validates the configuration.
    pub fn validate(&self) -> anyhow::Result<()> {
        // Check provider
        if self.provider != "claude" {
            return Err(anyhow::anyhow!(
                "Unsupported LLM provider: '{}'. Currently only 'claude' is supported.",
                self.provider
            ));
        }
        
        // Check if API key is resolvable when use_for_routing is true
        if self.use_for_routing {
            if self.api_key.is_none() || self.api_key.as_ref().map_or(true, |k| k.is_empty()) {
                // Check if environment variable is set
                if std::env::var(&self.api_key_env_var).is_err() {
                    return Err(anyhow::anyhow!(
                        "LLM API key not provided and environment variable '{}' not set, but use_for_routing is enabled",
                        self.api_key_env_var
                    ));
                }
            }
        }
        
        // Validate temperature
        if self.temperature < 0.0 || self.temperature > 1.0 {
            return Err(anyhow::anyhow!(
                "Invalid temperature value: {}. Must be between 0.0 and 1.0.",
                self.temperature
            ));
        }
        
        // Validate max tokens
        if self.max_tokens < 1 {
            return Err(anyhow::anyhow!(
                "Invalid max_tokens value: {}. Must be at least 1.",
                self.max_tokens
            ));
        }
        
        // Validate timeout
        if self.timeout_seconds < 1 {
            return Err(anyhow::anyhow!(
                "Invalid timeout_seconds value: {}. Must be at least 1.",
                self.timeout_seconds
            ));
        }
        
        // Validate use case specific overrides
        self.validate_use_case_override("routing", 
            self.use_cases.routing.temperature, 
            self.use_cases.routing.max_tokens)?;
            
        self.validate_use_case_override("decomposition", 
            self.use_cases.decomposition.temperature, 
            self.use_cases.decomposition.max_tokens)?;
            
        self.validate_use_case_override("synthesis", 
            self.use_cases.synthesis.temperature, 
            self.use_cases.synthesis.max_tokens)?;
        
        Ok(())
    }
    
    /// Validates a specific use case override
    fn validate_use_case_override(
        &self, 
        use_case: &str, 
        temperature: Option<f32>, 
        max_tokens: Option<u32>
    ) -> anyhow::Result<()> {
        // Validate temperature if provided
        if let Some(temp) = temperature {
            if temp < 0.0 || temp > 1.0 {
                return Err(anyhow::anyhow!(
                    "Invalid temperature value for {}: {}. Must be between 0.0 and 1.0.",
                    use_case, temp
                ));
            }
        }
        
        // Validate max tokens if provided
        if let Some(tokens) = max_tokens {
            if tokens < 1 {
                return Err(anyhow::anyhow!(
                    "Invalid max_tokens value for {}: {}. Must be at least 1.",
                    use_case, tokens
                ));
            }
        }
        
        Ok(())
    }
    
    /// Resolves the API key from config or environment
    pub fn resolve_api_key(&self) -> anyhow::Result<String> {
        match &self.api_key {
            Some(key) if !key.is_empty() => Ok(key.clone()),
            _ => std::env::var(&self.api_key_env_var).map_err(|_| {
                anyhow::anyhow!(
                    "LLM API key not found in config or in environment variable '{}'",
                    self.api_key_env_var
                )
            }),
        }
    }
    
    /// Creates an LlmClient with the base configuration
    pub fn create_client(&self) -> anyhow::Result<LlmClient> {
        let api_key = self.resolve_api_key()?;
        
        let api_base_url = self.api_base_url.clone();
        
        let client_config = LlmClientConfig {
            api_key,
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            timeout_seconds: self.timeout_seconds,
            ..Default::default()
        };
        
        let client = LlmClient::new(client_config).map_err(|e| {
            anyhow::anyhow!("Failed to initialize LLM client: {}", e)
        })?;
        
        Ok(client)
    }
    
    /// Creates a client for routing operations with use-case specific overrides
    pub fn create_routing_client(&self) -> anyhow::Result<LlmClient> {
        let api_key = self.resolve_api_key()?;
        
        let client_config = LlmClientConfig {
            api_key,
            model: self.use_cases.routing.model.clone().unwrap_or_else(|| self.model.clone()),
            max_tokens: self.use_cases.routing.max_tokens.unwrap_or(self.max_tokens),
            temperature: self.use_cases.routing.temperature.unwrap_or(self.temperature),
            timeout_seconds: self.timeout_seconds,
            ..Default::default()
        };
        
        let client = LlmClient::new(client_config).map_err(|e| {
            anyhow::anyhow!("Failed to initialize routing LLM client: {}", e)
        })?;
        
        Ok(client)
    }
    
    /// Creates a client for decomposition operations with use-case specific overrides
    pub fn create_decomposition_client(&self) -> anyhow::Result<LlmClient> {
        let api_key = self.resolve_api_key()?;
        
        let client_config = LlmClientConfig {
            api_key,
            model: self.use_cases.decomposition.model.clone().unwrap_or_else(|| self.model.clone()),
            max_tokens: self.use_cases.decomposition.max_tokens.unwrap_or(self.max_tokens),
            temperature: self.use_cases.decomposition.temperature.unwrap_or(self.temperature),
            timeout_seconds: self.timeout_seconds,
            ..Default::default()
        };
        
        let client = LlmClient::new(client_config).map_err(|e| {
            anyhow::anyhow!("Failed to initialize decomposition LLM client: {}", e)
        })?;
        
        Ok(client)
    }
    
    /// Creates a client for synthesis operations with use-case specific overrides
    pub fn create_synthesis_client(&self) -> anyhow::Result<LlmClient> {
        let api_key = self.resolve_api_key()?;
        
        let client_config = LlmClientConfig {
            api_key,
            model: self.use_cases.synthesis.model.clone().unwrap_or_else(|| self.model.clone()),
            max_tokens: self.use_cases.synthesis.max_tokens.unwrap_or(self.max_tokens),
            temperature: self.use_cases.synthesis.temperature.unwrap_or(self.temperature),
            timeout_seconds: self.timeout_seconds,
            ..Default::default()
        };
        
        let client = LlmClient::new(client_config).map_err(|e| {
            anyhow::anyhow!("Failed to initialize synthesis LLM client: {}", e)
        })?;
        
        Ok(client)
    }
    
    /// Gets the routing prompt template
    pub fn routing_prompt_template(&self) -> &str {
        &self.use_cases.routing.prompt_template
    }
    
    /// Gets the decomposition prompt template
    pub fn decomposition_prompt_template(&self) -> &str {
        &self.use_cases.decomposition.prompt_template
    }
    
    /// Gets the synthesis prompt template
    pub fn synthesis_prompt_template(&self) -> &str {
        &self.use_cases.synthesis.prompt_template
    }
}

/// Authentication configuration.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    /// Authentication type required by this agent's server component.
    #[serde(default)]
    pub server_auth_type: ServerAuthType,
    /// Credentials this agent uses when acting as a client.
    /// Keyed by authentication scheme (e.g., "Bearer", "ApiKey").
    #[serde(default)]
    pub client_credentials: HashMap<String, String>,
    /// Path to the client certificate for mTLS (when acting as client).
    pub client_cert_path: Option<String>,
    /// Path to the client private key for mTLS (when acting as client).
    pub client_key_path: Option<String>,
}

/// Types of authentication the server component can require.
#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServerAuthType {
    #[default]
    None,
    Bearer,
    ApiKey,
    // Add other types like OAuth2 later if needed
}

/// Network configuration.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    /// Optional HTTP/HTTPS proxy URL for outgoing client requests.
    pub proxy_url: Option<String>,
    /// Optional proxy authentication (username, password).
    pub proxy_auth: Option<(String, String)>,
    /// Optional path to a custom CA certificate bundle for outgoing TLS verification.
    pub ca_cert_path: Option<String>,
    // Add server TLS config later (cert/key paths)
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            proxy_url: None,
            proxy_auth: None,
            ca_cert_path: None,
        }
    }
}

/// Loads the agent configuration from a TOML file.
pub fn load_config(path: &str) -> Result<BidirectionalAgentConfig> {
    let config_path = Path::new(path);
    let config_str = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read configuration file: {}", path))?;

    let config: BidirectionalAgentConfig = toml::from_str(&config_str)
        .with_context(|| format!("Failed to parse TOML configuration from: {}", path))?;

    // Basic validation (more can be added)
    if config.self_id.is_empty() {
        anyhow::bail!("Configuration error: 'self_id' cannot be empty.");
    }
    if config.base_url.is_empty() {
         anyhow::bail!("Configuration error: 'base_url' cannot be empty.");
    }
    
    // Validate LLM configuration if routing is enabled
    #[cfg(feature = "bidir-local-exec")]
    if config.llm.use_for_routing {
        config.llm.validate()
            .with_context(|| "Failed to validate LLM configuration")?;
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_minimal_config() {
        let config_str = r#"
            self_id = "agent1@example.com"
            base_url = "http://localhost:8081"
        "#;
        let config: BidirectionalAgentConfig = toml::from_str(config_str).unwrap();
        assert_eq!(config.self_id, "agent1@example.com");
        assert_eq!(config.base_url, "http://localhost:8081");
        assert!(config.discovery.is_empty());
        assert_eq!(config.auth.server_auth_type, ServerAuthType::None);
        assert!(config.auth.client_credentials.is_empty());
        assert!(config.network.proxy_url.is_none());
        #[cfg(feature = "bidir-local-exec")]
        assert!(config.tools.specific_configs.is_empty());
        #[cfg(feature = "bidir-delegate")]
        assert_eq!(config.tool_discovery_interval_minutes, default_tool_discovery_interval()); // Check default
        
        // Check LLM config defaults
        #[cfg(feature = "bidir-local-exec")]
        {
            assert_eq!(config.llm.use_for_routing, false);
            assert_eq!(config.llm.use_new_core, false);
            assert_eq!(config.llm.provider, "claude");
            assert_eq!(config.llm.api_key, None);
            assert_eq!(config.llm.api_key_env_var, "ANTHROPIC_API_KEY");
            assert_eq!(config.llm.model, "claude-3-haiku-20240307");
            assert_eq!(config.llm.api_base_url, None);
            assert_eq!(config.llm.max_tokens, 2048);
            assert_eq!(config.llm.temperature, 0.1);
            assert_eq!(config.llm.timeout_seconds, 30);
        }
    }
    
    #[test]
    #[cfg(feature = "bidir-local-exec")]
    fn test_llm_config_parsing() {
        let config_str = r#"
            self_id = "agent1@example.com"
            base_url = "http://localhost:8081"
            
            [llm]
            use_for_routing = true
            use_new_core = true
            provider = "claude"
            api_key = "sk-ant-api03-test-key"
            api_key_env_var = "CUSTOM_API_KEY"
            model = "claude-3-sonnet-20240229"
            api_base_url = "https://custom-anthropic-api.example.com"
            max_tokens = 4096
            temperature = 0.7
            timeout_seconds = 60
            
            [llm.use_cases.routing]
            model = "claude-3-haiku-20240307"
            max_tokens = 1024
            temperature = 0.2
            
            [llm.use_cases.decomposition]
            model = "claude-3-opus-20240229"
            max_tokens = 8192
            temperature = 0.3
            
            [llm.use_cases.synthesis]
            model = "claude-3-sonnet-20240229"
            max_tokens = 2048
            temperature = 0.4
        "#;
        
        let config: BidirectionalAgentConfig = toml::from_str(config_str).unwrap();
        
        // Test base LLM config
        assert_eq!(config.llm.use_for_routing, true);
        assert_eq!(config.llm.use_new_core, true);
        assert_eq!(config.llm.provider, "claude");
        assert_eq!(config.llm.api_key, Some("sk-ant-api03-test-key".to_string()));
        assert_eq!(config.llm.api_key_env_var, "CUSTOM_API_KEY");
        assert_eq!(config.llm.model, "claude-3-sonnet-20240229");
        assert_eq!(config.llm.api_base_url, Some("https://custom-anthropic-api.example.com".to_string()));
        assert_eq!(config.llm.max_tokens, 4096);
        assert_eq!(config.llm.temperature, 0.7);
        assert_eq!(config.llm.timeout_seconds, 60);
        
        // Test routing config
        assert_eq!(config.llm.use_cases.routing.model, Some("claude-3-haiku-20240307".to_string()));
        assert_eq!(config.llm.use_cases.routing.max_tokens, Some(1024));
        assert_eq!(config.llm.use_cases.routing.temperature, Some(0.2));
        
        // Test decomposition config
        assert_eq!(config.llm.use_cases.decomposition.model, Some("claude-3-opus-20240229".to_string()));
        assert_eq!(config.llm.use_cases.decomposition.max_tokens, Some(8192));
        assert_eq!(config.llm.use_cases.decomposition.temperature, Some(0.3));
        
        // Test synthesis config
        assert_eq!(config.llm.use_cases.synthesis.model, Some("claude-3-sonnet-20240229".to_string()));
        assert_eq!(config.llm.use_cases.synthesis.max_tokens, Some(2048));
        assert_eq!(config.llm.use_cases.synthesis.temperature, Some(0.4));
    }
    
    #[test]
    #[cfg(feature = "bidir-local-exec")]
    fn test_llm_config_validation() {
        // Test invalid provider
        let config = LlmConfig {
            provider: "invalid".to_string(),
            use_for_routing: true,
            ..Default::default()
        };
        assert!(config.validate().is_err());
        
        // Test invalid temperature
        let config = LlmConfig {
            temperature: 1.5,
            use_for_routing: true,
            ..Default::default()
        };
        assert!(config.validate().is_err());
        
        let config = LlmConfig {
            temperature: -0.1,
            use_for_routing: true,
            ..Default::default()
        };
        assert!(config.validate().is_err());
        
        // Test invalid max tokens
        let config = LlmConfig {
            max_tokens: 0,
            use_for_routing: true,
            ..Default::default()
        };
        assert!(config.validate().is_err());
        
        // Test invalid timeout
        let config = LlmConfig {
            timeout_seconds: 0,
            use_for_routing: true,
            ..Default::default()
        };
        assert!(config.validate().is_err());
        
        // Test invalid use case override
        let mut use_cases = LlmUseCases::default();
        use_cases.routing.temperature = Some(2.0);
        
        let config = LlmConfig {
            use_cases,
            use_for_routing: true,
            ..Default::default()
        };
        assert!(config.validate().is_err());
        
        // Test valid config
        let config = LlmConfig {
            provider: "claude".to_string(),
            api_key: Some("test-key".to_string()),
            temperature: 0.5,
            max_tokens: 100,
            timeout_seconds: 30,
            use_for_routing: true,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }
}

    #[test]
    fn test_load_full_config() {
        #[cfg(feature = "bidir-local-exec")]
        let config_str = r#"
            self_id = "agent2"
            base_url = "https://agent2.example.com"
            discovery = ["http://agent1.example.com", "http://agent3.example.com"]

            [auth]
            server_auth_type = "bearer"
            client_credentials = { Bearer = "agent2-secret-token", ApiKey = "xyz789" }
            client_cert_path = "/path/to/client.crt"
            client_key_path = "/path/to/client.key"

            [network]
            proxy_url = "http://proxy.example.com:8080"
            proxy_auth = ["proxy_user", "proxy_pass"]
            ca_cert_path = "/path/to/ca.pem"

            [tools]
            shell_allowed_commands = ["ls", "echo"] # Example tool config
            http_max_redirects = 5
            
            [llm]
            use_for_routing = true
            use_new_core = true
            provider = "claude"
            api_key = "sk-ant-api03-example-key"
            model = "claude-3-opus-20240229"
            max_tokens = 4096
            temperature = 0.5
            
            [llm.use_cases.routing]
            model = "claude-3-haiku-20240307"
            max_tokens = 1024
            
            [llm.use_cases.decomposition]
            temperature = 0.3
            
            [llm.use_cases.synthesis]
            max_tokens = 8192

            [directory] # Add directory config section
            db_path = "/var/lib/myagent/directory.sqlite"
            verification_interval_minutes = 10
            max_failures_before_inactive = 5
            backoff_seconds = 30
            health_endpoint_path = "/api/v1/status"

            # Add tool discovery interval if delegate feature is on
            #[cfg(feature = "bidir-delegate")]
            tool_discovery_interval_minutes = 45
        "#;

        #[cfg(not(feature = "bidir-local-exec"))]
        let config_str = r#"
            self_id = "agent2"
            base_url = "https://agent2.example.com"
            discovery = ["http://agent1.example.com", "http://agent3.example.com"]

            [auth]
            server_auth_type = "bearer"
            client_credentials = { Bearer = "agent2-secret-token", ApiKey = "xyz789" }
            client_cert_path = "/path/to/client.crt"
            client_key_path = "/path/to/client.key"

            [network]
            proxy_url = "http://proxy.example.com:8080"
            proxy_auth = ["proxy_user", "proxy_pass"]
            ca_cert_path = "/path/to/ca.pem"

            [directory] # Add directory config section
            db_path = "/var/lib/myagent/directory.sqlite"
            verification_interval_minutes = 10
            max_failures_before_inactive = 5
            backoff_seconds = 30
            health_endpoint_path = "/api/v1/status"

            # Add tool discovery interval if delegate feature is on
            #[cfg(feature = "bidir-delegate")]
            tool_discovery_interval_minutes = 45
        "#;

        let config: BidirectionalAgentConfig = toml::from_str(config_str).unwrap();
        assert_eq!(config.self_id, "agent2");
        assert_eq!(config.discovery.len(), 2);
        assert_eq!(config.auth.server_auth_type, ServerAuthType::Bearer);
        assert_eq!(config.auth.client_credentials.get("Bearer").unwrap(), "agent2-secret-token");
        assert_eq!(config.auth.client_cert_path, Some("/path/to/client.crt".to_string()));
        assert_eq!(config.network.proxy_url, Some("http://proxy.example.com:8080".to_string()));
        assert_eq!(config.network.proxy_auth, Some(("proxy_user".to_string(), "proxy_pass".to_string())));
        assert_eq!(config.network.ca_cert_path, Some("/path/to/ca.pem".to_string()));

        #[cfg(feature = "bidir-local-exec")]
        {
            // Check tool configs
            assert!(config.tools.specific_configs.contains_key("shell_allowed_commands"));
            assert!(config.tools.specific_configs.contains_key("http_max_redirects"));
            assert_eq!(config.tools.specific_configs["http_max_redirects"], Value::Number(5.into())); // Use Value::Number
            
            // Check LLM configs
            assert_eq!(config.llm.use_for_routing, true);
            assert_eq!(config.llm.use_new_core, true);
            assert_eq!(config.llm.provider, "claude");
            assert_eq!(config.llm.api_key, Some("sk-ant-api03-example-key".to_string()));
            assert_eq!(config.llm.model, "claude-3-opus-20240229");
            assert_eq!(config.llm.max_tokens, 4096);
            assert_eq!(config.llm.temperature, 0.5);
            
            // Check use case specific configs
            assert_eq!(config.llm.use_cases.routing.model, Some("claude-3-haiku-20240307".to_string()));
            assert_eq!(config.llm.use_cases.routing.max_tokens, Some(1024));
            assert_eq!(config.llm.use_cases.routing.temperature, None);
            
            assert_eq!(config.llm.use_cases.decomposition.model, None);
            assert_eq!(config.llm.use_cases.decomposition.max_tokens, None);
            assert_eq!(config.llm.use_cases.decomposition.temperature, Some(0.3));
            
            assert_eq!(config.llm.use_cases.synthesis.model, None);
            assert_eq!(config.llm.use_cases.synthesis.max_tokens, Some(8192));
            assert_eq!(config.llm.use_cases.synthesis.temperature, None);
        }

        // Assert directory config is loaded correctly
        #[cfg(feature = "bidir-core")]
        {
            assert_eq!(config.directory.db_path, "/var/lib/myagent/directory.sqlite");
            assert_eq!(config.directory.verification_interval_minutes, 10);
            assert_eq!(config.directory.max_failures_before_inactive, 5);
            assert_eq!(config.directory.backoff_seconds, 30);
            assert_eq!(config.directory.health_endpoint_path, "/api/v1/status");
        }
        #[cfg(feature = "bidir-delegate")]
        {
             assert_eq!(config.tool_discovery_interval_minutes, 30); // Check default value
        }
    }

     #[test]
    fn test_load_config_missing_required() {
        let config_str = r#"
            base_url = "http://localhost:8081"
        "#; // Missing self_id
        // TOML parsing itself doesn't enforce required fields defined in struct,
        // our load_config function does.
        // Let's simulate calling load_config indirectly.
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("bad_config.toml");
        std::fs::write(&file_path, config_str).unwrap();

        let load_result = load_config(file_path.to_str().unwrap());
        assert!(load_result.is_err());
        // Store the error before unwrapping to avoid consuming the Result
        let err = load_result.unwrap_err();
        println!("Error (Display): {}", err); // Print the Display representation for debugging
        println!("Error (Debug): {:?}", err); // Keep Debug representation for more detail if needed
        println!("Error string for assertion: '{}'", err.to_string()); // Print the exact string
        // Check the root cause for the specific TOML deserialization error message
        let root_cause = err.root_cause();
        assert!(root_cause.to_string().contains("missing field `self_id`"));
    }
}


#[cfg(feature = "bidir-core")]
/// Configuration specific to the Agent Directory feature.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)] // Makes the whole [directory] section optional in TOML
pub struct DirectoryConfig {
    /// Path to the SQLite database file. Can be relative (to CWD) or absolute.
    pub db_path: String,
    /// Default interval (in minutes) between successful agent verification checks.
    /// Actual checks depend on `next_probe_at`.
    pub verification_interval_minutes: u64,
    /// Timeout (in seconds) for HTTP requests made during agent verification (HEAD/GET).
    pub request_timeout_seconds: u64,
    /// Number of *consecutive* verification failures required to mark an agent as inactive.
    pub max_failures_before_inactive: u32,
    /// Base duration (in seconds) for exponential backoff after the *first* failure.
    /// The delay before the next check will be `backoff_seconds * 2^(failures - 1)`.
    pub backoff_seconds: u64,
    /// Optional path (e.g., "/health", "/status") appended to the agent's base URL
    /// for the GET request fallback during liveness checks. If empty, uses the base URL.
    pub health_endpoint_path: String,
}

#[cfg(feature = "bidir-core")]
impl Default for DirectoryConfig {
    fn default() -> Self {
        Self {
            // Default to storing DB in a 'data' subdirectory relative to executable CWD.
            // Ensure this directory exists or can be created.
            db_path: "data/agent_directory.db".to_string(),
            verification_interval_minutes: 15,
            request_timeout_seconds: 10,
            max_failures_before_inactive: 3, // Mark inactive after 3 consecutive failures
            backoff_seconds: 60, // Start with 1 minute backoff after first failure
            health_endpoint_path: "/health".to_string(), // Common default health check path
        }
    }
}
