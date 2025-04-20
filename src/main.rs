mod validator;
mod property_tests;
mod mock_server;
mod fuzzer;
mod types;
mod client;
#[cfg(test)]
mod client_tests;

use clap::{Parser, Subcommand};
use futures_util::StreamExt;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate A2A messages against the schema
    Validate {
        /// Path to the A2A message JSON file
        #[arg(short, long)]
        file: String,
    },
    /// Run property-based tests
    Test {
        /// Number of test cases to generate
        #[arg(short, long, default_value_t = 100)]
        cases: usize,
    },
    /// Start a mock A2A server
    Server {
        /// Port to listen on
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
    /// Run fuzzing on A2A message handlers
    Fuzz {
        /// Target to fuzz (e.g., "parse", "handle")
        #[arg(short, long)]
        target: String,
        /// Maximum time to fuzz (in seconds)
        #[arg(short, long, default_value_t = 60)]
        time: u64,
    },
    /// Client commands for interacting with A2A servers
    Client {
        /// Subcommand for the client
        #[command(subcommand)]
        command: ClientCommands,
    },
}

#[derive(Subcommand)]
enum ClientCommands {
    /// Get the agent card from a server
    GetAgentCard {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
    },
    /// Send a task to an A2A server
    SendTask {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Message to send
        #[arg(short, long)]
        message: String,
        /// Optional metadata as JSON string (e.g. '{"_mock_delay_ms": 2000}')
        #[arg(long)]
        metadata: Option<String>,
        /// Optional authentication header name (e.g., "Authorization", "X-API-Key")
        #[arg(long)]
        header: Option<String>,
        /// Optional authentication value (e.g., "Bearer token123")
        #[arg(long)]
        value: Option<String>,
    },
    /// Get a task from an A2A server
    GetTask {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Task ID
        #[arg(short, long)]
        id: String,
        /// Optional authentication header name (e.g., "Authorization", "X-API-Key")
        #[arg(long)]
        header: Option<String>,
        /// Optional authentication value (e.g., "Bearer token123")
        #[arg(long)]
        value: Option<String>,
    },
    /// Cancel a task
    CancelTask {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Task ID
        #[arg(short, long)]
        id: String,
        /// Optional authentication header name (e.g., "Authorization", "X-API-Key")
        #[arg(long)]
        header: Option<String>,
        /// Optional authentication value (e.g., "Bearer token123")
        #[arg(long)]
        value: Option<String>,
    },
    /// Send a task and subscribe to streaming updates 
    StreamTask {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Message to send
        #[arg(short, long)]
        message: String,
        /// Optional metadata as JSON string (e.g. '{"_mock_chunk_delay_ms": 1000}')
        #[arg(long)]
        metadata: Option<String>,
    },
    /// Resubscribe to an existing task's streaming updates
    ResubscribeTask {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Task ID
        #[arg(short, long)]
        id: String,
    },
    /// Set push notifications for a task
    SetPushNotification {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Task ID
        #[arg(short, long)]
        id: String,
        /// Webhook URL to receive push notifications
        #[arg(short, long)]
        webhook: String,
        /// Authentication scheme (e.g., Bearer)
        #[arg(short, long)]
        auth_scheme: Option<String>,
        /// Authentication token
        #[arg(short, long)]
        token: Option<String>,
    },
    /// Get push notification configuration for a task
    GetPushNotification {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Task ID
        #[arg(short, long)]
        id: String,
    },
    /// Get the state transition history for a task
    GetStateHistory {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Task ID
        #[arg(short, long)]
        id: String,
    },
    /// Get metrics about a task's state transitions
    GetStateMetrics {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Task ID
        #[arg(short, long)]
        id: String,
    },
    /// List all available skills from an agent
    ListSkills {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Optional tags to filter skills (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },
    /// Get detailed information about a specific skill
    GetSkillDetails {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Skill ID
        #[arg(short, long)]
        id: String,
    },
    /// Invoke a skill with parameters
    InvokeSkill {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Skill ID
        #[arg(short, long)]
        id: String,
        /// Message to send to the skill
        #[arg(short, long)]
        message: String,
        /// Optional input mode (e.g., text/plain, text/html)
        #[arg(short = 'n', long)]
        input_mode: Option<String>,
        /// Optional output mode (e.g., text/plain, image/png)
        #[arg(short = 'p', long)]
        output_mode: Option<String>,
    },
    /// Upload a file to the server
    UploadFile {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Path to the file to upload
        #[arg(short, long)]
        file: String,
        /// Optional task ID to associate with the file
        #[arg(short, long)]
        task_id: Option<String>,
    },
    /// Send a task with a file attachment
    SendTaskWithFile {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Message to send
        #[arg(short, long)]
        message: String,
        /// Path to the file to attach
        #[arg(short, long)]
        file_path: String,
    },
    /// Send a task with structured data
    SendTaskWithData {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Message to send
        #[arg(short, long)]
        message: String,
        /// JSON data to include
        #[arg(short, long)]
        data: String,
    },
    /// Download a file from the server
    DownloadFile {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// File ID to download
        #[arg(short, long)]
        id: String,
        /// Optional output path to save the file (defaults to the file's name in current directory)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// List files on the server
    ListFiles {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Optional task ID to filter files
        #[arg(short, long)]
        task_id: Option<String>,
    },
    /// Validate authentication with the server
    ValidateAuth {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Authentication header name (e.g., "Authorization", "X-API-Key")
        #[arg(long)]
        header: String,
        /// Authentication value (e.g., "Bearer token123")
        #[arg(short, long)]
        value: String,
    },
    /// Create a batch of tasks
    CreateBatch {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// List of tasks to create (comma-separated)
        #[arg(short, long)]
        tasks: String,
        /// Optional name for the batch
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Get details of a task batch
    GetBatch {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Batch ID
        #[arg(short, long)]
        id: String,
        /// Include full task details
        #[arg(long, default_value_t = false)]
        include_tasks: bool,
    },
    /// Get status summary of a task batch
    GetBatchStatus {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Batch ID
        #[arg(short, long)]
        id: String,
    },
    /// Cancel all tasks in a batch
    CancelBatch {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Batch ID
        #[arg(short, long)]
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Validate { file } => {
            println!("Validating A2A messages in file: {}", file);
            // Call validation module
            if let Err(error) = validator::validate_file(file) {
                eprintln!("{}", error);
                std::process::exit(1);
            }
        }
        Commands::Test { cases } => {
            println!("Running {} property-based tests...", cases);
            // Call property testing module
            property_tests::run_property_tests(*cases);
        }
        Commands::Server { port } => {
            println!("Starting mock A2A server on port {}...", port);
            // Call mock server module
            mock_server::start_mock_server(*port);
        }
        Commands::Fuzz { target, time } => {
            println!("Fuzzing {} for {} seconds...", target, time);
            // Call fuzzing module
            fuzzer::run_fuzzer(target, *time);
        }
        Commands::Client { command } => {
            // Create a runtime for async client commands
            let rt = tokio::runtime::Runtime::new().unwrap();
            
            match command {
                ClientCommands::GetAgentCard { url } => {
                    println!("Getting agent card from: {}", url);
                    rt.block_on(async {
                        let client = client::A2aClient::new(url);
                        match client.get_agent_card().await {
                            Ok(card) => println!("{}", serde_json::to_string_pretty(&card).unwrap()),
                            Err(e) => {
                                eprintln!("Error getting agent card: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                }
                ClientCommands::SendTask { url, message, metadata, header, value } => {
                    println!("Sending task to: {}", url);
                    if let Some(meta) = &metadata {
                        println!("Using metadata: {}", meta);
                    }
                    
                    rt.block_on(async {
                        // Create client with authentication if provided
                        let mut client = if let (Some(h), Some(v)) = (header, value) {
                            println!("Using authentication with header: {}", h);
                            client::A2aClient::new(url).with_auth(&h, &v)
                        } else {
                            client::A2aClient::new(url)
                        };
                        
                        // Use metadata if provided
                        let result = if let Some(meta) = metadata.as_deref() {
                            client.send_task_with_metadata(message, Some(meta)).await
                        } else {
                            client.send_task(message).await
                        };
                        
                        match result {
                            Ok(task) => println!("Task created: {}", serde_json::to_string_pretty(&task).unwrap()),
                            Err(e) => {
                                eprintln!("Error sending task: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                }
                ClientCommands::GetTask { url, id, header, value } => {
                    println!("Getting task from: {}", url);
                    rt.block_on(async {
                        // Create client with authentication if provided
                        let mut client = if let (Some(h), Some(v)) = (header, value) {
                            println!("Using authentication with header: {}", h);
                            client::A2aClient::new(url).with_auth(&h, &v)
                        } else {
                            client::A2aClient::new(url)
                        };
                        
                        match client.get_task(id).await {
                            Ok(task) => println!("Task: {}", serde_json::to_string_pretty(&task).unwrap()),
                            Err(e) => {
                                eprintln!("Error getting task: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::CancelTask { url, id, header, value } => {
                    println!("Cancelling task: {}", id);
                    rt.block_on(async {
                        // Create client with authentication if provided
                        let mut client = if let (Some(h), Some(v)) = (header, value) {
                            println!("Using authentication with header: {}", h);
                            client::A2aClient::new(url).with_auth(&h, &v)
                        } else {
                            client::A2aClient::new(url)
                        };
                        
                        match client.cancel_task(id).await {
                            Ok(task_id) => println!("Successfully cancelled task: {}", task_id),
                            Err(e) => {
                                eprintln!("Error cancelling task: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::StreamTask { url, message, metadata } => {
                    println!("Streaming task to: {}", url);
                    if let Some(meta) = &metadata {
                        println!("Using metadata: {}", meta);
                    }
                    
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        
                        // Use simple version without metadata for now
                        let stream_result = client.send_task_subscribe(message).await;
                        
                        match stream_result {
                            Ok(mut stream) => {
                                println!("Connected to streaming endpoint. Waiting for updates...");
                                
                                // Process the stream until it ends
                                while let Some(response) = stream.next().await {
                                    match response {
                                        Ok(client::streaming::StreamingResponse::Status(task)) => {
                                            println!("Status update: {} ({})", 
                                                     task.status.state, 
                                                     task.status.timestamp.unwrap_or_default());
                                        },
                                        Ok(client::streaming::StreamingResponse::Artifact(artifact)) => {
                                            // Extract text from artifact parts
                                            for part in &artifact.parts {
                                                if let crate::types::Part::TextPart(text_part) = part {
                                                    print!("{}", text_part.text);
                                                    // Flush stdout to show streaming content immediately
                                                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                                                }
                                            }
                                            
                                            if artifact.last_chunk.unwrap_or(false) {
                                                println!("\n[Last chunk received]");
                                            }
                                        },
                                        Ok(client::streaming::StreamingResponse::Final(task)) => {
                                            println!("\nTask completed with status: {}", task.status.state);
                                            break;
                                        },
                                        Err(e) => {
                                            eprintln!("Error in stream: {}", e);
                                            std::process::exit(1);
                                        }
                                    }
                                }
                                
                                // Return Ok result with explicit type annotation
                                Ok::<(), Box<std::io::Error>>(())
                            },
                            Err(e) => {
                                eprintln!("Error starting stream: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::ResubscribeTask { url, id } => {
                    println!("Resubscribing to task: {}", id);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        // Fixed version without using undefined metadata variable
                        let task_result = client.resubscribe_task(id).await;
                        
                        match task_result {
                            Ok(mut stream) => {
                                println!("Connected to streaming endpoint. Waiting for updates...");
                                
                                // Process the stream until it ends
                                while let Some(response) = stream.next().await {
                                    match response {
                                        Ok(client::streaming::StreamingResponse::Status(task)) => {
                                            println!("Status update: {} ({})", 
                                                     task.status.state, 
                                                     task.status.timestamp.unwrap_or_default());
                                        },
                                        Ok(client::streaming::StreamingResponse::Artifact(artifact)) => {
                                            // Extract text from artifact parts
                                            for part in &artifact.parts {
                                                if let crate::types::Part::TextPart(text_part) = part {
                                                    print!("{}", text_part.text);
                                                    // Flush stdout to show streaming content immediately
                                                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                                                }
                                            }
                                            
                                            if artifact.last_chunk.unwrap_or(false) {
                                                println!("\n[Last chunk received]");
                                            }
                                        },
                                        Ok(client::streaming::StreamingResponse::Final(task)) => {
                                            println!("\nTask completed with status: {}", task.status.state);
                                            break;
                                        },
                                        Err(e) => {
                                            eprintln!("Error in stream: {}", e);
                                            std::process::exit(1);
                                        }
                                    }
                                }
                                
                                // Return Ok result with explicit type annotation
                                Ok::<(), Box<std::io::Error>>(())
                            },
                            Err(e) => {
                                eprintln!("Error resubscribing: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::SetPushNotification { url, id, webhook, auth_scheme, token } => {
                    println!("Setting push notification for task {} to webhook: {}", id, webhook);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.set_task_push_notification(id, webhook, auth_scheme.as_deref(), token.as_deref()).await {
                            Ok(task_id) => println!("Successfully set push notification for task: {}", task_id),
                            Err(e) => {
                                eprintln!("Error setting push notification: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::GetPushNotification { url, id } => {
                    println!("Getting push notification configuration for task: {}", id);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.get_task_push_notification(id).await {
                            Ok(config) => {
                                println!("Push notification configuration:");
                                println!("  URL: {}", config.url);
                                
                                if let Some(auth) = config.authentication {
                                    println!("  Authentication schemes: {:?}", auth.schemes);
                                    if let Some(creds) = auth.credentials {
                                        println!("  Credentials: {}", creds);
                                    }
                                }
                                
                                if let Some(token) = config.token {
                                    println!("  Token: {}", token);
                                }
                            },
                            Err(e) => {
                                eprintln!("Error getting push notification config: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::ListSkills { url, tags } => {
                    println!("Listing skills from: {}", url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        
                        // Parse comma-separated tags if provided
                        let tag_list = tags.as_ref().map(|t| {
                            t.split(',')
                                .map(|s| s.trim().to_string())
                                .collect::<Vec<String>>()
                        });
                        
                        match client.list_skills(tag_list).await {
                            Ok(response) => {
                                println!("Available skills:");
                                for (i, skill) in response.skills.iter().enumerate() {
                                    println!("{}. {} ({})", i+1, skill.name, skill.id);
                                    if let Some(desc) = &skill.description {
                                        println!("   Description: {}", desc);
                                    }
                                    if let Some(tags) = &skill.tags {
                                        println!("   Tags: {}", tags.join(", "));
                                    }
                                    println!("");
                                }
                                
                                if let Some(metadata) = response.metadata {
                                    if let Some(count) = metadata.get("total_count") {
                                        println!("Total skills: {}", count);
                                    }
                                }
                            },
                            Err(e) => {
                                eprintln!("Error listing skills: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::GetSkillDetails { url, id } => {
                    println!("Getting details for skill: {}", id);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.get_skill_details(id).await {
                            Ok(response) => {
                                println!("Skill: {} ({})", response.skill.name, response.skill.id);
                                
                                if let Some(desc) = &response.skill.description {
                                    println!("Description: {}", desc);
                                }
                                
                                if let Some(tags) = &response.skill.tags {
                                    println!("Tags: {}", tags.join(", "));
                                }
                                
                                if let Some(examples) = &response.skill.examples {
                                    println!("Examples:");
                                    for (i, example) in examples.iter().enumerate() {
                                        println!("  {}. {}", i+1, example);
                                    }
                                }
                                
                                if let Some(input_modes) = &response.skill.input_modes {
                                    println!("Input modes: {}", input_modes.join(", "));
                                }
                                
                                if let Some(output_modes) = &response.skill.output_modes {
                                    println!("Output modes: {}", output_modes.join(", "));
                                }
                            },
                            Err(e) => {
                                eprintln!("Error getting skill details: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::InvokeSkill { url, id, message, input_mode, output_mode } => {
                    println!("Invoking skill {} with message: {}", id, message);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.invoke_skill(id, message, input_mode.clone(), output_mode.clone()).await {
                            Ok(task) => {
                                println!("Skill invoked successfully!");
                                println!("Task ID: {}", task.id);
                                println!("Status: {}", task.status.state);
                                
                                // Display artifacts if any
                                if let Some(artifacts) = &task.artifacts {
                                    println!("\nArtifacts:");
                                    for (i, artifact) in artifacts.iter().enumerate() {
                                        println!("{}. {}", i+1, artifact.name.as_deref().unwrap_or("Unnamed artifact"));
                                        
                                        if let Some(desc) = &artifact.description {
                                            println!("   Description: {}", desc);
                                        }
                                        
                                        // Print text content if any
                                        for part in &artifact.parts {
                                            if let crate::types::Part::TextPart(text_part) = part {
                                                println!("\n{}", text_part.text);
                                            }
                                        }
                                        println!("");
                                    }
                                }
                            },
                            Err(e) => {
                                eprintln!("Error invoking skill: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::GetStateHistory { url, id } => {
                    println!("Getting state history for task: {}", id);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.get_task_state_history(id).await {
                            Ok(history) => {
                                println!("State transitions for task {}:", history.task_id);
                                for (i, transition) in history.transitions.iter().enumerate() {
                                    println!("{}. State: {}", i + 1, transition.state);
                                    println!("   Time: {}", transition.timestamp);
                                    
                                    // Show message if available
                                    if let Some(msg) = &transition.message {
                                        println!("   Message from: {}", msg.role);
                                        // Print text content if any
                                        for part in &msg.parts {
                                            if let crate::types::Part::TextPart(text_part) = part {
                                                let text = if text_part.text.len() > 50 {
                                                    format!("{}...", &text_part.text[..47])
                                                } else {
                                                    text_part.text.clone()
                                                };
                                                println!("   Text: {}", text);
                                            }
                                        }
                                    }
                                    println!("");
                                }
                            },
                            Err(e) => {
                                eprintln!("Error getting state history: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::GetStateMetrics { url, id } => {
                    println!("Getting state metrics for task: {}", id);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.get_state_transition_metrics(id).await {
                            Ok(metrics) => {
                                println!("Task metrics for {}:", metrics.task_id);
                                println!("Total transitions: {}", metrics.total_transitions);
                                
                                // Show duration if available
                                if let Some(duration) = metrics.duration {
                                    println!("Duration: {}ms", duration);
                                    if let (Some(start), Some(end)) = (metrics.start_time, metrics.end_time) {
                                        println!("Started at: {}", start);
                                        println!("Ended at: {}", end);
                                    }
                                }
                                
                                // Show state counts
                                println!("\nState counts:");
                                println!("  Submitted: {}", metrics.submitted_count);
                                println!("  Working: {}", metrics.working_count);
                                println!("  Input Required: {}", metrics.input_required_count);
                                println!("  Completed: {}", metrics.completed_count);
                                println!("  Canceled: {}", metrics.canceled_count);
                                println!("  Failed: {}", metrics.failed_count);
                                if metrics.unknown_count > 0 {
                                    println!("  Unknown: {}", metrics.unknown_count);
                                }
                            },
                            Err(e) => {
                                eprintln!("Error getting state metrics: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::UploadFile { url, file, task_id } => {
                    println!("Uploading file {} to {}", file, url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        
                        // Create metadata with task ID if provided
                        let metadata = if let Some(tid) = &task_id {
                            let mut map = serde_json::Map::new();
                            map.insert("taskId".to_string(), serde_json::Value::String(tid.clone()));
                            Some(map)
                        } else {
                            None
                        };
                        
                        match client.upload_file(file, metadata).await {
                            Ok(response) => {
                                println!("File uploaded successfully!");
                                println!("File ID: {}", response.file_id);
                                println!("Name: {}", response.name);
                                println!("MIME Type: {}", response.mime_type);
                                println!("Size: {} bytes", response.size);
                                println!("URI: {}", response.uri);
                                println!("Uploaded at: {}", response.uploaded_at);
                            },
                            Err(e) => {
                                eprintln!("Error uploading file: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::DownloadFile { url, id, output } => {
                    println!("Downloading file {} from {}", id, url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.download_file(id).await {
                            Ok(response) => {
                                println!("File details:");
                                println!("  ID: {}", response.file_id);
                                println!("  Name: {}", response.name);
                                println!("  MIME Type: {}", response.mime_type);
                                println!("  Size: {} bytes", response.size);
                                
                                // Decode the base64 content
                                let decoded = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &response.bytes) {
                                    Ok(bytes) => bytes,
                                    Err(e) => {
                                        eprintln!("Error decoding file content: {}", e);
                                        std::process::exit(1);
                                    }
                                };
                                
                                // Determine output path
                                let output_path = match &output {
                                    Some(path) => std::path::PathBuf::from(path),
                                    None => std::path::PathBuf::from(&response.name),
                                };
                                
                                // Write file to disk
                                if let Err(e) = std::fs::write(&output_path, decoded) {
                                    eprintln!("Error writing file to disk: {}", e);
                                    std::process::exit(1);
                                }
                                
                                println!("File saved to: {}", output_path.display());
                            },
                            Err(e) => {
                                eprintln!("Error downloading file: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::ListFiles { url, task_id } => {
                    if let Some(tid) = &task_id {
                        println!("Listing files for task {} from {}", tid, url);
                    } else {
                        println!("Listing all files from {}", url);
                    }
                    
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.list_files(task_id.as_deref()).await {
                            Ok(files) => {
                                if files.is_empty() {
                                    println!("No files found.");
                                    return;
                                }
                                
                                println!("Files:");
                                for (i, file) in files.iter().enumerate() {
                                    println!("{}. {} ({})", i+1, file.name, file.file_id);
                                    println!("   MIME Type: {}", file.mime_type);
                                    println!("   Size: {} bytes", file.size);
                                    println!("   Uploaded at: {}", file.uploaded_at);
                                    println!("   URI: {}", file.uri);
                                    println!("");
                                }
                            },
                            Err(e) => {
                                eprintln!("Error listing files: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::ValidateAuth { url, header, value } => {
                    println!("Validating authentication with server: {}", url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url)
                            .with_auth(&header, &value);
                        
                        match client.validate_auth().await {
                            Ok(valid) => {
                                if valid {
                                    println!("✅ Authentication validated successfully!");
                                } else {
                                    println!("❌ Authentication rejected by server.");
                                    std::process::exit(1);
                                }
                            },
                            Err(e) => {
                                eprintln!("❌ Error validating authentication: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::SendTaskWithFile { url, message, file_path } => {
                    println!("Sending task with file attachment {} to {}", file_path, url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.send_task_with_file(message, file_path).await {
                            Ok(task) => {
                                println!("Task created: {}", serde_json::to_string_pretty(&task).unwrap());
                            },
                            Err(e) => {
                                eprintln!("Error sending task with file: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::SendTaskWithData { url, message, data } => {
                    println!("Sending task with structured data to {}", url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        
                        // Parse the JSON data
                        let data_value: serde_json::Value = match serde_json::from_str(&data) {
                            Ok(value) => value,
                            Err(e) => {
                                eprintln!("Error parsing JSON data: {}", e);
                                std::process::exit(1);
                            }
                        };
                        
                        match client.send_task_with_data(message, &data_value).await {
                            Ok(task) => {
                                println!("Task created: {}", serde_json::to_string_pretty(&task).unwrap());
                            },
                            Err(e) => {
                                eprintln!("Error sending task with data: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::CreateBatch { url, tasks, name } => {
                    println!("Creating task batch in {}", url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        
                        // Parse comma-separated tasks
                        let task_list = tasks.split(',')
                            .map(|s| s.trim().to_string())
                            .collect::<Vec<String>>();
                            
                        // Create batch parameters
                        let params = client::task_batch::BatchCreateParams {
                            id: None, // Auto-generate ID
                            name: name.clone(),
                            tasks: task_list,
                            metadata: None,
                        };
                        
                        match client.create_task_batch(params).await {
                            Ok(batch) => {
                                println!("Batch created successfully!");
                                println!("Batch ID: {}", batch.id);
                                println!("Tasks: {}", batch.task_ids.len());
                                if let Some(batch_name) = batch.name {
                                    println!("Name: {}", batch_name);
                                }
                                println!("Created at: {}", batch.created_at);
                            },
                            Err(e) => {
                                eprintln!("Error creating batch: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::GetBatch { url, id, include_tasks } => {
                    println!("Getting batch {} from {}", id, url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.get_batch(id, *include_tasks).await {
                            Ok(batch) => {
                                println!("Batch ID: {}", batch.id);
                                if let Some(name) = batch.name {
                                    println!("Name: {}", name);
                                }
                                println!("Created at: {}", batch.created_at);
                                println!("Tasks: {} total", batch.task_ids.len());
                                
                                // Show task IDs
                                for (i, task_id) in batch.task_ids.iter().enumerate() {
                                    println!("  {}. {}", i+1, task_id);
                                }
                                
                                // If include_tasks was requested, get all task details
                                if *include_tasks {
                                    println!("\nFetching detailed task information...");
                                    match client.get_batch_tasks(&batch.id).await {
                                        Ok(tasks) => {
                                            for (i, task) in tasks.iter().enumerate() {
                                                println!("\nTask {}: {}", i+1, task.id);
                                                println!("  Status: {}", task.status.state);
                                                if let Some(artifacts) = &task.artifacts {
                                                    println!("  Artifacts: {}", artifacts.len());
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            eprintln!("Error fetching detailed task information: {}", e);
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                eprintln!("Error getting batch: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::GetBatchStatus { url, id } => {
                    println!("Getting status for batch {} from {}", id, url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.get_batch_status(id).await {
                            Ok(status) => {
                                println!("Batch status for {}", status.batch_id);
                                println!("Overall status: {:?}", status.overall_status);
                                println!("Total tasks: {}", status.total_tasks);
                                println!("Status breakdown:");
                                println!("  Submitted: {}", status.state_counts.get(&crate::types::TaskState::Submitted).unwrap_or(&0));
                                println!("  Working: {}", status.state_counts.get(&crate::types::TaskState::Working).unwrap_or(&0));
                                println!("  Input Required: {}", status.state_counts.get(&crate::types::TaskState::InputRequired).unwrap_or(&0));
                                println!("  Completed: {}", status.state_counts.get(&crate::types::TaskState::Completed).unwrap_or(&0));
                                println!("  Canceled: {}", status.state_counts.get(&crate::types::TaskState::Canceled).unwrap_or(&0));
                                println!("  Failed: {}", status.state_counts.get(&crate::types::TaskState::Failed).unwrap_or(&0));
                                
                                println!("Status checked at: {}", status.timestamp);
                            },
                            Err(e) => {
                                eprintln!("Error getting batch status: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::CancelBatch { url, id } => {
                    println!("Canceling all tasks in batch {} from {}", id, url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.cancel_batch(id).await {
                            Ok(status) => {
                                println!("Batch cancellation completed.");
                                println!("New status: {:?}", status.overall_status);
                                println!("Canceled: {}", status.state_counts.get(&crate::types::TaskState::Canceled).unwrap_or(&0));
                                println!("Failed to cancel: {}", 
                                    status.total_tasks - status.state_counts.get(&crate::types::TaskState::Canceled).unwrap_or(&0));
                            },
                            Err(e) => {
                                eprintln!("Error canceling batch: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                }
            }
        }
    }
}
