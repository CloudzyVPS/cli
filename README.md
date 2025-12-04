# Zy CLI

**Zy** — Control and manage your Cloudzy services from the command line or web interface.

Zy is a command-line tool and web server that allows you to manage Cloudzy VPS instances, regions, products, and more. Whether you prefer a web UI or CLI commands, Zy gives you full control over your Cloudzy infrastructure.

## 🚀 Quick Start

### Download

Get the latest pre-built binary for your platform from the [Releases page](https://github.com/CloudzyVPS/cli/releases).

**Available platforms:**
- **Linux** (x86_64, ARM64)
- **macOS** (Intel, Apple Silicon)
- **Windows** (x86_64)

### Installation

**Linux/macOS:**
```bash
# Download the appropriate binary for your platform from the Releases page
# Replace {VERSION} with the actual version (e.g., 0.1.0) you downloaded
# Replace {TARGET} with your platform target from the list below:
#
# Linux x86_64:        x86_64-unknown-linux-gnu
# Linux ARM64:         aarch64-unknown-linux-gnu
# macOS Intel:         x86_64-apple-darwin
# macOS Apple Silicon: aarch64-apple-darwin

# Make it executable (use the actual filename you downloaded)
chmod +x zy-{VERSION}-{TARGET}

# Move to a location in your PATH
sudo mv zy-{VERSION}-{TARGET} /usr/local/bin/zy

# Verify
zy --help
```

**Windows:**
1. Download the `zy-{VERSION}-x86_64-pc-windows-msvc.exe` file from the [Releases page](https://github.com/CloudzyVPS/cli/releases)
2. Rename it to `zy.exe`
3. Move it to a directory in your PATH:
   ```powershell
   # Option 1: User-specific location (recommended, no admin required)
   New-Item -ItemType Directory -Force -Path "$env:LOCALAPPDATA\Programs\zy"
   Move-Item zy.exe "$env:LOCALAPPDATA\Programs\zy\"
   
   # Add to PATH permanently (User level)
   $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
   [Environment]::SetEnvironmentVariable('Path', "$userPath;$env:LOCALAPPDATA\Programs\zy", 'User')
   
   # Restart your terminal after this step
   
   # Option 2: System-wide (requires admin)
   Move-Item zy.exe C:\Windows\System32\
   ```
4. Verify: `zy --help`

### Configuration

Create a `.env` file or set environment variables:

```bash
# Required - Get your API token from Cloudzy dashboard
API_BASE_URL=https://api.cloudzy.com/developers
API_TOKEN=YOUR_ACTUAL_API_TOKEN_HERE  # ⚠️ Replace with your real token

# Optional
PUBLIC_BASE_URL=http://localhost:5000
DISABLED_INSTANCE_IDS=
```

See [.env.example](.env.example) for a complete configuration template.

## 📖 Usage

### Web Server

Start the web interface to manage your instances through a browser:

```bash
# Using defaults (0.0.0.0:5000)
zy serve

# Custom host and port
zy serve --host 127.0.0.1 --port 8080

# With custom .env file
zy serve --env-file /path/to/.env
```

Access the web interface at `http://localhost:5000`. 

**⚠️ Security Note:** On first run, a default owner account is created with username `owner` and password `owner123` (stored in `users.json`). **Change this password immediately** after your first login using the web interface or the command:
```bash
zy users reset-password owner YOUR_NEW_SECURE_PASSWORD
```

### CLI Commands

Manage instances directly from the command line:

```bash
# List instances
zy instances list

# Show instance details
zy instances show <instance-id>

# Power management
zy instances power-on <instance-id>
zy instances power-off <instance-id>
zy instances reset <instance-id>

# Delete an instance
zy instances delete <instance-id>
```

### User Management

```bash
# List users
zy users list

# Add a user
zy users add <username> <password> <role>

# Reset password
zy users reset-password <username> <new-password>
```

### Configuration Check

```bash
# Validate your configuration
zy check-config
```

For complete command documentation, use:
```bash
zy --help
zy <command> --help
```

## 🔧 Building from Source

**Prerequisites:**
- Rust 1.91.1 or later
- Cargo

**Build:**
```bash
# Clone the repository
git clone https://github.com/CloudzyVPS/cli.git
cd cli

# Build release binary
cargo build --release

# Binary location
./target/release/zy
```

**Development:**
```bash
# Run directly with cargo
cargo run -- serve --host 127.0.0.1 --port 5000
```

## 📋 Requirements

- **Linux**: glibc 2.31+ (Ubuntu 20.04+, Debian 11+, or equivalent)
- **macOS**: macOS 10.15+ (Catalina or later)
- **Windows**: Windows 10 or later

## 🤝 Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## 📝 License

See [LICENSE](LICENSE) for details.

## 🔗 Links

- [Cloudzy Website](https://cloudzy.com)
- [API Documentation](https://api.cloudzy.com/developers)
- [GitHub Releases](https://github.com/CloudzyVPS/cli/releases)
