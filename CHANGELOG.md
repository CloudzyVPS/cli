# Changelog

## 2.0.3 — 2026-10-08

- Reject unknown plan-list regions and billing cycles in CLI and MCP, list
  supported cycles, and distinguish missing prices from regional base-price
  fallbacks. Base-price listings without a region continue to work.
- Use the server’s public pricing engine when an older deployment explicitly
  reports the authenticated quote route as absent. Configuration, create, resize
  and snapshot-spawn previews retain server-calculated prices and IPv4 charges.
- Recognize the deployed structured route-absence response, including request
  references, for reserved-IP tariff previews and unknown capacity preflights.
  Authentication, missing-resource, stock, quota and backend errors still block.
  Legacy reserved-IP previews remain advisory; authoritative IP quotes, charge
  receipts and live capacity require the corresponding backend deployment.

## 2.0.2 — 2026-10-07

- Read selected-plan compute/stock preflight with configuration quotes. CLI and MCP
  refuse creation when the server reports no capacity, before any purchase.
- Read authoritative reserved-IP quotes and include expected price/currency guards
  on purchases. Show acknowledged charged amounts and pending/unavailable receipt
  status. Add the read-only `quote_reserved_ips` MCP tool.
- Older deployments without the new read endpoints retain explicitly labelled
  catalog/published-tariff fallbacks. Authentication, quota, stock and server errors
  never fall back to an assumed price. Backend fixes and deployment are still
  required for issues #28, #31, #36 and #37.

## 2.0.1 — 2026-10-07

- Normalize firewall directions and send the transport confirmation required for
  authorized snapshot restores in both CLI and MCP.
- Render real activity fields and usage units, resolve catalog plan labels in
  tables, and retain micro-amount precision in billing tables.
- Accept the official `v2.0` update tag. Keep release diagnostics behind debug
  on stderr, and provide a read-only JSON update check.
- Honor region/cycle selections in plan JSON, show base hourly rates, and add
  configuration quotes including IPv4 charges, create/resize/spawn previews,
  and snapshot-spawn billing-cycle selection and bounded waiting.
- Supply selected snapshot-plan resources explicitly, show financial previews,
  and require acceptance for snapshot spawns and reserved-IP purchases.
- Reject unsupported pool IPv4, unavailable IPv6, and unverifiable automatic
  backups before mutation. Show pending operations and recovery instructions,
  allow up to 15 minutes for mutations, and retain problem JSON diagnostics in
  structured errors.
- Run updater contract tests (previously unregistered).

Upstream gaps remain: live per-plan capacity can disagree with catalog stock;
reserved-IP receipts omit actual charged amounts; reserved-IP ledger descriptions
can contain upstream encoding corruption; rebuilds can fail at the API/edge despite
valid request schemas. These require backend changes and live validation.

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
