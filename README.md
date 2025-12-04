# Zy — control and manage your Cloudzy services right from home.

## Releases

Pre-built binaries for Zy are automatically built and published through GitHub Releases for multiple platforms.

### Download

Download the latest release from the [Releases page](https://github.com/CloudzyVPS/cli/releases).

Available platforms:
- **Linux**: x86_64 and aarch64 (GNU libc)
  - `zy-{version}-x86_64-unknown-linux-gnu.tar.gz` - Linux x86_64
  - `zy-{version}-aarch64-unknown-linux-gnu.tar.gz` - Linux ARM64
- **macOS**: Intel and Apple Silicon
  - `zy-{version}-x86_64-apple-darwin.tar.gz` - macOS Intel
  - `zy-{version}-aarch64-apple-darwin.tar.gz` - macOS Apple Silicon
- **Windows**: x86_64
  - `zy-{version}-x86_64-pc-windows-msvc.zip` - Windows 64-bit

### Platform Notes

**Linux**: We provide binaries for systems with GNU libc (glibc). These binaries work on most modern Linux distributions including Ubuntu, Debian, Fedora, CentOS, and others. MUSL-based static binaries are not provided due to cross-compilation complexity with dependencies. If you need a static binary or use a MUSL-based distribution (like Alpine Linux), please build from source.

**aarch64 (ARM64)**: ARM64 Linux binaries are cross-compiled and should work on ARM64 Linux systems with glibc. If you encounter issues, please report them or build from source on your target platform.

**Compatibility**: The binaries require:
- Linux: glibc 2.31+ (Ubuntu 20.04+, Debian 11+, or equivalent)
- macOS: macOS 10.15+ (Catalina or later)
- Windows: Windows 10 or later

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

### Platform Support Decisions

**Why no MUSL targets?**
MUSL-based static binaries were removed from CI/releases due to cross-compilation complexity. While Rust projects can target MUSL, managing dependencies (especially when involving pkg-config and various system libraries) during cross-compilation proved problematic and increased maintenance burden. Users needing static binaries or MUSL support can:
- Build from source on their target system
- Use containerized builds with appropriate toolchains
- Request specific target support through GitHub issues if there's sufficient demand

**aarch64 (ARM64) Support:**
ARM64 Linux builds use the `cross` tool for reliable cross-compilation. These binaries are tested in CI but may encounter issues with specific ARM64 configurations. If you experience problems, please report them via GitHub issues or build from source on your native ARM64 system.

**Future Target Extensions:**
To add new platforms or architectures:
1. Test thoroughly with all project dependencies
2. Prefer native compilation when possible
3. Document any special build requirements
4. Consider maintenance burden vs. user demand

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
