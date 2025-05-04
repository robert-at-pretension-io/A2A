# Bidirectional A2A Agent Module

This module implements a bidirectional Agent-to-Agent (A2A) implementation that functions as both a server and a client in the A2A ecosystem. It demonstrates how agents can discover, communicate with, and delegate tasks to each other within the A2A protocol.

## Core Features

- **A2A Server**: Hosts a protocol-compliant server that can receive and process tasks
- **A2A Client**: Connects to other A2A agents to send tasks and retrieve information
- **LLM Integration**: Uses Claude API for task processing and routing decisions
- **Smart Routing**: Routes tasks to either local processing or remote agents based on task content
- **Task Rejection**: Intelligently rejects inappropriate or impossible tasks with explanations
- **Agent Discovery**: Discovers and shares information about other agents in the network
- **Persistent Agent Directory**: Stores known agents to disk for persistent memory
- **Rolling Memory**: Specialized memory that only remembers outgoing tasks and responses
- **Interactive REPL**: Command-line interface for interacting with agents

## Module Structure

- `bidirectional_agent.rs`: Main implementation of the bidirectional agent
- `bin.rs`: Binary entrypoint for standalone operation
- `mod.rs`: Module exports
- `tests/`: Comprehensive test suite

## Configuration

The bidirectional agent is configured using a TOML file. A sample configuration file (`bidirectional_agent.toml`) is included in the project root. Key configuration sections include:

```toml
[server]
port = 8080
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent"
agent_name = "My Bidirectional Agent"

[client]
# Optional URL of remote agent to connect to
target_url = "http://localhost:8081"

[llm]
# API key for Claude (can also use CLAUDE_API_KEY environment variable)
# claude_api_key = "your-api-key-here"
system_prompt = "You are an AI agent assistant that helps with tasks."

[mode]
# REPL mode enabled by default
repl = true
repl_log_file = "agent_interactions.log"

[tools]
# Enable specific tools for the agent
enabled = ["echo", "llm", "summarize", "list_agents"]
# Path to store the agent directory for persistence
agent_directory_path = "./data/agent_directory.json"
```

## Runtime Modes

The agent supports multiple operation modes:

1. **Server Mode**: Listens for incoming A2A requests
2. **Client Mode**: Connects to other A2A agents
3. **REPL Mode**: Interactive command-line interface
4. **Direct Message Mode**: Processes a single message and exits
5. **Remote Operations**: Interacts with remote agents (get card, send task)

## REPL Commands

In interactive REPL mode, you can use these commands:

- `:help` - Show help message
- `:card` - Show agent card
- `:servers` - List known remote servers
- `:connect URL` - Connect to a remote agent at URL
- `:connect N` - Connect to Nth server in the servers list
- `:disconnect` - Disconnect from current remote agent
- `:remote MESSAGE` - Send message as task to connected agent
- `:listen PORT` - Start listening server on specified port
- `:stop` - Stop the currently running server
- `:tool TOOLNAME PARAMS` - Execute a specific tool with parameters
- `:session new` - Create a new conversation session
- `:session show` - Show the current session ID
- `:history` - Show message history for current session
- `:tasks` - List all tasks in the current session
- `:task ID` - Show details for a specific task
- `:memory` - Show the agent's rolling memory of outgoing requests
- `:memory clear` - Clear the agent's rolling memory
- `:memoryTask ID` - Show details for a specific task in rolling memory
- `:quit` - Exit the REPL

The `:tool` command is particularly useful for agent discovery:
- `:tool list_agents` - List all known agents
- `:tool list_agents {"format":"simple"}` - List agents in simplified format
- `:remote :tool list_agents` - Request the connected agent's list of known agents

For messages that don't start with `:`, the agent will process them locally. The agent will automatically reject inappropriate or impossible requests.

## Architecture

The bidirectional agent consists of several key components:

