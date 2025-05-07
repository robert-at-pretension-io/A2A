//! Registry implementation for bidirectional agents
//! 
//! This module provides a registry for storing and retrieving information about
//! other agents in the network. It includes URL parsing capabilities and JSON
//! persistence. Unlike the main agent registry which focuses on agent cards and
//! capabilities, this registry is focused on URL-based agent discovery.

use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use dashmap::DashMap;
use tracing::{debug, error, info, trace, warn};
use url::Url;

/// Entry in the agent directory
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentDirectoryEntry {
    /// URL of the agent
    pub url: String,
    /// Name of the agent (if known)
    pub name: Option<String>,
    /// When this agent was last contacted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_contacted: Option<String>,
    /// Agent card information (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_card: Option<crate::types::AgentCard>,
    /// Any extra metadata about this agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Agent directory structure
#[derive(Clone, Debug)]
pub struct AgentDirectory {
    /// Map of agent URLs to directory entries
    agents: Arc<DashMap<String, AgentDirectoryEntry>>,
    /// Path to the JSON file for persistence
    pub directory_path: Option<String>,
}

impl AgentDirectory {
    /// Create a new agent directory
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            directory_path: None,
        }
    }

    /// Load an agent directory from a JSON file
    pub fn load(path: &str) -> Result<Self> {
        let mut directory = Self::new();
        directory.directory_path = Some(path.to_string());

        // Check if file exists before attempting to load
        if Path::new(path).exists() {
            let content = fs::read_to_string(path)?;
            let entries: Vec<AgentDirectoryEntry> = serde_json::from_str(&content)?;
            
            for entry in entries {
                directory.agents.insert(entry.url.clone(), entry);
            }
            
            info!(path = %path, agent_count = %directory.agents.len(), "Loaded agent directory");
        } else {
            info!(path = %path, "Agent directory file does not exist, creating new directory");
        }

        Ok(directory)
    }

    /// Save the agent directory to a JSON file
    pub fn save(&self) -> Result<()> {
        if let Some(path) = &self.directory_path {
            // Create parent directory if it doesn't exist
            if let Some(parent) = Path::new(path).parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                }
            }

            // Convert to list for serialization
            let entries: Vec<AgentDirectoryEntry> = self.agents
                .iter()
                .map(|entry| entry.value().clone())
                .collect();

            let json = serde_json::to_string_pretty(&entries)?;
            fs::write(path, json)?;
            debug!(path = %path, agent_count = %entries.len(), "Saved agent directory");
            Ok(())
        } else {
            warn!("No directory path specified, agent directory not saved");
            Err(anyhow!("No directory path specified"))
        }
    }

    /// Add an agent to the directory
    pub fn add_agent(&self, url: &str, name: Option<String>) -> Result<()> {
        // Validate and normalize URL
        let normalized_url = Self::normalize_url(url)?;
        
        // Create entry
        let entry = AgentDirectoryEntry {
            url: normalized_url.clone(),
            name,
            last_contacted: Some(chrono::Utc::now().to_rfc3339()),
            agent_card: None,
            metadata: None,
        };

        // Insert and save
        debug!(url = %normalized_url, name = ?entry.name, "Adding agent to directory");
        self.agents.insert(normalized_url, entry);
        
        // Only attempt to save if directory_path is set
        if self.directory_path.is_some() {
            self.save()?;
        }
        
        Ok(())
    }
    
    /// Add an agent with full agent card information
    pub fn add_agent_with_card(&self, url: &str, agent_card: crate::types::AgentCard) -> Result<()> {
        // Validate and normalize URL
        let normalized_url = Self::normalize_url(url)?;
        
        // Create entry
        let entry = AgentDirectoryEntry {
            url: normalized_url.clone(),
            name: Some(agent_card.name.clone()),
            last_contacted: Some(chrono::Utc::now().to_rfc3339()),
            agent_card: Some(agent_card),
            metadata: None,
        };

        // Insert and save
        debug!(url = %normalized_url, name = ?entry.name, "Adding agent with card to directory");
        self.agents.insert(normalized_url, entry);
        
        // Only attempt to save if directory_path is set
        if self.directory_path.is_some() {
            self.save()?;
        }
        
        Ok(())
    }

    /// Update an existing agent in the directory
    pub fn update_agent(&self, url: &str, name: Option<String>) -> Result<()> {
        // Normalize URL
        let normalized_url = Self::normalize_url(url)?;

        // Check if agent exists
        if let Some(mut entry) = self.agents.get_mut(&normalized_url) {
            // Update fields
            if let Some(name_val) = name {
                entry.name = Some(name_val);
            }
            entry.last_contacted = Some(chrono::Utc::now().to_rfc3339());
            
            debug!(url = %normalized_url, "Updated agent in directory");
            
            // Only attempt to save if directory_path is set
            if self.directory_path.is_some() {
                self.save()?;
            }
            
            Ok(())
        } else {
            warn!(url = %normalized_url, "Attempted to update non-existent agent");
            Err(anyhow!("Agent not found in directory: {}", normalized_url))
        }
    }
    
    /// Update an agent with agent card information
    pub fn update_agent_card(&self, url: &str, agent_card: crate::types::AgentCard) -> Result<()> {
        // Normalize URL
        let normalized_url = Self::normalize_url(url)?;

        // Check if agent exists
        if let Some(mut entry) = self.agents.get_mut(&normalized_url) {
            // Update fields
            entry.name = Some(agent_card.name.clone());
            entry.agent_card = Some(agent_card);
            entry.last_contacted = Some(chrono::Utc::now().to_rfc3339());
            
            debug!(url = %normalized_url, "Updated agent card in directory");
            
            // Only attempt to save if directory_path is set
            if self.directory_path.is_some() {
                self.save()?;
            }
            
            Ok(())
        } else {
            warn!(url = %normalized_url, "Attempted to update card for non-existent agent");
            Err(anyhow!("Agent not found in directory: {}", normalized_url))
        }
    }

    /// Get all agents in the directory
    pub fn get_all_agents(&self) -> Vec<AgentDirectoryEntry> {
        self.agents.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Get an agent by URL
    pub fn get_agent(&self, url: &str) -> Result<AgentDirectoryEntry> {
        // Normalize URL first
        let normalized_url = Self::normalize_url(url)?;
        
        // Lookup in directory
        match self.agents.get(&normalized_url) {
            Some(entry) => Ok(entry.value().clone()),
            None => Err(anyhow!("Agent not found in directory: {}", normalized_url))
        }
    }

    /// Remove an agent from the directory
    pub fn remove_agent(&self, url: &str) -> Result<()> {
        // Normalize URL
        let normalized_url = Self::normalize_url(url)?;
        
        // Remove and save
        if self.agents.remove(&normalized_url).is_some() {
            debug!(url = %normalized_url, "Removed agent from directory");
            
            // Only attempt to save if directory_path is set
            if self.directory_path.is_some() {
                self.save()?;
            }
            
            Ok(())
        } else {
            warn!(url = %normalized_url, "Attempted to remove non-existent agent");
            Err(anyhow!("Agent not found in directory: {}", normalized_url))
        }
    }

    /// Parse a string to extract URLs that could be agent endpoints
    pub fn extract_agent_urls(text: &str) -> Vec<String> {
        // Various patterns for URLs
        let url_patterns = [
            // Standard URL with protocol
            r"https?://(?:[-\w.]|(?:%[\da-fA-F]{2}))+(?::\d+)?(?:/[-\w%!$&'()*+,;=:@/~]*)?",
            // Host:port format without protocol
            r"\b(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}(?::\d+)?\b",
            // Localhost with port
            r"localhost:\d+",
            // IP address with optional port
            r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(?::\d+)?\b",
        ];

        let mut found_urls = Vec::new();

        for pattern in url_patterns.iter() {
            if let Ok(regex) = Regex::new(pattern) {
                for cap in regex.find_iter(text) {
                    let url = cap.as_str().to_string();
                    // Only add if not already in the list (avoid duplicates from overlapping patterns)
                    if !found_urls.contains(&url) {
                        found_urls.push(url);
                    }
                }
            }
        }

        found_urls
    }

    /// Normalize a URL to ensure consistent format
    pub fn normalize_url(url: &str) -> Result<String> {
        // If URL doesn't start with http(s), add http://
        let url_with_scheme = if !url.starts_with("http://") && !url.starts_with("https://") {
            format!("http://{}", url)
        } else {
            url.to_string()
        };

        // Parse URL to validate and normalize
        match Url::parse(&url_with_scheme) {
            Ok(parsed_url) => {
                // Return normalized URL string
                let normalized = parsed_url.to_string();
                // Remove trailing slash for consistency
                let normalized = normalized.trim_end_matches('/').to_string();
                Ok(normalized)
            },
            Err(e) => {
                error!(url = %url, error = %e, "Failed to parse URL");
                Err(anyhow!("Invalid URL format: {}", e))
            }
        }
    }

    /// Check if a given text likely contains an agent URL
    pub fn contains_agent_url(text: &str) -> bool {
        !Self::extract_agent_urls(text).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_url_extraction() {
        let text = "Connect to agent at http://localhost:8080 and also check out test.example.com:9000 and 127.0.0.1:8081";
        let urls = AgentDirectory::extract_agent_urls(text);
        
        // Just check that the expected URLs are in the list, don't fix number of matches
        // as regex patterns can overlap and URL extraction can vary
        assert!(urls.contains(&"http://localhost:8080".to_string()));
        assert!(urls.contains(&"test.example.com:9000".to_string()));
        assert!(urls.contains(&"127.0.0.1:8081".to_string()));
    }

    #[test]
    fn test_url_normalization() {
        // Test with different URL formats
        assert_eq!(
            AgentDirectory::normalize_url("localhost:8080").unwrap(),
            "http://localhost:8080"
        );
        assert_eq!(
            AgentDirectory::normalize_url("http://example.com:9000/").unwrap(),
            "http://example.com:9000"
        );
        assert_eq!(
            AgentDirectory::normalize_url("https://test.com").unwrap(),
            "https://test.com"
        );
        assert_eq!(
            AgentDirectory::normalize_url("127.0.0.1:8081").unwrap(),
            "http://127.0.0.1:8081"
        );
    }

    #[test]
    fn test_add_and_retrieve_agent() {
        let directory = AgentDirectory::new();
        
        // Add an agent
        directory.add_agent("http://localhost:8080", Some("Test Agent".to_string())).unwrap();
        
        // Retrieve the agent
        let agent = directory.get_agent("http://localhost:8080").unwrap();
        assert_eq!(agent.url, "http://localhost:8080");
        assert_eq!(agent.name, Some("Test Agent".to_string()));
        
        // Check with different URL format but same underlying URL
        let agent2 = directory.get_agent("localhost:8080").unwrap();
        assert_eq!(agent2.url, "http://localhost:8080");
    }

    #[test]
    fn test_persistence() {
        // Create a temporary file for testing
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap().to_string();
        
        // Create directory with path
        let mut directory = AgentDirectory::new();
        directory.directory_path = Some(file_path.clone());
        
        // Add agents
        directory.add_agent("http://localhost:8080", Some("Local Agent".to_string())).unwrap();
        directory.add_agent("http://example.com:9000", Some("Remote Agent".to_string())).unwrap();
        
        // Save the directory
        directory.save().unwrap();
        
        // Load into a new instance
        let loaded_directory = AgentDirectory::load(&file_path).unwrap();
        
        // Verify contents
        assert_eq!(loaded_directory.agents.len(), 2);
        let agent1 = loaded_directory.get_agent("http://localhost:8080").unwrap();
        assert_eq!(agent1.name, Some("Local Agent".to_string()));
        
        let agent2 = loaded_directory.get_agent("http://example.com:9000").unwrap();
        assert_eq!(agent2.name, Some("Remote Agent".to_string()));
    }
}