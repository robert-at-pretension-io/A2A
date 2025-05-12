use crate::bidirectional::agent_helpers;
use crate::bidirectional::bidirectional_agent::BidirectionalAgent;
use anyhow::{anyhow, Result};
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode};
use std::convert::Infallible;
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, instrument, trace, warn};
use rustls::ServerConfig;
use rustls_pemfile::{certs, pkcs8_private_keys};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// Check if TLS certificates are available at the standard certbot paths
pub fn check_certbot_certs(domain: &str) -> Option<(PathBuf, PathBuf)> {
    let cert_path = PathBuf::from(format!("/etc/letsencrypt/live/{}/fullchain.pem", domain));
    let key_path = PathBuf::from(format!("/etc/letsencrypt/live/{}/privkey.pem", domain));
    
    if cert_path.exists() && key_path.exists() {
        debug!("Found TLS certificates at {:?} and {:?}", cert_path, key_path);
        Some((cert_path, key_path))
    } else {
        debug!("TLS certificates not found at standard certbot paths for domain: {}", domain);
        None
    }
}

/// Load TLS certificates from a given path
pub fn load_tls_config(cert_path: &Path, key_path: &Path) -> Result<ServerConfig> {
    debug!("Loading TLS certificates from {:?} and {:?}", cert_path, key_path);
    
    // Load certificate chain
    let cert_file = File::open(cert_path)
        .map_err(|e| anyhow!("Failed to open certificate file: {}", e))?;
    let mut cert_reader = BufReader::new(cert_file);
    let cert_data = certs(&mut cert_reader)
        .map_err(|e| anyhow!("Failed to parse certificate: {}", e))?;
    
    if cert_data.is_empty() {
        return Err(anyhow!("No certificates found in provided certificate file"));
    }
    
    // Convert to rustls Certificate objects
    let cert_chain: Vec<rustls::Certificate> = cert_data
        .into_iter()
        .map(rustls::Certificate)
        .collect();
    
    // Load private key
    let key_file = File::open(key_path)
        .map_err(|e| anyhow!("Failed to open private key file: {}", e))?;
    let mut key_reader = BufReader::new(key_file);
    let mut key_data = pkcs8_private_keys(&mut key_reader)
        .map_err(|e| anyhow!("Failed to parse private key: {}", e))?;
    
    if key_data.is_empty() {
        return Err(anyhow!("No private keys found in provided key file"));
    }
    
    // Convert to rustls PrivateKey
    let private_key = rustls::PrivateKey(key_data.remove(0));
    
    // Create TLS config
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)
        .map_err(|e| anyhow!("Failed to create TLS server config: {}", e))?;
    
    Ok(config)
}

/// Run the agent with HTTP - HTTPS is now handled by Nginx
///
/// NOTE: Direct TLS support has been removed due to issues with the TLS handshake implementation.
/// It is now recommended to use Nginx as a reverse proxy with TLS termination.
/// See the README.md section on "HTTPS Setup with Nginx" for details.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id, port = %agent.port, bind_address = %agent.bind_address))]
pub async fn run_server_with_https(agent: &BidirectionalAgent, domain: &str) -> Result<()> {
    // Previously this function would attempt to handle HTTPS directly,
    // but this caused issues with the TLS handshake implementation.
    // We now recommend using Nginx as a reverse proxy for HTTPS termination.
    info!("HTTPS support via direct TLS is disabled. Falling back to HTTP.");
    info!("For production use with HTTPS, configure Nginx as a reverse proxy (see README.md).");
    agent_helpers::run_server(agent).await
}

// The TlsHyperAcceptor has been removed as part of the TLS handling removal.
// This code previously had issues with connection handling during TLS handshakes,
// leading to connections stalling indefinitely.
//
// For HTTPS support, we now recommend using Nginx as a reverse proxy for TLS termination.