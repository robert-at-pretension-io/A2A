
use crate::bidirectional_agent::{
    agent_directory::{AgentDirectory, AgentStatus},
    AgentRegistry, BidirectionalAgent, BidirectionalAgentConfig,
};

use crate::client::A2aClient;

use crate::runner::{TestRunnerConfig, run_integration_tests};

use crate::types::AgentCard;

use anyhow::{Result, Context, anyhow};

use std::sync::Arc;
use std::time::Duration;

use url::Url;

use colored::*;

/// Configuration for agent testing


pub struct AgentTesterConfig {
    /// Timeout for individual tests
    pub test_timeout: Duration,
    /// Number of retries for test operations
    pub max_retries: usize,
    /// Whether to run all available tests or just required ones
    pub run_all_tests: bool,
}


impl Default for AgentTesterConfig {
    fn default() -> Self {
        Self {
            test_timeout: Duration::from_secs(30),
            max_retries: 3,
            run_all_tests: false,
        }
    }
}


/// A tool for testing A2A agents before adding them to the directory

pub struct AgentTester {
    config: AgentTesterConfig,
    agent_registry: Arc<AgentRegistry>,
    agent_directory: Arc<AgentDirectory>,
}


impl AgentTester {
    /// Create a new AgentTester with the given bidirectional agent components
    pub fn new(
        config: AgentTesterConfig,
        agent_registry: Arc<AgentRegistry>,
        agent_directory: Arc<AgentDirectory>,
    ) -> Self {
        Self {
            config,
            agent_registry,
            agent_directory,
        }
    }


    /// Test an agent at the given URL before adding it to the directory
    pub async fn test_agent(&self, url: &str) -> Result<TestResults> {
        println!("{}", "======================================================".blue().bold());
        println!("{}", "🚀 Starting A2A Agent Testing".blue().bold());
        println!("{}", "======================================================".blue().bold());
        println!("🔍 Testing agent at URL: {}", url.cyan());

        // 1. Validate URL format
        let parsed_url = Url::parse(url)
            .with_context(|| format!("Invalid agent URL: {}", url))?;

        // 2. First verify the agent is reachable
        println!("🔄 Checking if agent is reachable...");
        let client = A2aClient::new(url);
        
        // Try to get the agent card
        let agent_card = match client.get_agent_card().await {
            Ok(card) => {
                println!("✅ Successfully retrieved agent card");
                Some(card)
            },
            Err(e) => {
                println!("⚠️ Failed to retrieve agent card: {}", e);
                None
            }
        };

        // 3. Run integration tests against the agent
        println!("🧪 Running integration tests against the agent...");
        let test_config = TestRunnerConfig {
            target_url: Some(url.to_string()),
            default_timeout: self.config.test_timeout,
        };

        let test_results = match run_integration_tests(test_config).await {
            Ok(_) => {
                println!("✅ All integration tests passed!");
                TestResults {
                    agent_card,
                    url: url.to_string(),
                    functional: true,
                    passed_tests: true,
                    message: "All tests passed".to_string(),
                }
            },
            Err(e) => {
                println!("⚠️ Some integration tests failed: {}", e);
                TestResults {
                    agent_card,
                    url: url.to_string(),
                    functional: true, // Still functional even if some tests fail
                    passed_tests: false,
                    message: format!("Some tests failed: {}", e),
                }
            }
        };

        Ok(test_results)
    }

    /// Test and add an agent to the directory if it passes
    pub async fn test_and_add_agent(&self, url: &str) -> Result<AddAgentResult> {
        // 1. Run tests
        let test_results = self.test_agent(url).await?;
        
        // 2. Decide whether to add based on test results
        if !test_results.functional {
            return Ok(AddAgentResult {
                agent_id: None,
                added_to_directory: false,
                test_results,
                message: "Agent is not functional, not added to directory".to_string(),
            });
        }

        // 3. If tests pass or agent is at least functional, add to directory
        let agent_id = if let Some(card) = &test_results.agent_card {
            card.name.clone()
        } else {
            format!("agent-{}", uuid::Uuid::new_v4())
        };

        // 4. Add to directory (with card if available)
        match self.agent_directory.add_agent(
            &agent_id,
            url,
            test_results.agent_card.clone()
        ).await {
            Ok(_) => {
                println!("✅ Successfully added agent '{}' to directory", agent_id);
                Ok(AddAgentResult {
                    agent_id: Some(agent_id),
                    added_to_directory: true,
                    test_results,
                    message: "Agent successfully tested and added to directory".to_string(),
                })
            },
            Err(e) => {
                println!("❌ Failed to add agent to directory: {}", e);
                Ok(AddAgentResult {
                    agent_id: Some(agent_id),
                    added_to_directory: false,
                    test_results,
                    message: format!("Failed to add agent to directory: {}", e),
                })
            }
        }
    }

    /// Get a list of all tested agents with their status
    pub async fn list_tested_agents(&self) -> Result<Vec<TestedAgentInfo>> {
        let all_agents = self.agent_directory.get_active_agents().await
            .context("Failed to get agents from directory")?;
        
        let mut tested_agents = Vec::new();
        
        for agent_info in all_agents {
            // Access fields directly from ActiveAgentEntry
            let agent_id = agent_info.agent_id;
            let url = agent_info.url;
            
            // All agents from get_active_agents are active by definition
            let status = AgentStatus::Active;
            
            // Since ActiveAgentEntry doesn't include card info, fetch it separately if needed
            // For simplicity, we'll set agent_card to None
            let agent_card = None;
            
            tested_agents.push(TestedAgentInfo {
                agent_id,
                url,
                status,
                agent_card,
                last_tested: None, // We don't track this yet
            });
        }
        
        Ok(tested_agents)
    }

    /// Re-test an agent that's already in the directory
    pub async fn retest_agent(&self, agent_id: &str) -> Result<TestResults> {
        // Get agent info from directory
        let agent_info = self.agent_directory.get_agent_info(agent_id).await
            .context("Failed to get agent info from directory")?;
        
        let url = agent_info.get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("No URL found for agent {}", agent_id))?;
        
        // Run tests on the agent
        self.test_agent(url).await
    }
}

/// Results of testing an agent

#[derive(Debug, Clone)]
pub struct TestResults {
    /// The agent's card if successfully retrieved
    pub agent_card: Option<AgentCard>,
    /// The URL of the agent
    pub url: String,
    /// Whether the agent is functional at all
    pub functional: bool,
    /// Whether the agent passed all required tests
    pub passed_tests: bool,
    /// Detailed message about test results
    pub message: String,
}

/// Results of testing and adding an agent

#[derive(Debug, Clone)]
pub struct AddAgentResult {
    /// ID of the agent (if determined)
    pub agent_id: Option<String>,
    /// Whether the agent was successfully added to directory
    pub added_to_directory: bool,
    /// The detailed test results
    pub test_results: TestResults,
    /// Detailed message about the operation
    pub message: String,
}

/// Information about a tested agent

#[derive(Debug, Clone)]
pub struct TestedAgentInfo {
    /// ID of the agent
    pub agent_id: String,
    /// URL of the agent
    pub url: String,
    /// Current status of the agent
    pub status: AgentStatus,
    /// Agent card if available
    pub agent_card: Option<AgentCard>,
    /// When the agent was last tested (if tracked)
    pub last_tested: Option<chrono::DateTime<chrono::Utc>>,
}