mod validator;
mod property_tests;
mod mock_server;
mod fuzzer;
mod types;
mod client;
mod schema_utils; // Add this line
mod runner; // Add the runner module
mod server; // Add the reference server module
// Re-enable bidirectional_agent module
#[cfg(feature = "bidir-core")]
pub mod bidirectional_agent;
#[cfg(test)]
mod client_tests;

use std::time::Duration; // Add Duration import
use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use std::fs; // Add this
use std::io::{self, BufRead, Write}; // Add this
use std::path::{Path, PathBuf}; // Add this
use std::sync::Arc; // For reference server
use std::process::Command; // Keep this for now
use sha2::{Digest, Sha256}; // For hash calculation

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
    /// Start a reference A2A server implementation
    ReferenceServer {
        /// Port to listen on
        #[arg(short, long, default_value_t = 8081)]
        port: u16,
    },
    /// Run fuzzing on A2A message handlers
    Fuzz {
        /// Target to fuzz (e.g., "parse", "handle")
        #[arg(short, long)]
        target: String,
        /// Maximum time to fuzz (in seconds)
        #[arg(short = 'd', long, default_value_t = 60)]
        time: u64,
    },
    /// Run the integration test suite
    RunTests {
        /// Optional URL of the target A2A server (starts local mock server if omitted)
        #[arg(short, long)]
        url: Option<String>,
        // Removed run_unofficial flag
        /// Timeout for each individual test step in seconds
        #[arg(long, default_value_t = 15)]
        timeout: u64,
    },
    /// Manage configuration settings
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Start a bidirectional A2A agent (requires 'bidir-core' feature)
    #[cfg(feature = "bidir-core")]
    BidirectionalAgent {
        /// Configuration file path
        #[arg(short, long, default_value = "bidirectional_agent.toml")]
        config: String,
        // Port binding will be handled internally based on config or defaults
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Set the active schema version to use for builds
    SetSchemaVersion {
        /// The version to set (e.g., "v1", "v2")
        #[arg(short, long)]
        version: String,
    },
    /// Check remote schema and download if different, without building
    CheckSchema,
    /// Generate types from the active schema
    GenerateTypes {
        /// Force regeneration even if types are up to date
        #[arg(short, long)]
        force: bool,
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
        Commands::ReferenceServer { port } => {
            println!("Starting reference A2A server on port {}...", port);
            
            // Create a runtime for the async server
            let rt = tokio::runtime::Runtime::new().unwrap();
            if let Err(e) = rt.block_on(async {
                // Use default settings for the reference server
                let bind_address = "127.0.0.1";
                let task_repo = Arc::new(server::repositories::task_repository::InMemoryTaskRepository::new());
                
                // Create services
                let task_service = Arc::new(server::services::task_service::TaskService::standalone(task_repo.clone()));
                let streaming_service = Arc::new(server::services::streaming_service::StreamingService::new(task_repo.clone()));
                let notification_service = Arc::new(server::services::notification_service::NotificationService::new(task_repo));
                
                // Create cancellation token for graceful shutdown
                let shutdown_token = tokio_util::sync::CancellationToken::new();
                
                // Start the server
                server::run_server(
                    *port,
                    bind_address,
                    task_service,
                    streaming_service,
                    notification_service,
                    shutdown_token
                ).await
            }) {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Fuzz { target, time } => {
            println!("Fuzzing {} for {} seconds...", target, time);
            // Call fuzzing module
            fuzzer::run_fuzzer(target, *time);
        }
        Commands::RunTests { url, timeout } => { // Removed run_unofficial
            // Create a runtime for the async test runner
            let rt = tokio::runtime::Runtime::new().unwrap();
            let config = runner::TestRunnerConfig {
                target_url: url.clone(),
                // Removed run_unofficial_tests field
                default_timeout: Duration::from_secs(*timeout),
            };

            // Run the tests (now only official tests)
            let result = rt.block_on(runner::run_integration_tests(config));

            // Set exit code based on result
            if result.is_err() {
                eprintln!("Test suite finished with errors.");
                std::process::exit(1);
            } else {
                println!("Test suite finished successfully.");
                std::process::exit(0);
            }
        }
        Commands::Config { command } => {
            match command {
                ConfigCommands::SetSchemaVersion { version } => {
                    match set_active_schema_version(version) {
                        Ok(_) => println!("✅ Successfully set active schema version to '{}' in a2a_schema.config", version),
                        Err(e) => {
                            eprintln!("❌ Error setting schema version: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
                ConfigCommands::CheckSchema => {
                    println!("🔎 Checking remote schema...");
                    match schema_utils::check_and_download_remote_schema() {
                        Ok(schema_utils::SchemaCheckResult::NewVersionSaved(path)) => {
                             println!("\n======================================================================");
                             println!("🚀 A new version of the A2A schema was detected and saved to:");
                             println!("   {}", path);
                             println!("👉 To use this new version, update 'a2a_schema.config' and run 'cargo run -- config generate-types'.");
                             println!("======================================================================\n");
                        },
                        Ok(schema_utils::SchemaCheckResult::NoChange) => {
                            println!("✅ Remote schema matches the active local version. No changes needed.");
                        },
                        Err(e) => {
                            eprintln!("❌ Error checking remote schema: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
                ConfigCommands::GenerateTypes { force } => {
                    if let Err(e) = generate_types_from_schema(*force) {
                        eprintln!("❌ Error generating types: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        // Handle the new BidirectionalAgent command
        #[cfg(feature = "bidir-core")]
        Commands::BidirectionalAgent { config } => {
            println!("🚀 Attempting to start Bidirectional A2A Agent...");
            // Create a runtime for the async agent
            let rt = tokio::runtime::Runtime::new().unwrap();
            // Run the agent using the updated function from the bidirectional_agent module
            // The port is now handled internally based on the config file
            if let Err(e) = rt.block_on(crate::bidirectional_agent::run(config)) {
                 eprintln!("❌ Bidirectional Agent failed to run: {:?}", e); // Use debug format for anyhow::Error
                 std::process::exit(1);
            }
             println!("🏁 Bidirectional Agent finished."); // Should ideally run indefinitely
        }
    }
}

// Constants used for schema handling
const SCHEMAS_DIR: &str = "schemas";
const CONFIG_FILE: &str = "a2a_schema.config";
const OUTPUT_PATH: &str = "src/types.rs";
const HASH_FILE_NAME_PREFIX: &str = "a2a_schema_";

/// Calculate a SHA256 hash for a string
fn calculate_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// Get active schema version and path from config file
fn get_active_schema_info() -> Result<(String, PathBuf), String> {
    let config_content = fs::read_to_string(CONFIG_FILE)
        .map_err(|e| format!("Failed to read config file '{}': {}", CONFIG_FILE, e))?;

    for line in config_content.lines() {
        if let Some(stripped) = line.trim().strip_prefix("active_version") {
            if let Some(version) = stripped.trim().strip_prefix('=') {
                let version_str = version.trim().trim_matches('"').to_string();
                if !version_str.is_empty() {
                    let filename = format!("a2a_schema_{}.json", version_str);
                    let path = Path::new(SCHEMAS_DIR).join(&filename);
                    return Ok((version_str, path));
                }
            }
        }
    }
    Err(format!("Could not find 'active_version = \"...\"' in {}", CONFIG_FILE))
}

/// Generate Rust types from the active schema
fn generate_types_from_schema(force: bool) -> Result<(), String> {
    // 1. Determine active schema
    let (active_version, active_schema_path) = get_active_schema_info()?;
    
    // 2. Check if schema file exists
    if !active_schema_path.exists() {
        return Err(format!("Active schema file not found: {}", active_schema_path.display()));
    }
    
    // 3. Read schema content
    let schema_content = fs::read_to_string(&active_schema_path)
        .map_err(|e| format!("Failed to read active schema '{}': {}", active_schema_path.display(), e))?;
    
    // 4. Determine if regeneration is needed
    let current_hash = calculate_hash(&schema_content);
    let output_path = Path::new(OUTPUT_PATH);
    let output_exists = output_path.exists();
    
    // Store hashes in current directory for simpler management
    let hash_dir = Path::new(".");
    let hash_file_name = format!("{}{}_hash.txt", HASH_FILE_NAME_PREFIX, active_version);
    let hash_file_path = hash_dir.join(hash_file_name);
    
    let previous_hash = fs::read_to_string(&hash_file_path).ok();
    let needs_regeneration = force || !output_exists || previous_hash.map_or(true, |ph| ph != current_hash);
    
    if needs_regeneration {
        println!("💡 Generating types from active schema (version '{}', path: {})...",
                 active_version, active_schema_path.display());
        
        // Ensure the output directory exists
        if let Some(parent) = output_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create output directory '{}': {}", parent.display(), e))?;
            }
        }
        
        // Run cargo typify
        let typify_status = Command::new("cargo")
            .args([
                "typify",
                "-o",
                OUTPUT_PATH,
                &active_schema_path.to_string_lossy(),
            ])
            .status()
            .map_err(|e| format!("Failed to execute cargo-typify command: {}. Is it installed?", e))?;
        
        if !typify_status.success() {
            return Err(format!(
                "Failed to run 'cargo typify' for schema '{}'. Is cargo-typify installed (`cargo install cargo-typify`) and in your PATH? Exit status: {}",
                active_schema_path.display(),
                typify_status
            ));
        }
        
        // Write new hash
        fs::write(&hash_file_path, &current_hash)
            .map_err(|e| format!("Warning: Failed to write new schema hash to '{}': {}", hash_file_path.display(), e))?;
        
        println!("✅ Successfully generated types into '{}' using schema version '{}'", OUTPUT_PATH, active_version);
    } else {
        println!("✅ Schema hash matches for active version '{}' and output exists. Skipping type generation. Use --force to regenerate.", active_version);
    }
    
    Ok(())
}

/// Reads a2a_schema.config, updates the active_version, and writes it back.
fn set_active_schema_version(new_version: &str) -> io::Result<()> {
    let config_path = Path::new("a2a_schema.config");
    let mut new_lines = Vec::new();
    let mut found = false;

    // Read the existing file line by line
    if config_path.exists() {
        let file = fs::File::open(&config_path)?;
        let reader = io::BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().starts_with("active_version") {
                // Replace the version part of the line
                new_lines.push(format!("active_version = \"{}\"", new_version));
                found = true;
            } else {
                // Keep other lines as they are
                new_lines.push(line);
            }
        }
    }

    // If the active_version line wasn't found, add it
    if !found {
        new_lines.push(format!("active_version = \"{}\"", new_version));
        // Add a comment if the file was empty or didn't have the line
        if new_lines.len() == 1 {
             new_lines.insert(0, "# Specifies the schema version to use for generating src/types.rs".to_string());
        }
    }

    // Write the modified content back to the file
    let mut file = fs::File::create(&config_path)?;
    for line in new_lines {
        writeln!(file, "{}", line)?;
    }

    Ok(())
}