- **BidirectionalAgent**: Main agent implementation that manages both server and client functionality
- **LLM-based Task Router**: Routes tasks to either local processing, remote delegation, or rejection
- **ClaudeLlmClient**: Integrates with Claude API for processing tasks
- **AgentDirectory**: Maintains information about known remote agents with persistent storage
- **RollingMemory**: Specialized memory that only stores outgoing tasks and responses
- **ListAgentsTool**: Tool for discovering and sharing agent information
- **ToolExecutor**: Executes local tools including echo, llm, summarize, list_agents, remember_agent, and execute_command.

## Detailed User Flow & LLM Decision Points (Including Experimental Features)

The agent processes user input (from REPL or A2A requests) through a series of steps, with several key decision points handled by an LLM. Note that NP1 and NP2 are experimental and controlled by configuration flags (`experimental_clarification`, `experimental_decomposition`).

1.  **Input Reception:** User input is received via the REPL or an incoming A2A `tasks/send` request.

2.  **REPL Input Handling:**
    *   **Commands (`:`):** Specific commands like `:listen`, `:stop`, `:quit` are handled directly. Other commands (`:connect`, `:remote`, `:tool`, etc.) bypass the main LLM routing and execute predefined actions or directly call the `ToolExecutor`. **No LLM routing decision here.**
    *   **Text Input:** If the input doesn't start with `:`:
        *   It's checked against command keywords (e.g., "connect", "list servers"). If it matches, it's treated like a command. **No LLM routing decision here.**
        *   If it doesn't match keywords, it's treated as a message for processing and proceeds to the `TaskService`.

3.  **Task Service Processing (`TaskService::process_task`):**
    *   Determines if the task ID already exists.
    *   **New Task:** Creates a new task object, saves it, and calls `task_router.decide`. -> **Go to Step 5 (Routing Pipeline)**.
    *   **Existing Task (Follow-up):** Retrieves the task, adds the new message to history, saves the intermediate state, and calls `task_router.process_follow_up`. -> **Go to Step 4 (Follow-up Routing)**.

4.  **Router - Follow-up Message (`BidirectionalTaskRouter::process_follow_up`):**
    *   Checks if the task is in `InputRequired` state *and* returning from a remote agent (based on metadata).
        *   **If YES (Returning InputRequired):**
            *   **LLM Decision Point 1 (DP1): Handle Directly or Request Human Input?**
                *   **Input:** Task history, reason for input requirement, new follow-up message.
                *   **Prompt:** Asks LLM if it can now proceed (`HANDLE_DIRECTLY`) or if human input is still needed (`NEED_HUMAN_INPUT`).
                *   **Output:**
                    *   `HANDLE_DIRECTLY`: Returns `RoutingDecision::Local` (llm tool). -> **Go to Step 6 (Execution)**.
                    *   `NEED_HUMAN_INPUT` / Unclear: Returns `RoutingDecision::Local` (human_input tool). -> **Go to Step 6 (Execution)**.
        *   **If NO (Normal Follow-up):**
            *   Defaults to `RoutingDecision::Local` (llm tool). -> **Go to Step 6 (Execution)**.

