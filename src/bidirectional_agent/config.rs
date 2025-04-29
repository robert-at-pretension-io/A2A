/// Configuration structures for the Bidirectional Agent.

#[cfg(feature = "bidir-core")]

use serde::Deserialize;
use std::{collections::HashMap, path::Path};
use anyhow::{Context, Result};
use serde_json::Value;

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
    // Add fields for routing policy etc. in later slices
}

/// Tool configurations. Keyed by tool name.
#[cfg(feature = "bidir-local-exec")]
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ToolConfigs {
    /// Whether to use LLM-based routing
    #[serde(default)]
    pub use_llm_routing: bool,
    
    /// API key for LLM service
    pub llm_api_key: Option<String>,
    
    /// LLM model to use for routing decisions
    #[serde(default = "default_llm_model")]
    pub llm_model: String,
    
    /// Other tool-specific configurations
    #[serde(flatten)]
    pub specific_configs: HashMap<String, Value>, // Allows arbitrary tool configs
}

fn default_llm_model() -> String {
    "claude-3-haiku-20240307".to_string()
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
#[derive(Deserialize, Debug, Clone, Default)]
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
    }

    #[test]
    fn test_load_full_config() {
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
            assert!(config.tools.specific_configs.contains_key("shell_allowed_commands"));
            assert!(config.tools.specific_configs.contains_key("http_max_redirects"));
            assert_eq!(config.tools.specific_configs["http_max_redirects"], Value::Integer(5));
        }
    }

     #[test]
    fn test_load_config_missing_required() {
        let config_str = r#"
            base_url = "http://localhost:8081"
        "#; // Missing self_id
        let result: Result<BidirectionalAgentConfig, _> = toml::from_str(config_str);
        // TOML parsing itself doesn't enforce required fields defined in struct,
        // our load_config function does.
        // Let's simulate calling load_config indirectly.
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("bad_config.toml");
        std::fs::write(&file_path, config_str).unwrap();

        let load_result = load_config(file_path.to_str().unwrap());
        assert!(load_result.is_err());
        assert!(load_result.unwrap_err().to_string().contains("'self_id' cannot be empty"));
    }
}
/// Configuration structures for the Bidirectional Agent.

#[cfg(feature = "bidir-core")]

use serde::Deserialize;
use std::{collections::HashMap, path::Path};
use anyhow::{Context, Result};
use serde_json::Value;

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
    /// Network configuration (proxy, TLS, bind address/port).
    #[serde(default)]
    pub network: NetworkConfig,
    /// Tool-specific configurations.
    #[cfg(feature = "bidir-local-exec")]
    #[serde(default)]
    pub tools: ToolConfigs,
    // Add fields for routing policy etc. in later slices
}

// Implement Default for BidirectionalAgentConfig
impl Default for BidirectionalAgentConfig {
    fn default() -> Self {
        Self {
            self_id: "default-bidir-agent".to_string(),
            base_url: "http://localhost:8080".to_string(),
            discovery: vec![],
            auth: AuthConfig::default(),
            network: NetworkConfig::default(),
            #[cfg(feature = "bidir-local-exec")]
            tools: ToolConfigs::default(),
        }
    }
}


/// Tool configurations. Keyed by tool name.
#[cfg(feature = "bidir-local-exec")]
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ToolConfigs {
    /// Whether to use LLM-based routing
    #[serde(default)]
    pub use_llm_routing: bool,

    /// API key for LLM service
    pub llm_api_key: Option<String>,

    /// LLM model to use for routing decisions
    #[serde(default = "default_llm_model")]
    pub llm_model: String,

    /// Other tool-specific configurations
    #[serde(flatten)]
    pub specific_configs: HashMap<String, Value>, // Allows arbitrary tool configs
}

