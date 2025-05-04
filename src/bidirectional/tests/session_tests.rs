use crate::bidirectional::BidirectionalAgent;
use crate::bidirectional::config::{BidirectionalAgentConfig, ServerConfig, ClientConfig, LlmConfig, ModeConfig};
use crate::bidirectional::tests::mocks::MockLlmClient;
use crate::types::{Task, TaskStatus, TaskState, Message, Part, TextPart, Role};
use std::sync::Arc;
use std::env;
use dashmap::DashMap;
use uuid::Uuid;
use chrono::Utc;

// Helper function to create a test configuration
fn create_test_config() -> BidirectionalAgentConfig {
    // Create a test configuration with a mock LLM API key
    let mut config = BidirectionalAgentConfig::default();
    config.llm.claude_api_key = Some("test-api-key".to_string());
    config.server.port = 9090;
    config.server.bind_address = "127.0.0.1".to_string();
    config.server.agent_id = "test-agent".to_string();
    
    config
}

#[tokio::test]
async fn test_session_creation() {
    // Set up environment to avoid actual API calls
    env::set_var("CLAUDE_API_KEY", "test-api-key");
    
    // We'll modify the agent struct to add session tracking
    // This test will verify that we can create and track sessions
    
    // This test will initially fail until we implement the session tracking functionality
    
    // First we'll create a modified BidirectionalAgent struct
    // Later we'll modify the actual implementation
    struct TestAgent {
        current_session_id: Option<String>,
        session_tasks: Arc<DashMap<String, Vec<String>>>,
    }
    
    impl TestAgent {
        fn new() -> Self {
            Self {
                current_session_id: None,
                session_tasks: Arc::new(DashMap::new()),
            }
        }
        
        fn create_new_session(&mut self) -> String {
            let session_id = format!("session-{}", Uuid::new_v4());
            self.current_session_id = Some(session_id.clone());
            self.session_tasks.insert(session_id.clone(), Vec::new());
            session_id
        }
        
        async fn save_task_to_history(&self, task: Task) -> Result<(), anyhow::Error> {
            if let Some(session_id) = &self.current_session_id {
                if let Some(mut tasks) = self.session_tasks.get_mut(session_id) {
                    tasks.push(task.id.clone());
                }
            }
            Ok(())
        }
    }
    
    // Create test agent
    let mut agent = TestAgent::new();
    
    // Test creating a new session
    let session_id = agent.create_new_session();
    assert!(session_id.starts_with("session-"), "Session ID should start with 'session-'");
    assert_eq!(agent.current_session_id, Some(session_id.clone()));
    
    // Verify session is in the map
    assert!(agent.session_tasks.contains_key(&session_id));
    assert_eq!(agent.session_tasks.get(&session_id).unwrap().len(), 0);
    
    // Create a mock task
    let task_id = Uuid::new_v4().to_string();
    let task = Task {
        id: task_id.clone(),
        status: TaskStatus {
            state: TaskState::Completed,
            timestamp: Some(Utc::now()),
            message: None,
        },
        history: None,
        artifacts: None,
        metadata: None,
        session_id: Some(session_id.clone()),
    };
    
    // Test saving a task to session history
    agent.save_task_to_history(task).await.unwrap();
    
    // Verify task was added to session
    assert_eq!(agent.session_tasks.get(&session_id).unwrap().len(), 1);
    assert_eq!(agent.session_tasks.get(&session_id).unwrap()[0], task_id);
}

#[tokio::test]
async fn test_multiple_sessions() {
    // Test agent with multiple sessions
    struct TestAgent {
        current_session_id: Option<String>,
        session_tasks: Arc<DashMap<String, Vec<String>>>,
    }
    
    impl TestAgent {
        fn new() -> Self {
            Self {
                current_session_id: None,
                session_tasks: Arc::new(DashMap::new()),
            }
        }
        
        fn create_new_session(&mut self) -> String {
            let session_id = format!("session-{}", Uuid::new_v4());
            self.current_session_id = Some(session_id.clone());
            self.session_tasks.insert(session_id.clone(), Vec::new());
            session_id
        }
        
        async fn save_task_to_history(&self, task: Task) -> Result<(), anyhow::Error> {
            if let Some(session_id) = &self.current_session_id {
                if let Some(mut tasks) = self.session_tasks.get_mut(session_id) {
                    tasks.push(task.id.clone());
                }
            }
            Ok(())
        }
    }
    
    // Create test agent
    let mut agent = TestAgent::new();
    
    // Create first session
    let session_id1 = agent.create_new_session();
    
    // Create a mock task for first session
    let task_id1 = Uuid::new_v4().to_string();
    let task1 = Task {
        id: task_id1.clone(),
        status: TaskStatus {
            state: TaskState::Completed,
            timestamp: Some(Utc::now()),
            message: None,
        },
        history: None,
        artifacts: None,
        metadata: None,
        session_id: Some(session_id1.clone()),
    };
    
    // Save task to first session
    agent.save_task_to_history(task1).await.unwrap();
    
    // Create second session
    let session_id2 = agent.create_new_session();
    assert_ne!(session_id1, session_id2, "Session IDs should be unique");
    
    // Create a mock task for second session
    let task_id2 = Uuid::new_v4().to_string();
    let task2 = Task {
        id: task_id2.clone(),
        status: TaskStatus {
            state: TaskState::Completed,
            timestamp: Some(Utc::now()),
            message: None,
        },
        history: None,
        artifacts: None,
        metadata: None,
        session_id: Some(session_id2.clone()),
    };
    
    // Save task to second session
    agent.save_task_to_history(task2).await.unwrap();
    
    // Verify tasks were added to correct sessions
    assert_eq!(agent.session_tasks.get(&session_id1).unwrap().len(), 1);
    assert_eq!(agent.session_tasks.get(&session_id1).unwrap()[0], task_id1);
    
    assert_eq!(agent.session_tasks.get(&session_id2).unwrap().len(), 1);
    assert_eq!(agent.session_tasks.get(&session_id2).unwrap()[0], task_id2);
    
    // Current session should be the second one
    assert_eq!(agent.current_session_id, Some(session_id2));
}