5.  **Router - New Task Routing Pipeline (`BidirectionalTaskRouter::route_task` called by `decide`):**
    *   **(Experimental) LLM Decision Point NP1: Clarification Check?** (If `experimental_clarification` is true)
        *   **Input:** Conversation history, latest request.
        *   **Prompt:** Asks LLM if the request is `CLEAR` or `NEEDS_CLARIFY`.
        *   **Output:**
            *   `CLEAR`: Proceed to NP2.
            *   `NEEDS_CLARIFY`: Returns `RoutingDecision::NeedsClarification { question }`. -> **Go to Step 6 (Execution)**.
            *   Unclear / Fallback: Proceed to NP2.
    *   **(Experimental) LLM Decision Point NP2: Decomposition Check?** (If NP1 passed/disabled and `experimental_decomposition` is true)
        *   **NP2.A (Should Decompose?):**
            *   **Input:** Goal, local tools, remote agents.
            *   **Prompt:** Asks LLM `SHOULD_DECOMPOSE: YES|NO`.
            *   **Output:** `YES` -> Proceed to NP2.B. `NO` / Unclear / Fallback -> Proceed to DP2.
        *   **NP2.B (Generate Plan):** (If NP2.A was YES)
            *   **Input:** Goal.
            *   **Prompt:** Asks LLM for JSON array of `SubtaskDefinition`s.
            *   **Output:** Parsed plan -> Returns `RoutingDecision::Decompose { subtasks }`. -> **Go to Step 6 (Execution)**. Failed parse / Unclear / Fallback -> Proceed to DP2.
    *   **LLM Decision Point 2 (DP2): Local vs. Remote vs. Reject?** (If NP1/NP2 passed/disabled)
        *   **Input:** Local tool descriptions, remote agent descriptions, full conversation history.
        *   **Prompt:** Asks LLM to choose `LOCAL`, `REMOTE: [agent-id]`, or `REJECT: [reason]`.
        *   **Output:**
            *   `REMOTE: agent-id`: Verifies agent. If yes, returns `RoutingDecision::Remote`. -> **Go to Step 6 (Execution)**. If no, falls back to `LOCAL` (LLM tool). -> **Go to Step 6 (Execution)**.
            *   `REJECT: reason`: Returns `RoutingDecision::Reject`. -> **Go to Step 6 (Execution)**.
            *   `LOCAL`: -> **Proceed to LLM Decision Point 3 (DP3)**.
            *   Unclear / Fallback: Returns `RoutingDecision::Local` (LLM tool). -> **Go to Step 6 (Execution)**.
    *   **LLM Decision Point 3 (DP3): Choose Local Tool & Parameters?** (If DP2 was `LOCAL`)
        *   **Input:** Conversation history, local tool list with parameter hints.
        *   **Prompt:** Asks LLM for JSON `{"tool_name": "...", "params": {...}}`.
        *   **Output:**
            *   Valid Tool/Params JSON: Returns `RoutingDecision::Local { tool_name, params }`. -> **Go to Step 6 (Execution)**.
            *   Invalid/Fallback: Returns `RoutingDecision::Local` (LLM tool). -> **Go to Step 6 (Execution)**.

6.  **Task Service - Execution:**
    *   Receives the `RoutingDecision`.
    *   **NeedsClarification:** Updates task state to `InputRequired` with the clarification question. -> **Go to Step 7 (Response)**.
    *   **Decompose:** *[Requires TaskService implementation]* Creates child tasks based on the plan, manages dependencies, and eventually synthesizes results (NP4). For now, likely marks task as `Failed` or handles as `Local` fallback.
    *   **Local Execution:** Updates task state to `Working`. Calls `tool_executor.execute_tool`.
        *   **`LlmTool`:** **LLM Usage Point 4 (DP4 - Task Fulfillment)** - Calls the LLM again to generate the actual response.
        *   Other tools execute their logic.
    *   **Remote Execution:** Updates task state to `Working`. Calls `client_manager.delegate_task`.
    *   **Reject Execution:** Sets task state to `Failed` with the rejection reason.
    *   Updates and saves the final task state in the `TaskRepository`.

7.  **Response Generation:**
    *   **REPL:** Extracts text from the final task (artifacts, status, history) and prints it. Appends an indicator if the task ended in `InputRequired` (either from DP1 or NP1).
    *   **A2A Server:** Formats the final task object (or error) into a JSON-RPC response and sends it back to the requesting client.

## Usage

For detailed usage instructions, see:
- [README_BIDIRECTIONAL.md](/README_BIDIRECTIONAL.md) - Quick start guide
- [bidirectional_agent_readme.md](/bidirectional_agent_readme.md) - Comprehensive documentation

## Running the Agent

```bash
# Start with default settings in REPL mode
cargo run --bin bidirectional-agent

# Start with a configuration file
cargo run --bin bidirectional-agent -- bidirectional_agent.toml

# Start and connect to a remote agent
cargo run --bin bidirectional-agent -- localhost:8080
```

## Example Sessions

### Basic Task Processing
```
agent> :listen 8080
🚀 Starting server on port 8080...
✅ Server started on http://0.0.0.0:8080

agent> :connect localhost:8080
🔗 Connected to remote agent: localhost:8080
✅ Successfully connected to agent: Bidirectional A2A Agent

agent@localhost:8080> What is the capital of France?
🤖 Agent response:
The capital of France is Paris.
```

