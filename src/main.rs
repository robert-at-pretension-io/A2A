mod validator;
mod property_tests;
mod mock_server;
mod fuzzer;
mod types;
mod client;
mod schema_utils; // Add this line
mod runner; // Add the runner module
#[cfg(test)]
mod client_tests;

use std::time::Duration; // Add Duration import
use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use std::fs; // Add this
use std::io::{self, BufRead, Write}; // Add this
use std::path::Path; // Add this
use std::process::Command; // Keep this for now

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
    /// Run the integration test suite
    RunTests {
        /// Optional URL of the target A2A server (starts local mock server if omitted)
        #[arg(short, long)]
        url: Option<String>,
        /// Include unofficial tests (specific to mock server or non-standard features)
        #[arg(long, default_value_t = false)]
        run_unofficial: bool,
        /// Timeout for each individual test step in seconds
        #[arg(long, default_value_t = 15)]
        timeout: u64,
    },
    /// Manage configuration settings
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
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
    // Removed ForceRefresh as CheckSchema provides the core functionality directly
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
        Commands::RunTests { url, run_unofficial, timeout } => {
            // Create a runtime for the async test runner
            let rt = tokio::runtime::Runtime::new().unwrap();
            let config = runner::TestRunnerConfig {
                target_url: url.clone(),
                run_unofficial_tests: *run_unofficial,
                default_timeout: Duration::from_secs(*timeout),
            };

            // Run the tests
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
                             println!("👉 To use this new version, update 'a2a_schema.config' and run 'cargo build'.");
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
                }
            }
        }
    }
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
