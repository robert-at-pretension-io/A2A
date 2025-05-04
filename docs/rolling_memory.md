# Rolling Memory

The Rolling Memory feature provides a specialized memory system for Bidirectional Agents that **exclusively** tracks outgoing requests the agent makes to other agents, and the responses it receives.

## Purpose

The primary purpose of the Rolling Memory is to allow an agent to maintain an independent record of tasks it has initiated, separate from tasks requested of it. This creates a clear separation between:

1. Tasks the agent performs for others (not stored in rolling memory)
2. Tasks the agent requests from other agents (stored in rolling memory)

## Key Characteristics

- **Outgoing-Only**: Rolling Memory stores ONLY tasks that the agent itself initiates to other agents. 
  Tasks requested of the agent by others are NOT stored in rolling memory.

- **Size-Limited**: The memory has a configurable maximum size (default: 50 tasks).
  When this limit is reached, the oldest tasks are pruned.

- **Age-Limited**: Tasks older than a configurable age (default: 24 hours) are automatically pruned.

- **Chronological Tracking**: Tasks are stored in order of creation, making it easy to retrieve the history of outgoing requests.

- **Thread-Safe**: The implementation is thread-safe for concurrent access.

## Usage

### In Code

Rolling Memory is implemented in the `agent_helpers.rs` file and integrated into the `BidirectionalAgent`. 
The agent automatically adds tasks to the rolling memory when it sends tasks to remote agents.

```rust
// Example: Adding a task to rolling memory (happens automatically in send_task_to_remote)
agent.rolling_memory.add_task(task);

// Example: Getting all tasks in chronological order
let tasks = agent.rolling_memory.get_tasks_chronological();

// Example: Clearing all memory
agent.rolling_memory.clear();
```

### REPL Commands

The following REPL commands are available for interacting with the rolling memory:

- `:memory` - View all outgoing tasks stored in the rolling memory
- `:memory clear` - Clear all tasks from the rolling memory
- `:memoryTask ID` - View details of a specific task in the rolling memory

## Implementation Details

The `RollingMemory` struct is defined in `agent_helpers.rs` and maintains:

1. A DashMap of task IDs to tasks for O(1) lookup
2. A thread-safe queue of task IDs to maintain chronological order
3. A timestamp map to track when tasks were added
4. Configurable limits for maximum size and age

## Example

When an agent initiates a request to another agent:

```
agent > :remote Tell me about the weather

✅ Task sent successfully!
   Task ID: 1234-5678-91011
   Initial state reported by remote: Working

agent > :memory

🧠 Rolling Memory (1 outgoing requests):

1. Task ID: 1234-5678-91011 (Status: Completed)
   Request: Tell me about the weather
```

## Important Distinction

It's critical to understand that the Rolling Memory differs from the general task history in the agent:

- **General Task History**: Contains ALL tasks, both those requested of the agent AND those it requests from others.
- **Rolling Memory**: Contains ONLY tasks the agent itself sends to other agents.

This separation ensures the agent can reason separately about:
1. What others have asked of it
2. What it has asked of others