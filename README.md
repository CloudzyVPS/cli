# Zy — AI-Native Cloud Infrastructure CLI

**Zy** is an AI-native command-line tool that gives AI assistants the ability to create, manage, and control cloud infrastructure on [Cloudzy](https://cloudzy.com). It implements the **Model Context Protocol (MCP)** so that any MCP-compatible AI — Claude, GPT, Copilot, or your own agent — can deploy servers, launch WordPress sites, manage VPS instances, and operate cloud resources through natural conversation.

> _"Create a WordPress website on the internet."_
>
> That's all a user needs to say. The AI assistant uses Zy's MCP tools to pick a region, choose a plan, select the WordPress one-click app, and spin up a live server — no manual steps required.

---

### Docker (one-liner)

Run the web interface instantly using the pre-built image from the GitHub Container Registry — no installation required:

```bash
docker run -e API_TOKEN=your_api_token -p 5000:5000 ghcr.io/cloudzvps/cli:latest
```

Then open `http://localhost:5000` in your browser.

**Pass additional configuration via environment variables:**

```bash
docker run \
  -e API_TOKEN=your_api_token \
  -e API_BASE_URL=https://api.cloudzy.com/developers \
  -e PUBLIC_BASE_URL=http://localhost:5000 \
  -p 5000:5000 \
  ghcr.io/cloudzvps/cli:latest
```

> **Note:** On first run, a default owner account is created with username `owner` and password `owner123`. Change this password immediately after your first login.

### Download
## 🤖 AI Integration (MCP)

Zy exposes cloud infrastructure operations as **MCP tools** over a standard JSON-RPC 2.0 stdio transport. Any AI assistant that supports the [Model Context Protocol](https://modelcontextprotocol.io) can use Zy to manage real servers on the internet.

### Quick Start — Connect Your AI

1. **Install Zy** ([download a binary](#installation) or build from source)
2. **Configure your API token:**
   ```bash
   export API_BASE_URL=https://api.cloudzy.com/developers
   export API_TOKEN=your_cloudzy_api_token
   ```
3. **Register Zy as an MCP server** in your AI client. For Claude Desktop, add to your `claude_desktop_config.json`:
   ```json
   {
     "mcpServers": {
       "cloudzy": {
         "command": "zy",
         "args": ["mcp"]
       }
     }
   }
   ```
4. **Ask your AI anything about cloud infrastructure.** It will call Zy's tools automatically.

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `list_instances` | List all compute instances on your account |
| `get_instance` | Get full details of a specific instance by ID |
| `power_on_instance` | Power on a stopped instance |
| `power_off_instance` | Power off a running instance |
| `reset_instance` | Reboot an instance |
| `delete_instance` | Permanently delete an instance |
| `list_regions` | List available cloud data center regions |
| `list_ssh_keys` | List SSH keys on the account |

The MCP server is **self-describing** — AI clients discover these tools automatically via the `tools/list` method. Visit `/mcp` on the web server for interactive, Swagger-like documentation.

### What Can an AI Do With Zy?

When connected to an AI assistant, users can make requests like:

- _"Deploy a WordPress site in the US"_
- _"Show me all my running servers"_
- _"Reboot my production instance"_
- _"Turn off the staging server to save costs"_
- _"List available data center regions"_
- _"Delete the test instance I created yesterday"_

The AI translates these natural-language requests into the right MCP tool calls.

---

## 🔖 AI System Prompt

Copy the prompt below into your AI assistant's system instructions (or place it in an `AGENTS.md` / tool-description file) so that it knows **when and how** to use Zy for cloud and infrastructure tasks.

<details>
<summary><strong>Click to expand — Cloudzy AI System Prompt</strong></summary>

```markdown
# Cloudzy Cloud Infrastructure Tool

You have access to the Cloudzy MCP server ("zy") which lets you create,
manage, and control cloud computing infrastructure on the internet.

## When to use this tool

Use the Cloudzy tools whenever the user asks about:
- Creating, deploying, or launching a **website**, **server**, **VPS**,
  or **virtual machine** on the internet / in the cloud
- Managing **cloud infrastructure** — starting, stopping, rebooting,
  or deleting servers
- Checking the status of their **online servers** or **instances**
- Deploying a specific application (e.g. **WordPress**, **Node.js**,
  **Docker**) to a live server on the internet
- Anything involving **hosting**, **cloud compute**, **data centers**,
  or **regions**

## Available tools

- **list_instances** — List all running and stopped compute instances.
- **get_instance(instance_id)** — Get detailed information about one instance
  (IP address, status, OS, region, specs).
- **power_on_instance(instance_id)** — Start a stopped instance.
- **power_off_instance(instance_id)** — Stop a running instance.
- **reset_instance(instance_id)** — Reboot an instance.
- **delete_instance(instance_id)** — Permanently destroy an instance.
- **list_regions** — List all available data center locations worldwide
  (use this to help the user pick where to host).
- **list_ssh_keys** — List SSH keys on the account
  (useful before creating an instance).

## How instance creation works

To deploy a new server (e.g. a WordPress site), guide the user through:

1. **Pick a region** — use `list_regions` to show options, then let the
   user choose based on proximity or preference.
2. **Pick a plan** — Cloudzy offers fixed plans (pre-set CPU/RAM/disk)
   and custom plans. Recommend a suitable size for the workload.
3. **Pick an OS and application** — For WordPress, select a Linux OS and
   the WordPress one-click application (OCA). For a plain server, just
   pick an OS.
4. **Create the instance** — The instance is provisioned via the Cloudzy
   API with the chosen region, plan, OS, and optional application.
5. **Report back** — Tell the user their server's IP address, status, and
   any next steps (like visiting their new WordPress site).

After creation, use `list_instances` or `get_instance` to retrieve the
server IP and confirm it is running.

## Tips

- Always confirm destructive actions (delete, power off) with the user.
- When the user says "my server" or "my website", use `list_instances`
  to find the relevant instance.
- Instance IDs are UUIDs — the user usually refers to instances by
  hostname or IP, so map between them using `list_instances`.
- Regions have human-friendly names (e.g. "Los Angeles", "Frankfurt") —
  present these to the user, not raw IDs.
```

</details>

---

## 📖 MCP Documentation & Logs

When running the web server (`zy serve`), Zy provides built-in MCP documentation:

| URL | Description |
|-----|-------------|
| `/mcp` | Interactive Swagger-like tool reference (auto-generated from MCP self-description) |
| `/mcp/tools` | Raw JSON tool definitions |
| `/mcp/logs-page` | Paginated MCP call log viewer with click-to-expand raw request/response dumps |
| `/mcp/logs` | Call logs as JSON (supports `?page=1&per_page=20`) |
| `/mcp/logs/:id` | Single log entry detail as JSON |

---

## 🚀 Installation

Download the latest binary for your platform from the [Releases page](https://github.com/CloudzyVPS/cli/releases).

**Platforms:** Linux (x86_64, ARM64) · macOS (Intel, Apple Silicon) · Windows (x86_64)

**Linux / macOS:**
```bash
chmod +x zy-*
sudo mv zy-* /usr/local/bin/zy
zy --help
```

**Windows:**
1. Download `zy-…-x86_64-pc-windows-msvc.exe` from Releases
2. Rename to `zy.exe` and place in your PATH
3. Verify: `zy --help`

### Configuration

```bash
# Required
export API_BASE_URL=https://api.cloudzy.com/developers
export API_TOKEN=your_api_token_here

# Or use a .env file
zy serve --env-file .env
zy mcp --env-file .env
```

See [.env.example](.env.example) for all options.

---

## 💻 CLI Usage

### Web Server

```bash
zy serve                              # Start on 0.0.0.0:5000
zy serve --host 127.0.0.1 --port 8080 # Custom bind
```

**⚠️** On first run a default owner account (`owner` / `owner123`) is created. Change it immediately:
```bash
zy users reset-password owner YOUR_NEW_SECURE_PASSWORD
```

### MCP Server (for AI assistants)

```bash
zy mcp                  # Start MCP stdio server
zy mcp --env-file .env  # With explicit config
```

### Instance Management

```bash
zy instances list
zy instances show <id>
zy instances power-on <id>
zy instances power-off <id>
zy instances reset <id>
zy instances delete <id>
```

### User Management

```bash
zy users list
zy users add <username> <password> <role>
zy users reset-password <username> <password>
```

### Other Commands

```bash
zy check-config   # Validate API credentials
zy update          # Self-update to latest version
zy --help          # Full help
```

---

## 🔧 Building from Source

```bash
git clone https://github.com/CloudzyVPS/cli.git
cd cli
cargo build --release
./target/release/zy --help
```

### Testing

```bash
cargo test
```

---

## 📋 Requirements

- **Linux**: glibc 2.31+ (Ubuntu 20.04+, Debian 11+)
- **macOS**: 10.15+ (Catalina)
- **Windows**: 10+

## 🤝 Contributing

Contributions welcome — please open issues or pull requests.

## 📝 License

See [LICENSE](LICENSE) for details.

## 🔗 Links

- [Cloudzy](https://cloudzy.com)
- [API Documentation](https://api.cloudzy.com/developers)
- [Model Context Protocol](https://modelcontextprotocol.io)
- [Releases](https://github.com/CloudzyVPS/cli/releases)
