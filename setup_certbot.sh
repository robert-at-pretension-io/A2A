#!/bin/bash
set -e

# Script to setup Certbot and configure TLS for the bidirectional agent
# This script must be run as root or with sudo

# Check if running as root
if [ "$EUID" -ne 0 ]; then
  echo "This script must be run as root or with sudo"
  exit 1
fi

# Get domain name
if [ -z "$1" ]; then
  read -p "Enter your domain name (e.g., agent.example.com): " DOMAIN
else
  DOMAIN="$1"
fi

# Check if certbot is installed
if ! command -v certbot &> /dev/null; then
  echo "Certbot not found. Installing certbot..."
  
  # Detect OS and install certbot accordingly
  if [ -f /etc/debian_version ]; then
    # Debian/Ubuntu
    apt-get update
    apt-get install -y certbot
  elif [ -f /etc/redhat-release ]; then
    # CentOS/RHEL
    dnf install -y certbot
  else
    echo "Unsupported OS. Please install Certbot manually."
    exit 1
  fi
  
  echo "Certbot installed successfully."
fi

# Obtain certificate
echo "Obtaining TLS certificate for $DOMAIN..."
certbot certonly --standalone -d "$DOMAIN" --agree-tos --email admin@"$DOMAIN" --no-eff-email

# Check if certificate was obtained successfully
if [ -d "/etc/letsencrypt/live/$DOMAIN" ]; then
  echo "Certificate obtained successfully!"
  
  # Create renewal hook script
  HOOK_DIR="/etc/letsencrypt/renewal-hooks/post"
  mkdir -p "$HOOK_DIR"
  
  HOOK_SCRIPT="$HOOK_DIR/restart_a2a_agent.sh"
  cat > "$HOOK_SCRIPT" << EOF
#!/bin/bash
# This script will be executed by Certbot after certificate renewal
# It restarts the a2a-agent service to load the new certificate

# Restart the service if it exists and is enabled
if systemctl is-enabled a2a-agent.service &> /dev/null; then
  systemctl restart a2a-agent.service
  echo "A2A agent service restarted with new certificate."
fi
EOF
  
  chmod +x "$HOOK_SCRIPT"
  echo "Created certificate renewal hook: $HOOK_SCRIPT"
  
  # Create service file if it doesn't exist
  SERVICE_FILE="/etc/systemd/system/a2a-agent.service"
  if [ ! -f "$SERVICE_FILE" ]; then
    
    # Find the project directory (assuming this script is in the project root)
    PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    
    cat > "$SERVICE_FILE" << EOF
[Unit]
Description=Agent-to-Agent Bidirectional Agent
After=network.target

[Service]
Type=simple
User=a2a
Group=a2a
WorkingDirectory=$PROJECT_DIR
Environment="CERTBOT_DOMAIN=$DOMAIN"
Environment="RUST_LOG=info"
ExecStart=$PROJECT_DIR/target/release/bidirectional-agent --listen
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
    
    echo "Created service file: $SERVICE_FILE"
    
    # Create a2a user if it doesn't exist
    if ! id "a2a" &>/dev/null; then
      useradd -r -s /bin/false a2a
      echo "Created a2a user for the service."
    fi
    
    # Set permissions for certbot certificates
    # Create certbot group if it doesn't exist
    if ! getent group certbot &>/dev/null; then
      groupadd -r certbot
      echo "Created certbot group."
    fi
    
    # Add a2a user to certbot group
    usermod -a -G certbot a2a
    
    # Set group ownership and permissions
    chgrp -R certbot /etc/letsencrypt/live
    chgrp -R certbot /etc/letsencrypt/archive
    chmod -R g+rx /etc/letsencrypt/live
    chmod -R g+rx /etc/letsencrypt/archive
    
    # Reload systemd and enable service
    systemctl daemon-reload
    systemctl enable a2a-agent.service
    
    # Provide instructions to the user
    echo
    echo "==========================="
    echo "Setup completed successfully!"
    echo "==========================="
    echo
    echo "Your TLS certificate for $DOMAIN has been obtained and is stored in /etc/letsencrypt/live/$DOMAIN/"
    echo
    echo "To start the a2a-agent service with HTTPS support:"
    echo "  systemctl start a2a-agent.service"
    echo
    echo "To check service status:"
    echo "  systemctl status a2a-agent.service"
    echo
    echo "Make sure to build the release binary before starting the service:"
    echo "  cd $PROJECT_DIR && RUSTFLAGS=\"-A warnings\" cargo build --release"
    echo
    echo "Certificate will be automatically renewed by Certbot when needed,"
    echo "and the service will be restarted after renewal."
  else
    echo "Service file already exists: $SERVICE_FILE"
    echo "You may need to manually add CERTBOT_DOMAIN=$DOMAIN to the environment variables."
  fi
else
  echo "Failed to obtain certificate. Please check the certbot logs for more information."
  exit 1
fi