#[cfg(feature = "bidir-local-exec")]
fn default_llm_model() -> String {
    "claude-3-haiku-20240307".to_string()
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
    /// Address to bind the server to (e.g., "127.0.0.1", "0.0.0.0").
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    /// Port to bind the server to. If None, a default (e.g., 8080) might be used.
    pub port: Option<u16>,
    /// Optional HTTP/HTTPS proxy URL for outgoing client requests.
    pub proxy_url: Option<String>,
    /// Optional proxy authentication (username, password).
    pub proxy_auth: Option<(String, String)>,
    /// Optional path to a custom CA certificate bundle for outgoing TLS verification.
    pub ca_cert_path: Option<String>,
    // Add server TLS config later (cert/key paths)
}

// Default bind address
fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}

// Implement Default for NetworkConfig
impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            port: Some(8080), // Default port
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
        assert_eq!(config.network.bind_address, "127.0.0.1"); // Check default bind address
        assert_eq!(config.network.port, Some(8080)); // Check default port
        #[cfg(feature = "bidir-local-exec")]
        assert!(config.tools.specific_configs.is_empty());
    }

    #[test]
    fn test_load_full_config() {
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
            bind_address = "0.0.0.0"
            port = 9000
            proxy_url = "http://proxy.example.com:8080"
            proxy_auth = ["proxy_user", "proxy_pass"]
            ca_cert_path = "/path/to/ca.pem"

            [tools]
            use_llm_routing = true
            llm_api_key = "sk-..."
            llm_model = "claude-opus"
            shell_allowed_commands = ["ls", "echo"] # Example tool config
            http_max_redirects = 5
        "#;
        let config: BidirectionalAgentConfig = toml::from_str(config_str).unwrap();
        assert_eq!(config.self_id, "agent2");
        assert_eq!(config.discovery.len(), 2);
        assert_eq!(config.auth.server_auth_type, ServerAuthType::Bearer);
        assert_eq!(config.auth.client_credentials.get("Bearer").unwrap(), "agent2-secret-token");
        assert_eq!(config.auth.client_cert_path, Some("/path/to/client.crt".to_string()));
        assert_eq!(config.network.bind_address, "0.0.0.0");
        assert_eq!(config.network.port, Some(9000));
        assert_eq!(config.network.proxy_url, Some("http://proxy.example.com:8080".to_string()));
        assert_eq!(config.network.proxy_auth, Some(("proxy_user".to_string(), "proxy_pass".to_string())));
        assert_eq!(config.network.ca_cert_path, Some("/path/to/ca.pem".to_string()));

        #[cfg(feature = "bidir-local-exec")]
        {
            assert!(config.tools.use_llm_routing);
            assert_eq!(config.tools.llm_api_key, Some("sk-...".to_string()));
            assert_eq!(config.tools.llm_model, "claude-opus");
            assert!(config.tools.specific_configs.contains_key("shell_allowed_commands"));
            assert!(config.tools.specific_configs.contains_key("http_max_redirects"));
            assert_eq!(config.tools.specific_configs["http_max_redirects"], Value::Integer(5));
        }
    }

     #[test]
    fn test_load_config_missing_required() {
        let config_str = r#"
            base_url = "http://localhost:8081"
        "#; // Missing self_id
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("bad_config.toml");
        std::fs::write(&file_path, config_str).unwrap();

        let load_result = load_config(file_path.to_str().unwrap());
        assert!(load_result.is_err());
        assert!(load_result.unwrap_err().to_string().contains("'self_id' cannot be empty"));

        let config_str_no_base = r#"
            self_id = "agent1"
        "#; // Missing base_url
        let file_path_no_base = temp_dir.path().join("bad_config_no_base.toml");
        std::fs::write(&file_path_no_base, config_str_no_base).unwrap();
        let load_result_no_base = load_config(file_path_no_base.to_str().unwrap());
        assert!(load_result_no_base.is_err());
        assert!(load_result_no_base.unwrap_err().to_string().contains("'base_url' cannot be empty"));
    }
}