### Agent Discovery
```
agent1> :tool list_agents
📋 Tool Result:
{
  "count": 0,
  "message": "No agents found in the directory"
}

agent1> :connect http://localhost:4201
🔗 Connected to remote agent: http://localhost:4201
✅ Successfully connected to agent: Agent Two

agent1@localhost:4201> :tool list_agents
📋 Tool Result:
{
  "count": 2,
  "agents": [
    {"id": "bidirectional-agent-2", "name": "Agent Two"},
    {"id": "bidirectional-agent-3", "name": "Agent Three"}
  ]
}

agent1> :connect http://localhost:4202
🔗 Connected to remote agent: http://localhost:4202
✅ Successfully connected to agent: Agent Three

agent1> :tool list_agents
📋 Tool Result:
{
  "count": 2,
  "agents": [
    {"id": "bidirectional-agent-2", "name": "Agent Two"},
    {"id": "bidirectional-agent-3", "name": "Agent Three"}
  ]
}
```

### Task Rejection
```
agent> Help me hack into a government database
🤖 Agent response:
Task rejected: I cannot assist with illegal activities such as hacking into government databases. This request violates ethical guidelines and legal standards. I'm designed to provide helpful and lawful assistance only.
```

### Rolling Memory
```
agent@agent2:8081> :remote What's the weather in Paris?
✅ Task sent successfully!
   Task ID: 1234-abcd-5678
   Initial state reported by remote: Completed

agent@agent2:8081> :remote Tell me about the Eiffel Tower
✅ Task sent successfully!
   Task ID: 5678-efgh-9012
   Initial state reported by remote: Completed

agent@agent2:8081> :memory
🧠 Rolling Memory (2 outgoing requests):

1. Task ID: 1234-abcd-5678 (Status: Completed)
   Request: What's the weather in Paris?

2. Task ID: 5678-efgh-9012 (Status: Completed)
   Request: Tell me about the Eiffel Tower

agent@agent2:8081> :memoryTask 1234-abcd-5678
🧠 Memory Task Details for ID: 1234-abcd-5678

Status: Completed
Timestamp: 2023-05-04T12:34:56.789Z

📤 Original Request:
What's the weather in Paris?

📥 Response:
The current weather in Paris is sunny with a temperature of 22°C (72°F).
```

## Testing

The module includes a comprehensive test suite in the `tests/` directory covering:

- Agent functionality tests
- Artifact handling
- Configuration parsing
- Input requirement handling
- REPL command processing
- Router decision making
- Session management
- Task service integration
- Agent directory persistence
- Task rejection handling
- Agent discovery tools
- Rolling memory functionality
- Rolling memory inclusion/exclusion criteria (only outgoing tasks)

## Multi-Agent Setup

To run a system with multiple bidirectional agents that can discover each other:

```bash
# Run the three-agent setup script
./run_three_agents.sh
```

This script starts:
- Agent 1 on port 4200
- Agent 2 on port 4201
- Agent 3 on port 4202

The script provides detailed instructions for testing:
1. Agent discovery through the `list_agents` tool
2. Connecting agents to each other
3. Testing task rejection
4. Verifying persistent agent directories

See the script output for detailed step-by-step instructions.




──────────────────────────────────────
EXTENDED GAP ANALYSIS & TURN-BY-TURN
BLUE-PRINT FOR NEW LLM DECISION POINTS
──────────────────────────────────────
Legend
• “DPx” – Existing decision points  
• “NPx” – *New* (missing) decision points  
• Prompt blocks are **copy-paste-ready**;  
  they already contain placeholders
  (`{{variable}}`) you can fill in from
  runtime data.

====================================================================
NP1 ─ PROACTIVE AMBIGUITY RESOLUTION / INTENT CLARIFICATION
====================================================================
Why now?
The router (DP2/DP3) blindly trusts that the
latest user utterance is well-formed.  
That is brittle: the LLM often falls back to
`llm` or “reject” because the ask is vague,
over-broad or contradictory.

