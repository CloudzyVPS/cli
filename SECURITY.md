# Security Best Practices - Zy CLI

This document outlines security best practices for deploying and operating the Zy CLI application.

## Table of Contents

- [Initial Setup](#initial-setup)
- [Authentication Security](#authentication-security)
- [Production Deployment](#production-deployment)
- [Secrets Management](#secrets-management)
- [Monitoring & Logging](#monitoring--logging)
- [Maintenance & Updates](#maintenance--updates)
- [Incident Response](#incident-response)

---

## Initial Setup

### 1. Change Default Credentials

**⚠️ CRITICAL:** The default owner account is created with:
- Username: `owner`
- Password: `owner123`

**Change this immediately** after first run:

```bash
# Method 1: Using the CLI
zy users reset-password owner YOUR_SECURE_PASSWORD_HERE

# Method 2: Through the web interface
# Login and navigate to User Management
```

### 2. Secure Sensitive Files

Ensure proper file permissions on Unix-like systems:

```bash
# Secure the users database
chmod 600 users.json

# Secure the environment configuration
chmod 600 .env

# Verify permissions
ls -la users.json .env
```

The application will warn on startup if these files have insecure permissions.

### 3. Configure Environment Variables

Create a `.env` file from the example:

```bash
cp .env.example .env
nano .env  # or your preferred editor
```

Required configuration:
```env
# Required: Your Cloudzy API credentials
API_BASE_URL=https://api.cloudzy.com/developers
API_TOKEN=your_actual_api_token_32_chars_minimum

# Required for production: Your public base URL
PUBLIC_BASE_URL=https://yourdomain.com

# Optional: Disable specific instances
DISABLED_INSTANCE_IDS=instance-id-1,instance-id-2
```

---

## Authentication Security

### Password Requirements

When creating new users or changing passwords, follow these guidelines:

- **Minimum length:** 8 characters (longer is better)
- **Complexity:** Mix of uppercase, lowercase, numbers, and symbols
- **Uniqueness:** Never reuse passwords across services
- **Storage:** Use a password manager

Example strong password: `MyS3cur3P@ssw0rd!2024`

### Session Management

The application implements secure session management:

- **Session Duration:** 24 hours maximum
- **Idle Timeout:** 2 hours of inactivity
- **Session Storage:** Server-side only (not in cookies)

Sessions are automatically invalidated when:
- Maximum age is reached
- Idle timeout expires
- User logs out

### User Roles

The application supports two roles:

1. **Owner:**
   - Full administrative access
   - User management
   - Access control
   - Instance management

2. **Admin:**
   - Limited to assigned instances
   - Cannot manage users
   - Cannot modify access controls

**Best Practice:** Use the principle of least privilege - only assign owner role when necessary.

---

## Production Deployment

### HTTPS/TLS Configuration

**⚠️ CRITICAL:** Always use HTTPS in production. The Secure cookie flag requires it.

#### Option 1: Reverse Proxy (Recommended)

Use nginx or Caddy as a reverse proxy to handle TLS:

**Nginx Configuration Example:**
```nginx
server {
    listen 443 ssl http2;
    server_name yourdomain.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    # Modern SSL configuration
    ssl_protocols TLSv1.3 TLSv1.2;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    location / {
        proxy_pass http://127.0.0.1:5000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name yourdomain.com;
    return 301 https://$server_name$request_uri;
}
```

**Caddy Configuration Example:**
```caddy
yourdomain.com {
    reverse_proxy localhost:5000
}
```

#### Option 2: Direct TLS (Future Enhancement)

Currently, the application doesn't support built-in TLS. Use a reverse proxy.

### Network Security

1. **Firewall Configuration:**
   ```bash
   # Allow HTTPS only (if using reverse proxy)
   sudo ufw allow 443/tcp
   sudo ufw allow 80/tcp   # For redirect to HTTPS
   sudo ufw enable
   ```

2. **Bind to Localhost:**
   When using a reverse proxy, bind only to localhost:
   ```bash
   zy serve --host 127.0.0.1 --port 5000
   ```

3. **Security Groups/Network ACLs:**
   - Restrict access to known IP ranges
   - Use VPN for administrative access
   - Implement DDoS protection

### Environment-Specific Configuration

**Development:**
```bash
export RUST_LOG=debug
zy serve --host 127.0.0.1 --port 5000
```

**Production:**
```bash
export RUST_LOG=info
zy serve --host 127.0.0.1 --port 5000
```

---

## Secrets Management

### API Token Security

1. **Generation:**
   - Use strong, random tokens (32+ characters)
   - Never use predictable tokens
   - Generate: `openssl rand -base64 32`

2. **Storage:**
   - Store in environment variables or `.env` file
   - Never commit to version control
   - Use secret management service in production (AWS Secrets Manager, HashiCorp Vault)

3. **Rotation:**
   - Rotate API tokens regularly (quarterly recommended)
   - Update immediately if compromised
   - Keep audit log of token rotations

### Environment Variables

**Best Practices:**

```bash
# Development: Use .env file
cp .env.example .env
# Edit .env with your secrets

# Production: Use environment variables
export API_TOKEN="$(cat /secure/path/to/token)"
export API_BASE_URL="https://api.cloudzy.com/developers"
export PUBLIC_BASE_URL="https://yourdomain.com"

# Or use systemd environment file
# /etc/systemd/system/zy.service.d/override.conf
[Service]
EnvironmentFile=/etc/zy/environment
```

### File Permissions

```bash
# Sensitive files should be readable only by the application user
sudo chown zy-user:zy-group /etc/zy/environment
sudo chmod 600 /etc/zy/environment
```

---

## Monitoring & Logging

### Log Configuration

Configure appropriate log levels:

```bash
# Development
export RUST_LOG=debug

# Production
export RUST_LOG=info

# Security auditing (verbose)
export RUST_LOG=zy=debug,info
```

### Security Events to Monitor

The application logs the following security events:

1. **Authentication:**
   - Successful logins: `User '{username}' logged in successfully`
   - Failed login attempts: `Failed login attempt for username: {username}`
   - Logout events: `User logged out successfully`

2. **Session Management:**
   - Expired sessions: `Removed expired/idle session`
   - Session validation failures

3. **API Errors:**
   - API request failures: `API request failed: {error}`
   - Response parsing errors: `Failed to parse API response: {error}`

4. **Configuration:**
   - Security warnings at startup
   - File permission issues
   - Invalid API tokens

### Log Collection

**Recommended Setup:**

```bash
# Systemd service with logging
sudo journalctl -u zy -f

# Or redirect to file
zy serve 2>&1 | tee -a /var/log/zy/app.log

# Use log rotation
sudo nano /etc/logrotate.d/zy
```

**Logrotate Configuration:**
```
/var/log/zy/*.log {
    daily
    rotate 30
    compress
    delaycompress
    notifempty
    create 0640 zy-user zy-group
    sharedscripts
    postrotate
        systemctl reload zy || true
    endscript
}
```

### Alerting

Set up alerts for:
- Multiple failed login attempts
- API authentication failures
- Unexpected error spikes
- Service downtime

**Example with systemd:**
```bash
# Monitor service failures
sudo systemctl status zy
sudo systemctl enable zy-failure-alert@zy.service
```

---

## Maintenance & Updates

### Dependency Updates

Regular maintenance schedule:

```bash
# Weekly: Check for security advisories
cargo audit

# Monthly: Update dependencies
cargo update
cargo audit
cargo test
cargo build --release

# Before deployment: Final checks
cargo clippy -- -W clippy::all
```

### Security Patches

1. **Monitor Security Advisories:**
   - Subscribe to RustSec advisory notifications
   - Watch the GitHub repository for security updates
   - Check Cloudzy API security announcements

2. **Update Process:**
   ```bash
   # 1. Backup current state
   sudo systemctl stop zy
   cp users.json users.json.backup
   cp .env .env.backup
   
   # 2. Update application
   cargo build --release
   
   # 3. Test in staging
   ./target/release/zy check-config
   
   # 4. Deploy
   sudo cp target/release/zy /usr/local/bin/
   sudo systemctl start zy
   
   # 5. Verify
   sudo systemctl status zy
   curl -k https://yourdomain.com/login
   ```

3. **Rollback Plan:**
   ```bash
   sudo systemctl stop zy
   sudo cp /usr/local/bin/zy.backup /usr/local/bin/zy
   sudo systemctl start zy
   ```

### User Management

Regular user reviews:

```bash
# List all users
zy users list

# Remove inactive users
# (Through web interface or by editing users.json)

# Audit user assignments
# Check assigned_instances for each admin
```

### Backup Strategy

**Critical Data:**
- `users.json` - User credentials and assignments
- `.env` - Configuration (store securely!)

**Backup Script Example:**
```bash
#!/bin/bash
BACKUP_DIR="/secure/backups/zy"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p "$BACKUP_DIR"
cp users.json "$BACKUP_DIR/users_$DATE.json"
# DON'T backup .env to shared locations - it contains secrets

# Encrypt backups
gpg --encrypt --recipient backup@yourdomain.com \
    "$BACKUP_DIR/users_$DATE.json"

# Remove unencrypted copy
rm "$BACKUP_DIR/users_$DATE.json"

# Retain only last 30 days
find "$BACKUP_DIR" -name "*.gpg" -mtime +30 -delete
```

---

## Incident Response

### Security Incident Types

1. **Compromised Credentials**
2. **Unauthorized Access**
3. **API Token Exposure**
4. **System Compromise**

### Response Procedures

#### 1. Compromised Credentials

**Immediate Actions:**
```bash
# 1. Reset affected user password
zy users reset-password <username> NEW_SECURE_PASSWORD

# 2. Check access logs
sudo journalctl -u zy | grep "login"

# 3. Review recent actions
# Check instance modifications, user changes

# 4. Notify affected parties
```

#### 2. API Token Exposure

**Immediate Actions:**
```bash
# 1. Rotate API token immediately
# Login to Cloudzy dashboard and generate new token

# 2. Update configuration
nano .env
# Update API_TOKEN value

# 3. Restart service
sudo systemctl restart zy

# 4. Audit API usage
# Check Cloudzy dashboard for unauthorized actions

# 5. Review logs for suspicious activity
sudo journalctl -u zy --since "1 hour ago" | grep "API"
```

#### 3. Unauthorized Access

**Investigation Steps:**
```bash
# 1. Check active sessions
# Current sessions are stored in memory
# Restart service to clear all sessions:
sudo systemctl restart zy

# 2. Review login attempts
sudo journalctl -u zy | grep "Failed login"

# 3. Check user list
zy users list

# 4. Review instance access
# Check for unexpected instance modifications
```

**Containment:**
```bash
# 1. Reset all user passwords
zy users reset-password owner NEW_PASSWORD
# Repeat for all users

# 2. Rotate API token
# As described above

# 3. Review and update firewall rules
sudo ufw status
```

### Post-Incident

1. **Document the Incident:**
   - Timeline of events
   - Actions taken
   - Root cause analysis
   - Lessons learned

2. **Improve Security:**
   - Update security policies
   - Enhance monitoring
   - Additional training

3. **Communicate:**
   - Notify stakeholders
   - Update security documentation
   - Share lessons learned (if appropriate)

---

## Security Checklist

### Initial Deployment
- [ ] Change default owner password
- [ ] Configure strong API token
- [ ] Set proper file permissions (`chmod 600`)
- [ ] Configure HTTPS/TLS
- [ ] Set up firewall rules
- [ ] Configure log rotation
- [ ] Set up monitoring/alerting
- [ ] Document credentials securely
- [ ] Test backup restoration

### Weekly
- [ ] Review failed login attempts
- [ ] Check system logs for errors
- [ ] Verify service health
- [ ] Run `cargo audit`

### Monthly
- [ ] Update dependencies
- [ ] Review user access
- [ ] Rotate logs
- [ ] Test backup restoration
- [ ] Security training/awareness

### Quarterly
- [ ] Rotate API tokens
- [ ] Security audit
- [ ] Update documentation
- [ ] Review incident response procedures
- [ ] Penetration testing (if applicable)

---

## Additional Resources

- [Zy CLI Repository](https://github.com/CloudzyVPS/cli)
- [Cloudzy API Documentation](https://api.cloudzy.com/developers)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
- [RustSec Advisory Database](https://rustsec.org/)

---

## Support

For security issues:
- **Public Issues:** Use GitHub Issues for non-sensitive matters
- **Security Vulnerabilities:** Email security contact (see repository)
- **General Support:** See README for support channels

---

**Document Version:** 1.0  
**Last Updated:** December 6, 2025  
**Next Review:** March 6, 2026
