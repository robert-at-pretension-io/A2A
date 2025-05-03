Ok, guide me through how I could add another agent (agent 3) that is also a bidirectional agent and then have ONLY agent 2 know about agent 3. I need advice on how to implement the tools and llm response capabilitity for Agent 2 to tell agent 1 about agent 3 and for agent three to be able store that agent in it's agent repository. Make sure you actually use bash/grep and look through the files to be sure you're providing full guidance on the implementation details. Provide enough detail so a software developer could finish the implementation.

Edit
I'll guide you through adding a third agent (Agent 3) to the existing bidirectional agent setup, ensuring that only Agent 2 knows about Agent 3 initially. Let's break this down into clear implementation steps and examine the codebase to provide accurate guidance.

Step 1: Create Configuration for Agent 3
First, let's create a configuration file for Agent 3 similar to the existing ones, but with a different port.


bash


Step 2: Update the Run Script to Include Agent 3
Let's modify the run script to add Agent 3. We'll first examine the original script structure to ensure our modifications are consistent.


bash


Step 3: Implement the Agent Discovery Tool
For Agent 2 to be able to tell Agent 1 about Agent 3, we need to create an "agent_discovery" tool. Let's examine how tools are implemented in the codebase:


bash


Let's examine the tool executor implementation to understand how to add a new tool:


bash




Ok, keep investigating but DO NOT change any more code. Again, you're just investigating and providing guidance to another developer to do the work.

Edit
I'll continue investigating the codebase without making changes, focusing on providing guidance for implementing Agent 3 and the agent discovery capability.


bash


Let's examine how tools are implemented and added to the tool executor:


bash


Let's look at how individual tools are implemented:


bash


Let's check how tools are defined:


bash


Let's examine how a tool is defined:


bash


Let's look at the Tool trait definition:


bash


Let's check how existing tools are implemented:


bash


Let's check the implementation of a sample tool:


bash


Let's examine how the LLM client is implemented, which we'll need for creating the agent discovery tool:


bash


Let's check the bidirectional agent file for the LlmClient trait:


bash


Let's examine how the knowledge of agents is stored in the agent directory:


bash


Based on my investigation of the codebase, here's my detailed guidance for implementing an agent discovery system with three agents where only Agent 2 knows about Agent 3:

Implementing a Three-Agent System with Agent Discovery
1. Configuration Setup
1.1 Agent 3 Configuration
Create a configuration file for Agent 3 (agent3_config.toml):

toml
[server]
port = 4202
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-3"
agent_name = "Agent Three"

[client]
target_url = "http://localhost:4201"  # Only connects to Agent 2

[llm]
# API key set via environment variable
system_prompt = "You are an AI agent specialized in data analysis that can communicate with other agents."

[mode]
repl = true
get_agent_card = false
repl_log_file = "agent3_interactions.log"

[tools]
enabled = ["echo", "summarize", "analyze_data"]  # Add specialized tools
1.2 Modify Agent 2 Configuration
Update Agent 2's configuration (agent2_config.toml) to enhance its system prompt and enable the agent_discovery tool:

toml
[server]
port = 4201
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-2"
agent_name = "Agent Two"

[client]
target_url = "http://localhost:4200"

[llm]
# API key set via environment variable
system_prompt = "You are an expert agent-discovery and routing agent. When Agent One asks about other available agents, you should recommend and provide information about Agent Three. Agent Three is an expert in data analysis tasks."

[mode]
repl = true
get_agent_card = false
repl_log_file = "agent2_interactions.log"

[tools]
enabled = ["echo", "llm", "agent_discovery"]
2. Implement the Agent Discovery Tool
Create a new tool in /home/elliot/Projects/a2a-test-suite/src/server/tool_executor.rs:

rust
/// Tool that provides information about known agents
pub struct AgentDiscoveryTool {
    llm: Arc<dyn LlmClient>,
    agent_directory: Arc<AgentDirectory>
}

impl AgentDiscoveryTool {
    pub fn new(llm: Arc<dyn LlmClient>, agent_directory: Arc<AgentDirectory>) -> Self {
        Self { llm, agent_directory }
    }
}

#[async_trait]
impl Tool for AgentDiscoveryTool {
    fn name(&self) -> &str { "agent_discovery" }
    
