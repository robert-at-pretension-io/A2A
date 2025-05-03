use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::bidirectional::bidirectional_agent::LlmClient;
use crate::client::{A2aClient, errors::ClientError};
use crate::types::{Task, AgentCard, AgentCapabilities, TaskStatus, TaskState, TaskSendParams, Message};
use crate::server::task_router::{RoutingDecision, LlmTaskRouterTrait};
use chrono::Utc;

/// Mock LLM client for testing
pub struct MockLlmClient {
    pub responses: Mutex<HashMap<String, String>>,
    pub default_response: String,
    pub calls: Mutex<Vec<String>>,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(HashMap::new()),
            default_response: "LOCAL".to_string(), // Default to local routing
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn with_response(mut self, prompt_substring: &str, response: &str) -> Self {
        self.responses.lock().unwrap().insert(prompt_substring.to_string(), response.to_string());
        self
    }

    pub fn with_default_response(mut self, response: &str) -> Self {
        self.default_response = response.to_string();
        self
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(&self, prompt: &str) -> Result<String> {
        // Record the call
        self.calls.lock().unwrap().push(prompt.to_string());
        
        // Check if we have a specific response for this prompt
        let responses = self.responses.lock().unwrap();
        for (key, value) in responses.iter() {
            if prompt.contains(key) {
                return Ok(value.clone());
            }
        }
        
        // Return default response
        Ok(self.default_response.clone())
    }
}

/// Wrapper for testing A2A client functionality
pub struct MockA2aClient {
    pub agent_card: Mutex<Option<AgentCard>>,
    pub tasks: Mutex<HashMap<String, Task>>,
    pub send_task_calls: Mutex<Vec<String>>,
    pub get_agent_card_calls: Mutex<usize>,
    pub base_url: String,
}

impl MockA2aClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            agent_card: Mutex::new(None),
            tasks: Mutex::new(HashMap::new()),
            send_task_calls: Mutex::new(Vec::new()),
            get_agent_card_calls: Mutex::new(0),
            base_url: base_url.to_string(),
        }
    }

    pub fn with_agent_card(self, card: AgentCard) -> Self {
        *self.agent_card.lock().unwrap() = Some(card);
        self
    }

    pub fn generate_task(&self, message: &str) -> Task {
        let task_id = uuid::Uuid::new_v4().to_string();
        let task = Task {
            id: task_id.clone(),
            status: TaskStatus {
                state: TaskState::Completed,
                timestamp: Some(Utc::now()),
                message: None,
            },
            history: Some(vec![]),
            artifacts: None,
            metadata: None,
            session_id: None,
        };
        
        self.tasks.lock().unwrap().insert(task_id.clone(), task.clone());
        task
    }

    // Mock methods that match A2aClient functionality
    pub async fn send_task(&self, message: &str) -> Result<Task, ClientError> {
        self.send_task_calls.lock().unwrap().push(message.to_string());
        let task = self.generate_task(message);
        Ok(task)
    }

    pub async fn send_task_with_metadata(&self, message: &str, _metadata: Option<&str>) -> Result<Task, ClientError> {
        self.send_task_calls.lock().unwrap().push(message.to_string());
        let task = self.generate_task(message);
        Ok(task)
    }

    pub async fn get_task(&self, task_id: &str) -> Result<Task, ClientError> {
        if let Some(task) = self.tasks.lock().unwrap().get(task_id) {
            Ok(task.clone())
        } else {
            // Create a "Task not found" A2A error
            let a2a_error = crate::client::errors::A2aError::new(
                crate::client::errors::error_codes::ERROR_TASK_NOT_FOUND,
                "Task not found",
                None
            );
            Err(ClientError::A2aError(a2a_error))
        }
    }

    pub async fn get_agent_card(&self) -> Result<AgentCard, ClientError> {
        let mut calls = self.get_agent_card_calls.lock().unwrap();
        *calls += 1;
        
        if let Some(card) = self.agent_card.lock().unwrap().clone() {
            Ok(card)
        } else {
            // Return a default agent card
            Ok(AgentCard {
                name: "Mock Agent".to_string(),
                description: Some("A mock agent for testing".to_string()),
                version: "1.0.0".to_string(),
                url: self.base_url.clone(),
                capabilities: AgentCapabilities {
                    push_notifications: true,
                    state_transition_history: true,
                    streaming: false,
                },
                authentication: None,
                default_input_modes: vec!["text".to_string()],
                default_output_modes: vec!["text".to_string()],
                documentation_url: None,
                provider: None,
                skills: vec![],
            })
        }
    }
}

/// Mock router that returns a predefined routing decision
pub struct MockRouter {
    decision: RoutingDecision,
    calls: Mutex<Vec<TaskSendParams>>,
}

impl MockRouter {
    pub fn new(decision: RoutingDecision) -> Self {
        Self {
            decision,
            calls: Mutex::new(Vec::new()),
        }
    }
    
    pub fn get_calls(&self) -> Vec<TaskSendParams> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl LlmTaskRouterTrait for MockRouter {
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, crate::server::ServerError> {
        // Record the call
        self.calls.lock().unwrap().push(params.clone());
        
        // Return the predefined decision
        Ok(self.decision.clone())
    }
    
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, crate::server::ServerError> {
        // Record the call
        self.calls.lock().unwrap().push(params.clone());
        
        // Return the predefined decision
        Ok(self.decision.clone())
    }
    
    async fn process_follow_up(&self, _task_id: &str, _message: &Message) -> Result<RoutingDecision, crate::server::ServerError> {
        // For follow-up messages, just return the same decision
        Ok(self.decision.clone())
    }
    
    async fn should_decompose(&self, _params: &TaskSendParams) -> Result<bool, crate::server::ServerError> {
        // By default, don't decompose tasks
        Ok(false)
    }
    
    async fn decompose_task(&self, _params: &TaskSendParams) -> Result<Vec<crate::server::task_router::SubtaskDefinition>, crate::server::ServerError> {
        // Return empty subtasks list
        Ok(Vec::new())
    }
}

/// Helper function to create a MockRouter with a specific decision
pub fn setup_mock_router(decision: RoutingDecision) -> MockRouter {
    MockRouter::new(decision)
}