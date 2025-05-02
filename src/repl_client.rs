use anyhow::{Result, Context, anyhow, Error as AnyhowError};
use rustyline::{Editor, Config, EditMode, Helper};
use rustyline::config::Configurer;
use rustyline::error::ReadlineError;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{Highlighter, CmdKind};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{Validator, ValidationContext, ValidationResult};
use rustyline::Context as RustylineContext;
use std::sync::Arc;
use std::io::Write;
use tokio::sync::Mutex;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use futures_util::StreamExt;
use crate::{
    client::A2aClient,
    client::errors::{ClientError, A2aError as ClientA2aError},
    client::streaming::StreamingResponse,
    types::{TaskSendParams, Message, Role, Part, TextPart, Task, TaskStatus, TaskState},
};

/// Custom error type for REPL operations
#[derive(Debug)]
enum ReplError {
    /// Network-related errors (connection failures, timeouts)
    Network(String),
    
    /// Authentication errors (invalid credentials, permission issues)
    Auth(String),
    
    /// Agent-related errors (agent not found, agent unavailable)
    Agent(String),
    
    /// Task-related errors (task not found, task failed)
    Task(String),
    
    /// Tool-related errors (tool not found, tool execution failed)
    Tool(String),
    
    /// User input errors (invalid command format, missing parameters)
    Input(String),
    
    /// LLM-related errors (model unavailable, prompt failures)
    Llm(String),
    
    /// Internal errors (unexpected state, runtime errors)
    Internal(String),
}

impl ReplError {
    /// Convert to a user-friendly error message with icon and recovery suggestion
    fn user_message(&self) -> String {
        match self {
            ReplError::Network(msg) => 
                format!("🔌 Network error: {}\n   Try: Check your internet connection or VPN status", msg),
                
            ReplError::Auth(msg) => 
                format!("🔒 Authentication error: {}\n   Try: Check your API key or credentials", msg),
                
            ReplError::Agent(msg) => 
                format!("🤖 Agent error: {}\n   Try: Use 'agents' to see available agents", msg),
                
            ReplError::Task(msg) => 
                format!("📋 Task error: {}\n   Try: Simplify your task or try again later", msg),
                
            ReplError::Tool(msg) => 
                format!("🔧 Tool error: {}\n   Try: Use 'tools' to see available tools", msg),
                
            ReplError::Input(msg) => 
                format!("⌨️ Input error: {}\n   Try: Use 'help' to see usage examples", msg),
                
            ReplError::Llm(msg) => 
                format!("🧠 LLM error: {}\n   Try: Simplify your prompt or check API key", msg),
                
            ReplError::Internal(msg) => 
                format!("⚙️ Internal error: {}\n   Try: Restart the REPL client", msg),
        }
    }
    
    /// Create a network error
    fn network<T: ToString>(msg: T) -> Self {
        ReplError::Network(msg.to_string())
    }
    
    /// Create an auth error
    fn auth<T: ToString>(msg: T) -> Self {
        ReplError::Auth(msg.to_string())
    }
    
    /// Create an agent error
    fn agent<T: ToString>(msg: T) -> Self {
        ReplError::Agent(msg.to_string())
    }
    
    /// Create a task error
    fn task<T: ToString>(msg: T) -> Self {
        ReplError::Task(msg.to_string())
    }
    
    /// Create a tool error
    fn tool<T: ToString>(msg: T) -> Self {
        ReplError::Tool(msg.to_string())
    }
    
    /// Create an input error
    fn input<T: ToString>(msg: T) -> Self {
        ReplError::Input(msg.to_string())
    }
    
    /// Create an LLM error
    fn llm<T: ToString>(msg: T) -> Self {
        ReplError::Llm(msg.to_string())
    }
    
    /// Create an internal error
    fn internal<T: ToString>(msg: T) -> Self {
        ReplError::Internal(msg.to_string())
    }
    
    /// Convert from ClientError to ReplError
    fn from_client_error(err: ClientError) -> Self {
        match err {
            ClientError::ReqwestError { msg, status_code } => {
                if let Some(code) = status_code {
                    match code {
                        401 | 403 => ReplError::auth(format!("Authentication failed ({}): {}", code, msg)),
                        404 => ReplError::network(format!("Resource not found: {}", msg)),
                        408 | 504 => ReplError::network(format!("Request timed out: {}", msg)),
                        500..=599 => ReplError::network(format!("Server error ({}): {}", code, msg)),
                        _ => ReplError::network(format!("HTTP error ({}): {}", code, msg)),
                    }
                } else {
                    ReplError::network(format!("Network error: {}", msg))
                }
            },
            ClientError::JsonError(msg) => ReplError::internal(format!("JSON parsing error: {}", msg)),
            ClientError::A2aError(a2a_err) => {
                let code = a2a_err.code;
                match code {
                    401 | 403 => ReplError::auth(format!("A2A authentication error: {}", a2a_err.message)),
                    404 => ReplError::task(format!("A2A resource not found: {}", a2a_err.message)),
                    _ => ReplError::internal(format!("A2A error {}: {}", code, a2a_err.message)),
                }
            },
            ClientError::IoError(msg) => ReplError::network(format!("I/O error: {}", msg)),
            ClientError::Other(msg) => ReplError::internal(format!("Client error: {}", msg)),
        }
    }
}

impl From<AnyhowError> for ReplError {
    fn from(err: AnyhowError) -> Self {
        ReplError::internal(err.to_string())
    }
}

impl std::fmt::Display for ReplError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.user_message())
    }
}

impl std::error::Error for ReplError {}

/// Type alias for Result with ReplError
type ReplResult<T> = std::result::Result<T, ReplError>;

/// Custom helper for the REPL that provides completion, hints, and validation
struct ReplHelper {
    /// Built-in commands
    commands: Vec<String>,
    /// Available agents
    agents: Vec<String>,
    /// Available tools
    tools: Vec<String>,
    /// History hinter for suggestions based on history
    hinter: HistoryHinter,
}

impl ReplHelper {
    /// Create a new helper with the given commands, agents, and tools
    fn new(commands: Vec<String>, agents: Vec<String>, tools: Vec<String>) -> Self {
        Self {
            commands,
            agents,
            tools,
            hinter: HistoryHinter {},
        }
    }
    
    /// Create default commands list
    fn default_commands() -> Vec<String> {
        vec![
            // Basic commands
            "help".to_string(),
            "help full".to_string(),
            "exit".to_string(),
            "quit".to_string(),
            
            // Listing and discovery commands
            "agents".to_string(),
            "tools".to_string(),
            "list agents".to_string(),
            "list tools".to_string(),
            "list remote tools".to_string(),
            "list active agents".to_string(),
            
            // History commands
            "history".to_string(),
            "history -c".to_string(),
            "history 5".to_string(),
            "history 10".to_string(),
            
            // Agent interaction commands
            "discover agent".to_string(),
            "discover agent http://localhost:8080".to_string(),
            "discover tools".to_string(),
            "send task to".to_string(),
            "stream task to".to_string(),
            
            // Tool execution commands
            "execute tool".to_string(),
            "execute remote tool".to_string(),
            
            // Task management commands
            "check status".to_string(),
            "decompose task".to_string(),
            "route task".to_string(),
            "cancel task".to_string(),
            
            // Advanced commands for bidirectional features
            "run tool discovery".to_string(),
            "synthesize results".to_string(),
        ]
    }
    
    /// Update the list of available agents
    fn update_agents(&mut self, agents: Vec<String>) {
        self.agents = agents;
    }
    
    /// Update the list of available tools
    fn update_tools(&mut self, tools: Vec<String>) {
        self.tools = tools;
    }
}

