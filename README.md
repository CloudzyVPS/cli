# Zy — control and manage your Cloudzy services right from home.

## Releases

Pre-built binaries for Zy are automatically built and published through GitHub Releases for multiple platforms.

### Download

Download the latest release from the [Releases page](https://github.com/CloudzyVPS/cli/releases).

Available platforms:
- **Linux**: x86_64 and aarch64 (both GNU and MUSL variants)
  - `zy-{version}-x86_64-unknown-linux-gnu.tar.gz` - Standard Linux x86_64
  - `zy-{version}-x86_64-unknown-linux-musl.tar.gz` - Static Linux x86_64 (no dependencies)
  - `zy-{version}-aarch64-unknown-linux-gnu.tar.gz` - Standard Linux ARM64
  - `zy-{version}-aarch64-unknown-linux-musl.tar.gz` - Static Linux ARM64 (no dependencies)
- **macOS**: Intel and Apple Silicon
  - `zy-{version}-x86_64-apple-darwin.tar.gz` - macOS Intel
  - `zy-{version}-aarch64-apple-darwin.tar.gz` - macOS Apple Silicon
- **Windows**: x86_64
  - `zy-{version}-x86_64-pc-windows-msvc.zip` - Windows 64-bit

### Verification

Each release includes a `SHA256SUMS.txt` file containing checksums for all release artifacts. To verify your download:

```bash
# Linux/macOS
sha256sum -c SHA256SUMS.txt --ignore-missing

# Or verify a specific file
sha256sum zy-{version}-{target}.tar.gz
# Compare with the checksum in SHA256SUMS.txt
```

### Installation

**Linux/macOS:**
```bash
# Download and extract
tar -xzf zy-{version}-{target}.tar.gz

# Move to a location in your PATH
sudo mv zy /usr/local/bin/

# Verify installation
zy --help
```

**Windows:**
```powershell
# Extract the zip file
# Add the directory containing zy.exe to your PATH
# Or move zy.exe to a directory already in your PATH

# Verify installation
zy --help
```

### Release Triggers

New releases are automatically triggered by:
- **Tag Push**: Pushing a tag matching `v*` (e.g., `v0.1.0`, `v1.2.3`)
- **GitHub Release**: Publishing a new GitHub Release
- **Manual**: Using the "Run workflow" button in the Actions tab

When a version tag is pushed (e.g., `v0.1.0`), the workflow:
1. Builds binaries for all supported platforms
2. Runs tests to ensure quality
3. Packages each binary with README and LICENSE (if present)
4. Generates SHA256 checksums
5. Creates a GitHub Release with all artifacts attached

## Building from Source

Generate binary (production):

1. Build a release binary:

	```bash
	cargo build --release
	```

2. The produced binary is located at `target/release/zy`.

Run the server (serve command):

1. Use the built binary and the `serve` subcommand:

	```bash
	./target/release/zy serve --host 0.0.0.0 --port 5000 --env-file /path/to/.env
	```

2. For development, you can run through cargo directly:

	```bash
	cargo run -- serve --host 127.0.0.1 --port 5000
	```

Only the binary generation and `serve` command are documented here. For additional commands and configuration, consult the source or CLI help.
