//! Configuration structures and loading logic for the Bidirectional Agent.

use anyhow::{anyhow, Result};
use serde::{Deserialize}; // Removed Serialize
use std::fs;
use std::path::Path;
use toml;
use tracing::{debug, info, trace, warn}; // Use tracing for logging within config loading
use uuid::Uuid;

// Constants used for defaults
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0";
const SYSTEM_PROMPT: &str = r#"
You are an AI agent assistant that helps with tasks. You can:
1. Process tasks directly (for simple questions or tasks you can handle)
2. Delegate tasks to other agents when appropriate
3. Use tools when needed

Always think step-by-step about the best way to handle each request.
"#;

// --- Configuration Structs ---

#[derive(Clone, Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_agent_id")]
    pub agent_id: String,
    // Optional name for the agent card, defaults to agent_id if not set
    pub agent_name: Option<String>,
}

/// Client configuration section
#[derive(Clone, Debug, Deserialize)]
pub struct ClientConfig {
    pub target_url: Option<String>,
}

/// LLM configuration section
#[derive(Clone, Debug, Deserialize)]
pub struct LlmConfig {
    pub claude_api_key: Option<String>,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
}

/// Tool configuration section
#[derive(Clone, Debug, Deserialize, Default)]
pub struct ToolsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,

    /// Path to store/load the agent directory as JSON
    #[serde(default)]
    pub agent_directory_path: Option<String>,
}

/// Configuration for the bidirectional agent
#[derive(Clone, Debug, Deserialize)]
pub struct BidirectionalAgentConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)] // <-- Add default back
    pub client: ClientConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub tools: ToolsConfig, // <-- new

    // Mode configuration
    #[serde(default)]
    pub mode: ModeConfig,

    // Path to the config file (for reference)
    #[serde(skip)]
    pub config_file_path: Option<String>,
}

/// Operation mode configuration
#[derive(Clone, Debug, Deserialize, Default)]
pub struct ModeConfig {
    // Interactive REPL mode
    #[serde(default)]
    pub repl: bool,

    // Direct message to process (non-interactive mode)
    pub message: Option<String>,

    // Remote agent operations
    #[serde(default)] // Make this optional, defaults to false if missing
    pub get_agent_card: bool,
    pub remote_task: Option<String>,

    // Auto-listen on server port at startup
    #[serde(default)]
    pub auto_listen: bool,

    // Optional file to append REPL interactions (input/output)
    #[serde(default)]
    pub repl_log_file: Option<String>,
}

// --- Default Functions ---

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_bind_address() -> String {
    DEFAULT_BIND_ADDRESS.to_string()
}

fn default_agent_id() -> String {
    format!("bidirectional-{}", Uuid::new_v4())
}

fn default_system_prompt() -> String {
    SYSTEM_PROMPT.to_string()
}

// --- Default Implementations ---

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            bind_address: default_bind_address(),
            agent_id: default_agent_id(),
            agent_name: None, // Initialize the optional agent_name field
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            target_url: None,
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            claude_api_key: None,
            system_prompt: default_system_prompt(),
        }
    }
}

impl Default for BidirectionalAgentConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            client: ClientConfig::default(),
            llm: LlmConfig::default(),
            tools: ToolsConfig::default(), // <-- new
            mode: ModeConfig::default(),
            config_file_path: None,
        }
    }
}

// --- Config Loading Logic ---

impl BidirectionalAgentConfig {
    /// Load configuration from a TOML file
    // Note: This uses tracing, assuming logging might be partially set up
    // before full config load, or that these logs are acceptable if not captured.
    #[tracing::instrument(skip(path), fields(path = %path.as_ref().display()))]
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        debug!("Loading configuration from file.");
        let config_str = fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        trace!(content_len = config_str.len(), "Read config file content.");

        debug!("Parsing TOML configuration.");
        let mut config: BidirectionalAgentConfig = toml::from_str(&config_str)
            .map_err(|e| anyhow!("Failed to parse config file: {}", e))?;
        trace!("TOML parsing successful.");

        // Check for environment variable override for API key
        debug!("Checking for CLAUDE_API_KEY environment variable override.");
        if config.llm.claude_api_key.is_none() {
            if let Ok(env_key) = std::env::var("CLAUDE_API_KEY") {
                info!("Using Claude API key from environment variable.");
                config.llm.claude_api_key = Some(env_key);
            } else {
                debug!("CLAUDE_API_KEY environment variable not found.");
            }
        } else {
            debug!("Claude API key already set in config file.");
        }

        // Store the path from which the config was loaded
        config.config_file_path = Some(path.as_ref().display().to_string());

        debug!("Configuration loaded successfully from file.");
        Ok(config)
    }
}

/// Helper function to load config from path, used in main arg parsing.
/// Logs using eprintln because tracing might not be initialized yet.
pub fn load_config_from_path(config: &mut BidirectionalAgentConfig, config_path: &str) -> Result<()> {
    eprintln!("[PRE-LOG] Attempting to load configuration from path: {}", config_path);
    match BidirectionalAgentConfig::from_file(config_path) { // from_file now uses debug/trace internally if logging is up
        Ok(mut loaded_config) => {
            eprintln!("[PRE-LOG] Config file '{}' loaded successfully. Merging with existing/default config.", config_path);
            // Preserve env var API key if it was set and config file doesn't have one
            if config.llm.claude_api_key.is_some() && loaded_config.llm.claude_api_key.is_none() {
                eprintln!("[PRE-LOG] Preserving Claude API key from environment variable over config file.");
                loaded_config.llm.claude_api_key = config.llm.claude_api_key.clone();
            }
            // Preserve command-line overrides if they were set before loading the file
            // Example: Preserve port if set via --port= or --listen before the config file path
             if config.server.port != default_port() && loaded_config.server.port == default_port() {
                 eprintln!("[PRE-LOG] Preserving server port override ({}) from command line over config file.", config.server.port);
                 loaded_config.server.port = config.server.port;
             }
             // Example: Preserve target_url if set via host:port before the config file path
             if config.client.target_url.is_some() && loaded_config.client.target_url.is_none() {
                 eprintln!("[PRE-LOG] Preserving target URL override ('{}') from command line over config file.", config.client.target_url.as_deref().unwrap_or("N/A"));
                 loaded_config.client.target_url = config.client.target_url.clone();
             }
             // Example: Preserve auto_listen if set via --listen before the config file path
             if config.mode.auto_listen && !loaded_config.mode.auto_listen {
                 eprintln!("[PRE-LOG] Preserving auto-listen override from command line over config file.");
                 loaded_config.mode.auto_listen = true;
             }
             // Preserve REPL log file if set via command line? (Less common, maybe not needed)

            // Preserve the config file path itself
            loaded_config.config_file_path = Some(config_path.to_string());
            eprintln!("[PRE-LOG] Setting config_file_path reference to: {}", config_path);

            *config = loaded_config; // Overwrite existing config with loaded, potentially merged, config
            eprintln!("[PRE-LOG] Successfully loaded and applied configuration from {}", config_path);
        },
        Err(e) => {
            // If a config file was specified but failed to load, it's a fatal error.
            eprintln!("[PRE-LOG] ERROR: Failed to load configuration from '{}': {}", config_path, e);
            eprintln!("[PRE-LOG] Please check the configuration file path and syntax.");
            // Use context to chain the error
            return Err(anyhow!("Configuration file loading failed for path: {}", config_path).context(e));
        }
    }
    Ok(())
}
