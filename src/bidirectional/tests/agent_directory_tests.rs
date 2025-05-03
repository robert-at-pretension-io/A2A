use std::path::Path;
use std::fs;
use tempfile::tempdir;
use chrono::Utc;
use crate::types::AgentCard;
use crate::bidirectional::bidirectional_agent::AgentDirectory;

#[tokio::test]
async fn test_agent_directory_save_and_load() {
    // Create a temp directory for the test
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let file_path = temp_dir.path().join("test_agent_directory.json");
    
    // Create an agent card for testing
    let card = AgentCard {
        name: "Test Agent".to_string(),
        url: "http://localhost:1234".to_string(),
        description: Some("Test agent for unit tests".to_string()),
        documentation_url: None,
        provider: None,
        capabilities: Default::default(),
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        authentication: None,
        skills: vec![], // Empty skills array
        version: "1.0.0".to_string(),
    };
    
    // First directory instance - add agents and save
    {
        let directory = AgentDirectory::new();
        
        // Add some test agents
        directory.add_or_update_agent("agent-1".to_string(), card.clone());
        
        let mut card2 = card.clone();
        card2.name = "Another Agent".to_string();
        card2.url = "http://localhost:5678".to_string();
        card2.description = Some("Second test agent".to_string());
        directory.add_or_update_agent("agent-2".to_string(), card2);
        
        // Save to file
        directory.save_to_file(&file_path).expect("Failed to save directory");
        
        // Check that file exists
        assert!(file_path.exists(), "Directory file was not created");
    }
    
    // Read file content for verification
    let file_content = fs::read_to_string(&file_path).expect("Failed to read directory file");
    assert!(file_content.contains("Test Agent"), "File content missing agent name");
    assert!(file_content.contains("Another Agent"), "File content missing second agent name");
    assert!(file_content.contains("http://localhost:1234"), "File content missing agent URL");
    
    // Second directory instance - load from file
    {
        let directory = AgentDirectory::new();
        
        // Directory should be empty initially
        assert_eq!(directory.list_all_agents().len(), 0, "New directory should be empty");
        
        // Load from file
        directory.load_from_file(&file_path).expect("Failed to load directory");
        
        // Now should have agents
        let agents = directory.list_all_agents();
        assert_eq!(agents.len(), 2, "Directory should have 2 agents after loading");
        
        // Verify agent details
        let agent_map: std::collections::HashMap<String, AgentCard> = agents
            .into_iter()
            .collect();
            
        assert!(agent_map.contains_key("agent-1"), "Missing agent-1");
        assert!(agent_map.contains_key("agent-2"), "Missing agent-2");
        
        assert_eq!(agent_map["agent-1"].name, "Test Agent");
        assert_eq!(agent_map["agent-2"].name, "Another Agent");
        assert_eq!(agent_map["agent-1"].url, "http://localhost:1234");
        assert_eq!(agent_map["agent-2"].url, "http://localhost:5678");
    }
}

#[tokio::test]
async fn test_agent_directory_empty_file_handling() {
    // Test handling of a non-existent file
    let non_existent_path = Path::new("/tmp/non_existent_directory_12345/test.json");
    
    let directory = AgentDirectory::new();
    
    // Should not error when file doesn't exist
    let result = directory.load_from_file(non_existent_path);
    assert!(result.is_ok(), "Should handle non-existent file gracefully");
    
    // Directory should still be empty
    assert_eq!(directory.list_all_agents().len(), 0);
}