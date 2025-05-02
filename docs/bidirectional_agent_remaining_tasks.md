# A2A Test Suite: LLM-Driven Future Implementation Roadmap

After examining how the project currently uses LLM prompting (specifically in `llm_routing/claude_client.rs` and `task_router_llm.rs`), I've revised all implementation suggestions to leverage LLM-based decision making as the central mechanism for handling complex, "fuzzy" decisions throughout the system.

## 1. Enhanced Delegation Capabilities

The existing `LlmTaskRouter` provides a foundation we can build upon to create a fully LLM-powered delegation system.

### Actionable Implementation Steps:

1. **Create LLM-Powered Delegation Manager**
   ```rust
   // New file: src/bidirectional_agent/llm_delegation/mod.rs
   
   pub struct LlmDelegationManager {
       llm_client: Arc<dyn LlmClient>,
       agent_registry: Arc<AgentRegistry>,
       client_manager: Arc<ClientManager>,
   }
   
   impl LlmDelegationManager {
       pub async fn plan_task_delegation(&self, task: &Task) -> Result<DelegationPlan, AgentError> {
           // Extract task information
           let task_description = self.extract_task_description(task);
           
           // Get available agents information
           let available_agents = self.get_available_agents_info().await?;
           
           // Build comprehensive LLM prompt
           let prompt = format!(
               "You are an expert at delegating tasks to the most appropriate agents.\n\n\
               TASK DESCRIPTION:\n{}\n\n\
               AVAILABLE AGENTS:\n{}\n\n\
               Based on the task description and available agents, please create a delegation plan.\n\
               Consider:\n\
               1. Whether the task should be decomposed into subtasks\n\
               2. Which agent(s) should handle each task/subtask\n\
               3. Dependencies between subtasks (if any)\n\
               4. The execution pattern (sequential, parallel, conditional)\n\n\
               Respond in the following JSON format:\n{}",
               task_description,
               available_agents,
               DELEGATION_PLAN_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete(prompt).await?;
           
           // Parse response into DelegationPlan
           self.parse_delegation_plan(response)
       }
       
       pub async fn execute_delegation_plan(&self, plan: &DelegationPlan, original_task: &Task) -> Result<Vec<Task>, AgentError> {
           match &plan.execution_strategy {
               ExecutionStrategy::Single { agent_id } => {
                   // Simple delegation to a single agent
                   let result = self.delegate_to_agent(agent_id, original_task).await?;
                   Ok(vec![result])
               },
               ExecutionStrategy::Sequential { steps } => {
                   // Execute steps in sequence, passing results forward
                   let mut results = Vec::new();
                   let mut last_result = None;
                   
                   for step in steps {
                       // Include previous step results in context if available
                       let step_task = if let Some(prev_result) = &last_result {
                           self.create_task_with_context(original_task, step, Some(prev_result)).await?
                       } else {
                           self.create_task_with_context(original_task, step, None).await?
                       };
                       
                       // Delegate the step
                       let result = self.delegate_to_agent(&step.agent_id, &step_task).await?;
                       results.push(result.clone());
                       last_result = Some(result);
                   }
                   
                   Ok(results)
               },
               ExecutionStrategy::Parallel { tasks } => {
                   // Execute all tasks in parallel using tokio::join!
                   let mut futures = Vec::new();
                   
                   for task_spec in tasks {
                       let sub_task = self.create_subtask(original_task, task_spec).await?;
                       let agent_id = task_spec.agent_id.clone();
                       
                       // Create future but don't await yet
                       let future = self.delegate_to_agent(&agent_id, &sub_task);
                       futures.push(future);
                   }
                   
                   // Execute all futures concurrently
                   let results = futures::future::join_all(futures).await;
                   
                   // Collect results, propagating errors
                   let mut tasks = Vec::new();
                   for result in results {
                       tasks.push(result?);
                   }
                   
                   Ok(tasks)
               },
               ExecutionStrategy::Conditional { condition, if_true, if_false } => {
                   // Let LLM evaluate the condition based on task context
                   let condition_met = self.evaluate_condition(condition, original_task).await?;
                   
                   if condition_met {
                       self.execute_delegation_plan(if_true, original_task).await
                   } else {
                       self.execute_delegation_plan(if_false, original_task).await
                   }
               }
           }
       }
       
       async fn evaluate_condition(&self, condition: &str, task: &Task) -> Result<bool, AgentError> {
           // Extract task description
           let task_description = self.extract_task_description(task);
           
           // Create prompt for condition evaluation
           let prompt = format!(
               "Evaluate if the following condition is true for this task:\n\n\
               TASK: {}\n\n\
               CONDITION: {}\n\n\
               Respond with only 'true' or 'false'.",
               task_description,
               condition
           );
           
           // Call LLM
           let response = self.llm_client.complete(prompt).await?;
           
           // Parse response
           let result = response.trim().to_lowercase();
           Ok(result == "true")
       }
   }
   
   // Definition of structured types used for delegation
   pub struct DelegationPlan {
       pub execution_strategy: ExecutionStrategy,
       pub reasoning: String,
       pub confidence: f32,
   }
   
   pub enum ExecutionStrategy {
       Single { agent_id: String },
       Sequential { steps: Vec<DelegationStep> },
       Parallel { tasks: Vec<SubTaskSpec> },
       Conditional { 
           condition: String, 
           if_true: Box<DelegationPlan>, 
           if_false: Box<DelegationPlan> 
       },
   }
   ```