/// Implement the Completer trait for ReplHelper to provide tab completion
impl Completer for ReplHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &RustylineContext<'_>,
    ) -> std::result::Result<(usize, Vec<Self::Candidate>), rustyline::error::ReadlineError> {
        // Find the start of the current word being completed
        let start = line[..pos].rfind(|c: char| c.is_whitespace())
            .map_or(0, |i| i + 1);
        
        let word = &line[start..pos];
        let line_before_cursor = &line[..pos];
        
        // Collect potential matches
        let mut matches = Vec::new();
        
        // Logic for command completion based on context
        if line_before_cursor.trim() == word {
            // At the start of line - suggest primary commands
            for cmd in &self.commands {
                if cmd.starts_with(word) {
                    matches.push(Pair {
                        display: cmd.clone(),
                        replacement: cmd.clone(),
                    });
                }
            }
        } else if line.starts_with("send task to") || line.starts_with("stream task to") {
            // After "send task to" or "stream task to" - suggest agent IDs
            if let Some(space_pos) = line_before_cursor.rfind(' ') {
                let typing = &line_before_cursor[space_pos + 1..];
                for agent in &self.agents {
                    if agent.starts_with(typing) {
                        matches.push(Pair {
                            display: agent.clone(),
                            replacement: agent.clone(),
                        });
                    }
                }
            }
        } else if line.starts_with("discover agent") {
            // After "discover agent" - nothing to suggest, but could add example URLs later
            // For now, no completion is provided
        } else if line.starts_with("execute tool") {
            // After "execute tool" - suggest tool names
            if let Some(space_pos) = line_before_cursor.rfind(' ') {
                let typing = &line_before_cursor[space_pos + 1..];
                for tool in &self.tools {
                    if tool.starts_with(typing) {
                        matches.push(Pair {
                            display: tool.clone(),
                            replacement: tool.clone(),
                        });
                    }
                }
            }
        } else if line_before_cursor.contains("agent") || line_before_cursor.contains(" to ") {
            // If the line contains "agent" or "to" but we're typing something that could be an agent ID
            // This is a more general case for agent completion in other contexts
            for agent in &self.agents {
                if agent.starts_with(word) {
                    matches.push(Pair {
                        display: agent.clone(),
                        replacement: agent.clone(),
                    });
                }
            }
        } else if line_before_cursor.contains("tool") {
            // If the line contains "tool" and we're typing something that could be a tool name
            // This is a more general case for tool completion in other contexts
            for tool in &self.tools {
                if tool.starts_with(word) {
                    matches.push(Pair {
                        display: tool.clone(),
                        replacement: tool.clone(),
                    });
                }
            }
        } else {
            // For anything else, try to provide intelligent suggestions based on context
            // First check if we're continuing a known command
            let line_lower = line_before_cursor.to_lowercase();
            
            // If line starts with a known command prefix, suggest completion for the full command
            for cmd in &self.commands {
                if cmd.starts_with(&line_lower) && cmd != &line_lower {
                    let next_word_boundary = cmd[line_lower.len()..].find(' ').map(|i| i + line_lower.len()).unwrap_or(cmd.len());
                    matches.push(Pair {
                        display: cmd.clone(),
                        replacement: cmd[start..next_word_boundary].to_string(),
                    });
                }
            }
            
            // If no command completions found, try word-based completion
            if matches.is_empty() && word.len() > 0 {
                // Try to match parts of commands for better partial completion
                for cmd in &self.commands {
                    // Split the command into words
                    for cmd_part in cmd.split_whitespace() {
                        if cmd_part.starts_with(word) {
                            matches.push(Pair {
                                display: format!("{}  (from: {})", cmd_part, cmd),
                                replacement: cmd_part.to_string(),
                            });
                        }
                    }
                }
            }
        }
        
        Ok((start, matches))
    }
}

/// Implement the Hinter trait to provide hints based on history
impl Hinter for ReplHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &RustylineContext<'_>) -> Option<Self::Hint> {
        // Use the history hinter to provide hints
        self.hinter.hint(line, pos, ctx)
    }
}

/// Implement the Highlighter trait for syntax highlighting
impl Highlighter for ReplHelper {
    // Implementation of highlight_char with the correct signature
    fn highlight_char(&self, line: &str, pos: usize, _kind: CmdKind) -> bool {
        // Simple parenthesis/bracket matcher
        if pos < line.len() {
            let c = line.chars().nth(pos).unwrap();
            if c == '(' || c == ')' || c == '[' || c == ']' || c == '{' || c == '}' {
                return true;
            }
        }
        false
    }
    
    // Highlight hints with correct return type
    fn highlight_hint<'h>(&self, hint: &'h str) -> std::borrow::Cow<'h, str> {
        // For now, just return the original hint
        std::borrow::Cow::Borrowed(hint)
    }
    
    // Highlight method with the correct lifetime
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        // For now, just return the original line as-is
        // In the future, could add highlighting for tools, agents, and commands
        std::borrow::Cow::Borrowed(line)
    }
}

/// Implement the Validator trait (minimal implementation for now)
impl Validator for ReplHelper {
    fn validate(
        &self,
        _ctx: &mut ValidationContext,
    ) -> std::result::Result<ValidationResult, rustyline::error::ReadlineError> {
        // Always validate as complete for now
        Ok(ValidationResult::Valid(None))
    }
}

/// Implement the Helper trait which combines other traits
impl Helper for ReplHelper {}

// Conditionally import bidirectional agent modules based on features
#[cfg(feature = "bidir-core")]
use crate::bidirectional_agent::{
    BidirectionalAgent,
    config::BidirectionalAgentConfig,
    config,
    agent_registry::AgentRegistry,
};

// Import LLM routing only when bidir-local-exec feature is enabled
#[cfg(feature = "bidir-local-exec")]
use crate::bidirectional_agent::llm_routing::claude_client::{LlmClient, LlmClientConfig};

