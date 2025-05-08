# HTTPS Setup for A2A Bidirectional Agent

> Related Documentation:
> - [Project Overview](README.md)
> - [Bidirectional Agent](src/bidirectional/README.md)
> - [Bidirectional Agent Quickstart](README_BIDIRECTIONAL.md)
> - [Bidirectional Agent Documentation](bidirectional_agent_readme.md)

This guide explains how to set up HTTPS/TLS for your bidirectional agent using Certbot.

## Prerequisites

- A server with a public IP address
- A domain name pointing to your server
- Root/sudo access on your server
- Port 80 and 443 accessible from the internet

## Automatic Setup

The fastest way to set up HTTPS for your agent is to use the provided setup script:

```bash
sudo ./setup_certbot.sh your-domain.example.com
```

This script will:
1. Install Certbot if not already installed
2. Obtain a TLS certificate for your domain
3. Create a systemd service to run your agent with HTTPS
4. Configure auto-renewal for your certificate
5. Set up proper permissions

After running the script, build a release version of your agent:

```bash
RUSTFLAGS="-A warnings" cargo build --release
```

Then start the service:

```bash
sudo systemctl start a2a-agent.service
```

## Manual Setup

If you prefer to set up HTTPS manually, follow these steps:

### 1. Install Certbot

On Debian/Ubuntu:
```bash
sudo apt-get update
sudo apt-get install certbot
```

On CentOS/RHEL:
```bash
sudo dnf install certbot
```

### 2. Obtain a Certificate

```bash
sudo certbot certonly --standalone -d your-domain.example.com
```

### 3. Run Agent with HTTPS

Set the CERTBOT_DOMAIN environment variable to your domain name when running the agent:

```bash
CERTBOT_DOMAIN=your-domain.example.com RUSTFLAGS="-A warnings" cargo run -- --listen
```

### 4. Configure Auto-Renewal

Create a renewal hook:

```bash
sudo mkdir -p /etc/letsencrypt/renewal-hooks/post
sudo nano /etc/letsencrypt/renewal-hooks/post/restart_a2a_agent.sh
```

Add the following content:

```bash
#!/bin/bash
systemctl restart a2a-agent.service
```

Make it executable:

```bash
sudo chmod +x /etc/letsencrypt/renewal-hooks/post/restart_a2a_agent.sh
```

### 5. Create a Systemd Service

```bash
sudo nano /etc/systemd/system/a2a-agent.service
```

Add the following content:

```
[Unit]
Description=Agent-to-Agent Bidirectional Agent
After=network.target

[Service]
Type=simple
User=a2a
Group=a2a
WorkingDirectory=/path/to/a2a-test-suite
Environment="CERTBOT_DOMAIN=your-domain.example.com"
Environment="RUST_LOG=info"
ExecStart=/path/to/a2a-test-suite/target/release/bidirectional-agent --listen
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable a2a-agent.service
sudo systemctl start a2a-agent.service
```

## How It Works

1. The agent checks for the CERTBOT_DOMAIN environment variable at startup
2. If set, it looks for TLS certificates in `/etc/letsencrypt/live/your-domain.example.com/`
3. If certificates are found, the agent starts with HTTPS. Otherwise, it falls back to HTTP
4. Certbot automatically renews certificates when they're close to expiration
5. After renewal, the hook script restarts the agent to use the new certificates

## Troubleshooting

### Certificate Issues

Check certbot status:
```bash
sudo certbot certificates
```

Manually renew certificates:
```bash
sudo certbot renew --dry-run  # Test renewal
sudo certbot renew            # Actually renew
```

### Permission Issues

If the agent can't read the certificates:
```bash
sudo usermod -a -G certbot a2a
sudo chmod -R g+rx /etc/letsencrypt/live
sudo chmod -R g+rx /etc/letsencrypt/archive
sudo systemctl restart a2a-agent.service
```

### Service Issues

Check service status:
```bash
sudo systemctl status a2a-agent.service
sudo journalctl -u a2a-agent.service
```

## Security Considerations

- Certificates contain sensitive private keys, ensure proper permissions
- The agent should run with minimal privileges (not as root)
- Keep your server and Certbot up to date with security patches