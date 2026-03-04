#!/usr/bin/env bash
#
# ZeroBuild Install Script
# 
# This script provides a quick way to install ZeroBuild.
# It delegates to the main install.sh for consistency.
#
# Quick install (one-liner):
#   curl -fsSL https://raw.githubusercontent.com/PotLock/zerobuild/main/install.sh | bash
#
# Or from local repo:
#   ./zerobuild_install.sh

set -euo pipefail

info() {
  echo "==> $*"
}

error() {
  echo "error: $*" >&2
}

# Check if running from repo or remotely
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ -f "$SCRIPT_DIR/install.sh" ]]; then
  # Running from repo - use local install script
  info "Running local install script..."
  exec bash "$SCRIPT_DIR/install.sh" "$@"
elif [[ -f "$SCRIPT_DIR/scripts/bootstrap.sh" ]]; then
  # Fallback to bootstrap for development
  info "Running bootstrap script..."
  exec bash "$SCRIPT_DIR/scripts/bootstrap.sh" "$@"
else
  # Running remotely or standalone - download and run main install script
  info "Downloading ZeroBuild installer..."
  
  if command -v curl >/dev/null 2>&1; then
    exec bash -c "$(curl -fsSL https://raw.githubusercontent.com/PotLock/zerobuild/main/install.sh)" -- "$@"
  elif command -v wget >/dev/null 2>&1; then
    exec bash -c "$(wget -qO- https://raw.githubusercontent.com/PotLock/zerobuild/main/install.sh)" -- "$@"
  else
    error "curl or wget is required to download the installer"
    exit 1
  fi
fi