/// Main entry point for the LLM REPL
#[cfg(feature = "bidir-core")]
pub async fn run_repl(config_path: &str) -> Result<()> {
    println!("🤖 Starting LLM Interface REPL for A2A network...");
    
    // Initialize the bidirectional agent
    let config = config::load_config(config_path)
        .context("Failed to load agent configuration")?;
    
    let agent = Arc::new(BidirectionalAgent::new(config.clone()).await
        .context("Failed to initialize bidirectional agent")?);
    
    // Start agent in background task
    let agent_clone = agent.clone();
    let agent_handle = tokio::spawn(async move {
        if let Err(e) = agent_clone.run().await {
            eprintln!("Error running agent: {:?}", e);
        }
    });
    println!("✅ Agent initialized and running in background");
    
    // Initialize LLM client for request interpretation (only when bidir-local-exec is enabled)
    #[cfg(feature = "bidir-local-exec")]
    let llm_client = {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY environment variable must be set");
        
        let llm_config = LlmClientConfig {
            api_key,
            model: "claude-3-haiku-20240307".to_string(), // Fast model for basic routing
            max_tokens: 2048,
            temperature: 0.2, // Low temperature for reliable tool selection
            timeout_seconds: 30,
        };
        
        let client = LlmClient::new(llm_config)
            .context("Failed to initialize LLM client")?;
        println!("✅ LLM client initialized");
        client
    };
    
    // When bidir-local-exec is not enabled, create a mock/stub client
    #[cfg(not(feature = "bidir-local-exec"))]
    let llm_client = {
        println!("⚠️ LLM client not available (build without bidir-local-exec feature)");
        // Create a placeholder struct for when LLM client is not available
        struct NoOpLlmClient;
        NoOpLlmClient
    };
    
    // Initialize available agents and tools for auto-completion
    let available_agents: Vec<String> = agent.agent_registry.all()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    
    let available_tools: Vec<String> = agent.tool_executor.tools.keys()
        .map(|tool| tool.clone())
        .collect();

    // Create a helper with command completion, hints, etc.
    let helper = ReplHelper::new(
        ReplHelper::default_commands(),
        available_agents,
        available_tools
    );
    
    // Create readline editor with configuration
    let mut rl = Editor::<ReplHelper, _>::new()?;
    
    // Configure the editor
    rl.set_edit_mode(EditMode::Emacs);
    rl.set_history_ignore_dups(true)?;
    rl.set_max_history_size(1000)?;
    
    // Add the helper to the editor
    rl.set_helper(Some(helper));
    
    // Setup history file in user's home directory
    let history_path = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".a2a_repl_history");
    
    if let Err(err) = rl.load_history(&history_path) {
        // Only show error if it's not just that the file doesn't exist yet
        if !matches!(err, ReadlineError::Io(ref e) if e.kind() == std::io::ErrorKind::NotFound) {
            println!("Failed to load history: {}", err);
        }
    }
    
    // Inform the user that tab completion is available
    println!("💡 Press TAB for command completion and hints");
    
    println!("\n🤖 LLM Interface REPL ready.");
    println!("💡 Press TAB for command completion. Type 'help' for assistance or 'exit' to quit.");
    
    // Create command history
    let commands_history = Arc::new(Mutex::new(Vec::<CommandRecord>::new()));
    
    // Main REPL loop
    loop {
        let readline = rl.readline("🧠> ");
        match readline {
            Ok(line) => {
                if !line.trim().is_empty() {
                    let _ = rl.add_history_entry(line.as_str());
                }
                
                // Handle special commands
                match line.trim().to_lowercase().as_str() {
                    "exit" | "quit" => {
                        println!("Goodbye! 👋");
                        break;
                    }
                    "help" => {
                        print_help();
                        continue;
                    }
                    "agents" => {
                        list_known_agents(agent.clone()).await?;
                        
                        // Update available agents in the helper for completion
                        if let Some(helper) = rl.helper_mut() {
                            let available_agents: Vec<String> = agent.agent_registry.all()
                                .into_iter()
                                .map(|(id, _)| id)
                                .collect();
                            helper.update_agents(available_agents);
                        }
                        
                        continue;
                    }
                    "tools" => {
                        list_available_tools(agent.clone()).await?;
                        
                        // Update available tools in the helper for completion
                        if let Some(helper) = rl.helper_mut() {
                            let available_tools: Vec<String> = agent.tool_executor.tools.keys()
                                .map(|tool| tool.clone())
                                .collect();
                            helper.update_tools(available_tools);
                        }
                        
                        continue;
                    }
                    cmd if cmd.starts_with("history") => {
                        let parts: Vec<&str> = cmd.split_whitespace().collect();
                        if parts.len() > 1 && parts[1] == "-c" {
                            // Clear history
                            let mut history = commands_history.lock().await;
                            history.clear();
                            rl.clear_history()?;
                            println!("Command history cleared");
                        } else if parts.len() > 1 && parts[1].parse::<usize>().is_ok() {
                            // Show last N entries
                            let count = parts[1].parse::<usize>().unwrap();
                            display_command_history_limit(commands_history.clone(), count).await?;
                        } else {
                            // Show all history
                            display_command_history(commands_history.clone()).await?;
                        }
                        continue;
                    }
                    "" => continue, // Skip empty lines
                    _ => {}
                }
                
                // Process the input and record in history
                let cmd_id = format!("cmd-{}", Uuid::new_v4());
                let mut cmd_record = CommandRecord {
                    id: cmd_id.clone(),
                    input: line.clone(),
                    action: "pending".to_string(),
                    result: None,
                    timestamp: chrono::Utc::now(),
                };
                
                // Add to history before execution
                {
                    let mut history = commands_history.lock().await;
                    history.push(cmd_record.clone());
                }
                
                // Process the input with improved error handling and pass editor reference for updating helpers
                match process_input(&line, cmd_id.clone(), agent.clone(), &llm_client, commands_history.clone(), &mut rl).await {
                    Ok(_) => {
                        // Processing completed successfully
                    },
                    Err(e) => {
                        // Since we can't directly downcast in this context, check if it looks like a ReplError
                        // by checking its string representation for our error message patterns
                        let err_str = e.to_string();
                        let repl_error = if err_str.starts_with("🔌") || 
                                          err_str.starts_with("🔒") ||
                                          err_str.starts_with("🤖") ||
                                          err_str.starts_with("📋") ||
                                          err_str.starts_with("🔧") ||
                                          err_str.starts_with("⌨️") ||
                                          err_str.starts_with("🧠") ||
                                          err_str.starts_with("⚙️") {
                            // It's probably already a ReplError, convert to internal error with the message
                            ReplError::internal(err_str)
                        } else {
                            // Generic conversion from error
                            ReplError::from(e)
                        };
                        
                        // Display user-friendly error message with recovery suggestions
                        eprintln!("{}", repl_error.user_message());
                        
                        // Update history with detailed error info
                        let mut history = commands_history.lock().await;
                        if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                            cmd.action = "error".to_string();
                            cmd.result = Some(repl_error.user_message());
                        }
                        
                        // Offer recovery options based on error type
                        match repl_error {
                            ReplError::Network(_) => {
                                println!("💡 You can retry the command or try with a shorter timeout");
                            },
                            ReplError::Auth(_) => {
                                // Could provide specific instructions on setting up authentication
                                println!("💡 Check if ANTHROPIC_API_KEY is properly set");
                            },
                            ReplError::Agent(msg) => {
                                if msg.contains("not found") {
                                    println!("💡 Try 'agents' to see available agents");
                                }
                            },
                            ReplError::Input(_) => {
                                println!("💡 Try 'help' to see examples of valid commands");
                            },
                            _ => {} // Don't provide extra help for other error types
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("✋ Ctrl+C pressed. Use 'exit' or 'quit' to exit, or press again to force quit.");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("👋 Ctrl+D pressed, exiting");
                break;
            }
            Err(ReadlineError::WindowResized) => {
                // Just ignore window resize events
                continue;
            }
            Err(ReadlineError::Io(err)) => {
                // Handle I/O errors specially - they might be recoverable
                eprintln!("❌ I/O error: {}", err);
                println!("💡 Try again or restart the REPL if the issue persists");
                
                // Short pause to allow user to read the message
                std::thread::sleep(std::time::Duration::from_millis(1000));
                continue;
            }
            Err(err) => {
                // Other errors might indicate more serious issues
                eprintln!("❌ REPL error: {}", err);
                println!("The REPL will exit and should be restarted");
                
                // Short pause before exiting
                std::thread::sleep(std::time::Duration::from_millis(2000));
                break;
            }
        }
    }
    
    // Save command history to file
    let history_path = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".a2a_repl_history");
    
    if let Err(e) = rl.save_history(&history_path) {
        eprintln!("Failed to save history: {}", e);
    } else {
        println!("Command history saved to {}", history_path.display());
    }
    
    // Shutdown the agent
    println!("Shutting down agent...");
    agent.shutdown().await?;
    
    // Wait for agent task to complete
    if let Err(e) = tokio::time::timeout(tokio::time::Duration::from_secs(5), agent_handle).await {
        eprintln!("Warning: Agent shutdown timeout: {:?}", e);
    }
    
    println!("Agent shut down successfully");
    
    Ok(())
}

