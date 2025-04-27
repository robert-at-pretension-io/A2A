# Bidirectional A2A Agent: Detailed Remaining Implementation Tasks

This document outlines the specific implementation steps required to complete the bidirectional A2A agent functionality based on the existing codebase and the agreed-upon plan.

## Task 3: Background Polling Task Repository Integration

**Goal:** Implement the background loop in `ClientManager` to periodically check the status of delegated tasks and update the local task state accordingly.

**File:** `src/bidirectional_agent/client_manager.rs`

**Method:** `ClientManager::run_delegated_task_poll_loop`

**Detailed Steps:**

1.  **Query Delegated Tasks:**
    *   Inside the `loop`, use `self.task_repository.get_tasks_by_origin_type(TaskOriginType::Delegated).await?`.
        *   *(Dependency: `get_tasks_by_origin_type` needs to be added to `TaskRepositoryExt` and implemented in `InMemoryTaskRepository` to query the `task_origins` DashMap).*
    *   Iterate through the returned `(local_task_id, TaskOrigin::Delegated { agent_id, remote_task_id })`.
2.  **Check Remote Status:**
    *   For each delegated task, call `self.get_task_status(&agent_id, &remote_task_id).await`.
3.  **Handle Remote Status:**
    *   **On Success (`Ok(remote_task)`):**
        *   Fetch the corresponding local task: `self.task_repository.get_task(&local_task_id).await?`.
        *   If the local task exists and its state is not already final (Completed, Failed, Canceled):
            *   Compare `remote_task.status` with `local_task.status`.
            *   If the remote state is different or newer (check timestamp), update the local task:
                *   Set `local_task.status = remote_task.status.clone()`.
                *   Set `local_task.artifacts = remote_task.artifacts.clone()`.
                *   Save the updated local task: `self.task_repository.save_task(&local_task).await?`.
                *   Save state history: `self.task_repository.save_state_history(&local_task_id, &local_task).await?`.
            *   If the `remote_task.status.state` is final (Completed, Failed, Canceled):
                *   Log completion/failure.
                *   *(Optional: Trigger result synthesis if applicable, though synthesis might be handled elsewhere based on state changes).*
                *   *(Optional: Remove the task from active polling if desired, or let the `get_tasks_by_origin_type` filter handle it).*
    *   **On Error (`Err(e)`):**
        *   Log the polling error (e.g., `println!("⚠️ Failed to get status for remote task '{}': {}", remote_task_id, e);`).
        *   Implement error handling strategy:
            *   *Retry Logic:* Implement a retry mechanism with backoff for transient errors (e.g., network issues).
            *   *Failure Threshold:* After a certain number of consecutive failures, mark the local task as Failed.
            *   Fetch the local task: `self.task_repository.get_task(&local_task_id).await?`.
            *   Update local task status to `TaskState::Failed` with an appropriate error message in `status.message`.
            *   Save the updated local task and its history.
4.  **Loop Control:** Ensure the loop respects the `CancellationToken` passed to the background task spawner in `BidirectionalAgent::run`. Use `tokio::select!`.

## Task 4: Task Origin/Relationship Persistence Integration

**Goal:** Correctly integrate the `TaskRepositoryExt` trait for managing task origin and parent/child relationships within `TaskFlow` and `ResultSynthesizer`.

**Files:**
*   `src/bidirectional_agent/task_flow.rs`
*   `src/bidirectional_agent/result_synthesis.rs`
*   `src/bidirectional_agent/task_extensions.rs` (Verify trait definition)
*   `src/server/repositories/task_repository.rs` (Verify `InMemoryTaskRepository` implementation)

**Detailed Steps:**

1.  **Update Constructors:**
    *   Modify `TaskFlow::new`: Change the `task_repository` parameter type from `Arc<dyn TaskRepository>` to `Arc<dyn TaskRepositoryExt>`.
    *   Modify `ResultSynthesizer::new`: Change the `task_repository` parameter type from `Arc<dyn TaskRepository>` to `Arc<dyn TaskRepositoryExt>`.
    *   Update the instantiation points of `TaskFlow` and `ResultSynthesizer` (likely within `TaskService` or `BidirectionalAgent`) to pass the repository that implements `TaskRepositoryExt`.
2.  **Implement `TaskFlow::set_task_origin`:**
    *   Remove the `as_any().downcast_ref` logic.
    *   Directly call `self.task_repository.set_task_origin(&self.task_id, origin).await`.
    *   Handle the `Result<(), ServerError>` appropriately, converting to `AgentError` if needed.
