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
# Download and install
curl -LO https://github.com/CloudzyVPS/cli/releases/latest/download/zy-{version}-{target}
chmod +x zy-{version}-{target}
sudo mv zy-{version}-{target} /usr/local/bin/zy

# Verify
zy --help
```

**Windows:**
Download the `.exe` file from the [Releases page](https://github.com/CloudzyVPS/cli/releases) and add it to your PATH.

### Configuration

Create a `.env` file or set environment variables:

```bash
# Required
API_BASE_URL=https://api.cloudzy.com/developers
API_TOKEN=your_api_token_here

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

Access the web interface at `http://localhost:5000` (default credentials: `owner` / `owner123`).

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