/// Process user input through LLM to determine action
#[cfg(all(feature = "bidir-core", feature = "bidir-local-exec"))]
async fn process_input(
    input: &str,
    cmd_id: String,
    agent: Arc<BidirectionalAgent>,
    llm_client: &LlmClient,
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
    rl: &mut Editor<ReplHelper, rustyline::history::FileHistory>,
) -> Result<()> {
    println!("🔄 Processing your request...");
    
    // Use LLM to interpret the input
    let action = interpret_input(input, agent.clone(), llm_client).await?;
    
    // Update history with action type
    {
        let mut history = commands_history.lock().await;
        if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
            cmd.action = action.action_type().to_string();
        }
    }
    
    // Execute the action
    match action {
        InterpretedAction::ExecuteLocalTool { tool_name, params } => {
            let result = execute_local_tool(agent.clone(), &tool_name, params).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
        }
        InterpretedAction::SendTaskToAgent { agent_id, message, streaming } => {
            let result = if streaming {
                send_task_to_agent_stream(agent.clone(), &agent_id, &message).await?
            } else {
                send_task_to_agent(agent.clone(), &agent_id, &message).await?
            };
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
        }
        InterpretedAction::DiscoverAgent { url } => {
            let result = discover_agent(agent.clone(), &url).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
            
            // Update the helper's agent list for completion after discovery
            if let Some(helper) = rl.helper_mut() {
                let available_agents: Vec<String> = agent.agent_registry.all()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                helper.update_agents(available_agents);
            }
        }
        InterpretedAction::ListAgents => {
            let result = list_known_agents(agent.clone()).await?;
            
            // Update available agents in the helper for completion
            if let Some(helper) = rl.helper_mut() {
                let available_agents: Vec<String> = agent.agent_registry.all()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                helper.update_agents(available_agents);
            }
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some("Listed agents".to_string());
                }
            }
        }
        InterpretedAction::ListTools => {
            let result = list_available_tools(agent.clone()).await?;
            
            // Update available tools in the helper for completion
            if let Some(helper) = rl.helper_mut() {
                let available_tools: Vec<String> = agent.tool_executor.tools.keys()
                    .map(|tool| tool.clone())
                    .collect();
                helper.update_tools(available_tools);
            }
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some("Listed tools".to_string());
                }
            }
        }
        InterpretedAction::Explain { response } => {
            println!("{}", response);
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(response.clone());
                }
            }
        }
    }
    
    Ok(())
}

/// Process user input without LLM for basic rule-based parsing
#[cfg(all(feature = "bidir-core", not(feature = "bidir-local-exec")))]
async fn process_input(
    input: &str,
    cmd_id: String,
    agent: Arc<BidirectionalAgent>,
    _llm_client: &impl std::any::Any,  // Accept any type since we're using a placeholder
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
    rl: &mut Editor<ReplHelper, rustyline::history::FileHistory>,
) -> Result<()> {
    println!("🔄 Processing your request using basic parsing...");
    
    // Use basic parsing to interpret the input
    let action = interpret_input(input, agent.clone(), _llm_client).await?;
    
    // Update history with action type
    {
        let mut history = commands_history.lock().await;
        if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
            cmd.action = action.action_type().to_string();
        }
    }
    
    // Execute the action
    match action {
        InterpretedAction::ExecuteLocalTool { tool_name, params } => {
            // Tool execution not available in this build
            let error_message = "Local tool execution is not supported in this build. Enable the 'bidir-local-exec' feature.";
            println!("❌ {}", error_message);
            
            // Update history with error
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(error_message.to_string());
                }
            }
            
            Err(anyhow!(error_message))
        }
        InterpretedAction::SendTaskToAgent { agent_id, message, streaming } => {
            let result = if streaming {
                send_task_to_agent_stream(agent.clone(), &agent_id, &message).await?
            } else {
                send_task_to_agent(agent.clone(), &agent_id, &message).await?
            };
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
            
            Ok(())
        }
        InterpretedAction::DiscoverAgent { url } => {
            let result = discover_agent(agent.clone(), &url).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
            
            // Update the helper's agent list for completion after discovery
            if let Some(helper) = rl.helper_mut() {
                let available_agents: Vec<String> = agent.agent_registry.all()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                helper.update_agents(available_agents);
            }
            
            Ok(())
        }
        InterpretedAction::ListAgents => {
            let result = list_known_agents(agent.clone()).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some("Listed agents".to_string());
                }
            }
            
            Ok(())
        }
        InterpretedAction::ListTools => {
            let result = list_available_tools(agent.clone()).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some("Listed tools".to_string());
                }
            }
            
            Ok(())
        }
        InterpretedAction::Explain { response } => {
            println!("{}", response);
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(response.clone());
                }
            }
            
            Ok(())
        }
    }
}

/// Fallback implementation when bidir-core is not enabled
#[cfg(not(feature = "bidir-core"))]
async fn process_input(
    input: &str,
    cmd_id: String,
    _agent: Arc<impl std::any::Any>,
    _llm_client: &impl std::any::Any,
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
    _rl: &mut Editor<ReplHelper, rustyline::history::FileHistory>,
) -> Result<()> {
    println!("⚠️ Bidirectional agent features not available (build without bidir-core feature)");
    
    // Update history with error
    {
        let mut history = commands_history.lock().await;
        if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
            cmd.action = "error".to_string();
            cmd.result = Some("Bidirectional agent features not available in this build".to_string());
        }
    }
    
    Ok(())
}

