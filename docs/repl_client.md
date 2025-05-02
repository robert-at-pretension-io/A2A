I'll help you improve the REPL client for better user experience by researching documentation for any implementation details I'm unsure about. Let me analyze the suggestions from the file and look up implementation details for the ones that need clarification.

First, let me understand what's already been suggested in the paste.txt file:

The suggestions cover several key areas of REPL improvement:
1. Persistent & searchable history
2. Auto-completion & hints
3. Better prompt & color output
4. Streaming & cancellation
5. Multi-line & JSON editing
6. Configurable settings
7. Better help & onboarding
8. Error surfacing
9. Code organization clean-ups
10. Testability/CI demos

Let me research implementation details for some of these features that may need additional clarity.

Let me look up information about auto-completion and hints in rustyline:

Now let me look up information about color output in Rust:

Let me also look up information about progress indicators in Rust:

Now, let me look up information about multi-line editing in rustyline:

Let me search for one more topic - implementation details for a configurable key-value store:

Based on my research, I can now provide comprehensive implementation details for the REPL client improvements suggested in the original document. Here's my enhanced explanation with implementation details:

# Enhanced A2A REPL Client Implementation Guide

Based on the suggestions in the original document and my research into the relevant Rust libraries, here's how you can implement each improvement for a better REPL user experience:

## 1. Persistent & Searchable History

To implement persistent history in your REPL, use the rustyline library's history functionality:

```rust
use rustyline::{DefaultEditor, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    
    // Load history from $HOME/.a2a_history
    let history_path = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".a2a_history");
    
    if let Err(_) = rl.load_history(&history_path) {
        println!("No previous history found.");
    }
    
    // Configure better history options with Config::builder()
    let config = rustyline::Config::builder()
        .history_ignore_dups(true)  // Avoid duplicate entries
        .max_history_size(1000)     // Set reasonable limit
        .build();
    
    // After each command 
    rl.add_history_entry(line.as_str())?;
    
    // Save history when done
    rl.save_history(&history_path)?;
    
    Ok(())
}
```

For implementing history commands, add command handlers:

```rust
fn handle_history_command(args: &[&str], rl: &mut DefaultEditor) {
    match args.get(0) {
        Some(&"-c") => {
            rl.clear_history().unwrap_or_else(|_| println!("Failed to clear history"));
            println!("History cleared");
        },
        Some(n) if n.parse::<usize>().is_ok() => {
            let count = n.parse::<usize>().unwrap();
            // Display last n entries
            for (i, entry) in rl.history().iter().rev().take(count).enumerate() {
                println!("{}: {}", i + 1, entry);
            }
        },
        _ => {
            // Show all history
            for (i, entry) in rl.history().iter().enumerate() {
                println!("{}: {}", i + 1, entry);
            }
        }
    }
}
```

## 2. Auto-completion & Hints

To implement auto-completion in your REPL, you need to create a custom `Completer` implementation for the rustyline library. It needs to implement the `complete` method that takes the current line and cursor position and returns completion candidates.

Here's how to implement a custom completion system for your REPL:

```rust
use rustyline::completion::{Completer, Pair};
use rustyline::Context;
use rustyline::Result;

struct A2aCompleter {
    commands: Vec<String>,
    agents: Vec<String>,
    tools: Vec<String>,
}

impl Completer for A2aCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Self::Candidate>)> {
        // Find the start of the current word
        let start = line[..pos].rfind(char::is_whitespace).map_or(0, |i| i + 1);
        let word = &line[start..pos];
        
        // Collect matching commands, agents, or tools
        let matches: Vec<Pair> = self.commands.iter()
            .chain(self.agents.iter())
            .chain(self.tools.iter())
            .filter(|s| s.starts_with(word))
            .map(|s| Pair {
                display: s.clone(),
                replacement: s.clone(),
            })
            .collect();
        
        Ok((start, matches))
    }
}
```

This completer needs to be populated with your available commands, agents, and tools:

