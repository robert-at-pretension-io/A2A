#!/bin/bash
set -e

# Script to fix Certbot permissions for the bidirectional agent
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

echo "Fixing permissions for certificates for $DOMAIN..."

# Check if certbot certificates exist
if [ ! -d "/etc/letsencrypt/live/$DOMAIN" ]; then
  echo "Certificates for $DOMAIN not found. Run certbot first."
  exit 1
fi

# Create certbot group if it doesn't exist
if ! getent group certbot &>/dev/null; then
  groupadd -r certbot
  echo "Created certbot group."
else
  echo "Certbot group already exists."
fi

# Create a2a user if it doesn't exist
if ! id "a2a" &>/dev/null; then
  useradd -r -s /bin/false a2a
  echo "Created a2a user for the service."
else
  echo "User a2a already exists."
fi

# Add a2a user to certbot group
usermod -a -G certbot a2a
echo "Added a2a user to certbot group."

# Set group ownership and permissions
chgrp -R certbot /etc/letsencrypt/live
chgrp -R certbot /etc/letsencrypt/archive
chmod -R g+rx /etc/letsencrypt/live
chmod -R g+rx /etc/letsencrypt/archive
echo "Set group ownership and permissions for certificate directories."

# Check if service file exists
SERVICE_FILE="/etc/systemd/system/a2a-agent.service"
if [ -f "$SERVICE_FILE" ]; then
  # Add CERTBOT_DOMAIN environment variable if not already there
  if ! grep -q "CERTBOT_DOMAIN=$DOMAIN" "$SERVICE_FILE"; then
    # Use sed to add environment variable to the service file
    sed -i "/\[Service\]/a Environment=\"CERTBOT_DOMAIN=$DOMAIN\"" "$SERVICE_FILE"
    echo "Added CERTBOT_DOMAIN=$DOMAIN to service file."
  else
    echo "CERTBOT_DOMAIN already set in service file."
  fi
  
  # Reload systemd
  systemctl daemon-reload
  echo "Reloaded systemd configuration."
  
  echo "Would you like to restart the a2a-agent service now? (y/n)"
  read -r RESTART
  if [[ "$RESTART" =~ ^[Yy]$ ]]; then
    systemctl restart a2a-agent.service
    echo "Service restarted. Check status with: systemctl status a2a-agent.service"
  fi
else
  echo "Service file not found. You may need to create it manually."
  echo "Consider running setup_certbot.sh to create the service file."
fi

echo "Permissions fixed successfully!"
echo "You can now run the agent with HTTPS support."