#!/usr/bin/env bash

# Package script for Zy CLI Release Workflow
#
# This script creates properly named archive files for each target platform.
# It includes the binary, README.md, and LICENSE (if present) in each archive.
#
# Expected structure:
# - artifacts/binary-{target}/zy (or zy.exe for Windows)
# - README.md (in repository root)
# - LICENSE (optional, in repository root)
#
# Output:
# - dist/zy-{version}-{target}.tar.gz (for Linux/macOS)
# - dist/zy-{version}-{target}.zip (for Windows)
#
# Each archive contains:
# - zy (or zy.exe)
# - README.md
# - LICENSE (if present)

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

# Function to create a tar.gz archive
create_tar_archive() {
    local target=$1
    local binary_name=$2
    local archive_name="zy-${VERSION}-${target}.tar.gz"
    
    echo "Creating ${archive_name}..."
    
    # Create temporary directory for staging
    local temp_dir="temp-${target}"
    mkdir -p "${temp_dir}"
    
    # Copy binary
    cp "artifacts/binary-${target}/${binary_name}" "${temp_dir}/zy"
    chmod +x "${temp_dir}/zy"
    
    # Copy README
    cp README.md "${temp_dir}/"
    
    # Copy LICENSE if it exists
    if [[ -f LICENSE ]]; then
        cp LICENSE "${temp_dir}/"
    fi
    
    # Create archive
    tar -czf "dist/${archive_name}" -C "${temp_dir}" .
    
    # Cleanup
    rm -rf "${temp_dir}"
    
    echo "Created ${archive_name}"
}

# Function to create a zip archive (for Windows)
create_zip_archive() {
    local target=$1
    local binary_name=$2
    local archive_name="zy-${VERSION}-${target}.zip"
    
    echo "Creating ${archive_name}..."
    
    # Create temporary directory for staging
    local temp_dir="temp-${target}"
    mkdir -p "${temp_dir}"
    
    # Copy binary (keep .exe extension for Windows)
    cp "artifacts/binary-${target}/${binary_name}" "${temp_dir}/"
    
    # Copy README
    cp README.md "${temp_dir}/"
    
    # Copy LICENSE if it exists
    if [[ -f LICENSE ]]; then
        cp LICENSE "${temp_dir}/"
    fi
    
    # Create archive
    (cd "${temp_dir}" && zip -r "../dist/${archive_name}" .)
    
    # Cleanup
    rm -rf "${temp_dir}"
    
    echo "Created ${archive_name}"
}

# Package each target
# Linux GNU targets
if [[ -d "artifacts/binary-x86_64-unknown-linux-gnu" ]]; then
    create_tar_archive "x86_64-unknown-linux-gnu" "zy"
fi

if [[ -d "artifacts/binary-aarch64-unknown-linux-gnu" ]]; then
    create_tar_archive "aarch64-unknown-linux-gnu" "zy"
fi

# macOS targets
if [[ -d "artifacts/binary-x86_64-apple-darwin" ]]; then
    create_tar_archive "x86_64-apple-darwin" "zy"
fi

if [[ -d "artifacts/binary-aarch64-apple-darwin" ]]; then
    create_tar_archive "aarch64-apple-darwin" "zy"
fi

# Windows target
if [[ -d "artifacts/binary-x86_64-pc-windows-msvc" ]]; then
    create_zip_archive "x86_64-pc-windows-msvc" "zy.exe"
fi

echo ""
echo "Packaging complete!"
echo "Created archives:"
ls -lh dist/
