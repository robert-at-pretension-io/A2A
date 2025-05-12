# HTTPS Setup Guide for A2A Bidirectional Agent

This guide provides step-by-step instructions for setting up HTTPS for your A2A bidirectional agent.

## Prerequisites

- A server with a public IP address
- A domain name pointing to your server (e.g., agent.example.com)
- Root/sudo access on your server
- Port 80 and 443 accessible from the internet

## Option 1: Automatic Setup (Recommended)

The easiest way to set up HTTPS is to use the provided setup script:

```bash
sudo ./setup_certbot.sh your-domain.example.com
```

This script will:
1. Install Certbot if needed
2. Obtain TLS certificates for your domain
3. Set up proper permissions
4. Create a systemd service for your agent with HTTPS enabled
5. Configure automatic certificate renewal

After running the script, build the release version:

```bash
RUSTFLAGS="-A warnings" cargo build --release
```

Then start the service:

```bash
sudo systemctl start a2a-agent.service
```

## Option 2: Manual Setup

If you prefer to set up HTTPS manually, follow these steps:

### 1. Install Certbot

On Debian/Ubuntu:
```bash
sudo apt-get update
sudo apt-get install -y certbot
```

On CentOS/RHEL:
```bash
sudo dnf install -y certbot
```

### 2. Obtain TLS Certificates

```bash
sudo certbot certonly --standalone -d your-domain.example.com
```

### 3. Fix Certificate Permissions

Create a `certbot` group and add your agent user to it:

```bash
sudo groupadd -r certbot
sudo usermod -a -G certbot your-agent-user
sudo chgrp -R certbot /etc/letsencrypt/live
sudo chgrp -R certbot /etc/letsencrypt/archive
sudo chmod -R g+rx /etc/letsencrypt/live
sudo chmod -R g+rx /etc/letsencrypt/archive
```

You can also use the provided script:

```bash
sudo ./fix_certbot_permissions.sh your-domain.example.com
```

### 4. Run the Agent with HTTPS

Set the `CERTBOT_DOMAIN` environment variable when running the agent:

```bash
CERTBOT_DOMAIN=your-domain.example.com RUSTFLAGS="-A warnings" cargo run -- bidirectional-agent
```

## How It Works

The agent's HTTPS implementation:

1. Checks for the `CERTBOT_DOMAIN` environment variable at startup
2. If set, it looks for TLS certificates in `/etc/letsencrypt/live/your-domain.example.com/`
3. If certificates are found, the agent starts with HTTPS using them
4. If not found, it falls back to HTTP mode automatically

## Troubleshooting

### Certificate Issues

Check certificate status:
```bash
sudo certbot certificates
```

Renew certificates:
```bash
sudo certbot renew --dry-run  # Test renewal
sudo certbot renew            # Actual renewal
```

### Permission Issues

If the agent can't read the certificates:
```bash
sudo ./fix_certbot_permissions.sh your-domain.example.com
```

### Server Issues

Check the server status if running as a service:
```bash
sudo systemctl status a2a-agent.service
sudo journalctl -u a2a-agent.service
```

## Security Considerations

- TLS certificates contain sensitive private keys
- Never run the agent as root; use a dedicated service user
- Keep your server, agent, and Certbot up to date
- Follow security best practices for your server