3.  **Implement `ResultSynthesizer::get_child_task_ids`:**
    *   Remove the `as_any().downcast_ref` logic.
    *   Directly call `self.task_repository.get_task_relationships(&self.parent_task_id).await`.
    *   Extract the `children` vector from the returned `TaskRelationships` struct.
    *   Handle the `Result<TaskRelationships, ServerError>` appropriately, converting to `AgentError` if needed.
4.  **Verify `TaskRepositoryExt` Implementation:**
    *   Ensure `src/server/repositories/task_repository.rs` correctly implements all methods defined in `TaskRepositoryExt` for `InMemoryTaskRepository`, using the `task_origins` and `task_relationships` DashMaps (guarded by `#[cfg(feature = "bidir-delegate")]`).

## Task 5: Tool Implementation

**Goal:** Implement concrete tools (`ShellTool`, `HttpTool`, etc.) and register them with the `ToolExecutor`.

**Files:**
*   `src/bidirectional_agent/tools/mod.rs` (Define `Tool` trait, uncomment `pub use`)
*   `src/bidirectional_agent/tools/shell_tool.rs` (Create & Implement)
*   `src/bidirectional_agent/tools/http_tool.rs` (Create & Implement)
*   `src/bidirectional_agent/tools/ai_tool.rs` (Create & Implement - Optional/Example)
*   `src/bidirectional_agent/tool_executor.rs` (Update `new` method)

**Detailed Steps:**

1.  **Define `Tool` Trait:**
    *   In `src/bidirectional_agent/tools/mod.rs`, define the `async_trait` `Tool` with methods `name()`, `description()`, `execute(task: &mut Task, context: &ToolContext)`, and `capabilities()`.
    *   Define the `ToolContext` struct (containing logger, config access, etc.) to be passed to `execute`.
2.  **Implement `ShellTool`:**
    *   Create `src/bidirectional_agent/tools/shell_tool.rs`.
    *   Implement the `Tool` trait for `ShellTool`.
    *   `execute`:
        *   Extract the command string from `task.message` or `task.metadata`.
        *   Implement robust command validation (allowlist, deny dangerous patterns like `rm -rf`, `|`, `>`, `;`).
        *   Use `tokio::process::Command` to execute the command safely.
        *   Apply a timeout (`tokio::time::timeout`).
        *   Capture stdout and stderr.
        *   Update `task.status` to `Completed` or `Failed` based on exit code.
        *   Add stdout/stderr to `task.artifacts` or `task.status.message`.
3.  **Implement `HttpTool`:**
    *   Create `src/bidirectional_agent/tools/http_tool.rs`.
    *   Implement the `Tool` trait for `HttpTool`.
    *   `execute`:
        *   Extract URL, method (GET/POST), headers, and body from `task.message` or `task.metadata`.
        *   Use the `reqwest::Client` (potentially passed via `ToolContext` or configured internally, respecting agent network config like proxies).
        *   Perform the HTTP request.
        *   Handle response status codes.
        *   Update `task.status` to `Completed` or `Failed`.
        *   Add response headers and body (truncated if necessary) to `task.artifacts` or `task.status.message`.
4.  **Implement `AiTool` (Example):**
    *   Create `src/bidirectional_agent/tools/ai_tool.rs`.
    *   Implement the `Tool` trait for `AiTool`.
    *   `execute`:
        *   Extract prompt/parameters from the task.
        *   Use an LLM client (like the `ClaudeClient` or a generic one) to call an external AI API.
        *   Update task status and artifacts with the LLM response.
5.  **Register Tools:**
    *   In `src/bidirectional_agent/tool_executor.rs`, modify the `ToolExecutor::new` method.
    *   Uncomment or add calls to `self.register_tool(Box::new(ShellTool::new()))`, etc., for each implemented tool.
6.  **Update `tools/mod.rs`:** Uncomment the `pub use` lines for the implemented tools.

## Task 6: Advanced Routing & Decomposition Logic

**Goal:** Implement sophisticated task routing using capability matching and LLM decisions, handle task decomposition, and implement result synthesis.

**Files:**
*   `src/bidirectional_agent/task_router.rs`
*   `src/bidirectional_agent/llm_routing/mod.rs`
*   `src/bidirectional_agent/llm_routing/claude_client.rs`
*   `src/bidirectional_agent/task_flow.rs`
*   `src/bidirectional_agent/result_synthesis.rs`

**Detailed Steps:**

