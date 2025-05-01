/// Configuration structures for the Bidirectional Agent.

#[cfg(feature = "bidir-core")]

use serde::Deserialize;
use std::{collections::HashMap, path::Path};
use anyhow::{Context, Result};
use serde_json::Value;
use tempfile::tempdir; // Import tempdir for tests

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
            assert!(config.tools.specific_configs.contains_key("shell_allowed_commands"));
            assert!(config.tools.specific_configs.contains_key("http_max_redirects"));
            assert_eq!(config.tools.specific_configs["http_max_redirects"], Value::Number(5.into())); // Use Value::Number
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
             assert_eq!(config.tool_discovery_interval_minutes, 45); // Check specific value
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