```rust
fn create_completer(agent: &Agent) -> A2aCompleter {
    A2aCompleter {
        commands: vec![
            "help".to_string(), 
            "exit".to_string(), 
            "history".to_string(),
            // add more commands here
        ],
        agents: agent.agent_registry.all().iter()
            .map(|id| id.to_string())
            .collect(),
        tools: agent.tool_executor.tools.keys()
            .map(|k| k.to_string())
            .collect(),
    }
}
```

To integrate with your REPL:

```rust
let completer = create_completer(&agent);
let helper = MyHelper { completer };
let mut rl = Editor::with_config(config)?;
rl.set_helper(Some(helper));
```

## 3. Better Prompt & Color Output

For colored terminal output, the owo-colors crate is recommended as it's zero-allocation, supports the NO_COLOR/FORCE_COLOR environment variables, checks for terminal capabilities, and works on various platforms.

Here's how to implement colorized output:

```rust
use owo_colors::{OwoColorize, Stream};

// For success messages
println!("{}", "Task completed successfully!".green());

// For warnings
println!("{}", "Warning: This might take a while...".yellow());

// For errors
eprintln!("{}", "Error: Invalid command".red().bold());

// For prompt
fn get_dynamic_prompt(active_agents: usize, running_tasks: usize) -> String {
    format!(
        "[{}🕸  {}⚙] > ", 
        active_agents.to_string().blue().bold(),
        running_tasks.to_string().green().bold()
    )
}
```

## 4. Streaming & Cancellation with Progress Indicators

For progress indicators, the indicatif crate provides ProgressBar and spinner capabilities that can be used instead of printing dots during long-running operations.

```rust
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

fn process_task(task_id: &str) {
    // Create a spinner for tasks that don't have clear progress metrics
    let spinner = ProgressBar::new_spinner();
    
    // Set a nice spinner style
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner} {msg}")
            .unwrap()
    );
    
    spinner.set_message(format!("Processing task {}", task_id));
    spinner.enable_steady_tick(Duration::from_millis(100));
    
    // Your task execution code here
    // ...
    
    // When done:
    spinner.finish_with_message(format!("Task {} completed!", task_id));
}
```

For cancellation functionality:

```rust
struct TaskManager {
    running_tasks: HashMap<String, JoinHandle<()>>,
}

impl TaskManager {
    fn new() -> Self {
        Self {
            running_tasks: HashMap::new(),
        }
    }
    
    fn add_task(&mut self, task_id: String, handle: JoinHandle<()>) {
        self.running_tasks.insert(task_id, handle);
    }
    
    fn cancel_task(&mut self, task_id: &str) -> bool {
        if let Some(handle) = self.running_tasks.remove(task_id) {
            handle.abort(); // For tokio tasks
            true
        } else {
            false
        }
    }
    
    fn list_tasks(&self) -> Vec<&String> {
        self.running_tasks.keys().collect()
    }
}
```

## 5. Multi-line & JSON Editing

To implement multi-line editing, especially for JSON input, you need to implement the Validator trait from rustyline. This trait determines whether the current input is complete or requires more input.

```rust
use rustyline::validate::{ValidationContext, ValidationResult, Validator};

struct JsonValidator;

impl Validator for JsonValidator {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<ValidationResult> {
        let input = ctx.input();
        
        // Check for unbalanced braces/brackets
        let mut brace_count = 0;
        let mut bracket_count = 0;
        
        for c in input.chars() {
            match c {
                '{' => brace_count += 1,
                '}' => brace_count -= 1,
                '[' => bracket_count += 1,
                ']' => bracket_count -= 1,
                _ => {}
            }
        }
        
        if brace_count > 0 || bracket_count > 0 {
            // Input is incomplete, request more
            Ok(ValidationResult::Incomplete)
        } else if brace_count < 0 || bracket_count < 0 {
            // Too many closing braces/brackets
            Ok(ValidationResult::Invalid(Some("Unbalanced JSON: too many closing braces/brackets".to_string())))
        } else {
            // Input looks balanced - it might still be invalid JSON
            // but we'll parse that later
            Ok(ValidationResult::Valid(None))
        }
    }
}
```

This validator should be added to your helper struct:

```rust
struct MyHelper {
    completer: A2aCompleter,
    validator: JsonValidator,
}

impl Validator for MyHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<ValidationResult> {
        self.validator.validate(ctx)
    }
}
```