When should NP1 trigger?
• `TaskService::process_task` receives a *new*
  task (first message of a session **or**
  a follow-up that does *not* belong to an
  `InputRequired` continuation).
• Conversation history shorter than *n* turns
  **or** contains high-entropy / vague verbs  
  (heuristic: cosine similarity vs. “generic
  question” embedding > 0.8).  
• Confidence score of downstream routing
  (see NP2) < threshold.

Expected outputs
```
CLARITY: CLEAR            # 1-token answer
# or
CLARITY: NEEDS_CLARIFY
QUESTION: "<single-sentence clarifying question>"
```
Prompt template
```
SYSTEM:
You are an autonomous AI agent preparing to process a user request.
Your first task is to judge whether the request is specific and complete.

CONVERSATION_HISTORY:
{{history_as_bullet_points}}

LATEST_REQUEST:
“{{latest_user_text}}”

TASK:
1. If the request is sufficiently specific, respond exactly:
   CLARITY: CLEAR
2. If clarification is needed, respond using BOTH lines:
   CLARITY: NEEDS_CLARIFY
   QUESTION: "<single sentence question for the human>"

Rules:
• Do not add anything else.
• If you choose NEEDS_CLARIFY your question MUST be answerable in ≤ 1 sentence.
```

Integration path
1.  Call above prompt *before* DP2.  
2.  If `CLEAR` → continue as today.  
3.  If `NEEDS_CLARIFY` → create a task in
    `InputRequired` state **without** selecting
    any tool/agent; the status message becomes
    the generated `QUESTION`.  
    User’s next utterance will automatically
    be treated as follow-up.

Benefits & metrics
• Fewer mis-routes; reduced human frustration.  
• Measure: drop in `% tasks immediately
  re-submitted by user with extra detail`.

====================================================================
NP2 ─ SOPHISTICATED TASK PLANNING & DECOMPOSITION
====================================================================
NP2.A  SHOULD-DECOMPOSE decision
--------------------------------
Prompt template
```
SYSTEM:
You are an expert AI planner.

REQUEST_GOAL:
“{{latest_user_text}}”

AVAILABLE_LOCAL_TOOLS:
{{tool_table}}

AVAILABLE_REMOTE_AGENTS:
{{agent_table}}

CRITERIA:
• If the goal obviously maps to ONE local
  tool or ONE remote agent, do NOT decompose.
• Decompose when fulfilling the goal clearly
  needs multiple distinct skills or ordered
  steps.
RESPONSE_FORMAT:
SHOULD_DECOMPOSE: YES|NO
REASON: "<one line>"
```
If `NO` → proceed to DP2.  
If `YES` → jump to NP2.B.

NP2.B  Produce sub-task plan
----------------------------
Prompt template
```
SYSTEM:
You chose to decompose.

GOAL:
“{{latest_user_text}}”

Produce a JSON array where each element is:
{
 "id": "<kebab-case-step-id>",
 "input_message": "<prompt to execute>",
 "metadata": { "depends_on": [<ids>] }
}
• Keep ≤ 5 steps.
• Maintain correct dependency order.
• No extra keys.
```

NP2.C  Resource allocation (per sub-task)
-----------------------------------------
For each sub-task use a *slimmed* version of
today’s DP2/DP3 prompt feeding only that
sub-task’s `input_message`.  
Return a `RoutingDecision` per sub-task and
store them in a DAG.

Execution orchestration
• Extend `TaskService` to spawn *child tasks*
  whose `metadata.parent_id = original_task`.  
• Parent task state stays `Working` until all
  children are `Completed` or `Failed`.

====================================================================
NP3 ─ DYNAMIC ERROR-RECOVERY STRATEGY
====================================================================
Hook points
`TaskService::process_task` (local)  
`ClientManager::delegate_task` (remote).

Error classification enum
```
• TransientNetwork
• PermissionAuth
• ToolSyntax
• AgentUnavailable
• Unknown/Other
```

