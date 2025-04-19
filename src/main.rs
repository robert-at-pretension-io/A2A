mod validator;
mod property_tests;
mod mock_server;
mod fuzzer;
mod types;

use clap::{Parser, Subcommand};

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
    }
}
