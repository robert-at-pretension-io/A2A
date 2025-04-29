//! Types specific to the Bidirectional Agent implementation.

#![cfg(feature = "bidir-core")]

use serde::{Deserialize, Serialize};

/// Represents the origin of a task within the bidirectional agent.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskOrigin {
    /// Task originated locally within this agent.
    Local,
    /// Task was delegated to another agent.
    Delegated {
        agent_id: String,
        remote_task_id: String,
    },
    /// Task was received from another agent (i.e., handled by our server component).
    Remote {
        requesting_agent_id: Option<String>, // ID of the agent that sent the task, if known
    },
}

/// Represents the relationships between tasks (e.g., parent/child for decomposition).
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TaskRelationships {
    pub parent_id: Option<String>,
    #[serde(default)]
    pub children: Vec<String>,
}

// Add other bidirectional-specific types here as needed in later slices,
// such as RoutingDecision, TaskRequirements, ToolDefinition, etc.
