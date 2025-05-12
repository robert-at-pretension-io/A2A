#!/bin/bash
set -e

# Development script to generate self-signed certificates for local HTTPS testing
# Unlike the production setup_certbot.sh, this doesn't require a domain or public server

# Default directory where certificates will be stored
CERT_DIR="./certs"
DOMAIN="localhost"

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --domain)
      DOMAIN="$2"
      shift 2
      ;;
    --dir)
      CERT_DIR="$2"
      shift 2
      ;;
    *)
      echo "Unknown option: $1"
      echo "Usage: $0 [--domain domain] [--dir cert_directory]"
      exit 1
      ;;
  esac
done

echo "Generating self-signed certificates for $DOMAIN in $CERT_DIR"

# Create directory if it doesn't exist
mkdir -p "$CERT_DIR"

# Generate a private key
openssl genrsa -out "$CERT_DIR/privkey.pem" 2048

# Create a certificate signing request
openssl req -new -key "$CERT_DIR/privkey.pem" -out "$CERT_DIR/cert.csr" -subj "/CN=$DOMAIN"

# Create a self-signed certificate
openssl x509 -req -days 365 -in "$CERT_DIR/cert.csr" -signkey "$CERT_DIR/privkey.pem" -out "$CERT_DIR/fullchain.pem"

# Clean up the CSR as it's no longer needed
rm "$CERT_DIR/cert.csr"

echo "Self-signed certificates generated successfully in $CERT_DIR/"
echo
echo "To use these certificates with the agent, run:"
echo "CERTBOT_DOMAIN=$DOMAIN RUSTFLAGS=\"-A warnings\" SSL_CERT_DIR=$CERT_DIR cargo run -- bidirectional-agent"
echo
echo "Note: For development use only. Browsers will show a security warning."