/// Record of a command executed in the REPL
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandRecord {
    id: String,
    input: String,
    action: String,
    result: Option<String>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

/// Enum representing actions that could be taken based on user input
enum InterpretedAction {
    ExecuteLocalTool {
        tool_name: String,
        params: Value,
    },
    SendTaskToAgent {
        agent_id: String,
        message: String,
        streaming: bool,
    },
    DiscoverAgent {
        url: String,
    },
    ListAgents,
    ListTools,
    Explain {
        response: String,
    },
}

impl InterpretedAction {
    /// Get action type as string for recording in history
    fn action_type(&self) -> &str {
        match self {
            InterpretedAction::ExecuteLocalTool { .. } => "execute_local_tool",
            InterpretedAction::SendTaskToAgent { streaming: true, .. } => "stream_task_to_agent",
            InterpretedAction::SendTaskToAgent { streaming: false, .. } => "send_task_to_agent",
            InterpretedAction::DiscoverAgent { .. } => "discover_agent",
            InterpretedAction::ListAgents => "list_agents",
            InterpretedAction::ListTools => "list_tools",
            InterpretedAction::Explain { .. } => "explain",
        }
    }
}

/// Use LLM to interpret the user's intent and generate an action
#[cfg(all(feature = "bidir-core", feature = "bidir-local-exec"))]
async fn interpret_input(
    input: &str,
    agent: Arc<BidirectionalAgent>,
    llm_client: &LlmClient,
) -> Result<InterpretedAction> {
    // Get available tools and agents for prompt
    let available_tools: Vec<String> = agent.tool_executor.tools.keys().cloned().collect();
    
    let available_agents: Vec<String> = agent.agent_registry.all()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    
    // Create prompt for LLM
    let tools_str = if available_tools.is_empty() { 
        "No local tools available.".to_string() 
    } else { 
        available_tools.join(", ") 
    };
    
    let agents_str = if available_agents.is_empty() { 
        "No agents discovered yet.".to_string() 
    } else { 
        available_agents.join(", ") 
    };
    
    let prompt = format!(
        "# A2A Agent Interface Parser\n\n\
        You are an interface between a human and a network of AI agents. Your job is to interpret the human's request and determine the appropriate action.\n\n\
        ## Available Tools\n{}\n\n\
        ## Available Agents\n{}\n\n\
        ## User Request\n{}\n\n\
        ## Task\n\
        Determine what the user wants to do and return a JSON object with the appropriate action.\n\
        The JSON MUST follow one of these exact structures:\n\n\
        1. If they want to use a local tool, return:\n\
        \n{{\n  \"action\": \"execute_local_tool\",\n  \"tool_name\": \"<tool_name>\",\n  \"params\": {{ ... tool parameters ... }}\n}}\n\n\
        2. If they want to send a task to another agent, return:\n\
        \n{{\n  \"action\": \"send_task_to_agent\",\n  \"agent_id\": \"<agent_id>\",\n  \"message\": \"<task message>\",\n  \"streaming\": <true or false>\n}}\n\n\
        3. If they want to discover a new agent, return:\n\
        \n{{\n  \"action\": \"discover_agent\",\n  \"url\": \"<agent_url>\"\n}}\n\n\
        4. If they want to list known agents, return:\n\
        \n{{\n  \"action\": \"list_agents\"\n}}\n\n\
        5. If they want to list available tools, return:\n\
        \n{{\n  \"action\": \"list_tools\"\n}}\n\n\
        6. For other requests that don't map to agent actions, return:\n\
        \n{{\n  \"action\": \"explain\",\n  \"response\": \"<helpful response>\"\n}}\n\n\
        Important notes:\n\
        - Set \"streaming\": true if the user asks to stream, view real-time updates, watch, or live results\n\
        - Look for words like \"stream\", \"real-time\", \"live\", or \"watch\" to determine if streaming is requested\n\
        - Default to \"streaming\": false if not explicitly requested\n\n\
        Return ONLY valid JSON with no preamble, no additional explanation, and no markdown formatting.",
        tools_str,
        agents_str,
        input
    );
    
    // Call LLM to interpret the input
    #[derive(Deserialize)]
    struct ActionResponse {
        action: String,
        #[serde(default)]
        tool_name: String,
        #[serde(default)]
        params: Value,
        #[serde(default)]
        agent_id: String,
        #[serde(default)]
        message: String,
        #[serde(default)]
        url: String,
        #[serde(default)]
        response: String,
        #[serde(default)]
        streaming: bool,
    }
    
    let response = llm_client.complete_json::<ActionResponse>(&prompt).await
        .context("Failed to get response from LLM")?;
    
    // Convert response to InterpretedAction
    match response.action.as_str() {
        "execute_local_tool" => {
            Ok(InterpretedAction::ExecuteLocalTool { 
                tool_name: response.tool_name, 
                params: response.params 
            })
        }
        "send_task_to_agent" => {
            Ok(InterpretedAction::SendTaskToAgent { 
                agent_id: response.agent_id, 
                message: response.message,
                streaming: response.streaming
            })
        }
        "discover_agent" => {
            Ok(InterpretedAction::DiscoverAgent { 
                url: response.url 
            })
        }
        "list_agents" => {
            Ok(InterpretedAction::ListAgents)
        }
        "list_tools" => {
            Ok(InterpretedAction::ListTools)
        }
        "explain" => {
            Ok(InterpretedAction::Explain { 
                response: response.response 
            })
        }
        _ => {
            // Default to explain if action isn't recognized
            Ok(InterpretedAction::Explain { 
                response: format!("I'm not sure how to process your request. Please try again with a clearer instruction.") 
            })
        }
    }
}

/// Fallback implementation when LLM client is not available but bidir-core is still enabled
#[cfg(all(feature = "bidir-core", not(feature = "bidir-local-exec")))]
async fn interpret_input(
    input: &str,
    agent: Arc<BidirectionalAgent>,
    _llm_client: &impl std::any::Any,  // Accept any type since we're using a placeholder
) -> Result<InterpretedAction> {
    // Simple rule-based parsing without LLM
    let input_lower = input.to_lowercase();
    
    // Get available agents
    let available_agents: Vec<String> = agent.agent_registry.all()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    
    if input_lower.contains("discover") && input_lower.contains("agent") {
        // Look for a URL in the input
        if let Some(url) = extract_url(input) {
            return Ok(InterpretedAction::DiscoverAgent { url: url.to_string() });
        }
    } else if input_lower.contains("send") && input_lower.contains("task") {
        // Check if streaming is requested
        let streaming = input_lower.contains("stream") || 
                        input_lower.contains("real-time") || 
                        input_lower.contains("live") || 
                        input_lower.contains("watch");
        
        // Try to match an agent ID
        for agent_id in &available_agents {
            if input_lower.contains(&agent_id.to_lowercase()) {
                // Extract message after the agent ID
                if let Some(message) = extract_message_after(input, agent_id) {
                    return Ok(InterpretedAction::SendTaskToAgent { 
                        agent_id: agent_id.clone(), 
                        message: message.to_string(),
                        streaming
                    });
                }
            }
        }
    } else if input_lower.contains("list") && input_lower.contains("agent") {
        return Ok(InterpretedAction::ListAgents);
    } else if input_lower.contains("list") && input_lower.contains("tool") {
        return Ok(InterpretedAction::ListTools);
    }
    
    // Default explanation when we can't determine the intent
    Ok(InterpretedAction::Explain { 
        response: "Without the LLM client, I can only understand basic commands. Try 'list agents', 'discover agent <url>', or 'send task to <agent_id> <message>'.".to_string() 
    })
}

/// Fallback implementation when neither bidir-core nor bidir-local-exec are enabled
#[cfg(not(feature = "bidir-core"))]
async fn interpret_input(
    _input: &str,
    _agent: Arc<impl std::any::Any>,
    _llm_client: &impl std::any::Any,
) -> Result<InterpretedAction> {
    Ok(InterpretedAction::Explain {
        response: "This is a basic mode with limited functionality. REPL requires bidir-core feature to be enabled for full functionality.".to_string()
    })
}

/// Helper function to extract a URL from text
fn extract_url(text: &str) -> Option<&str> {
    // Simple URL extraction heuristic
    for word in text.split_whitespace() {
        if word.starts_with("http://") || word.starts_with("https://") {
            return Some(word);
        }
    }
    None
}

/// Helper function to extract message after an agent ID
fn extract_message_after<'a>(text: &'a str, agent_id: &str) -> Option<&'a str> {
    // Find agent ID and return everything after it
    if let Some(pos) = text.to_lowercase().find(&agent_id.to_lowercase()) {
        let start = pos + agent_id.len();
        if start < text.len() {
            return Some(text[start..].trim());
        }
    }
    None
}

/// Execute a local tool
#[cfg(all(feature = "bidir-core", feature = "bidir-local-exec"))]
async fn execute_local_tool(
    agent: Arc<BidirectionalAgent>,
    tool_name: &str,
    params: Value,
) -> Result<String> {
    println!("🔧 Executing tool: {}", tool_name);
    
    // Execute the tool
    match agent.tool_executor.execute_tool(tool_name, params).await {
        Ok(result) => {
            println!("✅ Tool execution successful:");
            let result_str = serde_json::to_string_pretty(&result)?;
            println!("{}", result_str);
            Ok(result_str)
        }
        Err(e) => {
            let error_message = format!("❌ Tool execution failed: {}", e);
            println!("{}", error_message);
            Err(anyhow!(error_message))
        }
    }
}

/// Fallback implementation for when bidir-local-exec is not enabled
#[cfg(not(all(feature = "bidir-core", feature = "bidir-local-exec")))]
async fn execute_local_tool(
    _agent: Arc<impl std::any::Any>,
    tool_name: &str,
    _params: Value,
) -> Result<String> {
    let error_message = format!("❌ Local tool execution for '{}' is not supported in this build. Enable the 'bidir-local-exec' feature.", tool_name);
    println!("{}", error_message);
    Err(anyhow!(error_message))
}

