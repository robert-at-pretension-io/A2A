//! Interactive REPL (Read-Eval-Print Loop) for the Bidirectional Agent.

use anyhow::Result;
use std::io::{self, BufRead, Write};
use tokio::{self, /* sync::oneshot, time::sleep */}; // Removed unused
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, trace, warn};

use super::{agent_helpers, bidirectional_agent::BidirectionalAgent}; // Add agent_helpers
use crate::server::run_server;

// Import command handlers from the sibling module
mod commands;
use commands::*;

/// Run an interactive REPL (Read-Eval-Print Loop)
/// Takes a mutable reference to the agent to allow state changes.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub async fn run_repl(agent: &mut BidirectionalAgent) -> Result<()> {
    info!("Entering REPL mode. Type :help for commands."); // Updated info message
                                                           // Print the initial help message
    print_repl_help(agent); // Call the handler from commands module

    // --- Spawn Background Task for Initial Connection ---
    debug!("Checking if background connection task should be spawned.");
    if let Some(initial_url) = agent_helpers::client_url(agent) { // Call helper
        debug!(url = %initial_url, "Spawning background task for initial connection attempt."); // Changed to debug
        println!(
            "⏳ Configured target URL: {}. Attempting connection in background...",
            initial_url
        ); // Added emoji
           // Clone necessary data for the background task
           // Clone necessary data for the background task's tracing span
        let bg_agent_id = agent.agent_id.clone();
        let bg_initial_url = initial_url.clone();
        // Remove agent_directory clone
        let agent_registry = agent.agent_registry.clone(); // Canonical registry for execution
        let known_servers = agent.known_servers.clone(); // Clone Arc<DashMap>

        tokio::spawn(
            async move {
                // Create a tracing span for the background task
                let span = tracing::info_span!(
                    "bg_connect",
                    agent_id = %bg_agent_id,
                    target_url = %bg_initial_url
                );
                // Enter the span
                let _enter = span.enter();

                debug!("Background connection task started."); // Changed to debug

                let max_retries = 5;
                let retry_delay = std::time::Duration::from_secs(2);
                let mut connected = false;

                // Create a *new* client instance specifically for this background task
                debug!("Creating A2aClient for background connection.");
                let mut bg_client = crate::client::A2aClient::new(&initial_url); // Use full path

                for attempt in 1..=max_retries {
                    debug!(attempt, max_retries, "Attempting to get agent card in background.");
                    match bg_client.get_agent_card().await {
                        Ok(card) => {
                            let remote_agent_name = card.name.clone();
                            info!(remote_agent_name = %remote_agent_name, "Background connection successful."); // Keep info for success
                            println!(
                                "✅ Background connection successful to {} ({})",
                                remote_agent_name, initial_url
                            ); // Print to console as well

                            // Update canonical AgentRegistry via discover
                            debug!(url = %initial_url, "Attempting to update canonical AgentRegistry via discover.");
                            match agent_registry.discover(&initial_url).await {
                                Ok(discovered_agent_id) => {
                                    debug!(url = %initial_url, %discovered_agent_id, "Successfully updated canonical AgentRegistry."); // Changed to debug
                                                                                                                                       // Update known servers map with the ID from discover
                                    debug!(url = %initial_url, name = %discovered_agent_id, "Updating known servers map.");
                                    known_servers.insert(initial_url.clone(), discovered_agent_id);
                                }
                                Err(reg_err) => {
                                    // Log warning if updating canonical registry fails, but don't fail the connection
                                    warn!(error = %reg_err, url = %initial_url, "Failed to update canonical AgentRegistry after successful connection.");
                                    // Still update known_servers with the name from the card as a fallback
                                    debug!(url = %initial_url, name = %remote_agent_name, "Updating known servers map with card name as fallback.");
                                    known_servers
                                        .insert(initial_url.clone(), remote_agent_name.clone());
                                }
                            }

                            connected = true;
                            break; // Exit retry loop on success
                        }
                        Err(e) => {
                            warn!(attempt, max_retries, error = %e, "Background connection attempt failed.");
                            if attempt < max_retries {
                                debug!(delay_seconds = %retry_delay.as_secs(), "Retrying background connection..."); // Changed to debug
                                tokio::time::sleep(retry_delay).await;
                            } else {
                                error!(attempt, max_retries, "Final background connection attempt failed.");
                            }
                        }
                    }
                }

                if !connected {
                    error!(attempts = %max_retries, "Background connection failed after all attempts.");
                    println!(
                        "❌ Background connection to {} failed after {} attempts.",
                        initial_url, max_retries
                    ); // Print final failure to console
                } else {
                    debug!("Background connection task finished successfully."); // Changed to debug
                }
            }, // Background task ends here
        );
    } else {
        debug!("No initial target URL configured, skipping background connection task.");
    }
    // --- End Background Task Spawn ---

    // Flag to track if we have a listening server running
    let mut server_running = false;
    let mut server_shutdown_token: Option<CancellationToken> = None;
    // Removed misplaced match block and duplicate variable declarations here

    // Check if auto-listen should happen (based on config/env var checked during init)
    // We need to re-evaluate here as config isn't stored directly on self
    let should_auto_listen = agent.client_config.target_url.is_none() || // Auto-listen if no target URL
                                 std::env::var("AUTO_LISTEN").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false);
    // Note: We cannot check config.mode.auto_listen here easily as config is not stored.
    // Relying on env var set during init or absence of target_url.
    debug!(%should_auto_listen, "Determined auto-listen status for REPL.");

    // Start server automatically if auto-listen is enabled
    // Use the calculated should_auto_listen variable
    if should_auto_listen {
        info!(port = %agent.port, bind_address = %agent.bind_address, "Auto-starting server."); // Keep info for server start
        println!("🚀 Auto-starting server on port {}...", agent.port);

        // Create a cancellation token
        debug!("Creating cancellation token for auto-started server.");
        let token = CancellationToken::new();
        server_shutdown_token = Some(token.clone());

        // Create a channel to communicate server start status back to REPL
        debug!("Creating channel for server start status.");
        let (tx, rx) = tokio::sync::oneshot::channel();

        // Clone what we need for the task
        trace!("Cloning services and config for server task.");
        let task_service = agent.task_service.clone();
        let streaming_service = agent.streaming_service.clone();
        let notification_service = agent.notification_service.clone();
        let bind_address = agent.bind_address.clone();
        let port = agent.port;
        debug!("Creating agent card for auto-started server.");
        let agent_card = serde_json::to_value(agent.create_agent_card()).unwrap_or_else(|e| {
            // create_agent_card logs internally
            warn!(error = %e, "Failed to serialize agent card for server. Using empty object.");
            serde_json::json!({})
        });
        trace!(?agent_card, "Agent card JSON for server.");

        let agent_id_clone = agent.agent_id.clone(); // Clone for tracing span
        debug!("Spawning server task.");
        tokio::spawn(async move {
            // Create a tracing span for the server task
            let span = tracing::info_span!(
                "auto_server_task",
                agent_id = %agent_id_clone,
                %port,
                %bind_address
            );
            // Enter the span
            let _enter = span.enter();
            debug!("Auto-started server task running."); // Changed to debug

            match run_server(
                port,
                &bind_address,
                task_service,
                streaming_service,
                notification_service,
                token.clone(),
                Some(agent_card),
            )
            .await
            {
                Ok(handle) => {
                    debug!("Server task successfully started."); // Changed to debug
                                                                 // Send success status back to REPL
                    trace!("Sending success status back to REPL via channel.");
                    let _ = tx.send(Ok(())); // Ignore error if REPL already timed out

                    // Wait for the server to complete or be cancelled
                    debug!("Waiting for server task to complete or be cancelled."); // Changed to debug
                    match handle.await {
                        Ok(()) => info!("Server task shut down gracefully."), // Keep info for shutdown
                        Err(e) => error!(error = %e, "Server task join error."),
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to start server task.");
                    // Send error status back to REPL
                    trace!("Sending error status back to REPL via channel.");
                    let _ = tx.send(Err(format!("{}", e))); // Ignore error if REPL already timed out
                }
            }
        }); // Server task ends here

        // Wait for the server start status
        debug!("Waiting for server start status from channel.");
        let server_start_timeout = std::time::Duration::from_secs(5); // Slightly longer timeout
        match tokio::time::timeout(server_start_timeout, rx).await {
            Ok(Ok(Ok(()))) => {
                server_running = true;
                info!(bind_address = %agent.bind_address, port = %agent.port, "Auto-started server successfully confirmed by REPL."); // Keep info for confirmation
                println!(
                    "✅ Server started on http://{}:{}",
                    agent.bind_address, agent.port
                );
                println!("   (Server will run until you exit the REPL or send :stop)"); // Indented for clarity
            }
            Ok(Ok(Err(e))) => {
                error!(bind_address = %agent.bind_address, port = %agent.port, error = %e, "Error reported during auto-start server initialization.");
                println!("❌ Error starting server: {}", e);
                println!("The server could not be started. Try a different port or check for other services using this port.");
                server_shutdown_token = None; // Clean up token
            }
            Ok(Err(channel_err)) => {
                error!(bind_address = %agent.bind_address, port = %agent.port, error = %channel_err, "Server init channel error during auto-start (REPL side).");
                println!("❌ Error: Server initialization failed due to channel error");
                server_shutdown_token = None; // Clean up token
            }
            Err(timeout_err) => {
                error!(bind_address = %agent.bind_address, port = %agent.port, error = %timeout_err, "Timeout waiting for auto-start server confirmation.");
                println!("❌ Timeout waiting for server to start");
                println!("The server is taking too long to start. It might be starting in the background or could have failed.");
                println!("You can try :stop to cancel any server processes that might be running.");
                // Keep running flag true so user can try :stop, but clear token as we don't know its state
                server_running = true;
                server_shutdown_token = None;
                warn!("Server state unknown due to timeout, cleared shutdown token.");
            }
        }
    } else {
        debug!("Auto-start server disabled.");
    }

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut input = String::new();

    // Flag to track if we have a listening server running is now set above
    // let mut server_running = false; // Removed duplicate
    // let mut server_shutdown_token: Option<CancellationToken> = None; // Removed duplicate

    loop {
        // Display prompt (with connected agent information if available)
        let prompt = if let Some(url) = agent_helpers::client_url(agent) { // Call helper
            format!("agent@{} > ", url)
        } else {
            "agent > ".to_string()
        };
        trace!(prompt = %prompt, "Displaying REPL prompt.");
        print!("{}", prompt);
        io::stdout().flush().ok();

        input.clear();
        trace!("Reading line from stdin.");
        if reader.read_line(&mut input)? == 0 {
            // EOF reached (e.g., Ctrl+D)
            info!("EOF detected on stdin. Exiting REPL.");
            println!("\nEOF detected. Exiting REPL."); // Add newline for cleaner exit
                                                      // Perform cleanup similar to :quit
                                                      // REMOVED agent directory saving on exit

            if let Some(token) = server_shutdown_token.take() {
                info!("Shutting down server due to EOF.");
                println!("Shutting down server...");
                token.cancel();
                trace!("Server cancellation token cancelled.");
            } else {
                trace!("No server running or token available to cancel on EOF.");
            }
            break; // Exit loop
        }

        let input_trimmed = input.trim();
        debug!(user_input = %input_trimmed, "REPL input received.");

        if input_trimmed.is_empty() {
            trace!("Empty input received, continuing loop.");
            continue;
        }

        // --- Updated REPL Command Handling ---
        if input_trimmed.starts_with(":") {
            debug!(command = %input_trimmed, "Processing REPL command.");

            let parts: Vec<&str> = input_trimmed.splitn(2, ' ').collect();
            let command = parts[0].trim_start_matches(':').to_lowercase(); // Lowercase for matching
            let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            // Handle commands that need direct access to run_repl state (:listen, :stop, :quit)
            if command == "quit" {
                info!("User initiated :quit command.");
                println!("👋 Exiting REPL. Goodbye!");
                if let Some(token) = server_shutdown_token.take() {
                    info!("Shutting down server via :quit command.");
                    println!("🔌 Shutting down server...");
                    token.cancel();
                    trace!("Server cancellation token cancelled.");
                }
                break; // Exit loop
            } else if command == "listen" {
                debug!(port_str = %args, "Processing :listen command directly in run_repl.");
                if server_running {
                    warn!("Attempted to listen while server already running.");
                    println!("⚠️ Server already running. Stop it first with :stop");
                    continue;
                }
                match args.parse::<u16>() {
                    Ok(port) => {
                        agent.port = port; // Update agent's port config
                        let token = CancellationToken::new();
                        server_shutdown_token = Some(token.clone()); // Store token locally
                        info!(%port, "Attempting to start server.");
                        println!("🚀 Starting server on port {}...", port);

                        // Clone what we need for the task
                        let task_service = agent.task_service.clone();
                        let streaming_service = agent.streaming_service.clone();
                        let notification_service = agent.notification_service.clone();
                        let bind_address = agent.bind_address.clone();
                        let agent_card =
                            serde_json::to_value(agent.create_agent_card()).unwrap_or_else(|e| {
                                warn!("Failed to serialize agent card: {}", e);
                                serde_json::json!({})
                            });

                        // Create a channel to communicate server start status back to REPL
                        let (tx, rx) = tokio::sync::oneshot::channel();

                        tokio::spawn(async move {
                            match run_server(
                                port,
                                &bind_address,
                                task_service,
                                streaming_service,
                                notification_service,
                                token.clone(),
                                Some(agent_card),
                            )
                            .await
                            {
                                Ok(handle) => {
                                    let _ = tx.send(Ok(())); // Send success
                                    debug!(%port, "Server task started.");
                                    match handle.await {
                                        // Wait for server task completion
                                        Ok(()) => info!(%port, "Server shut down gracefully."),
                                        Err(e) => error!(%port, error = %e, "Server error."),
                                    }
                                }
                                Err(e) => {
                                    let error_msg = format!("Failed to start server: {}", e);
                                    let _ = tx.send(Err(error_msg.clone())); // Send error
                                    error!(%port, error = %e, "Failed to start server task.");
                                }
                            }
                        }); // Server task ends here

                        // Wait for the server start status
                        match tokio::time::timeout(std::time::Duration::from_secs(3), rx).await {
                            Ok(Ok(Ok(()))) => {
                                server_running = true; // Update local state
                                info!(%port, "Server started successfully via REPL command.");
                                println!(
                                    "✅ Server started on http://{}:{}",
                                    agent.bind_address, port
                                );
                                println!("   (Server will run until you exit the REPL or send :stop)");
                            }
                            Ok(Ok(Err(e))) => {
                                error!(%port, error = %e, "Error starting server via REPL command.");
                                println!("❌ Error starting server: {}", e);
                                println!("The server could not be started. Try a different port or check for other services using this port.");
                                server_shutdown_token = None; // Clean up token
                            }
                            Ok(Err(channel_err)) => {
                                error!(%port, error = %channel_err, "Server init channel error via REPL command.");
                                println!("❌ Error: Server initialization failed due to channel error");
                                server_shutdown_token = None; // Clean up token
                            }
                            Err(timeout_err) => {
                                error!(%port, error = %timeout_err, "Timeout waiting for server start via REPL command.");
                                println!("❌ Timeout waiting for server to start");
                                println!("The server is taking too long to start. It might be starting in the background or could have failed.");
                                println!("You can try :stop to cancel any server processes that might be running.");
                                server_running = true; // Keep running flag true so user can try :stop
                                                       // server_shutdown_token remains Some, allowing :stop attempt
                            }
                        }
                    }
                    Err(parse_err) => {
                        error!(input = %args, error = %parse_err, "Invalid port format for :listen command.");
                        println!("❌ Error: Invalid port number. Please provide a valid port.");
                    }
                }
            } else if command == "stop" {
                debug!("Processing :stop command directly in run_repl.");
                if let Some(token) = server_shutdown_token.take() {
                    // Take token from local variable
                    info!("Stopping server.");
                    println!("🔌 Shutting down server...");
                    token.cancel();
                    server_running = false; // Update local state
                    println!("✅ Server stop signal sent.");
                } else {
                    warn!("Attempted to stop server, but none was running or token missing.");
                    println!("⚠️ No server currently running or already stopped.");
                }
            } else {
                // Handle other commands via the central handler in commands module
                match handle_repl_command(agent, &command, args).await {
                    Ok(response) => println!("{}", response),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
        } else {
            // Input does not start with ':'
            // Process as local message (which now also checks for command keywords)
            debug!(message = %input_trimmed, "Processing input as local message/command.");
            match agent.process_message_locally(input_trimmed).await {
                Ok(response) => {
                    println!("\n🤖 Agent response:\n{}\n", response);
                    debug!(response_length = %response.len(), "Local processing successful, displayed response.");
                }
                Err(e) => {
                    let error_msg = format!("Error processing message: {}", e);
                    error!(error = %e, "Error processing message locally.");
                    println!("❌ {}", error_msg);
                }
            }
        }
        // --- End Updated REPL Command Handling ---
    }

    info!("Exiting REPL mode.");
    Ok(())
}