1.  **Capability Matching (`TaskRouter::decide`):**
    *   Implement `extract_capabilities(params: &TaskSendParams)` to analyze the task message/metadata and determine required capabilities (e.g., "execute_shell", "http_get", "summarize_text"). This might involve simple keyword matching or an LLM call.
    *   Implement `check_local_capabilities(needed: &[String])` using `self.tool_executor` to see which local tools satisfy the needed capabilities.
    *   Implement `find_remote_agent_for_capabilities(needed: &[String])` using `self.agent_registry` to query known agents' skills/capabilities (from their AgentCards) and find the best match(es).
    *   Refine the decision pipeline in `decide` to use these capability checks.
2.  **LLM Integration (`llm_routing/mod.rs`, `claude_client.rs`):**
    *   Implement the actual HTTP request logic within `ClaudeClient::complete` and `ClaudeClient::complete_json` to call the Anthropic API (or another LLM provider). Handle API keys, headers, request body formatting, and response parsing (including error handling).
    *   Implement the logic in `LlmRouter` methods (`decide`, `should_decompose`, `decompose_task`) to:
        *   Construct the appropriate prompts using the helper methods (`create_routing_prompt`, etc.).
        *   Call the `LlmClient` (`complete` or `complete_json`).
        *   Parse the structured JSON response (e.g., `RoutingOutput`, `Vec<SubtaskInfo>`) from the LLM.
        *   Convert the parsed LLM output into the corresponding `RoutingDecision` or `Vec<SubtaskDefinition>`.
3.  **Decomposition Handling (`TaskFlow::process_decision`):**
    *   Implement the `RoutingDecision::Decompose { subtasks }` match arm.
    *   For each `SubtaskDefinition` in `subtasks`:
        *   Create a new `TaskSendParams` based on the `subtask.input_message`.
        *   Add metadata linking it to the `parent_task_id`.
        *   *(Crucially)* Call the `TaskRouter` again (`self.task_router.decide(&subtask_params).await`) to determine how to handle *this specific subtask* (it could be local or delegated itself).
        *   Initiate the subtask's execution based on the sub-decision (either local execution via `ToolExecutor` or delegation via `ClientManager`). Store the new subtask ID.
    *   After initiating all subtasks, call `self.task_repository.link_tasks(&self.task_id, &subtask_ids).await?` to record the parent-child relationship.
    *   Update the parent task's status (e.g., to a new "Decomposed" or "WaitingForSubtasks" state).
4.  **Result Synthesis (`ResultSynthesizer::synthesize`):**
    *   Implement the logic to combine results.
    *   If using LLM synthesis:
        *   Instantiate `SynthesisAgent` (from `llm_routing/mod.rs`).
        *   Construct the prompt using `create_synthesis_prompt`.
        *   Call the LLM via `SynthesisAgent` to get the synthesized text.
        *   Update the parent task's final message/artifacts with the synthesized result.
    *   If using simple concatenation (fallback):
        *   Improve the formatting of the combined results in the parent task's final message.

## Task 7: Dynamic Agent Card Generation

**Goal:** Update the server's Agent Card generation to reflect the actual running configuration (URL, port) and potentially the skills from registered tools.

**File:** `src/server/mod.rs`

**Function:** `create_agent_card`

**Detailed Steps:**

1.  **Modify Signature:** Change `create_agent_card()` to accept necessary configuration, potentially `&BidirectionalAgentConfig` or specific fields like the calculated public URL and a reference to the `ToolExecutor`.
    ```rust
    // Example signature
    pub fn create_agent_card(
        config: &BidirectionalAgentConfig,
        #[cfg(feature = "bidir-local-exec")] tool_executor: &ToolExecutor
    ) -> serde_json::Value { ... }
    ```
    *(Alternatively, make it a method on `BidirectionalAgent`)*.
2.  **Dynamic URL:**
    *   Inside the function, construct the `url` field based on `config.network.public_url` or the combination of `config.network.bind_address` and the actual running `port`.
3.  **Dynamic Skills:**
    *   If the `bidir-local-exec` feature is enabled, call `tool_executor.generate_agent_skills()` to get the list of `AgentSkill` structs.
    *   Serialize this list and add it to the JSON card under the `"skills"` key (or `"x-a2a-skills"` if strict compliance is desired, though the spec *does* define `"skills"`).
4.  **Update Call Site:** Modify the place where `create_agent_card` is called (likely in the `/.well-known/agent.json` handler within `src/server/handlers/mod.rs` or potentially during `BidirectionalAgent` initialization) to pass the required configuration/components.

This detailed plan provides specific instructions for completing the remaining implementation tasks for the bidirectional A2A agent.