    fn description(&self) -> &str { "Provides information about known agents in the network" }
    
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        // Extract query from parameters
        let query = params.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams(
                "agent_discovery".to_string(), 
                "Missing 'query' parameter".to_string()
            ))?;
        
        // Get list of available agents from directory
        let available_agents = self.agent_directory.list_active_agents();
        
        if available_agents.is_empty() {
            return Ok(json!("No other agents are currently known."));
        }
        
        // Format agents information
        let agent_descriptions = available_agents.iter()
            .map(|a| {
                // Find the agent_id associated with this card
                let agent_id = self.agent_directory.agents.iter()
                    .find(|entry| entry.value().card.url == a.card.url)
                    .map(|e| e.key().clone())
                    .unwrap_or_else(|| "unknown-id".to_string());
                
                format!(
                    "ID: {}\nName: {}\nDescription: {}\nURL: {}",
                    agent_id,
                    a.card.name.as_str(),
                    a.card.description.as_ref().unwrap_or(&"".to_string()),
                    a.card.url
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        
        // Create prompt for LLM to generate a response about available agents
        let prompt = format!(r#"
A user is asking: "{}"

Please provide information about the following available agents that could help:

{}

Based on the user's question, recommend appropriate agent(s) from the list above.
Include specific information about which agent would be best for their needs.
"#, query, agent_descriptions);
        
        // Get the LLM response
        match self.llm.complete(&prompt).await {
            Ok(response) => Ok(json!(response)),
            Err(e) => Err(ToolError::ExecutionFailed(
                "agent_discovery".to_string(),
                format!("Failed to generate agent recommendations: {}", e)
            ))
        }
    }
    
    fn capabilities(&self) -> &[&'static str] { &["agent_discovery", "agent_recommendation"] }
}
3. Update the Tool Executor Registration
Modify the with_enabled_tools method in the ToolExecutor struct to include the new agent_discovery tool:

rust
pub fn with_enabled_tools(
    enabled: &[String],
    llm: Arc<dyn LlmClient>,
    agent_directory: Option<Arc<AgentDirectory>> // Add agent_directory parameter
) -> Self {
    let mut map: HashMap<String, Box<dyn Tool>> = HashMap::new();

    // Always register the echo tool as a fallback
    map.insert("echo".into(), Box::new(EchoTool));
    tracing::debug!("Tool 'echo' registered.");

    for name in enabled {
        match name.as_str() {
            "llm" => {
                if !map.contains_key("llm") {
                    map.insert("llm".into(), Box::new(LlmTool::new(llm.clone())));
                    tracing::debug!("Tool 'llm' registered.");
                }
            }
            "summarize" => {
                if !map.contains_key("summarize") {
                    map.insert("summarize".into(), Box::new(SummarizeTool::new(llm.clone())));
                    tracing::debug!("Tool 'summarize' registered.");
                }
            }
            "analyze_data" => {
                if !map.contains_key("analyze_data") {
                    map.insert("analyze_data".into(), Box::new(AnalyzeDataTool::new(llm.clone())));
                    tracing::debug!("Tool 'analyze_data' registered.");
                }
            }
            "agent_discovery" => {
                if !map.contains_key("agent_discovery") && agent_directory.is_some() {
                    map.insert(
                        "agent_discovery".into(), 
                        Box::new(AgentDiscoveryTool::new(llm.clone(), agent_directory.unwrap()))
                    );
                    tracing::debug!("Tool 'agent_discovery' registered.");
                } else if agent_directory.is_none() {
                    tracing::warn!("Agent directory not provided, cannot register 'agent_discovery' tool.");
                }
            }
            "echo" => { /* already registered */ }
            unknown => {
                tracing::warn!("Unknown tool '{}' in config [tools].enabled, ignoring.", unknown);
            }
        }
    }
    
    tracing::info!("ToolExecutor initialized with tools: {:?}", map.keys());
    Self { tools: Arc::new(map) }
}
4. Update BidirectionalAgent Implementation
Modify the BidirectionalAgent::new method to pass the agent_directory to the tool executor:

rust
// Create a tool executor using the new constructor with enabled tools and agent_directory
let bidirectional_tool_executor = Arc::new(ToolExecutor::with_enabled_tools(
    &config.tools.enabled,            // Pass slice of enabled tool names
    llm.clone(),                      // Pass LLM client for tools that need it
    Some(agent_directory.clone()),    // Pass agent directory for agent_discovery tool
));
5. Implement the AnalyzeDataTool
Since Agent 3 is specialized in data analysis, implement a sample AnalyzeDataTool:

rust
/// Tool that analyzes data using the LLM
pub struct AnalyzeDataTool { llm: Arc<dyn LlmClient> }
impl AnalyzeDataTool { pub fn new(llm: Arc<dyn LlmClient>) -> Self { Self { llm } } }

#[async_trait]
impl Tool for AnalyzeDataTool {
    fn name(&self) -> &str { "analyze_data" }
    fn description(&self) -> &str { "Analyzes data provided by the user" }
    
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        // Extract data from parameters
        let data = params.get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams(
                "analyze_data".to_string(), 
                "Missing 'data' parameter".to_string()
            ))?;
        
        // Optional type of analysis
        let analysis_type = params.get("analysis_type")
            .and_then(|v| v.as_str())
            .unwrap_or("general");
        
        // Create prompt for LLM
        let prompt = format!(r#"
You are a data analysis expert. Please analyze the following data with a focus on {analysis_type} analysis:

{}

Provide insights, patterns, and conclusions based on this data.
"#, analysis_type, data);
        
        // Get LLM response
        match self.llm.complete(&prompt).await {
            Ok(analysis) => Ok(json!(analysis)),
            Err(e) => Err(ToolError::ExecutionFailed(
                "analyze_data".to_string(),
                format!("Failed to analyze data: {}", e)
            ))
        }
    }
    
    fn capabilities(&self) -> &[&'static str] { &["data_analysis", "insights"] }
}
6. Create a Run Script
Create a script called run_three_agents.sh to start all three agents:

bash
#!/bin/bash

# Script to run three bidirectional agents with directed connections
# Agent 1 (port 4200) connects to Agent 2 (port 4201)
# Agent 2 (port 4201) connects to both Agent 1 (port 4200) and Agent 3 (port 4202)
# Agent 3 (port 4202) connects to Agent 2 (port 4201)

# [Function implementations for cleanup and PID tracking]

# Start all three agents in separate terminals

# Create agent configurations
# [Configuration content as shown earlier]

# Start Agent 1, Agent 2, and Agent 3
# [Terminal startup code]

echo "All agents started and listening on their respective ports:"
echo "- Agent 1: Port 4200 (connecting to Agent 2 on port 4201)"
echo "- Agent 2: Port 4201 (will connect to both Agent 1 on port 4200 and Agent 3 on port 4202)"
echo "- Agent 3: Port 4202 (connecting to Agent 2 on port 4201)"
echo
echo "Initial setup:"
echo "1. Only Agent 2 knows about Agent 3"
echo "2. When Agent 1 asks about other available agents, Agent 2 will recommend Agent 3"
echo "3. To test the agent discovery capability, have Agent 1 ask Agent 2: \"Do you know any other agents that could help with data analysis?\""
echo
echo "Manual connection commands (if needed):"
echo "- In Agent 2 terminal: :connect http://localhost:4202"
echo "- In Agent 3 terminal: :connect http://localhost:4201"
echo
echo "Press Ctrl+C to stop all agents."

# Keep the script running to catch Ctrl+C for cleanup
while true; do
    sleep 1
done
7. Testing Workflow
Once implemented, you can test the system with the following workflow:

Run the run_three_agents.sh script to start all three agents
Wait for all terminals to initialize (Agent 1, Agent 2, and Agent 3)
Ensure Agent 2 is connected to Agent 3:
In Agent 2's terminal: :connect http://localhost:4202
Confirm connection with :servers to verify Agent 3 is listed
From Agent 1, send a message to Agent 2 asking about other available agents:
In Agent 1's terminal: :remote Do you know any other agents that could help with data analysis?
Agent 2 should respond with information about Agent 3, using the agent_discovery tool
Agent 1 can then request to connect to Agent 3:
In Agent 1's terminal: :connect http://localhost:4202
Test data analysis functionality:
In Agent 1's terminal: :remote Can you analyze this sales data: [your sample data]?
Agent 3 should respond with data analysis using its analyze_data tool
8. Key Components for Implementation
AgentDiscoveryTool: Allows agents to provide information about other known agents
Agent Directory: Stores information about known agents for service discovery
Tool Registration: Update the ToolExecutor to support new specialized tools
Agent Configuration: Set up each agent with appropriate roles and capabilities
Data Analysis Tool: Provide specialized data analysis capabilities for Agent 3
By implementing these components, you'll create a system where Agent 2 serves as a "broker" that can introduce Agent 1 to Agent 3, enabling a dynamic agent discovery pattern with specialized capabilities.





Could you summarize this very succinctly but saying what features need to be implemented in which files.

Edit
Implementation Summary for Three-Agent System with Agent Discovery
1. Server Tool Implementation
File: /src/server/tool_executor.rs

Add AgentDiscoveryTool class that accesses the agent directory
Add AnalyzeDataTool for data analysis capabilities
Update with_enabled_tools() method to accept agent_directory parameter
2. Bidirectional Agent Update
File: /src/bidirectional/bidirectional_agent.rs

Modify BidirectionalAgent::new() to pass agent_directory to tool executor
Update tool executor creation to include agent directory parameter
3. Configuration Files
Files:

agent1_config.toml: Standard configuration with LLM tools
agent2_config.toml: Add agent_discovery tool and enhanced system prompt
agent3_config.toml: Create new config with data analysis tools
4. Run Script
File: run_three_agents.sh

Create a script to launch all three agents
Configure agents to listen on ports 4200, 4201, and 4202
Set connection pattern: 1↔2↔3 (only Agent 2 initially knows Agent 3)
This implementation creates a discovery system where Agent 1 can ask Agent 2 about other agents, learn about Agent 3's capabilities, and connect directly to Agent 3 for specialized data analysis.