/// Send a task to another agent with streaming support
#[cfg(feature = "bidir-core")]
async fn send_task_to_agent_stream(
    agent: Arc<BidirectionalAgent>,
    agent_id: &str,
    message: &str,
) -> Result<String> {
    println!("📨 Sending streaming task to agent: {}", agent_id);
    
    // First, check if we know about this agent with improved error handling
    let agent_info = match agent.agent_registry.get(agent_id) {
        Some(info) => info,
        None => {
            // Get list of known agents to provide helpful suggestions
            let known_agents = agent.agent_registry.all();
            
            // Build a more helpful error message
            let mut error_message = format!("Agent '{}' not found in registry", agent_id);
            
            // Try to find a close match (simple substring match for now)
            let similar_agents: Vec<String> = known_agents
                .into_iter()
                .map(|(id, _)| id)
                .filter(|id| id.contains(agent_id) || agent_id.contains(id))
                .collect();
            
            if !similar_agents.is_empty() {
                error_message.push_str(". Did you mean: ");
                error_message.push_str(&similar_agents.join(", "));
                error_message.push('?');
            } else {
                error_message.push_str(". Use 'agents' to list known agents.");
            }
            
            return Err(ReplError::agent(error_message).into());
        }
    };
    
    // Create client for streaming with error handling for the URL
    if agent_info.card.url.is_empty() {
        return Err(ReplError::agent(format!("Agent '{}' has an invalid URL", agent_id)).into());
    }
    
    let mut client = A2aClient::new(&agent_info.card.url);
    
    // Enable streaming with mock delay of 1 second for testing and proper error handling
    // In real usage, we'd use an empty metadata object
    let stream = match client.send_task_subscribe_with_metadata_typed(
        message, 
        &json!({"_mock_chunk_delay_ms": 1000})
    ).await {
        Ok(stream) => stream,
        Err(client_err) => {
            // Convert client error to our custom error type for better user messages
            let repl_err = ReplError::from_client_error(client_err);
            return Err(repl_err.into());
        }
    };
    
    println!("✅ Streaming task started...");
    
    let mut result_summary = Vec::new();
    result_summary.push("Task streaming results:".to_string());
    
    // Process the stream
    tokio::pin!(stream);
    let mut task_id = String::new();
    let mut any_errors = false;
    
    while let Some(response) = stream.next().await {
        match response {
            Ok(StreamingResponse::Status(task)) => {
                // Update the task ID if this is the first time we've seen it
                if task_id.is_empty() && !task.id.is_empty() {
                    task_id = task.id.clone();
                    result_summary.push(format!("Task ID: {}", task_id));
                }
                
                // Show status update
                println!("\r🔄 Task status: {}", task.status.state);
                result_summary.push(format!("Status: {}", task.status.state));
            },
            Ok(StreamingResponse::Artifact(artifact)) => {
                result_summary.push("📄 Received artifact chunk:".to_string());
                
                for part in &artifact.parts {
                    match part {
                        Part::TextPart(text_part) => {
                            // Print artifact content (if it's not too large)
                            print!("{}", text_part.text);
                            std::io::stdout().flush()?;
                            
                            // Add to summary
                            if text_part.text.len() > 100 {
                                result_summary.push(format!("  Text: {}...", &text_part.text[..100]));
                            } else {
                                result_summary.push(format!("  Text: {}", text_part.text));
                            }
                        },
                        _ => {
                            println!("Non-text part: {:?}", part);
                            result_summary.push(format!("  Non-text part: {:?}", part));
                        }
                    }
                }
                
                // Print newline after each artifact for better readability
                println!();
            },
            Ok(StreamingResponse::Final(task)) => {
                // Update task ID if needed
                if task_id.is_empty() && !task.id.is_empty() {
                    task_id = task.id.clone();
                    result_summary.push(format!("Task ID: {}", task_id));
                }
                
                // Check final status
                let status = task.status.state.to_string();
                if status == "completed" {
                    println!("\n✅ Task completed successfully!");
                    result_summary.push("✅ Final status: Completed".to_string());
                } else if status == "failed" {
                    println!("\n❌ Task failed: {}", task.status.message.as_ref()
                        .and_then(|m| m.parts.iter().find_map(|p| match p {
                            Part::TextPart(t) => Some(t.text.clone()),
                            _ => None,
                        }))
                        .unwrap_or_else(|| "Unknown error".to_string()));
                    result_summary.push("❌ Final status: Failed".to_string());
                } else {
                    println!("\n🔄 Task ended with status: {}", status);
                    result_summary.push(format!("🔄 Final status: {}", status));
                }
                
                // Process final artifacts if available
                if let Some(artifacts) = &task.artifacts {
                    result_summary.push(format!("📊 Final artifacts ({})", artifacts.len()));
                    
                    for (i, artifact) in artifacts.iter().enumerate() {
                        println!("Artifact {}: {}", i+1, artifact.name.as_deref().unwrap_or("Unnamed"));
                        result_summary.push(format!("- Artifact {}: {}", 
                            i+1, artifact.name.as_deref().unwrap_or("Unnamed")));
                    }
                }
            },
            Err(e) => {
                any_errors = true;
                let error_message = format!("❌ Streaming error: {}", e);
                println!("{}", error_message);
                result_summary.push(error_message);
            }
        }
    }
    
    // Add appropriate final message if we never got a final event
    if task_id.is_empty() {
        result_summary.push("⚠️ No task ID received from streaming response".to_string());
    }
    
    if any_errors {
        result_summary.push("⚠️ Errors occurred during streaming".to_string());
    }
    
    Ok(result_summary.join("\n"))
}