## 6. Configurable Settings Inside the REPL

For a configuration system using TOML files in the user's home directory:

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use toml;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct A2aConfig {
    // Default settings
    #[serde(default = "default_temperature")]
    temperature: f32,
    
    #[serde(default = "default_model")]
    model: String,
    
    // Add more settings as needed
}

fn default_temperature() -> f32 { 0.7 }
fn default_model() -> String { "claude-3-opus".to_string() }

impl Default for A2aConfig {
    fn default() -> Self {
        Self {
            temperature: default_temperature(),
            model: default_model(),
        }
    }
}

impl A2aConfig {
    fn load() -> Self {
        let config_path = PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".a2a_repl.toml");
            
        if let Ok(content) = fs::read_to_string(&config_path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }
    
    fn save(&self) -> std::io::Result<()> {
        let config_path = PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".a2a_repl.toml");
            
        let toml_string = toml::to_string(self).unwrap_or_default();
        fs::write(config_path, toml_string)
    }
    
    fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "temperature" => {
                self.temperature = value.parse::<f32>()
                    .map_err(|_| "Invalid temperature value".to_string())?;
            },
            "model" => {
                self.model = value.to_string();
            },
            // Add more settings
            _ => return Err(format!("Unknown setting: {}", key)),
        }
        self.save().map_err(|e| format!("Failed to save config: {}", e))?;
        Ok(())
    }
}
```

## 7. Better Help & Onboarding

For better help documentation:

```rust
fn show_help(full: bool) {
    if full {
        println!("{}", "A2A REPL Help".blue().bold());
        println!("=============\n");
        
        println!("{}", "Basic Commands:".yellow().bold());
        println!("  {} - Show this help message", "help".green());
        println!("  {} - Exit the REPL", "exit".green());
        println!("  {} - Show command history", "history".green());
        println!("  {} - Clear command history", "history -c".green());
        
        println!("\n{}", "Agent Commands:".yellow().bold());
        println!("  {} - List available agents", "agents".green());
        // Add more detailed help information
        
        // ... more categories
    } else {
        println!("Type {} for a full list of commands", "help full".green());
        println!("Common commands: {}, {}, {}", 
            "help".green(), 
            "exit".green(), 
            "history".green());
    }
}

fn detect_first_launch() -> bool {
    let history_path = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".a2a_history");
    !history_path.exists()
}

fn show_first_time_tips() {
    println!("{}", "Welcome to A2A REPL!".blue().bold());
    println!("Here are some tips to get you started:\n");
    
    println!("- Use {} to see available commands", "help".green());
    println!("- Your command history will be saved between sessions");
    println!("- Press {} for command history", "Up/Down".yellow());
    println!("- Press {} to search through history", "Ctrl+R".yellow());
    println!("- Command auto-completion is available with {}",  "Tab".yellow());
    
    println!("\nType {} to begin", "help".green());
}
```

## 8. Error Surfacing

For better error handling:

```rust
enum A2aError {
    Network(String),
    Timeout(String),
    Authentication(String),
    ClientError(String),
    Unknown(String),
}

impl A2aError {
    fn display(&self) -> String {
        match self {
            A2aError::Network(msg) => format!("🔌 Network error: {}\nCheck your internet connection or VPN status.", msg).red().to_string(),
            A2aError::Timeout(msg) => format!("⏱️ Request timed out: {}\nTry again or use a simpler request.", msg).red().to_string(),
            A2aError::Authentication(msg) => format!("🔑 Authentication error: {}\nTry 'discover --auth' to set credentials.", msg).red().to_string(),
            A2aError::ClientError(msg) => format!("❌ Client error: {}", msg).red().to_string(),
            A2aError::Unknown(msg) => format!("⚠️ Unknown error: {}", msg).red().to_string(),
        }
    }
    
