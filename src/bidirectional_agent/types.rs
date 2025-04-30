//! Custom types for the bidirectional agent system.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Represents a tool call part in a message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallPart {
    pub type_: String,
    pub id: String,
    pub tool_call: Value,
}

/// Represents a tool call
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub params: Value,
}

/// Custom Part enum that includes a ToolCallPart variant (extending the core types::Part)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtendedPart {
    TextPart(crate::types::TextPart),
    FilePart(crate::types::FilePart),
    DataPart(crate::types::DataPart),
    ToolCallPart(ToolCallPart),
}

/// Custom conversion from core Part to ExtendedPart
impl From<crate::types::Part> for ExtendedPart {
    fn from(part: crate::types::Part) -> Self {
        match part {
            crate::types::Part::TextPart(p) => ExtendedPart::TextPart(p),
            crate::types::Part::FilePart(p) => ExtendedPart::FilePart(p),
            crate::types::Part::DataPart(p) => ExtendedPart::DataPart(p),
        }
    }
}

/// Information about the origin of a task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskOrigin {
    /// Task originated locally within this agent
    Local,
    /// Task was delegated to another agent
    Delegated {
        /// The ID of the agent that this task was delegated to
        agent_id: String,
        /// Optional URL of the agent
        agent_url: Option<String>,
        /// Timestamp of when the task was delegated
        delegated_at: String,
    },
    /// Task was received from another agent
    External {
        /// The ID of the agent that created this task
        agent_id: String,
        /// Optional URL of the agent
        agent_url: Option<String>,
        /// Timestamp of when the task was created
        created_at: String,
    },
}

/// Relationships between tasks in a workflow
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRelationships {
    /// Parent task IDs that led to this task
    pub parent_task_ids: Vec<String>,
    /// Child task IDs created from this task
    pub child_task_ids: Vec<String>,
    /// Related task IDs that are associated but not direct parents/children
    pub related_task_ids: HashMap<String, String>,
}
