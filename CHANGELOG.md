# Changelog

## 2.0.0

Zy now works with the Cloudzy platform and is a command-line tool and MCP
server only. **This release is not compatible with 1.x**: the web interface,
the legacy developer gateway and its configuration are gone.

### Added

- `zy login` — browser sign-in with your Cloudzy account (OAuth 2.0 +
  PKCE), and `zy login --device` for SSH sessions and containers. Tokens
  refresh automatically; `zy logout` revokes them.
- `CLOUDZY_TOKEN` for developer API tokens in CI.
- Commands for servers (create, delete, power, rename, resize, rebuild,
  password reset, status, usage, activity, wait), snapshots, SSH keys,
  reserved IPs, server IPs, firewall rules, regions, plans with prices, OS
  templates, one-click apps, balance, ledger and invoices.
- `-o json` on every command, `--debug`, `--profile`.
- Typed confirmation for destructive commands, `--yes` for scripts.
- MCP server with 41 tools, JSON Schemas and destructive-action annotations,
  protocol 2025-06-18.

### Removed

- The web UI (`zy serve`), local users and workspaces (`zy users`,
  `users.json`), access control, clocked instances, and
  `DISABLED_INSTANCE_IDS`.
- `API_BASE_URL`, `API_TOKEN`, `PUBLIC_BASE_URL` and `.env` loading.
  Use `CLOUDZY_URL`, `zy login` or `CLOUDZY_TOKEN`.
- `zy check-config` (use `zy auth status` / `zy whoami`) and the
  `instances` command group (use `servers`).
- Traffic add-ons, ISO and custom image uploads, and backup profiles, which
  the Cloudzy platform API does not offer to API clients.