2. **Implement LLM-Based Task Decomposition**
   ```rust
   // New file: src/bidirectional_agent/llm_delegation/decomposition.rs
   
   pub struct TaskDecomposer {
       llm_client: Arc<dyn LlmClient>,
       agent_registry: Arc<AgentRegistry>,
   }
   
   impl TaskDecomposer {
       pub async fn decompose_task(&self, task: &Task) -> Result<Vec<SubTask>, AgentError> {
           // Extract task description
           let task_description = extract_task_description(task);
           
           // Get available agent capabilities to inform decomposition
           let available_capabilities = self.get_agent_capabilities().await?;
           
           // Create prompt for task decomposition
           let prompt = format!(
               "You are an expert at decomposing complex tasks into simpler subtasks.\n\n\
               TASK TO DECOMPOSE:\n{}\n\n\
               AVAILABLE AGENT CAPABILITIES:\n{}\n\n\
               Please decompose this task into smaller, more manageable subtasks.\n\
               For each subtask, provide:\n\
               1. A clear description\n\
               2. Required capabilities (from the available list)\n\
               3. Dependencies on other subtasks (if any)\n\
               4. Estimated complexity (simple, medium, complex)\n\
               5. Expected output format\n\n\
               Respond with a JSON array of subtasks in the following format:\n{}",
               task_description,
               available_capabilities,
               SUBTASK_FORMAT_EXAMPLE
           );
           
           // Call LLM with structured output format
           let response = self.llm_client.complete_with_json::<Vec<SubTaskSpec>>(prompt).await?;
           
           // Convert to SubTask objects
           let mut subtasks = Vec::new();
           for spec in response {
               subtasks.push(SubTask {
                   id: generate_subtask_id(),
                   description: spec.description,
                   parent_task_id: task.id.clone(),
                   dependencies: spec.dependencies,
                   required_capabilities: spec.required_capabilities,
                   params: serde_json::to_value(spec.params).unwrap_or(json!({})),
               });
           }
           
           Ok(subtasks)
       }
       
       pub async fn suggest_subtask_assignments(&self, 
           subtasks: &[SubTask], 
           available_agents: &[AgentSummary]
       ) -> Result<HashMap<String, String>, AgentError> {
           // Create a detailed prompt for the LLM to match subtasks to agents
           let prompt = format!(
               "You are an expert at assigning tasks to the most suitable agents.\n\n\
               SUBTASKS TO ASSIGN:\n{}\n\n\
               AVAILABLE AGENTS:\n{}\n\n\
               Please assign each subtask to the most appropriate agent based on its capabilities.\n\
               When making assignments, consider:\n\
               1. Agent capabilities matching task requirements\n\
               2. Agent specialization and expertise\n\
               3. Balanced workload distribution\n\
               4. Geographic proximity for data-intensive tasks\n\n\
               Respond with a JSON object mapping subtask IDs to agent IDs:\n{}",
               format_subtasks_for_prompt(subtasks),
               format_agents_for_prompt(available_agents),
               "{ \"subtask-123\": \"agent-abc\", \"subtask-456\": \"agent-xyz\" }"
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<HashMap<String, String>>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

3. **Create LLM-Driven Task Scheduler**
   ```rust
   // New file: src/bidirectional_agent/llm_delegation/scheduler.rs
   
   pub struct TaskScheduler {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl TaskScheduler {
       pub async fn create_execution_plan(&self, 
           subtasks: &[SubTask], 
           assignments: &HashMap<String, String>
       ) -> Result<ExecutionPlan, AgentError> {
           // Format the subtasks and their dependencies
           let subtasks_json = serde_json::to_string_pretty(subtasks)?;
           
           // Format the agent assignments
           let assignments_json = serde_json::to_string_pretty(assignments)?;
           
           // Create prompt for execution planning
           let prompt = format!(
               "You are an expert task scheduler optimizing execution of interdependent tasks.\n\n\
               SUBTASKS WITH DEPENDENCIES:\n{}\n\n\
               AGENT ASSIGNMENTS:\n{}\n\n\
               Create an execution plan that efficiently schedules these subtasks.\n\
               The plan should:\n\
               1. Respect all dependencies between subtasks\n\
               2. Maximize parallel execution where possible\n\
               3. Minimize overall execution time\n\
               4. Account for potential failures\n\n\
               Respond with a JSON execution plan in the following format:\n{}",
               subtasks_json,
               assignments_json,
               EXECUTION_PLAN_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<ExecutionPlan>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn adapt_execution_plan(&self, 
           current_plan: &ExecutionPlan,
           completed_tasks: &[String],
           failed_tasks: &[String],
       ) -> Result<ExecutionPlan, AgentError> {
           // Create prompt for adapting the plan based on execution results
           let prompt = format!(
               "You are an expert at adapting execution plans when tasks succeed or fail.\n\n\
               CURRENT EXECUTION PLAN:\n{}\n\n\
               COMPLETED TASKS:\n{}\n\n\
               FAILED TASKS:\n{}\n\n\
               Please adapt the execution plan based on the current progress.\n\
               Your adapted plan should:\n\
               1. Remove completed tasks from the plan\n\
               2. Handle failures by proposing alternative execution paths\n\
               3. Preserve the original plan structure where still applicable\n\
               4. Maintain all unaffected dependencies\n\n\
               Respond with a revised JSON execution plan in the same format as the current plan.",
               serde_json::to_string_pretty(current_plan)?,
               completed_tasks.join(", "),
               failed_tasks.join(", ")
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<ExecutionPlan>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

## 2. LLM-Powered Routing

Building on the existing `task_router_llm.rs` implementation, expand the LLM's role in making sophisticated routing decisions.

### Actionable Implementation Steps:

1. **Enhance the RoutingAgent Interface**
   ```rust
   // Enhance src/bidirectional_agent/llm_routing/mod.rs
   
   pub struct EnhancedRoutingRequest {
       pub task: Task,
       pub context: RoutingContext,
   }
   
   pub struct RoutingContext {
       pub available_tools: Vec<ToolInfo>,
       pub available_agents: Vec<AgentInfo>,
       pub system_state: SystemState,
       pub routing_history: Vec<HistoricalRouting>,
       pub user_preferences: Option<Value>,
   }
   
   pub struct EnhancedRoutingResponse {
       pub decision: RoutingDecision,
       pub explanation: String,
       pub confidence: f32,
       pub fallback_options: Vec<RoutingDecision>,
   }
   
   // Update LlmClient trait to support enhanced routing
   #[async_trait]
   pub trait RoutingAgent {
       async fn make_routing_decision(
           &self, 
           request: EnhancedRoutingRequest
       ) -> Result<EnhancedRoutingResponse, AgentError>;
       
       async fn explain_decision(
           &self, 
           decision: &RoutingDecision,
           original_task: &Task 
       ) -> Result<String, AgentError>;
       
       async fn evaluate_routing_success(
           &self,
           original_task: &Task,
           routing_decision: &RoutingDecision,
           execution_result: &Task
       ) -> Result<RoutingFeedback, AgentError>;
   }
   ```

2. **Create an Advanced Claude Client Implementation**
   ```rust
   // Enhance src/bidirectional_agent/llm_routing/claude_client.rs
   
   impl RoutingAgent for ClaudeClient {
       async fn make_routing_decision(
           &self, 
           request: EnhancedRoutingRequest
       ) -> Result<EnhancedRoutingResponse, AgentError> {
           // Create a comprehensive prompt that considers all context
           let tools_section = self.format_tools_for_prompt(&request.context.available_tools);
           let agents_section = self.format_agents_for_prompt(&request.context.available_agents);
           let system_state_section = self.format_system_state(&request.context.system_state);
           let history_section = self.format_routing_history(&request.context.routing_history);
           let task_section = self.format_task(&request.task);
           
           // Build the comprehensive prompt
           let prompt = format!(
               "You are the routing intelligence for an AI agent system, deciding how to handle tasks.\n\n\
               TASK TO ROUTE:\n{}\n\n\
               AVAILABLE LOCAL TOOLS:\n{}\n\n\
               AVAILABLE REMOTE AGENTS:\n{}\n\n\
               SYSTEM STATE:\n{}\n\n\
               PREVIOUS ROUTING DECISIONS:\n{}\n\n\
               Based on all available information, determine the optimal way to handle this task.\n\
               Consider the following factors:\n\
               1. Task complexity and requirements\n\
               2. Available tool and agent capabilities\n\
               3. Current system load and resource availability\n\
               4. Past routing decisions and their outcomes\n\
               5. Geographic/jurisdictional considerations\n\
               6. Performance and reliability requirements\n\n\
               Provide your routing decision in the following JSON format:\n{}",
               task_section,
               tools_section,
               agents_section,
               system_state_section,
               history_section,
               ROUTING_RESPONSE_FORMAT
           );
           
           // Call Claude with structured output format
           let response_json = self.call_claude_with_json::<EnhancedRoutingResponseRaw>(prompt).await?;
           
           // Convert to the proper response format
           let response = EnhancedRoutingResponse {
               decision: self.parse_routing_decision(&response_json.decision)?,
               explanation: response_json.explanation,
               confidence: response_json.confidence,
               fallback_options: self.parse_fallback_options(&response_json.fallback_options)?,
           };
           
           Ok(response)
       }
       
       async fn explain_decision(
           &self, 
           decision: &RoutingDecision,
           original_task: &Task 
       ) -> Result<String, AgentError> {
           // Format the decision and task for explanation
           let decision_str = format!("{:?}", decision);
           let task_str = self.format_task(original_task);
           
           // Create prompt for explanation
           let prompt = format!(
               "You are an expert at explaining AI system decisions in clear, understandable terms.\n\n\
               TASK:\n{}\n\n\
               ROUTING DECISION:\n{}\n\n\
               Please explain why this routing decision was made for this task.\n\
               Your explanation should be:\n\
               - Clear and concise (2-3 sentences)\n\
               - Focused on capabilities matched\n\
               - Non-technical for general users\n\
               - Justifying why this was the optimal choice",
               task_str,
               decision_str
           );
           
           // Call LLM
           let response = self.llm_client.complete(prompt).await?;
           
           Ok(response)
       }
       
       async fn evaluate_routing_success(
           &self,
           original_task: &Task,
           routing_decision: &RoutingDecision,
           execution_result: &Task
       ) -> Result<RoutingFeedback, AgentError> {
           // Format all information for evaluation
           let original_task_str = self.format_task(original_task);
           let decision_str = format!("{:?}", routing_decision);
           let result_str = self.format_task_result(execution_result);
           
           // Create prompt for evaluation
           let prompt = format!(
               "You are an expert at evaluating if routing decisions were optimal.\n\n\
               ORIGINAL TASK:\n{}\n\n\
               ROUTING DECISION MADE:\n{}\n\n\
               EXECUTION RESULT:\n{}\n\n\
               Evaluate whether this routing decision was successful and optimal.\n\
               Consider:\n\
               1. Was the task completed successfully?\n\
               2. Was this the best choice given the results?\n\
               3. Were there any issues that suggest a different routing would have been better?\n\
               4. What can be learned from this for future routing decisions?\n\n\
               Respond in the following JSON format:\n{}",
               original_task_str,
               decision_str,
               result_str,
               ROUTING_FEEDBACK_FORMAT
           );
           
           // Call LLM with structured output
           let response = self.llm_client.complete_with_json::<RoutingFeedback>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

3. **Implement Learning from Past Routing Decisions**
   ```rust
   // New file: src/bidirectional_agent/llm_routing/learning.rs
   
   pub struct RoutingLearningManager {
       llm_client: Arc<dyn LlmClient>,
       history: Arc<RoutingHistoryManager>,
   }
   
   impl RoutingLearningManager {
       pub async fn analyze_routing_patterns(&self) -> Result<RoutingInsights, AgentError> {
           // Get recent routing history
           let recent_history = self.history.get_recent_history(100).await?;
           
           // Format history for LLM analysis
           let history_str = format_routing_history(&recent_history);
           
           // Create prompt for pattern analysis
           let prompt = format!(
               "You are an expert AI system analyzing patterns in routing decisions.\n\n\
               RECENT ROUTING HISTORY:\n{}\n\n\
               Please analyze this routing history and identify patterns, insights, and improvements.\n\
               Consider:\n\
               1. Common patterns in successful routings\n\
               2. Recurring issues in failed routings\n\
               3. Types of tasks that might benefit from different routing strategies\n\
               4. Opportunities to optimize routing efficiency\n\
               5. Suggestions for new routing heuristics\n\n\
               Respond with your analysis in the following JSON format:\n{}",
               history_str,
               ROUTING_INSIGHTS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<RoutingInsights>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn generate_routing_heuristics(&self, 
           insights: &RoutingInsights
       ) -> Result<Vec<RoutingHeuristic>, AgentError> {
           // Create prompt for generating heuristics
           let prompt = format!(
               "You are an expert AI system creating routing heuristics based on observed patterns.\n\n\
               ROUTING INSIGHTS:\n{}\n\n\
               Based on these insights, generate specific routing heuristics that can improve routing decisions.\n\
               Each heuristic should:\n\
               1. Target a specific task pattern or characteristic\n\
               2. Provide a clear condition for when it applies\n\
               3. Specify a concrete routing action\n\
               4. Include a confidence threshold\n\n\
               Respond with your heuristics in the following JSON format:\n{}",
               serde_json::to_string_pretty(insights)?,
               ROUTING_HEURISTICS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<RoutingHeuristic>>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

4. **Create Task Complexity Analyzer**
   ```rust
   // New file: src/bidirectional_agent/llm_routing/complexity.rs
   
   pub struct TaskComplexityAnalyzer {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl TaskComplexityAnalyzer {
       pub async fn analyze_task_complexity(&self, task: &Task) -> Result<ComplexityAnalysis, AgentError> {
           // Extract task description
           let task_text = self.extract_task_description(task);
           
           // Create prompt for complexity analysis
           let prompt = format!(
               "You are an expert at analyzing the complexity of tasks for optimal routing.\n\n\
               TASK TO ANALYZE:\n{}\n\n\
               Please analyze this task's complexity and requirements.\n\
               Consider the following dimensions:\n\
               1. Cognitive complexity (simple query vs complex reasoning)\n\
               2. Domain knowledge required (general vs specialized)\n\
               3. Tool requirements (basic vs advanced/multiple tools)\n\
               4. Multi-step reasoning needed (single-step vs multi-step)\n\
               5. Data processing volume (small vs large dataset)\n\
               6. Ambiguity level (clear vs ambiguous instructions)\n\n\
               Respond with your analysis in the following JSON format:\n{}",
               task_text,
               COMPLEXITY_ANALYSIS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<ComplexityAnalysis>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn estimate_resource_requirements(&self, 
           task: &Task,
           complexity: &ComplexityAnalysis
       ) -> Result<ResourceRequirements, AgentError> {
           // Create prompt for resource estimation
           let prompt = format!(
               "You are an expert at estimating resource requirements for AI tasks.\n\n\
               TASK DESCRIPTION:\n{}\n\n\
               COMPLEXITY ANALYSIS:\n{}\n\n\
               Based on this task and its complexity analysis, estimate the resources required.\n\
               Consider:\n\
               1. Computational resources (CPU, memory, GPU)\n\
               2. Expected execution time\n\
               3. External API calls needed\n\
               4. Storage requirements\n\
               5. Network bandwidth\n\n\
               Respond with your estimates in the following JSON format:\n{}",
               self.extract_task_description(task),
               serde_json::to_string_pretty(complexity)?,
               RESOURCE_REQUIREMENTS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<ResourceRequirements>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

## 3. Result Synthesis

Create a comprehensive LLM-powered synthesis system that can intelligently combine and refine results from multiple sources.

### Actionable Implementation Steps:

1. **Create LLM-Based Synthesizer**
   ```rust
   // New file: src/bidirectional_agent/llm_synthesis/mod.rs
   
   pub struct LlmSynthesizer {
       llm_client: Arc<dyn LlmClient>,
   }
   
   #[async_trait]
   impl ResultSynthesizer for LlmSynthesizer {
       async fn synthesize(&self, original_task: &Task, delegate_results: &[Task]) -> Result<SynthesisResult, AgentError> {
           // Extract the original task query
           let task_query = self.extract_task_query(original_task);
           
           // Extract results from all delegate tasks
           let results_context = self.format_delegate_results(delegate_results);
           
           // Define the synthesis approach based on result types
           let synthesis_strategy = self.determine_synthesis_strategy(delegate_results).await?;
           
           // Create a customized prompt based on the synthesis strategy
           let prompt = match synthesis_strategy {
               SynthesisStrategy::Integration => self.create_integration_prompt(task_query, results_context),
               SynthesisStrategy::Reconciliation => self.create_reconciliation_prompt(task_query, results_context),
               SynthesisStrategy::Prioritization => self.create_prioritization_prompt(task_query, results_context),
               SynthesisStrategy::MultiFormat => self.create_multiformat_prompt(task_query, results_context),
           };
           
           // Call the LLM
           let synthesis_response = self.llm_client.complete(prompt).await?;
           
           // Format the response into the expected structure
           self.format_synthesis_result(synthesis_response, original_task, delegate_results)
       }
   }
   
   impl LlmSynthesizer {
       async fn determine_synthesis_strategy(&self, delegate_results: &[Task]) -> Result<SynthesisStrategy, AgentError> {
           // Extract summary information about the results
           let result_summaries: Vec<String> = delegate_results.iter()
               .map(|task| self.summarize_task_result(task))
               .collect();
           
           // Create a prompt to determine the best synthesis strategy
           let prompt = format!(
               "You are an expert at determining the optimal strategy for synthesizing multiple results.\n\n\
               RESULTS TO SYNTHESIZE:\n{}\n\n\
               Based on these results, determine the best synthesis strategy from the following options:\n\
               1. INTEGRATION - Results complement each other and should be combined\n\
               2. RECONCILIATION - Results have conflicts that need resolution\n\
               3. PRIORITIZATION - Results vary in quality/relevance and should be ranked\n\
               4. MULTI-FORMAT - Results have different formats requiring specialized handling\n\n\
               Respond with only one word: the name of the best strategy.",
               result_summaries.join("\n\n")
           );
           
           // Call LLM
           let response = self.llm_client.complete(prompt).await?;
           
           // Parse the response
           match response.trim().to_uppercase().as_str() {
               "INTEGRATION" => Ok(SynthesisStrategy::Integration),
               "RECONCILIATION" => Ok(SynthesisStrategy::Reconciliation),
               "PRIORITIZATION" => Ok(SynthesisStrategy::Prioritization),
               "MULTI-FORMAT" => Ok(SynthesisStrategy::MultiFormat),
               _ => Ok(SynthesisStrategy::Integration), // Default to Integration
           }
       }
       
       fn create_integration_prompt(&self, task_query: &str, results_context: &str) -> String {
           format!(
               "You are an expert at synthesizing complementary information into coherent responses.\n\n\
               ORIGINAL QUERY:\n{}\n\n\
               RESULTS TO INTEGRATE:\n{}\n\n\
               Create a comprehensive response that integrates all relevant information from these results.\n\
               Your synthesis should:\n\
               1. Bring together complementary points\n\
               2. Avoid unnecessary repetition\n\
               3. Present a unified, coherent narrative\n\
               4. Maintain appropriate level of detail\n\
               5. Address all aspects of the original query\n\
               6. Attribute information to sources where helpful",
               task_query,
               results_context
           )
       }
       
       fn create_reconciliation_prompt(&self, task_query: &str, results_context: &str) -> String {
           format!(
               "You are an expert at reconciling potentially conflicting information.\n\n\
               ORIGINAL QUERY:\n{}\n\n\
               RESULTS WITH POTENTIAL CONFLICTS:\n{}\n\n\
               Create a carefully reconciled response that addresses conflicts in these results.\n\
               Your synthesis should:\n\
               1. Identify key points of agreement across sources\n\
               2. Highlight significant disagreements\n\
               3. Evaluate reliability of conflicting claims\n\
               4. Propose most probable conclusions\n\
               5. Acknowledge uncertainty where appropriate\n\
               6. Explain reasoning for reconciliation decisions",
               task_query,
               results_context
           )
       }
       
       // Additional prompt creation methods for other strategies...
   }
   ```

2. **Implement Conflict Detection and Resolution**
   ```rust
   // New file: src/bidirectional_agent/llm_synthesis/conflict_resolution.rs
   
   pub struct LlmConflictResolver {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl LlmConflictResolver {
       pub async fn detect_conflicts(&self, results: &[Task]) -> Result<Vec<Conflict>, AgentError> {
           // Format the results for analysis
           let results_text = results.iter()
               .map(|task| self.format_task_result(task))
               .collect::<Vec<_>>()
               .join("\n\n");
           
           // Create prompt for conflict detection
           let prompt = format!(
               "You are an expert at identifying conflicts between different information sources.\n\n\
               RESULTS TO ANALYZE FOR CONFLICTS:\n{}\n\n\
               Identify any conflicts or contradictions between these results.\n\
               For each conflict, specify:\n\
               1. The specific contradictory claims\n\
               2. Which sources make each claim\n\
               3. The nature of the contradiction\n\
               4. Significance of the conflict (minor detail vs major conclusion)\n\n\
               Respond in the following JSON format:\n{}",
               results_text,
               CONFLICTS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<Conflict>>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn resolve_conflicts(&self, 
           conflicts: &[Conflict], 
           results: &[Task],
           original_query: &str
       ) -> Result<Vec<ResolvedConflict>, AgentError> {
           // Format conflicts and results for the prompt
           let conflicts_json = serde_json::to_string_pretty(conflicts)?;
           let results_summary = self.summarize_results(results);
           
           // Create prompt for conflict resolution
           let prompt = format!(
               "You are an expert at resolving conflicts between information sources.\n\n\
               ORIGINAL QUERY:\n{}\n\n\
               CONFLICTING RESULTS:\n{}\n\n\
               IDENTIFIED CONFLICTS:\n{}\n\n\
               For each identified conflict, propose a resolution.\n\
               Your resolutions should:\n\
               1. Evaluate the reliability of each conflicting claim\n\
               2. Consider which source likely has more accurate information\n\
               3. Use logical reasoning to determine the most likely truth\n\
               4. Decide when to present multiple perspectives vs a single conclusion\n\
               5. Acknowledge uncertainty when a definitive resolution isn't possible\n\n\
               Respond in the following JSON format:\n{}",
               original_query,
               results_summary,
               conflicts_json,
               RESOLVED_CONFLICTS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<ResolvedConflict>>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

3. **Create Content-Type Specific Synthesizers**
   ```rust
   // New file: src/bidirectional_agent/llm_synthesis/specialized.rs
   
   pub struct CodeSynthesizer {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl CodeSynthesizer {
       pub async fn synthesize_code(&self, 
           code_fragments: &[String], 
           language: &str,
           purpose: &str
       ) -> Result<String, AgentError> {
           // Format code fragments for the prompt
           let code_context = code_fragments.iter()
               .enumerate()
               .map(|(i, code)| format!("CODE FRAGMENT {}:\n```\n{}\n```", i+1, code))
               .collect::<Vec<_>>()
               .join("\n\n");
           
           // Create specialized prompt for code synthesis
           let prompt = format!(
               "You are an expert software developer specializing in code integration.\n\n\
               LANGUAGE: {}\n\
               PURPOSE: {}\n\n\
               CODE FRAGMENTS TO SYNTHESIZE:\n{}\n\n\
               Create a cohesive, working program that integrates these code fragments.\n\
               Your synthesis should:\n\
               1. Resolve any naming conflicts or duplicated functionality\n\
               2. Ensure proper dependencies between components\n\
               3. Maintain consistent style and naming conventions\n\
               4. Add necessary imports/includes\n\
               5. Ensure the resulting code is efficient and well-structured\n\
               6. Add clarifying comments where helpful\n\n\
               Respond with ONLY the synthesized code in the proper format for {}.",
               language,
               purpose,
               code_context,
               language
           );
           
           // Call LLM
           let response = self.llm_client.complete(prompt).await?;
           
           // Extract code from response
           extract_code_from_llm_response(&response)
       }
   }
   
   pub struct DataSynthesizer {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl DataSynthesizer {
       pub async fn synthesize_structured_data(
           &self,
           data_objects: &[Value],
           schema: Option<&str>,
           purpose: &str
       ) -> Result<Value, AgentError> {
           // Format data objects for the prompt
           let data_context = data_objects.iter()
               .enumerate()
               .map(|(i, data)| format!("DATA OBJECT {}:\n{}", i+1, data))
               .collect::<Vec<_>>()
               .join("\n\n");
           
           // Include schema if available
           let schema_section = if let Some(schema_str) = schema {
               format!("TARGET SCHEMA:\n{}\n\n", schema_str)
           } else {
               "".to_string()
           };
           
           // Create specialized prompt for data synthesis
           let prompt = format!(
               "You are an expert data engineer specializing in data integration.\n\n\
               PURPOSE: {}\n\n\
               {}DATA OBJECTS TO SYNTHESIZE:\n{}\n\n\
               Create a unified data structure that integrates these data objects.\n\
               Your synthesis should:\n\
               1. Resolve property name conflicts appropriately\n\
               2. Handle duplicate or conflicting values\n\
               3. Ensure the structure is logical and well-organized\n\
               4. Follow the target schema if provided\n\
               5. Preserve important information from all sources\n\n\
               Respond with ONLY the synthesized data as a valid JSON object.",
               purpose,
               schema_section,
               data_context
           );
           
           // Call LLM
           let response = self.llm_client.complete(prompt).await?;
           
           // Parse JSON response
           let parsed = serde_json::from_str::<Value>(&response)
               .map_err(|e| AgentError::SynthesisError(format!("Failed to parse synthesized data: {}", e)))?;
           
           Ok(parsed)
       }
   }
   ```

4. **Implement Quality Evaluation**
   ```rust
   // New file: src/bidirectional_agent/llm_synthesis/quality.rs
   
   pub struct SynthesisQualityEvaluator {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl SynthesisQualityEvaluator {
       pub async fn evaluate_synthesis_quality(
           &self,
           original_task: &Task,
           delegate_results: &[Task],
           synthesis: &SynthesisResult
       ) -> Result<QualityEvaluation, AgentError> {
           // Format all relevant information
           let task_query = extract_task_query(original_task);
           let results_summary = summarize_results(delegate_results);
           let synthesis_text = format_synthesis(synthesis);
           
           // Create prompt for quality evaluation
           let prompt = format!(
               "You are an expert at evaluating the quality of synthesized responses.\n\n\
               ORIGINAL QUERY:\n{}\n\n\
               SOURCE RESULTS:\n{}\n\n\
               SYNTHESIZED RESPONSE:\n{}\n\n\
               Evaluate the quality of this synthesis based on the following criteria:\n\
               1. Completeness: Does it include all relevant information from sources?\n\
               2. Accuracy: Does it faithfully represent the source information?\n\
               3. Consistency: Is it free from internal contradictions?\n\
               4. Coherence: Is it well-organized and easy to follow?\n\
               5. Relevance: Does it directly address the original query?\n\
               6. Conflicts: Are conflicts or disagreements handled appropriately?\n\n\
               Respond in the following JSON format:\n{}",
               task_query,
               results_summary,
               synthesis_text,
               QUALITY_EVALUATION_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<QualityEvaluation>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn suggest_improvements(
           &self,
           evaluation: &QualityEvaluation
       ) -> Result<Vec<SynthesisImprovement>, AgentError> {
           // Create prompt for improvement suggestions
           let prompt = format!(
               "You are an expert at improving information synthesis quality.\n\n\
               QUALITY EVALUATION:\n{}\n\n\
               Based on this evaluation, suggest specific improvements to enhance the synthesis.\n\
               Focus on addressing the weakest aspects identified in the evaluation.\n\
               For each suggestion, provide:\n\
               1. A clear description of the improvement\n\
               2. The specific quality dimension it addresses\n\
               3. How to implement the improvement\n\
               4. The expected impact on overall quality\n\n\
               Respond in the following JSON format:\n{}",
               serde_json::to_string_pretty(evaluation)?,
               SYNTHESIS_IMPROVEMENTS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<SynthesisImprovement>>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

## 4. Remote Tool Discovery & Integration

Leverage LLM intelligence to discover, understand, and integrate tools across the agent network.

### Actionable Implementation Steps:

1. **Create LLM-Powered Tool Discovery**
   ```rust
   // New file: src/bidirectional_agent/llm_tools/discovery.rs
   
   pub struct LlmToolDiscovery {
       llm_client: Arc<dyn LlmClient>,
       agent_directory: Arc<AgentDirectory>,
   }
   
   impl LlmToolDiscovery {
       pub async fn analyze_agent_capabilities(
           &self,
           agent_id: &str,
           agent_card: &AgentCard
       ) -> Result<Vec<ToolCapability>, AgentError> {
           // Format agent card for analysis
           let card_json = serde_json::to_string_pretty(agent_card)?;
           
           // Create prompt for capability analysis
           let prompt = format!(
               "You are an expert at analyzing AI agent capabilities and identifying tools.\n\n\
               AGENT CARD TO ANALYZE:\n{}\n\n\
               Based on this agent card, identify all capabilities that could be exposed as tools.\n\
               For each capability/tool, determine:\n\
               1. A clear name for the tool\n\
               2. Expected input parameters\n\
               3. Expected output format\n\
               4. Use cases where this tool would be valuable\n\
               5. Limitations or constraints\n\n\
               Respond in the following JSON format:\n{}",
               card_json,
               TOOL_CAPABILITIES_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<ToolCapability>>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn discover_complementary_tools(
           &self,
           task: &Task,
           available_tools: &[RemoteTool]
       ) -> Result<Vec<ToolRecommendation>, AgentError> {
           // Extract task requirements
           let task_text = extract_task_description(task);
           
           // Format available tools
           let tools_summary = format_tools_summary(available_tools);
           
           // Create prompt for tool recommendations
           let prompt = format!(
               "You are an expert at identifying which tools would be most useful for tasks.\n\n\
               TASK:\n{}\n\n\
               CURRENTLY AVAILABLE TOOLS:\n{}\n\n\
               Based on this task, recommend which available tools would be most helpful.\n\
               For each recommended tool, explain:\n\
               1. Why it's relevant to this specific task\n\
               2. How it should be used in this context\n\
               3. What aspect of the task it addresses\n\
               4. Confidence in its utility (high/medium/low)\n\n\
               Respond in the following JSON format:\n{}",
               task_text,
               tools_summary,
               TOOL_RECOMMENDATIONS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<ToolRecommendation>>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn infer_missing_tools(
           &self,
           task: &Task,
           available_tools: &[RemoteTool]
       ) -> Result<Vec<MissingToolDescription>, AgentError> {
           // Extract task requirements
           let task_text = extract_task_description(task);
           
           // Format available tools
           let tools_summary = format_tools_summary(available_tools);
           
           // Create prompt for missing tool inference
           let prompt = format!(
               "You are an expert at identifying capability gaps in tool ecosystems.\n\n\
               TASK:\n{}\n\n\
               CURRENTLY AVAILABLE TOOLS:\n{}\n\n\
               Based on this task, identify capabilities/tools that are needed but not available.\n\
               For each missing capability, describe:\n\
               1. What the missing tool would do\n\
               2. Why it's necessary for this task\n\
               3. What inputs it would require\n\
               4. What outputs it would produce\n\
               5. How critical it is for task completion (essential/helpful/optional)\n\n\
               Respond in the following JSON format:\n{}",
               task_text,
               tools_summary,
               MISSING_TOOLS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<MissingToolDescription>>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

2. **Implement Remote Tool Parameter Mapping**
   ```rust
   // New file: src/bidirectional_agent/llm_tools/parameter_mapping.rs
   
   pub struct ToolParameterMapper {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl ToolParameterMapper {
       pub async fn map_task_to_tool_parameters(
           &self,
           task: &Task,
           tool: &RemoteTool
       ) -> Result<Value, AgentError> {
           // Extract task description
           let task_text = extract_task_description(task);
           
           // Format tool parameter schema
           let parameter_schema = tool.parameters_schema
               .as_ref()
               .map(|s| serde_json::to_string_pretty(s).unwrap_or_default())
               .unwrap_or_else(|| "{}".to_string());
           
           // Create prompt for parameter extraction
           let prompt = format!(
               "You are an expert at extracting structured parameters from natural language requests.\n\n\
               TASK DESCRIPTION:\n{}\n\n\
               TARGET TOOL: {}\n\
               PARAMETER SCHEMA:\n{}\n\n\
               Extract the appropriate parameters for this tool from the task description.\n\
               Be sure to:\n\
               1. Map concepts in the task to the correct parameter names\n\
               2. Format values according to their schema types\n\
               3. Use reasonable defaults for missing optional parameters\n\
               4. When unsure about a required parameter, make a reasonable inference\n\n\
               Respond with ONLY a valid JSON object containing the extracted parameters.",
               task_text,
               tool.name,
               parameter_schema
           );
           
           // Call LLM
           let response = self.llm_client.complete(prompt).await?;
           
           // Parse as JSON
           let params = serde_json::from_str::<Value>(&response)
               .map_err(|e| AgentError::ToolError(format!("Failed to parse parameter mapping: {}", e)))?;
           
           Ok(params)
       }
       
       pub async fn validate_parameter_mapping(
           &self,
           parameters: &Value,
           tool: &RemoteTool
       ) -> Result<ValidationResult, AgentError> {
           // Format tool parameter schema
           let parameter_schema = tool.parameters_schema
               .as_ref()
               .map(|s| serde_json::to_string_pretty(s).unwrap_or_default())
               .unwrap_or_else(|| "{}".to_string());
           
           // Format parameters
           let parameters_json = serde_json::to_string_pretty(parameters)?;
           
           // Create prompt for validation
           let prompt = format!(
               "You are an expert at validating parameters against schema requirements.\n\n\
               TOOL: {}\n\
               PARAMETER SCHEMA:\n{}\n\n\
               PARAMETERS TO VALIDATE:\n{}\n\n\
               Validate these parameters against the schema.\n\
               Check for:\n\
               1. Missing required parameters\n\
               2. Type mismatches (e.g., string vs number)\n\
               3. Values outside allowed ranges\n\
               4. Format errors (e.g., date format)\n\
               5. Other schema violations\n\n\
               Respond in the following JSON format:\n{}",
               tool.name,
               parameter_schema,
               parameters_json,
               VALIDATION_RESULT_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<ValidationResult>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

3. **Create Tool Call Translator**
   ```rust
   // New file: src/bidirectional_agent/llm_tools/translation.rs
   
   pub struct LlmToolTranslator {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl LlmToolTranslator {
       pub async fn translate_tool_call_to_task(
           &self,
           tool_call: &ToolCall,
           agent_card: &AgentCard
       ) -> Result<TaskSendParams, AgentError> {
           // Format tool call and agent card
           let tool_call_json = serde_json::to_string_pretty(tool_call)?;
           let agent_card_json = serde_json::to_string_pretty(agent_card)?;
           
           // Create prompt for translation
           let prompt = format!(
               "You are an expert at translating tool calls into natural language tasks.\n\n\
               TOOL CALL TO TRANSLATE:\n{}\n\n\
               TARGET AGENT CAPABILITIES:\n{}\n\n\
               Create a natural language task description that will accomplish what this tool call is trying to do.\n\
               Your translation should:\n\
               1. Express the tool's purpose in clear natural language\n\
               2. Include all relevant parameters as part of the description\n\
               3. Format the request according to the agent's expected input modes\n\
               4. Be specific enough to ensure the original tool functionality is maintained\n\
               5. Use appropriate language for the target agent's capabilities\n\n\
               Respond in the following JSON format for TaskSendParams:\n{}",
               tool_call_json,
               agent_card_json,
               TASK_SEND_PARAMS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<TaskSendParams>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn translate_task_result_to_tool_result(
           &self,
           task_result: &Task,
           original_tool_call: &ToolCall,
           expected_schema: Option<&Value>
       ) -> Result<Value, AgentError> {
           // Format task result and tool call
           let task_result_text = format_task_result(task_result);
           let tool_call_json = serde_json::to_string_pretty(original_tool_call)?;
           
           // Format expected schema if available
           let schema_section = if let Some(schema) = expected_schema {
               format!("EXPECTED RESULT SCHEMA:\n{}\n\n", serde_json::to_string_pretty(schema)?)
           } else {
               "".to_string()
           };
           
           // Create prompt for translation
           let prompt = format!(
               "You are an expert at translating task results back into structured tool results.\n\n\
               ORIGINAL TOOL CALL:\n{}\n\n\
               TASK RESULT TO TRANSLATE:\n{}\n\n\
               {}Extract the relevant information from this task result and format it as a structured result.\n\
               Your translation should:\n\
               1. Extract exactly the information requested in the original tool call\n\
               2. Format the data according to the expected schema (if provided)\n\
               3. Ensure types are correct (strings, numbers, booleans, etc.)\n\
               4. Include only the essential information, omitting conversational elements\n\
               5. Structure the response logically\n\n\
               Respond with ONLY a valid JSON object containing the extracted result.",
               tool_call_json,
               task_result_text,
               schema_section
           );
           
           // Call LLM
           let response = self.llm_client.complete(prompt).await?;
           
           // Parse as JSON
           let result = serde_json::from_str::<Value>(&response)
               .map_err(|e| AgentError::ToolError(format!("Failed to parse tool result: {}", e)))?;
           
           Ok(result)
       }
   }
   ```

4. **Implement Tool Capability Matcher**
   ```rust
   // New file: src/bidirectional_agent/llm_tools/capability_matcher.rs
   
   pub struct LlmCapabilityMatcher {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl LlmCapabilityMatcher {
       pub async fn extract_required_capabilities(
           &self,
           task: &Task
       ) -> Result<Vec<RequiredCapability>, AgentError> {
           // Extract task description
           let task_text = extract_task_description(task);
           
           // Create prompt for capability extraction
           let prompt = format!(
               "You are an expert at analyzing tasks to determine required capabilities.\n\n\
               TASK TO ANALYZE:\n{}\n\n\
               Based on this task, identify the specific capabilities required to complete it.\n\
               For each capability, specify:\n\
               1. A clear name/category for the capability\n\
               2. Why it's needed for this specific task\n\
               3. The importance level (essential, helpful, optional)\n\
               4. Any specific requirements within this capability\n\n\
               Respond in the following JSON format:\n{}",
               task_text,
               REQUIRED_CAPABILITIES_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<RequiredCapability>>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn match_tools_to_capabilities(
           &self,
           required_capabilities: &[RequiredCapability],
           available_tools: &[RemoteTool]
       ) -> Result<Vec<ToolMatch>, AgentError> {
           // Format capabilities and tools
           let capabilities_json = serde_json::to_string_pretty(required_capabilities)?;
           let tools_summary = format_tools_summary(available_tools);
           
           // Create prompt for matching
           let prompt = format!(
               "You are an expert at matching required capabilities to available tools.\n\n\
               REQUIRED CAPABILITIES:\n{}\n\n\
               AVAILABLE TOOLS:\n{}\n\n\
               Match each required capability to the most appropriate available tool(s).\n\
               For each match, specify:\n\
               1. The capability being matched\n\
               2. The tool(s) that can fulfill it\n\
               3. How well the tool satisfies the capability (fully, partially, minimally)\n\
               4. Any limitations or gaps in the tool's implementation\n\
               5. Confidence score (0-1) in this match\n\n\
               Respond in the following JSON format:\n{}",
               capabilities_json,
               tools_summary,
               TOOL_MATCHES_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<ToolMatch>>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

## 5. Robustness Improvements

Leverage LLM intelligence to predict, detect, and recover from errors throughout the system.

### Actionable Implementation Steps:

1. **Create LLM-Powered Error Analyzer**
   ```rust
   // New file: src/bidirectional_agent/llm_resilience/error_analyzer.rs
   
   pub struct LlmErrorAnalyzer {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl LlmErrorAnalyzer {
       pub async fn analyze_error(
           &self,
           error: &AgentError,
           context: &ErrorContext
       ) -> Result<ErrorAnalysis, AgentError> {
           // Format error and context
           let error_text = format!("{:?}", error);
           let context_json = serde_json::to_string_pretty(context)?;
           
           // Create prompt for error analysis
           let prompt = format!(
               "You are an expert at analyzing errors in AI agent systems.\n\n\
               ERROR:\n{}\n\n\
               ERROR CONTEXT:\n{}\n\n\
               Please analyze this error comprehensively.\n\
               Consider:\n\
               1. Root cause classification\n\
               2. Likely origin (client, network, server, etc.)\n\
               3. Severity assessment\n\
               4. Potential impact on overall system\n\
               5. Likelihood of recurrence\n\
               6. Retryability assessment\n\n\
               Respond in the following JSON format:\n{}",
               error_text,
               context_json,
               ERROR_ANALYSIS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<ErrorAnalysis>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn suggest_recovery_actions(
           &self,
           error_analysis: &ErrorAnalysis,
           context: &ErrorContext
       ) -> Result<Vec<RecoveryAction>, AgentError> {
           // Format analysis and context
           let analysis_json = serde_json::to_string_pretty(error_analysis)?;
           let context_json = serde_json::to_string_pretty(context)?;
           
           // Create prompt for recovery suggestions
           let prompt = format!(
               "You are an expert at suggesting recovery actions for errors in AI agent systems.\n\n\
               ERROR ANALYSIS:\n{}\n\n\
               ERROR CONTEXT:\n{}\n\n\
               Based on this analysis, suggest appropriate recovery actions.\n\
               For each suggested action:\n\
               1. Provide a clear action description\n\
               2. Explain how it addresses the root cause\n\
               3. Assess probability of success\n\
               4. Note any potential side effects\n\
               5. Specify the order in which actions should be attempted\n\n\
               Respond in the following JSON format:\n{}",
               analysis_json,
               context_json,
               RECOVERY_ACTIONS_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<RecoveryAction>>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

2. **Implement Predictive Failure Detection**
   ```rust
   // New file: src/bidirectional_agent/llm_resilience/predictive.rs
   
   pub struct PredictiveFailureDetector {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl PredictiveFailureDetector {
       pub async fn assess_task_risks(
           &self,
           task: &Task,
           system_state: &SystemState
       ) -> Result<RiskAssessment, AgentError> {
           // Format task and system state
           let task_text = extract_task_description(task);
           let state_json = serde_json::to_string_pretty(system_state)?;
           
           // Create prompt for risk assessment
           let prompt = format!(
               "You are an expert at predicting potential failures in AI task execution.\n\n\
               TASK TO ASSESS:\n{}\n\n\
               CURRENT SYSTEM STATE:\n{}\n\n\
               Predict potential risks and failure points for this task in the current system state.\n\
               Consider:\n\
               1. Task complexity relative to available resources\n\
               2. Required dependencies and their reliability\n\
               3. Historical failure patterns in similar tasks\n\
               4. Current system load and potential bottlenecks\n\
               5. Time constraints and deadline feasibility\n\n\
               Respond in the following JSON format:\n{}",
               task_text,
               state_json,
               RISK_ASSESSMENT_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<RiskAssessment>(prompt).await?;
           
           Ok(response)
       }
       
       pub async fn suggest_preventive_measures(
           &self,
           assessment: &RiskAssessment
       ) -> Result<Vec<PreventiveMeasure>, AgentError> {
           // Format assessment
           let assessment_json = serde_json::to_string_pretty(assessment)?;
           
           // Create prompt for preventive measures
           let prompt = format!(
               "You are an expert at recommending preventive measures for AI task execution.\n\n\
               RISK ASSESSMENT:\n{}\n\n\
               Based on this risk assessment, suggest preventive measures to mitigate identified risks.\n\
               For each measure:\n\
               1. Provide a clear description of the measure\n\
               2. Specify which risk(s) it addresses\n\
               3. Assess implementation complexity\n\
               4. Estimate effectiveness in reducing risk\n\
               5. Note any potential tradeoffs or costs\n\n\
               Respond in the following JSON format:\n{}",
               assessment_json,
               PREVENTIVE_MEASURES_FORMAT
           );
           
           // Call LLM
           let response = self.llm_client.complete_with_json::<Vec<PreventiveMeasure>>(prompt).await?;
           
           Ok(response)
       }
   }
   ```

3. **Create LLM-Based Fallback Generator**
   ```rust
   // New file: src/bidirectional_agent/llm_resilience/fallback.rs
   
   pub struct LlmFallbackGenerator {
       llm_client: Arc<dyn LlmClient>,
   }
   
   impl LlmFallbackGenerator {
       pub async fn generate_fallback_options(
           &self,
           failed_task: &Task,
           error: &AgentError,
           available_tools: &[RemoteTool],
           available_agents: &[AgentSummary]
       ) -> Result<Vec<FallbackOption>, AgentError> {
           // Format all relevant information
           let task_text = extract_task_description(failed_task);
           let error_text = format!("{:?}", error);
           let tools_summary = format_tools_summary(available_tools);
           let agents_summary = format_agents_summary(available_agents);
           
           // Create prompt for fallback generation
           let prompt = format!(
               "You are an expert at generating fallback options when primary execution paths fail.\n\n\
               FAILED TASK:\n{}\n\n\
               ERROR ENCOUNTERED:\n{}\n\n\
               AVAILABLE TOOLS:\n{}\n\n\
               AVAILABLE AGENTS:\n{}\n\n\
               Generate alternative approaches to accomplish the original task objective.\n\
               For each fallback option:\n\
               1. Describe a clear alternative approach\n\
               2. Specify which tools/agents would be involved\n\
               Let me continue with the implementation of the LLM-Based Fallback Generator and complete the remaining resilience improvements:

```rust
// Continuing from src/bidirectional_agent/llm_resilience/fallback.rs

impl LlmFallbackGenerator {
    pub async fn generate_fallback_options(
        &self,
        failed_task: &Task,
        error: &AgentError,
        available_tools: &[RemoteTool],
        available_agents: &[AgentSummary]
    ) -> Result<Vec<FallbackOption>, AgentError> {
        // Format all relevant information
        let task_text = extract_task_description(failed_task);
        let error_text = format!("{:?}", error);
        let tools_summary = format_tools_summary(available_tools);
        let agents_summary = format_agents_summary(available_agents);
        
        // Create prompt for fallback generation
        let prompt = format!(
            "You are an expert at generating fallback options when primary execution paths fail.\n\n\
            FAILED TASK:\n{}\n\n\
            ERROR ENCOUNTERED:\n{}\n\n\
            AVAILABLE TOOLS:\n{}\n\n\
            AVAILABLE AGENTS:\n{}\n\n\
            Generate alternative approaches to accomplish the original task objective.\n\
            For each fallback option:\n\
            1. Describe a clear alternative approach\n\
            2. Specify which tools/agents would be involved\n\
            3. Assess how closely it matches the original objective\n\
            4. Estimate probability of success\n\
            5. Note any limitations compared to the original approach\n\n\
            Respond in the following JSON format:\n{}",
            task_text,
            error_text,
            tools_summary,
            agents_summary,
            FALLBACK_OPTIONS_FORMAT
        );
        
        // Call LLM
        let response = self.llm_client.complete_with_json::<Vec<FallbackOption>>(prompt).await?;
        
        Ok(response)
    }
    
    pub async fn create_simplified_task(
        &self,
        original_task: &Task,
        constraints: &SimplificationConstraints
    ) -> Result<Task, AgentError> {
        // Format original task and constraints
        let task_text = extract_task_description(original_task);
        let constraints_json = serde_json::to_string_pretty(constraints)?;
        
        // Create prompt for task simplification
        let prompt = format!(
            "You are an expert at simplifying complex tasks to ensure they can be completed.\n\n\
            ORIGINAL TASK:\n{}\n\n\
            SIMPLIFICATION CONSTRAINTS:\n{}\n\n\
            Create a simplified version of this task that:\n\
            1. Preserves the core objective as much as possible\n\
            2. Eliminates complex requirements that might cause failure\n\
            3. Works within the specified constraints\n\
            4. Is more likely to succeed than the original task\n\
            5. Clearly communicates what has been simplified to the user\n\n\
            Respond with a new task description that follows these guidelines.",
            task_text,
            constraints_json
        );
        
        // Call LLM
        let simplified_description = self.llm_client.complete(prompt).await?;
        
        // Create new task based on original but with simplified description
        let mut simplified_task = original_task.clone();
        if let Some(history) = &mut simplified_task.history {
            if let Some(last_msg) = history.last_mut() {
                if let Some(Part::TextPart(text_part)) = last_msg.parts.first_mut() {
                    text_part.text = simplified_description;
                }
            }
        }
        
        Ok(simplified_task)
    }
}
```

4. **Implement Degraded Mode Manager with LLM Intelligence**
```rust
// New file: src/bidirectional_agent/llm_resilience/degraded_mode.rs

pub struct LlmDegradedModeManager {
    llm_client: Arc<dyn LlmClient>,
    state: Arc<RwLock<SystemState>>,
}

impl LlmDegradedModeManager {
    pub async fn determine_operating_mode(
        &self,
        current_state: &SystemState,
        recent_errors: &[ErrorEvent]
    ) -> Result<OperatingModeDecision, AgentError> {
        // Format system state and recent errors
        let state_json = serde_json::to_string_pretty(current_state)?;
        let errors_json = serde_json::to_string_pretty(recent_errors)?;
        
        // Create prompt for operating mode determination
        let prompt = format!(
            "You are an expert at managing AI systems in degraded operation scenarios.\n\n\
            CURRENT SYSTEM STATE:\n{}\n\n\
            RECENT ERROR EVENTS:\n{}\n\n\
            Based on the current system state and recent errors, determine the optimal operating mode.\n\
            Consider:\n\
            1. Error patterns and frequency\n\
            2. Resource availability and consumption\n\
            3. Critical vs non-critical functionality\n\
            4. Impact on user experience\n\
            5. Recovery probability in different modes\n\n\
            Respond in the following JSON format:\n{}",
            state_json,
            errors_json,
            OPERATING_MODE_FORMAT
        );
        
        // Call LLM
        let response = self.llm_client.complete_with_json::<OperatingModeDecision>(prompt).await?;
        
        Ok(response)
    }
    
    pub async fn generate_degraded_mode_policies(
        &self, 
        capabilities: &SystemCapabilities
    ) -> Result<Vec<DegradedModePolicy>, AgentError> {
        // Format system capabilities
        let capabilities_json = serde_json::to_string_pretty(capabilities)?;
        
        // Create prompt for policy generation
        let prompt = format!(
            "You are an expert at creating policies for graceful degradation of AI systems.\n\n\
            SYSTEM CAPABILITIES:\n{}\n\n\
            Generate a comprehensive set of degraded operation policies.\n\
            Each policy should specify:\n\
            1. Triggering conditions (e.g., specific error patterns, resource thresholds)\n\
            2. Target operating mode (e.g., normal, reduced, local-only, essential-only)\n\
            3. Feature restrictions in this mode\n\
            4. User communication approach\n\
            5. Recovery conditions to return to normal operation\n\n\
            Respond in the following JSON format:\n{}",
            capabilities_json,
            DEGRADED_MODE_POLICIES_FORMAT
        );
        
        // Call LLM
        let response = self.llm_client.complete_with_json::<Vec<DegradedModePolicy>>(prompt).await?;
        
        Ok(response)
    }
    
    pub async fn customize_operation_for_mode(
        &self,
        task: &Task,
        current_mode: &OperatingMode
    ) -> Result<CustomizedOperation, AgentError> {
        // Format task and operating mode
        let task_text = extract_task_description(task);
        let mode_json = serde_json::to_string_pretty(current_mode)?;
        
        // Create prompt for operation customization
        let prompt = format!(
            "You are an expert at adapting operations to work effectively in degraded modes.\n\n\
            TASK TO EXECUTE:\n{}\n\n\
            CURRENT OPERATING MODE:\n{}\n\n\
            Suggest how this task should be handled in the current operating mode.\n\
            Consider:\n\
            1. Whether the task should be allowed, modified, or rejected\n\
            2. Resource limits to apply during execution\n\
            3. Potential simplifications to increase success probability\n\
            4. Alternative approaches more suitable for current constraints\n\
            5. User communication to set appropriate expectations\n\n\
            Respond in the following JSON format:\n{}",
            task_text,
            mode_json,
            CUSTOMIZED_OPERATION_FORMAT
        );
        
        // Call LLM
        let response = self.llm_client.complete_with_json::<CustomizedOperation>(prompt).await?;
        
        Ok(response)
    }
}
```

5. **Create LLM-Based Recovery Orchestrator**
```rust
// New file: src/bidirectional_agent/llm_resilience/recovery.rs

pub struct LlmRecoveryOrchestrator {
    llm_client: Arc<dyn LlmClient>,
    error_analyzer: Arc<LlmErrorAnalyzer>,
    fallback_generator: Arc<LlmFallbackGenerator>,
}

impl LlmRecoveryOrchestrator {
    pub async fn create_recovery_plan(
        &self,
        failed_task: &Task,
        error: &AgentError,
        context: &ErrorContext
    ) -> Result<RecoveryPlan, AgentError> {
        // First analyze the error in depth
        let error_analysis = self.error_analyzer.analyze_error(error, context).await?;
        
        // Generate potential recovery actions
        let recovery_actions = self.error_analyzer.suggest_recovery_actions(&error_analysis, context).await?;
        
        // Generate fallback options if recovery isn't feasible
        let fallback_options = self.fallback_generator.generate_fallback_options(
            failed_task,
            error,
            &context.available_tools,
            &context.available_agents
        ).await?;
        
        // Create prompt for comprehensive recovery planning
        let prompt = format!(
            "You are an expert at orchestrating recovery from failures in AI systems.\n\n\
            FAILED TASK:\n{}\n\n\
            ERROR ANALYSIS:\n{}\n\n\
            SUGGESTED RECOVERY ACTIONS:\n{}\n\n\
            FALLBACK OPTIONS:\n{}\n\n\
            Create a comprehensive recovery plan that:\n\
            1. Prioritizes recovery actions in optimal sequence\n\
            2. Sets clear criteria for when to abandon recovery and use fallbacks\n\
            3. Specifies fallback selection logic if needed\n\
            4. Includes user communication strategy\n\
            5. Sets maximum retry limits and timeouts\n\n\
            Respond in the following JSON format:\n{}",
            extract_task_description(failed_task),
            serde_json::to_string_pretty(&error_analysis)?,
            serde_json::to_string_pretty(&recovery_actions)?,
            serde_json::to_string_pretty(&fallback_options)?,
            RECOVERY_PLAN_FORMAT
        );
        
        // Call LLM
        let response = self.llm_client.complete_with_json::<RecoveryPlan>(prompt).await?;
        
        Ok(response)
    }
    
    pub async fn execute_recovery_step(
        &self,
        plan: &RecoveryPlan,
        current_step: usize,
        previous_results: &[StepResult]
    ) -> Result<StepExecution, AgentError> {
        // Ensure step exists
        if current_step >= plan.steps.len() {
            return Err(AgentError::InvalidRecoveryStep(current_step));
        }
        
        // Get the current step
        let step = &plan.steps[current_step];
        
        // Format previous results
        let results_json = serde_json::to_string_pretty(previous_results)?;
        
        // Create prompt for step execution planning
        let prompt = format!(
            "You are an expert at executing recovery steps to restore AI system functionality.\n\n\
            CURRENT RECOVERY STEP:\n{}\n\n\
            PREVIOUS STEP RESULTS:\n{}\n\n\
            Determine the specific actions to take for this recovery step.\n\
            Consider:\n\
            1. Previous step outcomes and their implications\n\
            2. Precise API calls or operations needed\n\
            3. Parameters for each operation\n\
            4. Success criteria to evaluate after execution\n\
            5. Timeout and retry limits for this specific step\n\n\
            Respond in the following JSON format:\n{}",
            serde_json::to_string_pretty(step)?,
            results_json,
            STEP_EXECUTION_FORMAT
        );
        
        // Call LLM
        let response = self.llm_client.complete_with_json::<StepExecution>(prompt).await?;
        
        Ok(response)
    }
    
    pub async fn evaluate_recovery_progress(
        &self,
        plan: &RecoveryPlan,
        executed_steps: &[StepResult]
    ) -> Result<RecoveryEvaluation, AgentError> {
        // Format plan and results
        let plan_json = serde_json::to_string_pretty(plan)?;
        let steps_json = serde_json::to_string_pretty(executed_steps)?;
        
        // Create prompt for progress evaluation
        let prompt = format!(
            "You are an expert at evaluating recovery progress in AI systems.\n\n\
            RECOVERY PLAN:\n{}\n\n\
            EXECUTED STEPS AND RESULTS:\n{}\n\n\
            Evaluate the current recovery progress.\n\
            Consider:\n\
            1. Success of executed steps relative to expectations\n\
            2. Overall trajectory toward recovery\n\
            3. Whether to continue with next plan steps or abort\n\
            4. Estimated remaining work to complete recovery\n\
            5. Recommended adjustments to remaining plan steps\n\n\
            Respond in the following JSON format:\n{}",
            plan_json,
            steps_json,
            RECOVERY_EVALUATION_FORMAT
        );
        
        // Call LLM
        let response = self.llm_client.complete_with_json::<RecoveryEvaluation>(prompt).await?;
        
        Ok(response)
    }
}
```

## Integration and Implementation Strategy

To effectively implement these LLM-powered features, follow this integration strategy:

1. **Create a Unified LLM Client Interface**
```rust
// New file: src/bidirectional_agent/llm_core/mod.rs

#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Simple text completion
    async fn complete(&self, prompt: String) -> Result<String, AgentError>;
    
    /// Completion with JSON parsing
    async fn complete_with_json<T: DeserializeOwned>(&self, prompt: String) -> Result<T, AgentError> {
        let response = self.complete(prompt).await?;
        serde_json::from_str(&response)
            .map_err(|e| AgentError::LlmResponseParsingError(format!("Failed to parse JSON: {}", e)))
    }
    
    /// Streaming completion for long responses
    async fn complete_streaming(&self, prompt: String) -> Result<impl Stream<Item = Result<String, AgentError>>, AgentError>;
    
    /// Function calling capability
    async fn complete_with_function_call<T: DeserializeOwned>(
        &self, 
        prompt: String,
        functions: &[FunctionDefinition]
    ) -> Result<FunctionCallResponse<T>, AgentError>;
}

/// Implementation for Claude
pub struct ClaudeClient {
    api_key: String,
    model: String,
    client: reqwest::Client,
    max_tokens: usize,
}

#[async_trait]
impl LlmClient for ClaudeClient {
    async fn complete(&self, prompt: String) -> Result<String, AgentError> {
        // Implementation using Claude API
        let request = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });
        
        let response = self.client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::LlmApiError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(AgentError::LlmApiError(format!(
                "Claude API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }
        
        let json = response.json::<serde_json::Value>().await
            .map_err(|e| AgentError::LlmApiError(format!("Failed to parse response: {}", e)))?;
        
        // Extract content from Claude response format
        json["content"][0]["text"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AgentError::LlmApiError("Invalid response format".to_string()))
    }
    
    // Implement other methods...
}
```

2. **Create a Central LLM Manager**
```rust
// New file: src/bidirectional_agent/llm_core/manager.rs

pub struct LlmManager {
    primary_client: Arc<dyn LlmClient>,
    fallback_client: Option<Arc<dyn LlmClient>>,
    prompt_templates: HashMap<String, String>,
}

impl LlmManager {
    pub fn new(
        primary_client: Arc<dyn LlmClient>,
        fallback_client: Option<Arc<dyn LlmClient>>,
    ) -> Self {
        let mut manager = Self {
            primary_client,
            fallback_client,
            prompt_templates: HashMap::new(),
        };
        
        // Load default prompt templates
        manager.load_default_templates();
        
        manager
    }
    
    pub async fn execute_with_template<T: DeserializeOwned>(
        &self,
        template_name: &str,
        variables: HashMap<String, String>
    ) -> Result<T, AgentError> {
        let template = self.prompt_templates.get(template_name)
            .ok_or_else(|| AgentError::MissingTemplate(template_name.to_string()))?;
        
        // Replace variables in template
        let prompt = self.fill_template(template, variables)?;
        
        // Try primary client
        match self.primary_client.complete_with_json::<T>(prompt.clone()).await {
            Ok(result) => Ok(result),
            Err(e) => {
                if let Some(fallback) = &self.fallback_client {
                    // Log the primary failure
                    log::warn!("Primary LLM client failed: {}. Trying fallback.", e);
                    
                    // Try fallback
                    fallback.complete_with_json::<T>(prompt).await
                } else {
                    // No fallback, propagate error
                    Err(e)
                }
            }
        }
    }
    
    fn fill_template(&self, template: &str, variables: HashMap<String, String>) -> Result<String, AgentError> {
        let mut result = template.to_string();
        
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, &value);
        }
        
        // Check if any placeholders remain unfilled
        if result.contains("{{") && result.contains("}}") {
            return Err(AgentError::TemplateFillError(
                "Not all template variables were filled".to_string()
            ));
        }
        
        Ok(result)
    }
    
    fn load_default_templates(&mut self) {
        // Add templates for common operations
        self.prompt_templates.insert(
            "routing_decision".to_string(),
            include_str!("../templates/routing_decision.txt").to_string()
        );
        
        self.prompt_templates.insert(
            "task_decomposition".to_string(),
            include_str!("../templates/task_decomposition.txt").to_string()
        );
        
        // Additional templates...
    }
}
```

3. **Update BidirectionalAgent struct**
```rust
// src/bidirectional_agent/mod.rs (update)

pub struct BidirectionalAgent {
    // Existing fields...
    
    // Add LLM components
    pub llm_manager: Arc<LlmManager>,
    pub llm_routing: Arc<LlmTaskRouter>,
    pub llm_delegation: Arc<LlmDelegationManager>,
    pub llm_synthesis: Arc<LlmSynthesizer>,
    pub llm_tool_discovery: Arc<LlmToolDiscovery>,
    pub llm_resilience: Arc<LlmRecoveryOrchestrator>,
}

impl BidirectionalAgent {
    pub async fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        // Existing initialization...
        
        // Create LLM clients based on config
        let primary_llm = Arc::new(ClaudeClient::new(
            config.llm.api_key.clone(),
            config.llm.model.clone(),
            config.llm.max_tokens,
        ));
        
        // Create fallback client if configured
        let fallback_llm = if let Some(fallback_config) = &config.llm.fallback {
            Some(Arc::new(ClaudeClient::new(
                fallback_config.api_key.clone(),
                fallback_config.model.clone(),
                fallback_config.max_tokens,
            )) as Arc<dyn LlmClient>)
        } else {
            None
        };
        
        // Create LLM manager
        let llm_manager = Arc::new(LlmManager::new(
            primary_llm,
            fallback_llm,
        ));
        
        // Initialize LLM components
        let llm_routing = Arc::new(LlmTaskRouter::new(
            llm_manager.clone(),
            agent_registry.clone(),
            tool_executor.clone(),
        ));
        
        let llm_delegation = Arc::new(LlmDelegationManager::new(
            llm_manager.clone(),
            agent_registry.clone(),
            client_manager.clone(),
        ));
        
        let llm_synthesis = Arc::new(LlmSynthesizer::new(
            llm_manager.clone(),
        ));
        
        let llm_tool_discovery = Arc::new(LlmToolDiscovery::new(
            llm_manager.clone(),
            agent_directory.clone(),
        ));
        
        let llm_resilience = Arc::new(LlmRecoveryOrchestrator::new(
            llm_manager.clone(),
            Arc::new(LlmErrorAnalyzer::new(llm_manager.clone())),
            Arc::new(LlmFallbackGenerator::new(llm_manager.clone())),
        ));
        
        Ok(Self {
            // Existing fields...
            llm_manager,
            llm_routing,
            llm_delegation,
            llm_synthesis,
            llm_tool_discovery,
            llm_resilience,
        })
    }
    
    // Add method for integrated task handling with LLM support
    pub async fn handle_task_with_llm(&self, task: &mut Task) -> Result<(), AgentError> {
        log::info!("Handling task {} with LLM intelligence", task.id);
        
        // 1. First assess risks and prevent potential failures
        let system_state = self.get_current_system_state().await?;
        let risk_assessment = self.llm_resilience
            .predictive_detector
            .assess_task_risks(task, &system_state)
            .await?;
        
        // Handle high-risk tasks with preventive measures
        if risk_assessment.overall_risk_level > 0.7 {
            log::warn!("Task {} has high risk level: {}", task.id, risk_assessment.overall_risk_level);
            
            let preventive_measures = self.llm_resilience
                .predictive_detector
                .suggest_preventive_measures(&risk_assessment)
                .await?;
                
            self.apply_preventive_measures(task, &preventive_measures).await?;
        }
        
        // 2. Make routing decision with LLM
        let routing_context = self.build_routing_context(task).await?;
        let routing_request = EnhancedRoutingRequest {
            task: task.clone(),
            context: routing_context,
        };
        
        let routing_response = self.llm_routing
            .make_routing_decision(routing_request)
            .await?;
            
        log::info!("LLM routing decision for task {}: {:?}", task.id, routing_response.decision);
        
        // 3. Execute based on routing decision
        match routing_response.decision {
            RoutingDecision::ExecuteLocally { tool_id } => {
                self.tool_executor.execute_task_locally(task, &[tool_id]).await?;
            },
            
            RoutingDecision::DelegateToAgent { agent_id } => {
                // Check if delegation is working in the current mode
                let current_mode = self.get_current_operating_mode().await;
                if current_mode != OperatingMode::Normal && current_mode != OperatingMode::ReducedConcurrency {
                    // Switch to fallback option in degraded mode
                    if let Some(fallback) = routing_response.fallback_options.first() {
                        log::warn!("Using fallback option due to degraded mode: {:?}", fallback);
                        self.execute_fallback_decision(task, fallback).await?;
                    } else {
                        return Err(AgentError::OperatingModeRestriction(
                            "Delegation not available in current operating mode".to_string()
                        ));
                    }
                } else {
                    // Normal delegation
                    let delegation_plan = self.llm_delegation
                        .plan_task_delegation(task)
                        .await?;
                        
                    let delegation_results = self.llm_delegation
                        .execute_delegation_plan(&delegation_plan, task)
                        .await?;
                        
                    // Synthesize results if needed
                    if delegation_results.len() > 1 {
                        let synthesis = self.llm_synthesis
                            .synthesize(task, &delegation_results)
                            .await?;
                            
                        // Apply synthesis result to task
                        self.apply_synthesis_to_task(task, &synthesis).await?;
                    } else if let Some(result) = delegation_results.first() {
                        // Single result, copy directly
                        self.copy_result_to_task(task, result).await?;
                    }
                }
            },
            
            RoutingDecision::DecomposeTask { subtasks } => {
                // Execute decomposed task workflow
                let subtask_results = self.execute_decomposed_workflow(task, &subtasks).await?;
                
                // Synthesize results
                let synthesis = self.llm_synthesis
                    .synthesize(task, &subtask_results)
                    .await?;
                    
                // Apply synthesis result to task
                self.apply_synthesis_to_task(task, &synthesis).await?;
            },
            
            RoutingDecision::CannotHandle { reason } => {
                // Update task with failure
                task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(chrono::Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Unable to handle task: {}", reason),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
            }
        }
        
        Ok(())
    }
    
    // Implementation of other methods...
}
```

4. **Phased Implementation Plan**

I recommend implementing these features in the following order:

**Phase 1: Core LLM Infrastructure**
1. Implement the LLM client interface and concrete implementations
2. Create the LLM manager with template support
3. Enhance existing LLM routing logic to use the new infrastructure

**Phase 2: Resilience Features**
1. Implement error analysis and recovery suggestions
2. Add predictive failure detection
3. Create the fallback generator
4. Implement degraded mode intelligence

**Phase 3: Enhanced Task Management**
1. Implement LLM-based task decomposition
2. Create task delegation planning
3. Build the execution orchestrator
4. Add dependency tracking

**Phase 4: Result Synthesis**
1. Implement the basic LLM synthesizer
2. Add conflict detection and resolution
3. Create specialized synthesizers for code and data
4. Integrate quality evaluation

**Phase 5: Tool Discovery and Integration**
1. Implement LLM-based capability extraction
2. Create tool discovery and matching
3. Build parameter mapping intelligence
4. Add cross-agent tool call translation

## Additional Recommendations

1. **Prompt Engineering**
   
   Store prompts in separate template files rather than hardcoding them:
   
   ```
   /src/bidirectional_agent/templates/
     ├── routing_decision.txt
     ├── task_decomposition.txt
     ├── conflict_resolution.txt
     ├── error_analysis.txt
     └── ...
   ```
   
   This approach allows for:
   - Easier prompt optimization and testing
   - Prompt versioning
   - Centralized management of system prompts

2. **Structured Output Formats**
   
   Define clear JSON schemas for all LLM outputs:
   
   ```rust
   // src/bidirectional_agent/llm_core/schemas.rs
   
   pub const ROUTING_RESPONSE_FORMAT: &str = r#"{
     "decision": {
       "type": "string", // "ExecuteLocally", "DelegateToAgent", "DecomposeTask", or "CannotHandle"
       "tool_id": "string", // Required if type is "ExecuteLocally"
       "agent_id": "string", // Required if type is "DelegateToAgent"
       "subtasks": [ // Required if type is "DecomposeTask"
         {
           "description": "string",
           "required_capabilities": ["string"]
         }
       ],
       "reason": "string" // Required if type is "CannotHandle"
     },
     "explanation": "string",
     "confidence": 0.95, // 0.0 to 1.0
     "fallback_options": [
       // Additional routing decisions in same format as primary, in order of preference
     ]
   }"#;
   ```

3. **Testing Strategy**
   
   Create specific test cases for LLM-based decisions:
   
   ```rust
   // src/bidirectional_agent/tests/llm_routing_tests.rs
   
   #[tokio::test]
   async fn test_llm_routing_simple_task() {
       // Setup mock LLM client that returns predefined responses
       let mock_llm = Arc::new(MockLlmClient::new(vec![
           (
               "You are an expert at routing intelligence".to_string(), // Partial prompt match
               r#"{
                   "decision": {
                       "type": "ExecuteLocally",
                       "tool_id": "shell"
                   },
                   "explanation": "This is a simple task that can be handled locally",
                   "confidence": 0.95,
                   "fallback_options": []
               }"#.to_string()
           )
       ]));
       
       // Create router with mock
       let router = LlmTaskRouter::new(
           mock_llm,
           create_test_registry().await,
           create_test_tool_executor().await
       );
       
       // Create test task
       let task = create_simple_test_task("list the files in the current directory");
       
       // Call the router
       let result = router.make_routing_decision(task).await;
       
       // Assert decision
       assert!(result.is_ok());
       let decision = result.unwrap();
       assert!(matches!(decision.decision, RoutingDecision::ExecuteLocally { tool_id } if tool_id == "shell"));
   }
   ```

4. **Monitoring and Debugging**
   
   Add comprehensive logging of LLM interactions:
   
   ```rust
   impl LlmClient for ClaudeClient {
       async fn complete(&self, prompt: String) -> Result<String, AgentError> {
           // Log prompt (redact sensitive info)
           log::debug!("LLM Prompt (truncated): {}", truncate_and_redact(&prompt, 200));
           
           let start = std::time::Instant::now();
           let result = self.call_claude_api(prompt).await;
           let duration = start.elapsed();
           
           match &result {
               Ok(response) => {
                   log::debug!("LLM Response (truncated): {}", truncate_and_redact(response, 200));
                   log::info!("LLM call completed in {:?}", duration);
               },
               Err(e) => {
                   log::error!("LLM call failed after {:?}: {:?}", duration, e);
               }
           }
           
           result
       }
   }
   ```

By implementing these LLM-powered features, the A2A Test Suite will gain sophisticated intelligence for handling complex, fuzzy decisions across all aspects of agent operations, from task routing and delegation to error recovery and synthesis, while maintaining compatibility with the core A2A protocol.

## Critique of Implementation Plan

While the proposed implementation plan is comprehensive and well-structured, there are several areas that could benefit from refinement:

1. **Test-Driven Development Alignment**: The current plan lacks explicit test creation steps before implementation, which contradicts the project's TDD principles outlined in CLAUDE.md. Each feature should begin with test creation before any implementation code.

2. **Error Handling Complexity**: The error handling approaches are highly sophisticated but may be overengineered for initial implementation. A phased approach starting with basic error cases would be more practical, with advanced recovery mechanisms added later.

3. **Resource Efficiency**: Many of the proposed LLM prompts are verbose and would result in high token consumption. The design should include prompt optimization strategies to reduce costs while maintaining effectiveness.

4. **Interface Stability**: The frequent introduction of new struct types and JSON schemas could lead to interface instability. A more gradual evolution of the API would be preferable, with emphasis on backward compatibility.

5. **Integration Testing Gap**: While unit tests are mentioned, there's insufficient detail on integration testing across components, which is critical for a system where LLM decisions impact multiple subsystems.

6. **Performance Considerations**: The plan doesn't adequately address potential latency issues from sequential LLM calls. Caching strategies and parallel processing optimizations should be more explicitly detailed.

7. **Versioning Strategy**: Missing is a clear approach to versioning prompt templates and LLM response formats, which will be essential as models and capabilities evolve.

Implementing LLM-powered capabilities is a promising direction, but the implementation approach should be more iterative, with stronger emphasis on testing, performance optimization, and interface stability.



          
Critique of docs/bidirectional_agent_remaining_tasks.md      
( “LLM-Driven Future Implementation Road-map” )              

─────────────────────────────────────────────────────────────
───────────                                                  

 1 HIGH-LEVEL ASSESSMENT                                     
   ──────────────────────────────────────────────────────────
   ────────────── • Ambition vs. Reality                     
   The roadmap is visionary, but it assumes a green-field    
   agent.                                                    
   The current code base already implements:                 
   – AgentDirectory (sqlite + health checks)                 
   – AgentRegistry / ClientManager caches                    
   – LlmTaskRouter (LOCAL / REMOTE / REJECT)                 
   – ToolExecutor with pluggable remote tools                
   – Result streaming / TaskManager in REPL                  

Many sections of the roadmap re-implement these layers       
instead of extending them, risking duplicated logic.         

• Breadth                                                    
It spans error resilience, delegation planning, synthesis,   
discovery, scheduling, learning, quality evaluation and      
degraded-mode governance. Attempting all in one sprint will  
stall progress; we need staging.                             

• Dependencies                                               
Everything hangs on a “generic LlmClient” abstraction that   
does not yet exist, plus heavy prompt engineering, costly    
tokens, and new schema contracts that are not versioned.     

• Testing Strategy                                           
Doctrine in CLAUDE.md says TDD first; the roadmap mentions   
tests only in passing.  Several proposed APIs have no        
acceptance criteria.                                         

─────────────────────────────────────────────────────────────
─────────── 2.  STRENGTHS OF THE PLAN                        
─────────────────────────────────────────────────────────────
─────────── ✓ Unified LLM interface with JSON parsing helpers
– highly valuable.                                           
✓ Prompt templates stored in files – maintainable.           
✓ Clear JSON output contracts – aids validation & property   
tests.                                                       
✓ Phased “Core → Resilience → Delegation …” idea (needs      
sharper cuts).                                               

─────────────────────────────────────────────────────────────
─────────── 3.  KEY GAPS / RISKS                             
─────────────────────────────────────────────────────────────
───────────                                                  

 1 Overlapping Components                                    
   • RoutingAgent vs. LlmTaskRouter vs. future               
   EnhancedRoutingAgent.                                     
   → Danger of three routers with divergent logic.           
 2 Un-scoped Data Stores                                     
   • New history / learning managers imply a DB schema but   
   none is specified (migrations? retention?).               
 3 Feature-Flag Explosion                                    
   • We already rely on bidir-core / bidir-local-exec /      
   bidir-delegate. Roadmap adds synthesis, resilience,       
   tooling flags: compile matrix will blow up if not planned.
 4 Latency & Token Cost                                      
   • Many chained LLM calls (error analysis → recovery plan →
   step execution) could 3-4× current latency & cost.        
 5 Prompt Versioning                                         
   • No concrete strategy for prompt evolution, rollback, A/B
   testing.                                                  
 6 Conflicting Config Roots                                  
   • BidirectionalAgentConfig vs. multiple *_config structs  
   in the roadmap – risk of drift.                           


─────────────────────────────────────────────────────────────
             ─────────── 4.  ACTIONABLE FEEDBACK             
─────────────────────────────────────────────────────────────
        ─────────── PHASE 0 – HOUSEKEEPING (1–2 days)        

A.  Central LlmClient trait                                  
• Extract a minimal trait now (complete, complete_json).     
• Provide a mock implementation for tests.                   
• Refactor LlmTaskRouter & interpret_input to use it.        

B.  Prompt Template Loader                                   
• Put templates under src/bidirectional_agent/prompts/*.txt. 
• Provide helper render(name, vars) -> String.               
• Add unit test that all templates compile without “{{“      
residues.                                                    


           PHASE 1 – ROUTING & TOOLING (3–5 days)            

 1 Merge RoutingAgent & LlmTaskRouter                        
   • Keep decide, should_decompose, decompose_task.          
   • Expose a single async trait RoutingAgentTrait (already  
   exists) and register one instance in BidirectionalAgent.  
 2 Remote-Tool Execution Path                                
   • Finish stub in ClientManager::send_task (Slice 1        
   comment).                                                 
   • Expose REPL commands list remote tools, execute remote  
   tool.                                                     
   • Write integration test: remote echo tool via mock agent.
 3 Config Consolidation                                      
   • Extend BidirectionalAgentConfig with llm subsection     
   rather than new LlmRoutingConfig nested structures.       


            PHASE 2 – ERROR & RESILIENCE (1 week)            

 1 LlmErrorAnalyzer + RecoveryAction list only (no           
   orchestrator yet).                                        
   • Wire to existing ReplError translation pathway.         
   • Add property test: any ClientError maps to valid        
   RecoveryAction.                                           
 2 PredictiveFailureDetector (risk scoring) – read-only      
   reporting; don’t gate execution yet.                      


          PHASE 3 – DELEGATION & SYNTHESIS (later)           

• Requires bidir-delegate feature; blocked until Phase 1     
stabilises.                                                  
• Prototype on synthetic tasks with special echo tools.      
• Add slice-by-slice integration (decompose → delegate →     
synthesize).                                                 

─────────────────────────────────────────────────────────────
─────────── 5.  LOW-LEVEL GUIDANCE                           
─────────────────────────────────────────────────────────────
─────────── •  Concurrency                                   
– Use tokio::task::spawn_blocking for heavy prompt rendering 
if you load template files each call.                        

•  Database / Persistence                                    
– Put new tables (routing_history, error_events) behind a    
feature flag and sqlx migration; test with in-memory :memory:
for CI.                                                      

•  Feature Flags                                             
– Group them:                                                
llm-routing, llm-resilience, llm-delegate, llm-synthesis.    
– Avoid cross-feature cycles.                                

•  Testing Pattern                                           
– For each new module:                                       
1. unit test with MockLlmClient,                             
2. property test (quick-check style) ensuring JSON produced  
validates via validator::validate_json,                      
3. integration test behind the feature flag.                 

•  Logging & Metrics                                         
– Wrap every LLM call with timing + token count (Anthropic   
header anthropic-version gives usage).                       
– Emit Prometheus metrics (llm_request_total,                
tokens_sent_total) so we see cost regressions.               

•  Prompt Token Budget                                       
– Add a compile-time constant MAX_PROMPT_TOKENS = 4096 and   
assert in tests that rendered prompt length < 12 kB.         

─────────────────────────────────────────────────────────────
─────────── 6.  SUMMARY                                      
─────────────────────────────────────────────────────────────
─────────── The roadmap is solid but too broad for the       
current maturity level of the bidirectional agent.  Adopt an 
incremental plan:                                            

 1 Finish generic LLM client + unify existing routing code.  
 2 Expose remote-tool execution end-to-end.                  
 3 Layer basic error-analysis & recovery suggestions.        
 4 Only then tackle decomposition, synthesis, and            
   degraded-mode logic.               



  The feedback is comprehensive and provides specific, actionable guidance for implementing
  the LLM-driven features. Here's my response addressing the key points:

  Response to Additional Feedback

  Your detailed critique offers excellent structure and practical steps for implementing LLM
  capabilities in the A2A Test Suite. I completely agree with your assessment that the
  original roadmap, while ambitious, needs a more incremental approach aligned with the
  existing codebase.

  Strengths of Your Approach

  1. Recognizing Current Implementation: Your feedback correctly identifies that we already
  have working components (AgentDirectory, AgentRegistry, LlmTaskRouter, etc.) that should be
  extended rather than reimplemented.
  2. Practical Phasing: The suggested four-phase approach (Housekeeping → Routing & Tooling →
  Error & Resilience → Delegation & Synthesis) provides a clear, incremental path that aligns
  with our development resources.
  3. Concrete Technical Guidance: The low-level suggestions on concurrency, database design,
  feature flags, testing patterns, and performance metrics are extremely valuable.

  Implementation Strategy

  I'll follow your recommended phased approach:

  Phase 0 (Housekeeping):

  - Extract a minimal LlmClient trait with core methods
  - Implement the prompt template system with file-based templates
  - Add tests to verify template rendering

  Phase 1 (Routing & Tooling):

  - Merge and consolidate the routing components to avoid duplication
  - Complete the remote-tool execution path
  - Extend existing configuration structures rather than creating parallel hierarchies

  Phase 2 (Error & Resilience):

  - Implement basic error analysis and recovery suggestions
  - Add risk scoring that provides information without blocking execution
  - Add property testing for error mapping

  Phase 3 (Delegation & Synthesis):

  - Implement only after previous phases are stable
  - Start with controlled test scenarios using specialized echo tools
  - Build integration incrementally

  This approach addresses the fundamental issues you've identified while still moving toward
  the enhanced LLM capabilities we need.

  Additional Considerations

  - Testing Priority: I'll ensure we maintain our TDD approach, creating tests before
  implementation
  - Token Budget Control: The suggestion to enforce prompt token limits is excellent for
  controlling costs
  - Feature Flag Organization: Grouping related flags will keep the compilation matrix
  manageable
  - Performance Monitoring: Adding Prometheus metrics will help track token usage and latency
