#!/usr/bin/env bash

# Package script for Zy CLI Release Workflow
#
# This script prepares properly named binaries for each target platform.
#
# Expected structure:
# - artifacts/binary-{target}/zy (or zy.exe for Windows)
#
# Output:
# - dist/zy-{version}-{target} (Linux/macOS binaries)
# - dist/zy-{version}-{target}.exe (Windows binary)

set -euo pipefail

# Get version from git tag or use a default
VERSION="${GITHUB_REF_NAME#v}"
if [[ -z "${VERSION}" || "${VERSION}" == "${GITHUB_REF_NAME}" ]]; then
    VERSION="$(git describe --tags --always 2>/dev/null || echo 'dev')"
    VERSION="${VERSION#v}"
fi

echo "Packaging version: ${VERSION}"

# Create dist directory
mkdir -p dist

# Function to prepare a binary for release
prepare_binary() {
    local target=$1
    local binary_name=$2
    local output_name="zy-${VERSION}-${target}"
    
    echo "Preparing ${output_name}..."
    
    # Copy and rename binary
    if [[ "$binary_name" == *.exe ]]; then
        cp "artifacts/binary-${target}/${binary_name}" "dist/${output_name}.exe"
        echo "Created ${output_name}.exe"
    else
        cp "artifacts/binary-${target}/${binary_name}" "dist/${output_name}"
        chmod +x "dist/${output_name}"
        echo "Created ${output_name}"
    fi
}

# Package each target
# Linux GNU targets
if [[ -d "artifacts/binary-x86_64-unknown-linux-gnu" ]]; then
    prepare_binary "x86_64-unknown-linux-gnu" "zy"
fi

if [[ -d "artifacts/binary-aarch64-unknown-linux-gnu" ]]; then
    prepare_binary "aarch64-unknown-linux-gnu" "zy"
fi

# macOS targets
if [[ -d "artifacts/binary-x86_64-apple-darwin" ]]; then
    prepare_binary "x86_64-apple-darwin" "zy"
fi

if [[ -d "artifacts/binary-aarch64-apple-darwin" ]]; then
    prepare_binary "aarch64-apple-darwin" "zy"
fi

# Windows target
if [[ -d "artifacts/binary-x86_64-pc-windows-msvc" ]]; then
    prepare_binary "x86_64-pc-windows-msvc" "zy.exe"
fi

echo ""
echo "Packaging complete!"
echo "Created binaries:"
ls -lh dist/