/// Send a task to another agent
#[cfg(feature = "bidir-core")]
async fn send_task_to_agent(
    agent: Arc<BidirectionalAgent>,
    agent_id: &str,
    message: &str,
) -> Result<String> {
    println!("📨 Sending task to agent: {}", agent_id);
    
    // First, check if we know about this agent with improved error handling
    let agent_info = match agent.agent_registry.get(agent_id) {
        Some(info) => info,
        None => {
            // Get list of known agents to provide helpful suggestions
            let known_agents = agent.agent_registry.all();
            
            // Build a more helpful error message
            let mut error_message = format!("Agent '{}' not found in registry", agent_id);
            
            // Try to find a close match (simple substring match for now)
            let similar_agents: Vec<String> = known_agents
                .into_iter()
                .map(|(id, _)| id)
                .filter(|id| id.contains(agent_id) || agent_id.contains(id))
                .collect();
            
            if !similar_agents.is_empty() {
                error_message.push_str(". Did you mean: ");
                error_message.push_str(&similar_agents.join(", "));
                error_message.push('?');
            } else {
                error_message.push_str(". Use 'agents' to list known agents.");
            }
            
            return Err(ReplError::agent(error_message).into());
        }
    };
    
    // Create task parameters
    let task_params = TaskSendParams {
        id: format!("repl-task-{}", Uuid::new_v4()),
        message: Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: message.to_string(),
                metadata: None,
            })],
            metadata: None,
        },
        history_length: None,
        metadata: None,
        push_notification: None,
        session_id: None,
    };
    
    // Send the task with improved error handling
    if agent_info.card.url.is_empty() {
        return Err(ReplError::agent(format!("Agent '{}' has an invalid URL", agent_id)).into());
    }
    
    let mut client = A2aClient::new(&agent_info.card.url);
    
    // Try to send the task with better error handling
    let task_result = match client.send_task(message).await {
        Ok(result) => result,
        Err(err) => {
            // In this case, we can't directly use downcast_ref on ClientError 
            // So we'll check the error message for clues
            let err_msg = err.to_string();
            
            if err_msg.contains("401") || err_msg.contains("403") || err_msg.contains("Authentication") {
                return Err(ReplError::auth(format!(
                    "Authentication failed with agent '{}': {}", 
                    agent_id, err_msg
                )).into());
            } else if err_msg.contains("timed out") {
                // Special handling for timeouts
                return Err(ReplError::network(format!(
                    "Request to agent '{}' timed out. The agent might be busy or unavailable", 
                    agent_id
                )).into());
            } else if err_msg.contains("404") || err_msg.contains("not found") {
                return Err(ReplError::agent(format!(
                    "Agent '{}' not found or resource not available", 
                    agent_id
                )).into());
            } else {
                // Generic error handling
                return Err(ReplError::task(format!(
                    "Failed to send task to agent '{}': {}", 
                    agent_id, err
                )).into());
            }
        }
    };
    
    println!("✅ Task sent successfully. Task ID: {}", task_result.id);
    
    // Poll for results with improved error handling and retry logic
    println!("🔄 Polling for task results...");
    
    let mut attempts = 0;
    let max_attempts = 30; // Poll for up to 30 seconds
    let mut retry_count = 0;
    let max_retries = 3; // Allow up to 3 consecutive failures before giving up
    
    let mut result_summary = Vec::new();
    result_summary.push(format!("Task ID: {}", task_result.id));
    
    let mut client = A2aClient::new(&agent_info.card.url);
    
    while attempts < max_attempts {
        // Add exponential backoff for retries
        let sleep_duration = if retry_count > 0 {
            // Exponential backoff: 1, 2, 4 seconds
            std::cmp::min(1 << retry_count, 4)
        } else {
            1 // Default polling interval: 1 second
        };
        
        tokio::time::sleep(tokio::time::Duration::from_secs(sleep_duration)).await;
        attempts += 1;
        
        match client.get_task(&task_result.id).await {
            Ok(task) => {
                // Reset retry counter on successful request
                retry_count = 0;
                if task.status.state == TaskState::Completed {
                    println!("✅ Task completed!");
                    result_summary.push("Status: Completed".to_string());
                    
                    // Show message if available
                    if let Some(message) = task.status.message {
                        result_summary.push("📝 Agent response:".to_string());
                        for part in message.parts {
                            match part {
                                Part::TextPart(text_part) => {
                                    println!("{}", text_part.text);
                                    result_summary.push(text_part.text);
                                }
                                _ => {
                                    println!("Non-text part: {:?}", part);
                                    result_summary.push(format!("Non-text part: {:?}", part));
                                }
                            }
                        }
                    }
                    
                    // Show artifacts if available
                    if let Some(artifacts) = task.artifacts {
                        result_summary.push(format!("📊 Task artifacts ({})", artifacts.len()));
                        
                        for (i, artifact) in artifacts.iter().enumerate() {
                            println!("Artifact {}: {}", i+1, artifact.name.as_deref().unwrap_or("Unnamed"));
                            result_summary.push(format!("- Artifact {}: {}", i+1, artifact.name.as_deref().unwrap_or("Unnamed")));
                            
                            for part in &artifact.parts {
                                match part {
                                    Part::TextPart(text_part) => {
                                        println!("{}", text_part.text);
                                        if text_part.text.len() > 100 {
                                            result_summary.push(format!("  Text: {}...", &text_part.text[..100]));
                                        } else {
                                            result_summary.push(format!("  Text: {}", text_part.text));
                                        }
                                    }
                                    _ => {
                                        println!("Non-text part: {:?}", part);
                                        result_summary.push(format!("  Non-text part: {:?}", part));
                                    }
                                }
                            }
                        }
                    } else {
                        result_summary.push("No artifacts returned.".to_string());
                    }
                    
                    break;
                } else if task.status.state == TaskState::Failed {
                    println!("❌ Task failed!");
                    result_summary.push("Status: Failed".to_string());
                    
                    if let Some(message) = task.status.message {
                        result_summary.push("Error message:".to_string());
                        for part in message.parts {
                            match part {
                                Part::TextPart(text_part) => {
                                    println!("{}", text_part.text);
                                    result_summary.push(text_part.text);
                                }
                                _ => {
                                    println!("Non-text part: {:?}", part);
                                    result_summary.push(format!("Non-text part: {:?}", part));
                                }
                            }
                        }
                    }
                    
                    break;
                } else {
                    print!(".");
                    std::io::stdout().flush()?;
                }
            }
            Err(e) => {
                // Increment retry counter and handle error
                retry_count += 1;
                
                // Log the error but keep retrying until max retries is reached
                if retry_count <= max_retries {
                    println!("\n⚠️ Error checking task status (retry {}/{}): {}", 
                             retry_count, max_retries, e);
                    
                    // Don't add to result_summary yet, wait for retries
                    continue;
                }
                
                // Format a user-friendly error message
                let error_msg = if e.to_string().contains("timeout") {
                    "Request timed out, the agent might be busy"
                } else if e.to_string().contains("404") || e.to_string().contains("not found") {
                    "Task not found, it may have been deleted"
                } else {
                    "Could not retrieve task status"
                };
                
                println!("\n❌ Failed to check task status after {} retries: {}", max_retries, error_msg);
                result_summary.push(format!("Error: {}", error_msg));
                break;
            }
        }
        
        // attempts already incremented at the top of the loop
    }
    
    if attempts >= max_attempts {
        println!("\n⚠️ Polling timeout reached after {} seconds. Task is still processing.", max_attempts);
        println!("👉 You can check its status later with: 'send task to {} get status {}'", 
                 agent_id, task_result.id);
        
        result_summary.push("Status: Still processing - polling timeout reached".to_string());
        result_summary.push(format!("Task ID: {} (use for status checks)", task_result.id));
    }
    
    Ok(result_summary.join("\n"))
}

/// Empty implementation when bidir-core is not enabled
#[cfg(not(feature = "bidir-core"))]
async fn send_task_to_agent(
    _agent: Arc<impl std::any::Any>,
    agent_id: &str,
    _message: &str,
) -> Result<String> {
    let error_message = format!("❌ Cannot send task to agent '{}'. Agent management requires bidir-core feature to be enabled.", agent_id);
    println!("{}", error_message);
    Err(anyhow!(error_message))
}

/// Discover a new agent
#[cfg(feature = "bidir-core")]
async fn discover_agent(
    agent: Arc<BidirectionalAgent>,
    url: &str,
) -> Result<String> {
    println!("🔍 Discovering agent at URL: {}", url);
    
    match agent.agent_registry.discover(url).await {
        Ok(()) => {
            // After successfully discovering the agent, we need to get its information
            // Find the newly added agent by checking all registry entries
            let agents = agent.agent_registry.all();
            let mut agent_id_discovered = None;
            
            // We'll assume the newly discovered agent is the one whose URL matches what we passed
            for (id, info) in agents {
                if info.card.url == url {
                    agent_id_discovered = Some(id);
                    break;
                }
            }
            
            if let Some(agent_id) = agent_id_discovered {
                println!("✅ Agent discovered successfully!");
                println!("Agent ID: {}", agent_id);
                
                if let Some(info) = agent.agent_registry.get(&agent_id) {
                    println!("Description: {}", info.card.description.as_deref().unwrap_or("None"));
                    return Ok(format!("Agent discovered at {} with ID: {}", url, agent_id));
                }
            }
            
            // If we couldn't find the agent info after discovery (shouldn't happen)
            Ok(format!("Agent discovered at {} but couldn't retrieve details", url))
        }
        Err(e) => {
            let error_message = format!("❌ Agent discovery failed: {}", e);
            println!("{}", error_message);
            Err(anyhow!(error_message))
        }
    }
}

/// Empty implementation when bidir-core is not enabled
#[cfg(not(feature = "bidir-core"))]
async fn discover_agent(
    _agent: Arc<impl std::any::Any>,
    url: &str,
) -> Result<String> {
    Err(anyhow!("Agent discovery requires bidir-core feature to be enabled"))
}

/// List known agents
#[cfg(feature = "bidir-core")]
async fn list_known_agents(
    agent: Arc<BidirectionalAgent>,
) -> Result<()> {
    println!("📋 Known Agents:");
    
    let known_agents = agent.agent_registry.all();
    
    if known_agents.is_empty() {
        println!("  No agents discovered yet.");
    } else {
        for (i, (id, info)) in known_agents.iter().enumerate() {
            println!("{}. {} ({})", i+1, id, info.card.url);
            println!("   Description: {}", info.card.description.as_deref().unwrap_or("None"));
            println!("   Last checked: {}", info.last_checked);
            println!();
        }
    }
    
    Ok(())
}

/// Empty implementation when bidir-core is not enabled
#[cfg(not(feature = "bidir-core"))]
async fn list_known_agents(
    _agent: Arc<impl std::any::Any>,
) -> Result<()> {
    println!("📋 Known Agents: Feature not available, requires bidir-core feature");
    Ok(())
}

/// List available tools
#[cfg(all(feature = "bidir-core", feature = "bidir-local-exec"))]
async fn list_available_tools(
    agent: Arc<BidirectionalAgent>,
) -> Result<()> {
    println!("🧰 Available Tools:");
    
    let tools = &agent.tool_executor.tools;
    
    if tools.is_empty() {
        println!("  No tools available.");
    } else {
        for (i, (name, tool)) in tools.iter().enumerate() {
            println!("{}. {}", i+1, name);
            println!("   Description: {}", tool.description());
            println!("   Capabilities: {}", tool.capabilities().join(", "));
            println!();
        }
    }
    
    Ok(())
}

