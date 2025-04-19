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
    },
    /// Get a task from an A2A server
    GetTask {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Task ID
        #[arg(short, long)]
        id: String,
    },
    /// Cancel a task
    CancelTask {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Task ID
        #[arg(short, long)]
        id: String,
    },
    /// Send a task and subscribe to streaming updates 
    StreamTask {
        /// URL of the A2A server
        #[arg(short, long)]
        url: String,
        /// Message to send
        #[arg(short, long)]
        message: String,
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
                ClientCommands::SendTask { url, message } => {
                    println!("Sending task to: {}", url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.send_task(message).await {
                            Ok(task) => println!("Task created: {}", serde_json::to_string_pretty(&task).unwrap()),
                            Err(e) => {
                                eprintln!("Error sending task: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                }
                ClientCommands::GetTask { url, id } => {
                    println!("Getting task from: {}", url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.get_task(id).await {
                            Ok(task) => println!("Task: {}", serde_json::to_string_pretty(&task).unwrap()),
                            Err(e) => {
                                eprintln!("Error getting task: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::CancelTask { url, id } => {
                    println!("Cancelling task: {}", id);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.cancel_task(id).await {
                            Ok(task_id) => println!("Successfully cancelled task: {}", task_id),
                            Err(e) => {
                                eprintln!("Error cancelling task: {}", e);
                                std::process::exit(1);
                            }
                        }
                    });
                },
                ClientCommands::StreamTask { url, message } => {
                    println!("Streaming task to: {}", url);
                    rt.block_on(async {
                        let mut client = client::A2aClient::new(url);
                        match client.send_task_subscribe(message).await {
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
                        match client.resubscribe_task(id).await {
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
                }
            }
        }
    }
}
