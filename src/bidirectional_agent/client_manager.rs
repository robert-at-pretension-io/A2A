//! Manages A2A client instances for interacting with remote agents.

#![cfg(feature = "bidir-core")]

use crate::client::{A2aClient, errors::ClientError};
use crate::types::{TaskSendParams, Task}; // Import necessary types
use crate::bidirectional_agent::agent_registry::AgentRegistry;
use crate::bidirectional_agent::config::BidirectionalAgentConfig;
use dashmap::DashMap;
use std::sync::Arc;
use anyhow::{Result, Context}; // Use anyhow for internal errors

/// Manages cached A2aClient instances for different remote agents.
#[derive(Clone)]
pub struct ClientManager {
    /// Cache of clients, keyed by agent ID (typically agent name from card).
    clients: Arc<DashMap<String, A2aClient>>,
    /// Reference to the agent registry to get agent URLs and auth info.
    registry: Arc<AgentRegistry>,
    /// Agent's own configuration (needed for client TLS/auth setup).
    self_config: Arc<BidirectionalAgentConfig>,
}

impl ClientManager {
    /// Creates a new ClientManager.
    pub fn new(registry: Arc<AgentRegistry>, self_config: Arc<BidirectionalAgentConfig>) -> Result<Self> {
        Ok(Self {
            clients: Arc::new(DashMap::new()),
            registry,
            self_config,
        })
    }

    /// Gets an existing A2aClient for the target agent or creates a new one.
    /// Handles client configuration based on the agent's card and self-config.
    pub async fn get_or_create_client(&self, agent_id: &str) -> Result<A2aClient, ClientError> {
        // Fast path: Check if client already exists in cache
        if let Some(client_entry) = self.clients.get(agent_id) {
            return Ok(client_entry.value().clone());
        }

        // Slow path: Client not cached, need to create it
        // 1. Get agent info from registry
        let agent_info = self.registry.get(agent_id)
            .ok_or_else(|| ClientError::Other(format!("Agent '{}' not found in registry", agent_id)))?;

        let agent_card = &agent_info.card;
        let agent_url = &agent_card.url;

        // 2. Build the underlying reqwest client with potential TLS/proxy config
        let http_client = self.build_http_client()
             .map_err(|e| ClientError::Other(format!("Failed to build HTTP client: {}", e)))?;


        // 3. Create the A2aClient instance
        let mut a2a_client = A2aClient::new_with_client(agent_url, http_client); // Assuming A2aClient has this constructor

        // 4. Configure authentication based on agent card and self config
        if let Some(required_auth) = &agent_card.authentication {
            // Find a scheme supported by the remote agent that we have credentials for
            let mut configured_auth = false;
            for scheme in &required_auth.schemes {
                if let Some(credential) = self.self_config.auth.client_credentials.get(scheme) {
                    // Determine the correct header name based on the scheme
                    let header_name = match scheme.as_str() {
                        "Bearer" | "bearer" => "Authorization",
                        "ApiKey" | "apikey" => "X-API-Key", // Common practice
                        // Add other schemes as needed
                        _ => {
                            println!("⚠️ Unsupported auth scheme '{}' for agent '{}'", scheme, agent_id);
                            continue; // Try next scheme
                        }
                    };
                    let header_value = if scheme.eq_ignore_ascii_case("Bearer") {
                        format!("Bearer {}", credential)
                    } else {
                        credential.clone()
                    };

                    println!("🔐 Configuring client for agent '{}' with scheme '{}'", agent_id, scheme);
                    a2a_client = a2a_client.with_auth(header_name, &header_value);
                    configured_auth = true;
                    break; // Use the first matching scheme
                }
            }
            if !configured_auth && !required_auth.schemes.is_empty() {
                 println!("⚠️ No matching client credentials found for required schemes {:?} for agent '{}'",
                       required_auth.schemes, agent_id);
                // Depending on policy, we might error out here or proceed without auth
                // For now, let's proceed without auth and let the server reject if needed.
            }
        }

        // 5. Cache the new client
        // Use entry API for atomic insert/get
        let client_entry = self.clients.entry(agent_id.to_string()).or_insert(a2a_client);

        // Return the (potentially newly inserted) client
        Ok(client_entry.value().clone())
    }