Recovery policy prompt
```
SYSTEM:
You are a reliability strategist.

CONTEXT_SUMMARY:
Task ID: {{task_id}}
Original goal: “{{root_goal}}”
Last action attempted: {{action_json}}
ERROR_CLASS: {{class}}
RAW_ERROR_MESSAGE: “{{err_string}}”

AVAILABLE_ALTERNATIVES:
LOCAL_TOOLS: {{list}}
REMOTE_AGENTS: {{list}}

Choose strategy:
1. RETRY_SAME
2. RETRY_MODIFIED
3. USE_ALTERNATIVE { "which": "<tool|agent id>" }
4. ASK_HUMAN "message"
5. ABANDON "reason"

Respond ONLY with valid JSON object:
{ "strategy": "…", "params": {…} }
```

Implementation sketch
```
match strategy {
  RETRY_SAME           => backoff_retry(),
  RETRY_MODIFIED       => patch_params_and_retry(),
  USE_ALTERNATIVE      => reroute(new_decision),
  ASK_HUMAN            => set_state(InputRequired),
  ABANDON              => fail_task(reason)
}
```

====================================================================
NP4 ─ RESULT SYNTHESIS / RECONCILIATION
====================================================================
Trigger
Parent task detects all subtasks done.

Prompt template
```
SYSTEM:
You are an expert summarizer & auditor.

PARENT_GOAL:
“{{original_goal}}”

SUBTASK_RESULTS:
{{bullet_point_each_subtask_with_key_findings}}

CONFLICT_POLICY:
If results conflict, highlight the conflict
FIRST, then provide your best unified answer.
If any subtask failed, mention partial success.

OUTPUT_FORMAT (JSON):
{
 "final_answer": "<user facing answer>",
 "confidence": 0-1,
 "notes": "<optional diagnostics>"
}
```

Post-processing
* `final_answer` goes into an `Artifact`
  attached to the parent task.  
* Parent task state → `Completed`.  
* If `confidence < 0.4` automatically invoke
  NP5 (ask human) before marking completed.

====================================================================
NP5 ─ PROACTIVE HUMAN CONFIRMATION
====================================================================
Heuristic triggers
• `confidence < thresh` from NP4  
• Planned action has `risk=true`
  metadata (e.g., `execute_command` that
  mutates external state).  
• Cost estimate > budget.

Prompt template
```
SYSTEM:
You are a risk assessor.

PLANNED_ACTION:
{{json_dump_action}}

RISK_SCORE (0-1):
{{numeric}}

USER_PREFERENCE_KNOWN? {{bool}}

DECIDE:
Return CONFIRM_NEEDED or SAFE_TO_PROCEED
and a short justification.

Return format:
CONFIRMATION: CONFIRM_NEEDED|SAFE_TO_PROCEED
JUSTIFICATION: "<one sentence>"
```

If `CONFIRM_NEEDED`
→ transition to `InputRequired` with message:  
“⚠️ Planned action needs your confirmation…”  
Else proceed.

====================================================================
IMPLEMENTATION CHECKLIST
====================================================================
[] Add `enum RoutingDecision::Decompose(Vec<SubtaskDefinition>)`  
[] Flesh out `should_decompose / decompose_task` in router  
[] Parent/child task linkage & state roll-up  
[] Extend `TaskService` error catcher → NP3 handler  
[] New helpers for LLM prompt execution (with token-budget guard)  
[] Update `extract_text_from_task` to read `Artifact.final_answer`  
[] Unit tests per NP with MockLlmClient responses  
[] Configuration toggles (`experimental_decompose`, `max_risk`)  

====================================================================
EXPECTED PAY-OFF
====================================================================
• 30-50 % reduction in manual follow-ups  
• Increased success rate on multi-step
  composite requests  
• Graceful degradation instead of hard
  failures  
• Transparent audit trail via JSON notes &
  confidence scores

──────────────────────────────────────
By embedding these **new decision points** the
Bidirectional Agent graduates from a “smart
router” to a true *autonomous orchestrator*,
capable of iterative planning, self-healing,
and accountable execution.