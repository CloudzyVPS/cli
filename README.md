# Zy — the Cloudzy CLI and MCP server

**Zy** manages your [Cloudzy](https://cloudzy.com) cloud from the terminal, from scripts, and from AI assistants. It is a command-line tool and a [Model Context Protocol](https://modelcontextprotocol.io) server over the same API client, so everything you can do with `zy servers …` an assistant can do with a tool call.

> _"Create a WordPress site in Frankfurt with my laptop's SSH key."_
>
> The assistant lists plans and regions, picks the WordPress one-click app, calls `create_server`, and waits for it to come up.

---

## Install

Download the binary for your platform from [Releases](https://github.com/CloudzyVPS/cli/releases) — Linux (x86_64, ARM64), macOS (Intel, Apple Silicon), Windows (x86_64).

```bash
chmod +x zy-*
sudo mv zy-* /usr/local/bin/zy
zy --version
```

On Windows, rename the `.exe` to `zy.exe` and put it on your `PATH`. `zy update` keeps it current.

Or run it from the container image:

```bash
docker run --rm -it -v cloudzy:/config ghcr.io/cloudzyvps/cli login --device
docker run --rm -v cloudzy:/config ghcr.io/cloudzyvps/cli servers list
```

## Sign in

```bash
zy login            # opens your browser
zy login --device   # SSH session, container, no browser: approve a short code on any device
zy whoami
```

`zy login` signs you in with your Cloudzy account through the browser (OAuth 2.0 with PKCE). The first time, Cloudzy asks you to allow the **Cloudzy CLI** to manage servers, networking, backups and to read billing; after that, sign-ins go straight through. Over SSH or wherever no browser is available, zy switches to device sign-in automatically: it prints a link and a code, and you approve on your phone or laptop.

Access tokens are short-lived and refresh automatically. The sign-in is stored in `credentials.json` (owner-only permissions) under your config directory — `~/.config/cloudzy` on Linux, `~/Library/Application Support/cloudzy` on macOS, `%APPDATA%\cloudzy` on Windows — or `$CLOUDZY_CONFIG_DIR`.

```bash
zy auth status      # which credential, which account, when it refreshes
zy logout           # revokes the sign-in on Cloudzy and forgets it locally
```

You can withdraw access at any time in the Cloudzy dashboard under **Settings → Security → Connected applications**, which signs out every copy of the CLI at once.

### CI and automation

For unattended use, create a **developer API token** in the dashboard (**Developer API**), give it only the scopes the job needs, and pass it in the environment:

```bash
export CLOUDZY_TOKEN=hpt_…
zy servers list -o json
```

`CLOUDZY_TOKEN` takes precedence over a stored sign-in and is never refreshed or written to disk.

## Use

```bash
zy regions list
zy plans list --region fra
zy os list
zy ssh-keys add laptop --file ~/.ssh/id_ed25519.pub

zy servers create --hostname web-1 --plan std-4gb --region fra \
    --os ubuntu-24.04 --ssh-key laptop --wait
zy servers list
zy servers power web-1-id reboot
zy servers resize web-1-id --ram-mb 8192
zy snapshots create web-1-id --name before-upgrade
zy servers delete web-1-id
```

| Group | Commands |
|---|---|
| `servers` | `list` `get` `create` `delete` `power` `status` `rename` `resize` `rebuild` `reset-password` `usage` `activity` `wait` |
| `snapshots` | `list` `create` `delete` `restore` `spawn` |
| `ssh-keys` | `list` `add` `delete` |
| `reserved-ips` | `list` `create` `attach` `detach` `auto-renew` `release` |
| `ips` | `list` `add` `remove` |
| `firewall` | `list` `add` `delete` |
| `regions` `plans` `os` `apps` | `list` (and `apps get`) |
| `billing` | `balance` `ledger` `invoices` |
| account | `login` `logout` `auth status` `auth token` `whoami` |
| other | `mcp` `update` |

Run `zy <command> --help` for every flag.

- **Output.** Tables by default; `-o json` prints the API's JSON unchanged, for `jq` and scripts. Status notes go to stderr, so stdout stays clean.
- **Confirmation.** `delete`, `rebuild`, `reset-password`, snapshot `restore`/`delete`, and IP `release`/`remove` ask you to type the resource name. In scripts, pass `--yes`; without a terminal and without `--yes` they refuse.
- **Profiles.** `--profile staging` (or `CLOUDZY_PROFILE`) keeps separate sign-ins side by side.
- **Debugging.** `--debug` logs each request and response line to stderr. Credentials are never printed.

## AI assistants (MCP)

`zy mcp` serves the Model Context Protocol over stdio, using the same credential as the CLI. Sign in first with `zy login` (or set `CLOUDZY_TOKEN`), then register it:

**Claude Code**

```bash
claude mcp add cloudzy -- zy mcp
```

**Claude Desktop, Cursor, VS Code and other clients**

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

For a container, or to use a scoped developer token instead of your sign-in:

```json
{
  "mcpServers": {
    "cloudzy": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "-e", "CLOUDZY_TOKEN", "ghcr.io/cloudzyvps/cli", "mcp"],
      "env": { "CLOUDZY_TOKEN": "hpt_…" }
    }
  }
}
```

### Tools

| Area | Tools |
|---|---|
| Servers | `list_servers` `get_server` `create_server` `delete_server` `power_server` `rename_server` `resize_server` `rebuild_server` `reset_server_password` `server_status` `server_usage` `server_activity` |
| Snapshots | `list_snapshots` `create_snapshot` `delete_snapshot` `restore_snapshot` `spawn_server_from_snapshot` |
| SSH keys | `list_ssh_keys` `add_ssh_key` `delete_ssh_key` |
| Networking | `list_reserved_ips` `reserve_ips` `attach_reserved_ip` `detach_reserved_ip` `set_reserved_ip_auto_renew` `release_reserved_ip` `list_server_ips` `attach_server_ip` `detach_server_ip` `list_firewall_rules` `add_firewall_rule` `delete_firewall_rule` |
| Catalog | `list_regions` `list_plans` `list_os_templates` `list_apps` `get_app` |
| Account | `whoami` `get_balance` `list_ledger` `list_invoices` |

Every tool carries MCP annotations. Deletes, rebuilds, password resets, snapshot restores and IP releases are marked **destructive**, so a well-behaved client asks you before running them. Creating servers, snapshots and IPs charges your Cloudzy balance.

<details>
<summary><strong>Suggested system prompt</strong></summary>

```markdown
You can manage the user's Cloudzy cloud through the `cloudzy` MCP tools.

- Discover before acting: list_regions, list_plans (with a region for prices),
  list_os_templates and list_apps before create_server; list_servers for ids.
- Creating servers, snapshots and IPs costs money. State the plan, region and
  monthly price, and get a yes, before calling create_server, reserve_ips or
  spawn_server_from_snapshot.
- Never call a destructive tool (delete_server, rebuild_server,
  reset_server_password, restore_snapshot, delete_snapshot,
  release_reserved_ip, detach_server_ip, delete_ssh_key, delete_firewall_rule)
  without explicit confirmation naming the resource.
- After create_server, poll get_server until state is "active" and report the
  IP address. Passwords returned by rebuild or reset are shown once — hand
  them to the user and do not repeat them later.
- If a tool returns an error with a hint, follow the hint (for example, the
  balance is too low, or the credential lacks a scope) instead of retrying.
```

</details>

## Configuration

| Setting | Flag | Environment | Default |
|---|---|---|---|
| Platform URL | `--url` | `CLOUDZY_URL` | `https://dash.cloudzy.com` |
| Profile | `--profile` | `CLOUDZY_PROFILE` | `default` |
| Developer token | — | `CLOUDZY_TOKEN` | — |
| Config directory | — | `CLOUDZY_CONFIG_DIR` | platform config dir + `cloudzy` |
| Output | `-o, --output` | — | `table` |

A stored sign-in is only sent to the URL that issued it; pointing zy at another URL requires signing in there.

## Build from source

```bash
git clone https://github.com/CloudzyVPS/cli.git
cd cli
cargo build --release
cargo test
```

Requirements: Linux with glibc 2.31+ (Ubuntu 20.04+, Debian 11+), macOS 10.15+, or Windows 10+.

## Links

- [Cloudzy](https://cloudzy.com)
- [Releases](https://github.com/CloudzyVPS/cli/releases)
- [Changelog](CHANGELOG.md)
- [Model Context Protocol](https://modelcontextprotocol.io)

Contributions are welcome — please open an issue or a pull request.

© Cloudzy AI Information Technology L.L.C.