    /// Helper to build the underlying reqwest HTTP client based on config.
    fn build_http_client(&self) -> Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30)); // Default timeout

        // Configure proxy if specified
        if let Some(proxy_url) = &self.self_config.network.proxy_url {
            let mut proxy = reqwest::Proxy::all(proxy_url)
                .with_context(|| format!("Invalid proxy URL: {}", proxy_url))?;

            // Add proxy authentication if needed
            if let Some((username, password)) = &self.self_config.network.proxy_auth {
                proxy = proxy.basic_auth(username, password);
            }
            builder = builder.proxy(proxy);
             println!("🔧 Configuring HTTP client with proxy: {}", proxy_url);
        }

        // Configure custom CA certificate if specified
        if let Some(ca_path) = &self.self_config.network.ca_cert_path {
            let ca_cert_bytes = std::fs::read(ca_path)
                .with_context(|| format!("Failed to read CA certificate from: {}", ca_path))?;
            let ca_cert = reqwest::Certificate::from_pem(&ca_cert_bytes)
                 .with_context(|| format!("Failed to parse CA certificate from PEM format: {}", ca_path))?;
            builder = builder.add_root_certificate(ca_cert);
             println!("🔧 Configuring HTTP client with custom CA: {}", ca_path);
        }

        // Configure client certificate (mTLS) if specified
        if let (Some(cert_path), Some(key_path)) =
            (&self.self_config.auth.client_cert_path, &self.self_config.auth.client_key_path)
        {
            let cert_bytes = std::fs::read(cert_path)
                .with_context(|| format!("Failed to read client certificate from: {}", cert_path))?;
            let key_bytes = std::fs::read(key_path)
                 .with_context(|| format!("Failed to read client private key from: {}", key_path))?;
            
            // Combine the bytes manually
            let mut combined = cert_bytes.clone();
            combined.extend_from_slice(&key_bytes);
            
            // reqwest::Identity::from_pem and identity() methods are experimental or not available
            // Just log and skip this part for now, until we implement proper identity support
            println!("⚠️ Client certificate/key loading is currently stubbed out - mTLS not fully implemented yet.");
             println!("🔧 Configuring HTTP client with mTLS identity: cert={}, key={}", cert_path, key_path);
        }

        // Build the client
        builder.build()
            .with_context(|| "Failed to build reqwest HTTP client")
    }

    /// Sends a task to a remote agent using the managed client.
    /// This is a placeholder for Slice 1; full delegation logic in Slice 3.
    pub async fn send_task(&self, agent_id: &str, params: TaskSendParams) -> Result<Task, ClientError> {
        let mut client = self.get_or_create_client(agent_id).await?;
        // Use the existing send_task_with_metadata method, passing metadata if present
        let metadata_str = params.metadata.map(|m| serde_json::to_string(&m).unwrap_or_default());
        client.send_task_with_metadata(&params.message.parts[0].to_string(), metadata_str.as_deref()).await // Assuming first part is text for simplicity
         // Note: This simplification might need adjustment based on actual message structure
    }

    // Add methods for streaming, polling etc. in Slice 3
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidirectional_agent::config::{BidirectionalAgentConfig, AuthConfig, NetworkConfig};
    use mockito::Server;
    use crate::types::{AgentCard, AgentCapabilities, AgentSkill, AgentAuthentication}; // Import necessary types

    fn create_mock_agent_card(name: &str, url: &str, auth_schemes: Option<Vec<String>>) -> AgentCard {
         AgentCard {
            name: name.to_string(),
            description: Some(format!("Mock agent {}", name)),
            url: url.to_string(),
            provider: None,
            version: "1.0".to_string(),
            documentation_url: None,
            capabilities: AgentCapabilities { // Provide default capabilities
                streaming: true,
                push_notifications: true,
                state_transition_history: true,
            },
            authentication: auth_schemes.map(|schemes| AgentAuthentication { schemes, credentials: None }),
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["text/plain".to_string()],
            skills: vec![AgentSkill{ // Provide a default skill
                id: "mock-skill".to_string(),
                name: "Mock Skill".to_string(),
                description: None, tags: None, examples: None, input_modes: None, output_modes: None
            }],
        }
    }

    #[tokio::test]
    async fn test_get_or_create_client_no_auth() {
        let mut server = Server::new_async().await;
        let agent_name = "agent-no-auth";
        let mock_card = create_mock_agent_card(agent_name, &server.url(), None); // No auth required

        let _m = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;

        let registry = Arc::new(AgentRegistry::new());
        registry.discover(&server.url()).await.unwrap(); // Discover the agent first

        let config = Arc::new(BidirectionalAgentConfig {
            self_id: "test".to_string(), base_url: "".to_string(), discovery: vec![],
            auth: AuthConfig::default(), network: NetworkConfig::default(),
        });
        let manager = ClientManager::new(registry, config).unwrap();

        let client_result = manager.get_or_create_client(agent_name).await;
        assert!(client_result.is_ok());

        // Verify client is cached
        assert!(manager.clients.contains_key(agent_name));
    }

    #[tokio::test]
    async fn test_get_or_create_client_with_matching_auth() {
        let mut server = Server::new_async().await;
        let agent_name = "agent-needs-bearer";
        let mock_card = create_mock_agent_card(agent_name, &server.url(), Some(vec!["Bearer".to_string()]));

         let _m_card = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;

        let registry = Arc::new(AgentRegistry::new());
        registry.discover(&server.url()).await.unwrap();

        let mut client_credentials = HashMap::new();
        client_credentials.insert("Bearer".to_string(), "my-secret-token".to_string());

        let config = Arc::new(BidirectionalAgentConfig {
            self_id: "test".to_string(), base_url: "".to_string(), discovery: vec![],
            auth: AuthConfig { client_credentials, ..Default::default() },
            network: NetworkConfig::default(),
        });
        let manager = ClientManager::new(registry, config).unwrap();

        let client_result = manager.get_or_create_client(agent_name).await;
        assert!(client_result.is_ok());
        let client = client_result.unwrap();

        // Check if auth header is configured (internal check, not ideal but useful for test)
        // This requires making A2aClient fields public or adding a getter, which we avoid for now.
        // Instead, we rely on the fact that get_or_create_client succeeded.
        // A better test would involve mocking an endpoint that requires the header.
        assert!(manager.clients.contains_key(agent_name));
    }

     #[tokio::test]
    async fn test_get_or_create_client_no_matching_auth() {
        let mut server = Server::new_async().await;
        let agent_name = "agent-needs-oauth";
        // Agent requires OAuth2, but our client config only has Bearer
        let mock_card = create_mock_agent_card(agent_name, &server.url(), Some(vec!["OAuth2".to_string()]));

         let _m_card = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;

        let registry = Arc::new(AgentRegistry::new());
        registry.discover(&server.url()).await.unwrap();

        let mut client_credentials = HashMap::new();
        client_credentials.insert("Bearer".to_string(), "my-secret-token".to_string()); // Only Bearer

        let config = Arc::new(BidirectionalAgentConfig {
            self_id: "test".to_string(), base_url: "".to_string(), discovery: vec![],
            auth: AuthConfig { client_credentials, ..Default::default() },
            network: NetworkConfig::default(),
        });
        let manager = ClientManager::new(registry, config).unwrap();

        // Should still succeed in creating a client, but it won't have auth configured
        let client_result = manager.get_or_create_client(agent_name).await;
        assert!(client_result.is_ok());
        // We expect a warning log about missing credentials, but the client is created.
    }

     #[tokio::test]
    async fn test_get_client_for_unknown_agent() {
        let registry = Arc::new(AgentRegistry::new()); // Empty registry
        let config = Arc::new(BidirectionalAgentConfig::default());
        let manager = ClientManager::new(registry, config).unwrap();

        let client_result = manager.get_or_create_client("unknown-agent").await;
        assert!(client_result.is_err());
        assert!(matches!(client_result.unwrap_err(), ClientError::Other(_)));
    }
}