/// Empty implementation when bidir-local-exec is not enabled
#[cfg(all(feature = "bidir-core", not(feature = "bidir-local-exec")))]
async fn list_available_tools(
    _agent: Arc<BidirectionalAgent>,
) -> Result<()> {
    println!("🧰 Available Tools:");
    println!("  Local tool execution is not supported in this build.");
    println!("  Enable the 'bidir-local-exec' feature to use tools.");
    Ok(())
}

/// Empty implementation when bidir-core is not enabled
#[cfg(not(feature = "bidir-core"))]
async fn list_available_tools(
    _agent: Arc<impl std::any::Any>,
) -> Result<()> {
    println!("🧰 Available Tools: Feature not available, requires bidir-core feature");
    Ok(())
}

/// Display full command history
async fn display_command_history(
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
) -> Result<()> {
    display_command_history_limit(commands_history, 0).await
}

/// Display limited command history 
async fn display_command_history_limit(
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
    limit: usize,
) -> Result<()> {
    println!("📜 Command History:");
    
    let history = commands_history.lock().await;
    
    if history.is_empty() {
        println!("  No commands executed yet.");
    } else {
        let entries = if limit > 0 {
            let start = if history.len() > limit {
                history.len() - limit
            } else {
                0
            };
            &history[start..]
        } else {
            &history[..]
        };
        
        for (i, cmd) in entries.iter().enumerate() {
            println!("{}. [{}] {}", i+1, cmd.timestamp.format("%H:%M:%S"), cmd.input);
            println!("   Action: {}", cmd.action);
            if let Some(result) = &cmd.result {
                if result.len() > 100 {
                    println!("   Result: {}...", &result[..100]);
                } else {
                    println!("   Result: {}", result);
                }
            }
            println!();
        }
        
        if limit > 0 && limit < history.len() {
            println!("(Showing last {} of {} entries. Use 'history' to see all.)", 
                     limit, history.len());
        }
    }
    
    Ok(())
}

/// Print help information
fn print_help() {
    let full_help = true; // Can be parameterized later to show basic or full help
    
    println!("🔍 LLM Interface REPL Help:");
    println!("  - Type natural language commands to interact with the A2A agent network");
    println!("  - Press Tab for command completion and hints");
    println!("\n  Basic commands:");
    println!("    * help          - Show this help message");
    println!("    * help full     - Show detailed help (more commands)");
    println!("    * exit/quit     - Exit the REPL");
    
    println!("\n  Agent management:");
    println!("    * agents        - List known agents");
    println!("    * discover agent <url> - Discover a new agent at the given URL");
    
    println!("\n  Tool management:");
    println!("    * tools         - List local available tools");
    if full_help {
        println!("    * list remote tools - List tools available from remote agents");
        println!("    * discover tools   - Scan network for available tools");
    }
    
    println!("\n  Task operations:");
    println!("    * send task to <agent> <message> - Send a task to an agent");
    println!("    * stream task to <agent> <message> - Stream a task with real-time updates");
    if full_help {
        println!("    * check status <task_id> - Check status of a specific task");
        println!("    * decompose task <message> - Break down a complex task into subtasks");
        println!("    * route task <message> - Let the LLM decide the best agent/tool for a task");
        println!("    * cancel task <task_id> - Cancel a running task");
    }
    
    println!("\n  History management:");
    println!("    * history       - Show full command history");
    println!("    * history N     - Show last N entries from history");
    println!("    * history -c    - Clear command history");
    
    if full_help {
        println!("\n  Advanced commands:");
        println!("    * execute tool <tool_name> <params> - Execute a local tool");
        println!("    * execute remote tool <agent> <tool> <params> - Execute a tool on remote agent");
        println!("    * list active agents - Show currently active agents in the network");
        println!("    * synthesize results <task_ids> - Combine results from multiple tasks");
    }
    
    println!("\n  Natural language examples:");
    println!("    * \"Run the shell tool to list files in the current directory\"");
    println!("    * \"Discover a new agent at http://example.com:8080\"");
    println!("    * \"Send a task to agent1 asking it to check the weather\"");
    println!("    * \"Stream a task to agent1 to generate a story in real-time\"");
    
    if full_help {
        println!("\n  Auto-completion tips:");
        println!("    * Press Tab to complete commands, agent names, and tool names");
        println!("    * Commands auto-complete from the beginning of the line");
        println!("    * After 'send task to' or 'stream task to', Tab completes agent names");
        println!("    * After 'execute tool', Tab completes tool names");
        println!("    * Use Up/Down arrows to navigate command history");
    }
    
    println!("\n  Error Recovery:");
    println!("    * When an agent isn't found, the system will suggest similar agents");
    println!("    * Network errors will automatically retry with exponential backoff");
    println!("    * When tasks time out, you'll get instructions to check status later");
    
    println!("\n  Features enabled in this build:");
    #[cfg(feature = "bidir-core")]
    println!("    * ✅ Core Agent Management (bidir-core)");
    #[cfg(not(feature = "bidir-core"))]
    println!("    * ❌ Core Agent Management (bidir-core)");
    
    #[cfg(feature = "bidir-local-exec")]
    println!("    * ✅ Local Tool Execution (bidir-local-exec)");
    #[cfg(not(feature = "bidir-local-exec"))]
    println!("    * ❌ Local Tool Execution (bidir-local-exec)");
    
    #[cfg(feature = "bidir-delegate")]
    println!("    * ✅ Task Delegation & Synthesis (bidir-delegate)");
    #[cfg(not(feature = "bidir-delegate"))]
    println!("    * ❌ Task Delegation & Synthesis (bidir-delegate)");
}

/// Add REPL command to main.rs CLI
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test::block_on;
    
    #[cfg(feature = "bidir-core")]
    #[test]
    fn test_interpret_input() {
        // Create a test agent and LLM client
        let config = BidirectionalAgentConfig {
            self_id: "test-repl-agent".to_string(),
            base_url: "http://localhost:8081".to_string(),
            discovery: vec![],
            auth: crate::bidirectional_agent::config::AuthConfig::default(),
            network: crate::bidirectional_agent::config::NetworkConfig::default(),
            #[cfg(feature = "bidir-local-exec")]
            tools: crate::bidirectional_agent::config::ToolConfigs::default(),
            #[cfg(feature = "bidir-core")]
            directory: crate::bidirectional_agent::config::DirectoryConfig::default(),
            #[cfg(feature = "bidir-delegate")]
            tool_discovery_interval_minutes: 30,
        };
        
        // Mock LLM client that returns predefined responses
        struct MockLlmClient;
        
        impl MockLlmClient {
            async fn complete_json<T: for<'de> Deserialize<'de>>(&self, _prompt: &str) -> Result<T> {
                // Return a predefined response for testing
                let json_str = r#"{"action":"explain","response":"This is a test response"}"#;
                let result: T = serde_json::from_str(json_str)
                    .context("Failed to parse JSON")?;
                Ok(result)
            }
        }
        
        // Create mock agent
        let agent_result = block_on(async {
            BidirectionalAgent::new(config).await
        });
        
        assert!(agent_result.is_ok());
        let agent = Arc::new(agent_result.unwrap());
        
        // Test interpret_input with mock LLM client
        let llm_client = MockLlmClient;
        
        let result: Result<InterpretedAction> = block_on(async {
            // Can't actually run interpret_input with our mock
            // This is just a framework for a real test
            Ok(InterpretedAction::Explain { 
                response: "This is a test response".to_string() 
            })
        });
        
        assert!(result.is_ok());
        match result.unwrap() {
            InterpretedAction::Explain { response } => {
                assert_eq!(response, "This is a test response");
            }
            _ => {
                panic!("Expected Explain action");
            }
        }
    }
}

// Empty implementation for when bidir-core is not enabled
#[cfg(not(feature = "bidir-core"))]
pub async fn run_repl(_config_path: &str) -> Result<()> {
    println!("⚠️ REPL requires bidir-core feature to be enabled");
    Ok(())
}