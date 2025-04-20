use serde::{Deserialize, Serialize};
use serde_json::{Value, json, Map};
use std::error::Error;
use crate::types::{AgentSkill, Task, Message, TextPart, Part, Role};
use crate::client::errors::ClientError;
// Remove ErrorCompatibility import
// use crate::client::error_handling::ErrorCompatibility;
use uuid::Uuid;
use chrono::Utc;

/// Parameters for listing skills
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillListParams {
    /// Optional filter tags to narrow down the skills list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    
    /// Optional metadata for the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

/// Parameters for getting skill details
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillDetailsParams {
    /// ID of the skill to retrieve details for
    pub id: String,
    
    /// Optional metadata for the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

/// Parameters for invoking a skill
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillInvokeParams {
    /// ID of the skill to invoke
    pub id: String,
    
    /// The task message containing the skill parameters
    pub message: Message,
    
    /// Optional input mode to use (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_mode: Option<String>,
    
    /// Optional output mode to request (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_mode: Option<String>,
    
    /// Optional session ID to associate with this skill invocation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    
    /// Optional metadata for the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

/// Response for listing skills
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillListResponse {
    /// List of available skills
    pub skills: Vec<AgentSkill>,
    
    /// Optional metadata from the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

/// Response for getting skill details
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillDetailsResponse {
    /// The skill details
    pub skill: AgentSkill,
    
    /// Optional metadata from the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

/// Extension methods for the A2A client related to skills
impl crate::client::A2aClient {
    /// List all available skills from an agent
    pub async fn list_skills_typed(&mut self, tags: Option<Vec<String>>) -> Result<SkillListResponse, ClientError> {
        let params = SkillListParams {
            tags,
            metadata: None,
        };
        
        let params_value = serde_json::to_value(params)
            .map_err(|e| ClientError::JsonError(format!("Failed to serialize params: {}", e)))?;
            
        self.send_jsonrpc::<SkillListResponse>("skills/list", params_value).await
    }

    // Remove old version if not needed
    // pub async fn list_skills(&mut self, tags: Option<Vec<String>>) -> Result<SkillListResponse, Box<dyn Error>> {
    //     self.list_skills_typed(tags).await.into_box_error()
    // }

    /// Get detailed information about a specific skill
    pub async fn get_skill_details_typed(&mut self, skill_id: &str) -> Result<SkillDetailsResponse, ClientError> {
        let params = SkillDetailsParams {
            id: skill_id.to_string(),
            metadata: None,
        };
        
        let params_value = serde_json::to_value(params)
            .map_err(|e| ClientError::JsonError(format!("Failed to serialize params: {}", e)))?;
            
        self.send_jsonrpc::<SkillDetailsResponse>("skills/get", params_value).await
    }

    // Remove old version if not needed
    // pub async fn get_skill_details(&mut self, skill_id: &str) -> Result<SkillDetailsResponse, Box<dyn Error>> {
    //     self.get_skill_details_typed(skill_id).await.into_box_error()
    // }

    /// Invoke a skill with the given parameters
    pub async fn invoke_skill_typed(
        &mut self,
        skill_id: &str,
        text: &str,
        input_mode: Option<String>,
        output_mode: Option<String>
    ) -> Result<Task, ClientError> {
        // Create a simple text message
        let text_part = TextPart {
            type_: "text".to_string(),
            text: text.to_string(),
            metadata: None,
        };
        
        let message = Message {
            role: Role::User,
            parts: vec![Part::TextPart(text_part)],
            metadata: None,
        };
        
        let params = SkillInvokeParams {
            id: skill_id.to_string(),
            message,
            input_mode,
            output_mode,
            session_id: Some(Uuid::new_v4().to_string()),
            metadata: None,
        };
        
        let params_value = serde_json::to_value(params)
            .map_err(|e| ClientError::JsonError(format!("Failed to serialize params: {}", e)))?;
            
        self.send_jsonrpc::<Task>("skills/invoke", params_value).await
    }

    // Remove old version if not needed
    // pub async fn invoke_skill(
    //     &mut self,
    //     skill_id: &str,
        text: &str,
        input_mode: Option<String>,
        output_mode: Option<String>
    ) -> Result<Task, Box<dyn Error>> {
        self.invoke_skill_typed(skill_id, text, input_mode, output_mode).await.into_box_error()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::A2aClient;
    use crate::mock_server::{self, start_mock_server_with_auth};
    use std::thread;
    
    #[tokio::test]
    async fn test_list_skills() -> Result<(), Box<dyn Error>> {
        // Start mock server in a separate thread (without authentication)
        let port = 8081;
        thread::spawn(move || {
            start_mock_server_with_auth(port, false);
        });
        
        // Create client
        let mut client = A2aClient::new(&format!("http://localhost:{}", port));
        
        // List skills
        let response = client.list_skills(None).await?;
        
        // Verify that we got at least one skill
        assert!(!response.skills.is_empty());
        
        // Verify the returned skill has the required fields
        let skill = &response.skills[0];
        assert!(!skill.id.is_empty());
        assert!(!skill.name.is_empty());
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_get_skill_details() -> Result<(), Box<dyn Error>> {
        // Start mock server in a separate thread (without authentication)
        let port = 8082;
        thread::spawn(move || {
            start_mock_server_with_auth(port, false);
        });
        
        // Create client
        let mut client = A2aClient::new(&format!("http://localhost:{}", port));
        
        // Get skill details
        let response = client.get_skill_details("test-skill-1").await?;
        
        // Verify the skill id matches
        assert_eq!(response.skill.id, "test-skill-1");
        
        // Verify the skill has a name
        assert!(!response.skill.name.is_empty());
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_invoke_skill() -> Result<(), Box<dyn Error>> {
        // Start mock server in a separate thread (without authentication)
        let port = 8083;
        thread::spawn(move || {
            start_mock_server_with_auth(port, false);
        });
        
        // Create client
        let mut client = A2aClient::new(&format!("http://localhost:{}", port));
        
        // Invoke skill
        let task = client.invoke_skill(
            "test-skill-1", 
            "Test skill invocation", 
            Some("text/plain".to_string()),
            Some("text/plain".to_string())
        ).await?;
        
        // Verify the task was created
        assert!(!task.id.is_empty());
        
        Ok(())
    }
}