    fn from_client_error(err: ClientError) -> Self {
        match err {
            ClientError::Network(e) => A2aError::Network(e.to_string()),
            ClientError::Timeout => A2aError::Timeout("Request timed out".to_string()),
            ClientError::A2aError(status, msg) if status == 401 => 
                A2aError::Authentication(msg),
            ClientError::A2aError(_, msg) => A2aError::ClientError(msg),
            _ => A2aError::Unknown(format!("{:?}", err)),
        }
    }
}
```

## 9. Code Organization Clean-ups

Here's a suggested structure for better code organization:

```rust
// history_service.rs
pub struct HistoryService {
    /* ... */
}

impl HistoryService {
    pub fn new() -> Self { /* ... */ }
    pub fn add_entry(&mut self, entry: &str) { /* ... */ }
    pub fn clear(&mut self) { /* ... */ }
    pub fn list(&self, count: Option<usize>) { /* ... */ }
    pub fn save(&self) -> Result<(), io::Error> { /* ... */ }
    pub fn load(&mut self) -> Result<(), io::Error> { /* ... */ }
}

// ui_renderer.rs
pub struct UiRenderer {
    /* ... */
}

impl UiRenderer {
    pub fn new() -> Self { /* ... */ }
    pub fn print_success(&self, msg: &str) { /* ... */ }
    pub fn print_warning(&self, msg: &str) { /* ... */ }
    pub fn print_error(&self, msg: &str) { /* ... */ }
    pub fn render_prompt(&self, agents: usize, tasks: usize) -> String { /* ... */ }
    pub fn render_spinner(&self, msg: &str) -> ProgressBar { /* ... */ }
}

// action_executor.rs
pub struct ActionExecutor {
    /* ... */
}

impl ActionExecutor {
    pub fn new(/* ... */) -> Self { /* ... */ }
    pub fn execute_command(&self, cmd: &str, args: &[&str]) -> Result<(), A2aError> { /* ... */ }
    pub fn handle_task(&self, task_id: &str) -> Result<(), A2aError> { /* ... */ }
}

// logger.rs
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

pub struct Logger {
    /* ... */
}

impl Logger {
    pub fn new() -> Self { /* ... */ }
    pub fn log(&self, level: LogLevel, msg: &str) { /* ... */ }
    pub fn debug(&self, msg: &str) { self.log(LogLevel::Debug, msg); }
    pub fn info(&self, msg: &str) { self.log(LogLevel::Info, msg); }
    pub fn warn(&self, msg: &str) { self.log(LogLevel::Warning, msg); }
    pub fn error(&self, msg: &str) { self.log(LogLevel::Error, msg); }
}
```

## 10. Testability / CI Demos

For adding golden file tests and a CI-friendly test script:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    
    #[test]
    fn test_interpret_input_help() {
        let input = "help";
        let output = interpret_input(input);
        
        let expected = fs::read_to_string("tests/golden/help.txt")
            .expect("Failed to read golden file");
            
        assert_eq!(output, expected);
    }
    
    #[test]
    fn test_interpret_input_agents_list() {
        let input = "agents list";
        let output = interpret_input(input);
        
        let expected = fs::read_to_string("tests/golden/agents_list.txt")
            .expect("Failed to read golden file");
            
        assert_eq!(output, expected);
    }
    
    // Add more tests
}
```

For CI integration, you can create a script that exercises the REPL:

```bash
#!/bin/bash
# test_repl.sh - Record a asciinema session for CI verification

# Install asciinema if needed 
command -v asciinema >/dev/null 2>&1 || { 
    echo "Installing asciinema...";
    pip install asciinema; 
}

# Start recording
asciinema rec --command="./target/debug/a2a_repl" ci_test.cast << EOF
help
agents list
exit
EOF

# Verify the recorded session contains expected output
if grep -q "Available agents" ci_test.cast; then
    echo "REPL test passed!"
    exit 0
else
    echo "REPL test failed: missing expected output"
    exit 1
fi
```

## Implementation Priority

As suggested in the original document, for quick wins I recommend implementing:

1. Persistent history with load_history/save_history
2. Color output using owo-colors
3. Auto-completion for commands, tools, and agent names
4. Replace polling dots with a spinner from indicatif

These changes will significantly improve the user experience with minimal effort, providing a foundation for the more complex enhancements.


------------------


