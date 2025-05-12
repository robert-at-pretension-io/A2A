# HTTPS Setup for A2A Bidirectional Agent

> Related Documentation:
> - [Project Overview](README.md)
> - [Bidirectional Agent](src/bidirectional/README.md)
> - [Bidirectional Agent Quickstart](README_BIDIRECTIONAL.md)
> - [Bidirectional Agent Documentation](bidirectional_agent_readme.md)

**IMPORTANT UPDATE**: The direct HTTPS handling in the A2A agent has been disabled due to issues with the TLS handshake implementation. We now strongly recommend using Nginx as a reverse proxy for HTTPS termination. Please see the [HTTPS Setup with Nginx](#https-setup-with-nginx) section in the main README.md for the current recommended approach.

## HTTPS Setup with Nginx (Recommended)

For production deployments, we strongly recommend using Nginx as a reverse proxy for HTTPS termination. This provides better performance, security, and stability compared to the agent's built-in TLS support.

### Brief Overview

1. Nginx receives HTTPS connections from clients
2. Nginx handles the TLS encryption/decryption
3. Nginx forwards unencrypted HTTP requests to the A2A agent internally
4. Nginx adds CORS headers and handles OPTIONS preflight requests
5. The A2A agent responds with HTTP (which is fine because it's a local connection)
6. Nginx re-encrypts responses and sends them back to clients via HTTPS

### Setup Steps

1. Install Nginx and Certbot
2. Configure Nginx as a reverse proxy (see example in README.md)
3. Run your A2A agent with HTTP only (do not set CERTBOT_DOMAIN)
4. Access your agent through the HTTPS endpoint provided by Nginx

For detailed instructions, please refer to the "HTTPS Setup with Nginx" section in the main README.md file.

## Legacy Setup (Not Recommended)

The direct HTTPS handling in the A2A agent (using CERTBOT_DOMAIN) is no longer recommended or actively supported. The code remains for backward compatibility but will forward all connections to the HTTP handler.

If you previously used the CERTBOT_DOMAIN approach, please migrate to the Nginx-based setup for better reliability and security.

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