Read through /home/elliot/Projects/a2a-test-suite/src/bidirectional_agent/mod.rs and make sure that the repl client is using all of the features of the bidirectional client-server

Edit
I'll examine the bidirectional_agent/mod.rs file to check if the REPL client is using all available features of the bidirectional client-server architecture. Let me look at this file first and then analyze what might be missing in the REPL client implementation.


bash


Now let me check the REPL client implementation to see if it's using all available features from the bidirectional agent:


bash
Request

{
  `command`: `cat /home/elliot/Projects/a2a-test-suite/src/repl_client.rs`
}
Response

Command completed with status 0

STDOUT:
use anyhow::{Result, Context, anyhow, Error as AnyhowError};
use rustyline::{Editor, Config, EditMode};
use rustyline::error::ReadlineError;
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
                let code = a2a_err.code();
                match code {
                    401 | 403 => ReplError::auth(format!("A2A authentication error: {}", a2a_err.message())),
                    404 => ReplError::task(format!("A2A resource not found: {}", a2a_err.message())),
                    _ => ReplError::internal(format!("A2A error {}: {}", code, a2a_err.message())),
                }
            },
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
    
    // Create readline editor with config
    let mut rl = Editor::<(), rustyline::history::FileHistory>::with_config(
        Config::builder()
            .edit_mode(EditMode::Emacs)
            .history_ignore_dups(true)
            .max_history_size(1000)
            .build()
    )?;
    
    // Setup history file in user's home directory
    let history_path = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".a2a_repl_history");
    
    if let Err(err) = rl.load_history(&history_path) {
        // Only show error if it's not just that the file doesn't exist yet
        if !matches!(err, ReadlineError::Io(ref e) if e.kind() == std::io::ErrorKind::NotFound) {
            println!("Failed to load history: {}", err);
        }
    }
    
    println!("\n🤖 LLM Interface REPL ready. Type 'help' for assistance or 'exit' to quit.");
    
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
                        continue;
                    }
                    "tools" => {
                        list_available_tools(agent.clone()).await?;
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
                
                // Process the input with improved error handling
                match process_input(&line, cmd_id.clone(), agent.clone(), &llm_client, commands_history.clone()).await {
                    Ok(_) => {
                        // Processing completed successfully
                    },
                    Err(e) => {
                        // Convert anyhow error to ReplError for better presentation
                        let repl_error = match e.downcast::<ReplError>() {
                            Ok(repl_err) => repl_err,
                            Err(other_err) => ReplError::from(other_err),
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
                println!("Ctrl+C pressed, use 'exit' or 'quit' to exit");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Ctrl+D pressed, exiting");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
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
    
    // First, check if we know about this agent
    let agent_info = match agent.agent_registry.get(agent_id) {
        Some(info) => info,
        None => {
            let error_message = format!("❌ Agent not found in registry. Use 'agents' to list known agents.");
            println!("{}", error_message);
            return Err(anyhow!(error_message));
        }
    };
    
    // Create client for streaming
    let mut client = A2aClient::new(&agent_info.card.url);
    
    // Enable streaming with mock delay of 1 second for testing
    // In real usage, we'd use an empty metadata object
    let stream = client.send_task_subscribe_with_metadata_typed(
        message, 
        &json!({"_mock_chunk_delay_ms": 1000})
    ).await
    .context("Failed to create streaming task")?;
    
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
    
    // First, check if we know about this agent
    let agent_info = match agent.agent_registry.get(agent_id) {
        Some(info) => info,
        None => {
            let error_message = format!("❌ Agent not found in registry. Use 'agents' to list known agents.");
            println!("{}", error_message);
            return Err(anyhow!(error_message));
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
    
    // Send the task
    let mut client = A2aClient::new(&agent_info.card.url);
    let task_result = client.send_task(message).await
        .context("Failed to send task to agent")?;
    
    println!("✅ Task sent successfully. Task ID: {}", task_result.id);
    
    // Poll for results
    println!("🔄 Polling for task results...");
    
    let mut attempts = 0;
    let max_attempts = 30; // Poll for up to 30 seconds
    let mut result_summary = Vec::new();
    result_summary.push(format!("Task ID: {}", task_result.id));
    
    let mut client = A2aClient::new(&agent_info.card.url);
    
    while attempts < max_attempts {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        match client.get_task(&task_result.id).await {
            Ok(task) => {
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
                println!("\nError checking task status: {:?}", e);
                result_summary.push(format!("Error: {}", e));
                break;
            }
        }
        
        attempts += 1;
    }
    
    if attempts >= max_attempts {
        println!("\n⚠️ Task is still processing. You can check its status later with the task ID: {}", task_result.id);
        result_summary.push("Status: Still processing".to_string());
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
    println!("🔍 LLM Interface REPL Help:");
    println!("  - Type natural language commands to interact with the A2A agent network");
    println!("  - Special commands:");
    println!("    * help          - Show this help message");
    println!("    * exit/quit     - Exit the REPL");
    println!("    * agents        - List known agents");
    println!("    * tools         - List available tools");
    println!("    * history       - Show full command history");
    println!("    * history N     - Show last N entries from history");
    println!("    * history -c    - Clear command history");
    println!("  - Example commands:");
    println!("    * \"Run the shell tool to list files in the current directory\"");
    println!("    * \"Discover a new agent at http://example.com:8080\"");
    println!("    * \"Send a task to agent1 asking it to check the weather\"");
    println!("    * \"Stream a task to agent1 to generate a story in real-time\"");
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

STDERR:
After examining both the bidirectional_agent/mod.rs and repl_client.rs files, I can identify several areas where the REPL client could be enhanced to better utilize all the features of the bidirectional client-server architecture. Let me analyze what's currently implemented and what's missing:

Current Implementation Analysis
The current REPL client implements various features of the bidirectional agent, including:

Basic Connectivity:
Initializes and runs the bidirectional agent
Connects to agents using the A2aClient
Handles tasks via the agent registry
Task Management:
Can send tasks to remote agents
Supports streaming responses
Polls for task results
Agent Discovery:
Uses agent_registry.discover() to find new agents
Lists known agents with their details
Command Interpretation:
Uses LLM for natural language command interpretation (when available)
Falls back to rule-based parsing when LLM not available
History Management:
Tracks command history with timestamps and results
Provides history filtering and display options
Missing or Underutilized Features
Based on the bidirectional_agent/mod.rs file, here are key features not fully utilized in the current REPL client:

Remote Tool Integration:
The REPL client doesn't integrate with remote_tool_registry and remote_tool_executor components that are available when the bidir-delegate feature is enabled. These components allow discovery and execution of tools across agents in the network. 
Docs
Task Flow and Decomposition:
The REPL doesn't utilize task_flow features for complex task decomposition and delegation. These components (enabled with the bidir-delegate feature) could allow the REPL to break down complex instructions into multiple subtasks. 
Docs
Result Synthesis:
The result synthesis capabilities (ResultSynthesizer from result_synthesis module) for combining multiple subtask results isn't integrated, missing an opportunity for more sophisticated multi-agent workflows. 
Docs
LLM-Based Task Routing:
While the REPL uses LLM for command interpretation, it doesn't fully utilize the LlmTaskRouter for intelligent routing of tasks to the most appropriate tools or agents based on capabilities. 
Docs
Agent Directory Features:
The REPL client doesn't use the full capabilities of AgentDirectory for agent tracking and verification. This could provide better management of active vs. inactive agents in the network. 
Docs
Tool Discovery Loop:
The run_tool_discovery_loop and discover_remote_tools methods, which periodically scan for tools across agents, aren't integrated into the REPL's workflow. 
Docs
Notifications and Streaming:
The REPL client doesn't fully utilize the notification service for task updates from remote agents. 
Docs
Task Repository Extensions:
The TaskRepositoryExt functionality isn't fully leveraged to track task states and relationships between tasks. 
Docs
Implementation Recommendations
Based on these findings, here are specific recommendations to enhance the REPL client to better leverage all features of the bidirectional architecture:

Add Remote Tool Discovery and Execution:
rust
#[cfg(feature = "bidir-delegate")]
async fn discover_and_execute_remote_tools(
    agent: Arc<BidirectionalAgent>,
    tool_name: &str,
    params: Value,
    agent_id: &str,
) -> Result<String> {
    println!("🌐 Attempting to use remote tool: {} from agent: {}", tool_name, agent_id);
    
    // First check if we know about this agent and its tools
    if let Some(tool) = agent.remote_tool_registry.find_tool(agent_id, tool_name).await? {
        println!("✅ Found remote tool. Executing...");
        
        // Execute the remote tool
        match agent.remote_tool_executor.execute_remote_tool(
            agent_id, 
            tool_name,
            params
        ).await {
            Ok(result) => {
                println!("✅ Remote tool execution successful:");
                let result_str = serde_json::to_string_pretty(&result)?;
                println!("{}", result_str);
                Ok(result_str)
            }
            Err(e) => {
                let error_message = format!("❌ Remote tool execution failed: {}", e);
                println!("{}", error_message);
                Err(anyhow!(error_message))
            }
        }
    } else {
        let error_message = format!("❌ Remote tool not found: {} on agent {}", tool_name, agent_id);
        println!("{}", error_message);
        Err(anyhow!(error_message))
    }
}
Add Complex Task Decomposition and Flow:
rust
#[cfg(feature = "bidir-delegate")]
async fn decompose_task(
    agent: Arc<BidirectionalAgent>,
    message: &str,
) -> Result<String> {
    println!("🧩 Decomposing complex task...");
    
    // Use TaskFlow to create a plan
    let task_flow = TaskFlow::new(
        agent.client_manager.clone(),
        agent.agent_registry.clone(),
        agent.config.self_id.clone(),
    );
    
    // Generate a plan with LLM
    let plan = task_flow.create_plan(message).await?;
    println!("📋 Task decomposition plan:");
    for (i, task) in plan.tasks.iter().enumerate() {
        println!("  {}. {} -> {}", i+1, task.description, task.agent_id);
    }
    
    // Ask for confirmation
    println!("\nWould you like to execute this plan? (y/n)");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    if input.trim().eq_ignore_ascii_case("y") {
        println!("🚀 Executing plan...");
        let result = task_flow.execute_plan(&plan).await?;
        
        // Synthesize results
        println!("🔄 Synthesizing results...");
        let synthesizer = ResultSynthesizer::new();
        let final_result = synthesizer.synthesize(&result).await?;
        
        println!("✅ Task completed successfully!");
        println!("{}", final_result);
        Ok(final_result)
    } else {
        println!("❌ Plan execution cancelled");
        Ok("Plan cancelled".to_string())
    }
}
Add LLM-Based Task Routing:
rust
#[cfg(all(feature = "bidir-core", feature = "bidir-local-exec"))]
async fn route_task(
    agent: Arc<BidirectionalAgent>,
    message: &str,
) -> Result<String> {
    println!("🔀 Routing task to optimal handler...");
    
    // Use the LLM-based task router to decide where to send the task
    let routing_result = agent.task_router.route_task(message).await?;
    
    match routing_result {
        task_router::RoutingDecision::LocalTool { tool_name, params } => {
            println!("🔧 Routed to local tool: {}", tool_name);
            execute_local_tool(agent.clone(), &tool_name, params).await
        }
        task_router::RoutingDecision::RemoteAgent { agent_id, message } => {
            println!("🌐 Routed to remote agent: {}", agent_id);
            send_task_to_agent(agent.clone(), &agent_id, &message).await
        }
        task_router::RoutingDecision::Unknown => {
            println!("❓ Could not determine optimal routing");
            Ok("Could not automatically route task. Try specifying a specific agent or tool.".to_string())
        }
    }
}
Enhanced Agent Directory Integration:
rust
#[cfg(feature = "bidir-core")]
async fn list_active_agents(
    agent: Arc<BidirectionalAgent>,
) -> Result<()> {
    println!("🟢 Active Agents:");
    
    let active_agents = agent.agent_directory.get_active_agents().await?;
    
    if active_agents.is_empty() {
        println!("  No active agents found.");
    } else {
        for (i, agent_entry) in active_agents.iter().enumerate() {
            println!("{}. {} ({})", i+1, agent_entry.agent_id, agent_entry.url);
            println!("   Last active: {}", agent_entry.last_active);
            println!("   Verification status: {}", 
                    if agent_entry.verified { "✅ Verified" } else { "⚠️ Unverified" });
            println!();
        }
    }
    
    Ok(())
}
Manual Tool Discovery Command:
rust
#[cfg(feature = "bidir-delegate")]
async fn trigger_tool_discovery(
    agent: Arc<BidirectionalAgent>,
) -> Result<String> {
    println!("🔍 Scanning network for available tools...");
    
    match agent.discover_remote_tools().await {
        Ok(count) => {
            println!("✅ Tool discovery complete. Found tools from {} agents.", count);
            Ok(format!("Discovered tools from {} agents", count))
        }
        Err(e) => {
            let error_message = format!("❌ Tool discovery failed: {}", e);
            println!("{}", error_message);
            Err(anyhow!(error_message))
        }
    }
}
Add Command for Listing Available Remote Tools:
rust
#[cfg(feature = "bidir-delegate")]
async fn list_remote_tools(
    agent: Arc<BidirectionalAgent>,
) -> Result<()> {
    println!("🌐 Available Remote Tools:");
    
    let all_remote_tools = agent.remote_tool_registry.list_all_tools().await?;
    
    if all_remote_tools.is_empty() {
        println!("  No remote tools discovered yet.");
        println!("  Try 'discover tools' to scan for available tools.");
    } else {
        for (agent_id, tools) in all_remote_tools {
            println!("Agent: {}", agent_id);
            if tools.is_empty() {
                println!("  No tools available");
            } else {
                for (i, tool) in tools.iter().enumerate() {
                    println!("  {}. {}", i+1, tool.name);
                    println!("     Description: {}", tool.description);
                    println!("     Parameters: {:?}", tool.parameter_schema);
                    println!();
                }
            }
        }
    }
    
    Ok(())
}
Extend InterpretedAction to Support New Capabilities:
rust
enum InterpretedAction {
    // Existing actions...
    
    // New actions
    #[cfg(feature = "bidir-delegate")]
    ExecuteRemoteTool {
        agent_id: String,
        tool_name: String,
        params: Value,
    },
    #[cfg(feature = "bidir-delegate")]
    DecomposeTask {
        message: String,
    },
    #[cfg(feature = "bidir-core")]
    ListActiveAgents,
    #[cfg(feature = "bidir-delegate")]
    DiscoverTools,
    #[cfg(feature = "bidir-delegate")]
    ListRemoteTools,
    #[cfg(all(feature = "bidir-core", feature = "bidir-local-exec"))]
    RouteTask {
        message: String,
    },
}
Update LLM Prompt for Command Interpretation:
Expand the prompt to include the new capabilities
Include remote tools in the available tools section
Add examples for more complex workflows
Improved UI for Task Status Tracking:
rust
fn display_fancy_progress(task_id: &str, status: &str, progress: f32) {
    let bar_width = 30;
    let filled = (progress * bar_width as f32) as usize;
    let empty = bar_width - filled;
    
    print!("\r[");
    for _ in 0..filled {
        print!("=");
    }
    for _ in 0..empty {
        print!(" ");
    }
    print!("] {:.1}% - Task: {} - Status: {}", progress * 100.0, task_id, status);
    std::io::stdout().flush().unwrap();
}
Enhanced Help Command:
Update the help text to include new commands and examples
Add examples for complex workflows
Include information about which features are enabled in this build
Conclusion
The current REPL client provides a solid foundation but doesn't fully leverage all capabilities of the bidirectional architecture, particularly around tool discovery, task decomposition, and result synthesis. By implementing these recommendations, the REPL client can provide a more comprehensive and powerful interface to the bidirectional agent-agent network.

The most important improvements would be:

Remote tool discovery and execution
Complex task decomposition and delegation
Result synthesis for multi-step workflows
LLM-based task routing
These enhancements would transform the REPL from a simple command interpreter to a powerful orchestration interface that can coordinate complex workflows across multiple agents, making better use of the bidirectional client-server architecture.





