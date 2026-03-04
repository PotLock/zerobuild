#!/usr/bin/env bash
#
# ZeroBuild One-Click Installer
# 
# Install ZeroBuild quickly using:
#   curl -fsSL https://raw.githubusercontent.com/PotLock/zerobuild/main/install.sh | bash
#
# Or with wget:
#   wget -qO- https://raw.githubusercontent.com/PotLock/zerobuild/main/install.sh | bash

set -euo pipefail

REPO="PotLock/zerobuild"
REGISTRY="ghcr.io"
IMAGE="$REGISTRY/$REPO"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

info() {
  echo -e "\033[34m==>\033[0m $*"
}

error() {
  echo -e "\033[31merror:\033[0m $*" >&2
}

success() {
  echo -e "\033[32m✓\033[0m $*"
}

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

detect_platform() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"

  case "$os:$arch" in
    linux:x86_64)
      echo "linux-amd64"
      ;;
    linux:aarch64|linux:arm64)
      echo "linux-arm64"
      ;;
    darwin:x86_64)
      echo "darwin-amd64"
      ;;
    darwin:arm64|darwin:aarch64)
      echo "darwin-arm64"
      ;;
    *)
      echo ""
      ;;
  esac
}

install_docker() {
  info "Installing ZeroBuild via Docker..."
  
  if ! have_cmd docker && ! have_cmd podman; then
    error "Docker or Podman is required but not installed."
    error "Please install Docker: https://docs.docker.com/get-docker/"
    exit 1
  fi

  local container_cmd="docker"
  if ! have_cmd docker && have_cmd podman; then
    container_cmd="podman"
    info "Using Podman instead of Docker"
  fi

  # Pull the latest image
  info "Pulling ZeroBuild image from GitHub Container Registry..."
  $container_cmd pull "$IMAGE:latest"

  # Create data directory
  local data_dir="$HOME/.zerobuild"
  mkdir -p "$data_dir"/{workspace,.zerobuild}

  # Create wrapper script
  cat > "$INSTALL_DIR/zerobuild" <<EOF
#!/usr/bin/env bash
# ZeroBuild Docker wrapper

exec $container_cmd run --rm -it \\
  -v "$data_dir/.zerobuild:/zerobuild-data/.zerobuild" \\
  -v "$data_dir/workspace:/zerobuild-data/workspace" \\
  -e HOME=/zerobuild-data \\
  -e ZEROBUILD_WORKSPACE=/zerobuild-data/workspace \\
  -p 42617:42617 \\
  $IMAGE:latest \\
  "\$@"
EOF
  chmod +x "$INSTALL_DIR/zerobuild"

  success "ZeroBuild installed successfully!"
  info "Data directory: $data_dir"
  info "Usage: zerobuild <command>"
  info "Example: zerobuild --help"
  
  # Check if in PATH
  if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    info ""
    info "Add to your PATH:"
    echo "export PATH=\"$INSTALL_DIR:\$PATH\""
    info "Or run: echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.bashrc"
  fi
}

install_binary() {
  local platform="$(detect_platform)"
  
  if [[ -z "$platform" ]]; then
    error "Unsupported platform: $(uname -s) $(uname -m)"
    error "Try Docker installation instead:"
    echo "  curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | bash -s -- --docker"
    exit 1
  fi

  info "Installing ZeroBuild binary for $platform..."

  # Create temp directory
  local tmp_dir="$(mktemp -d)"
  trap "rm -rf $tmp_dir" EXIT

  # Download binary
  local download_url="https://github.com/$REPO/releases/latest/download/zerobuild-$platform.tar.gz"
  info "Downloading from $download_url..."
  
  if ! curl -fsSL "$download_url" -o "$tmp_dir/zerobuild.tar.gz"; then
    error "Failed to download binary"
    error "The binary may not be available yet. Try Docker installation:"
    echo "  curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | bash -s -- --docker"
    exit 1
  fi

  # Extract
  tar -xzf "$tmp_dir/zerobuild.tar.gz" -C "$tmp_dir"
  
  # Install
  mkdir -p "$INSTALL_DIR"
  mv "$tmp_dir/zerobuild" "$INSTALL_DIR/zerobuild"
  chmod +x "$INSTALL_DIR/zerobuild"
  
  success "ZeroBuild binary installed to $INSTALL_DIR/zerobuild"
}

install_source() {
  info "Building ZeroBuild from source..."
  
  if ! have_cmd cargo; then
    error "Rust/Cargo is required to build from source."
    error "Install Rust: https://rustup.rs/"
    error "Or use Docker installation:"
    echo "  curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | bash -s -- --docker"
    exit 1
  fi

  # Clone repo
  local tmp_dir="$(mktemp -d)"
  trap "rm -rf $tmp_dir" EXIT
  
  info "Cloning repository..."
  git clone --depth 1 "https://github.com/$REPO.git" "$tmp_dir/zerobuild"
  
  info "Building (this may take a few minutes)..."
  cd "$tmp_dir/zerobuild"
  cargo build --release --locked
  
  # Install
  mkdir -p "$INSTALL_DIR"
  cp "$tmp_dir/zerobuild/target/release/zerobuild" "$INSTALL_DIR/zerobuild"
  chmod +x "$INSTALL_DIR/zerobuild"
  
  success "ZeroBuild built and installed to $INSTALL_DIR/zerobuild"
}

# Main installation logic
main() {
  local method="${1:-auto}"

  # Parse arguments
  case "$method" in
    --docker|-d)
      method="docker"
      ;;
    --binary|-b)
      method="binary"
      ;;
    --source|-s)
      method="source"
      ;;
    --help|-h)
      cat <<'EOF'
ZeroBuild One-Click Installer

Usage:
  curl -fsSL https://raw.githubusercontent.com/PotLock/zerobuild/main/install.sh | bash
  
Options:
  --docker, -d     Install using Docker (recommended, fastest)
  --binary, -b     Install pre-built binary
  --source, -s     Build and install from source
  --help, -h       Show this help message

Environment:
  INSTALL_DIR      Installation directory (default: ~/.local/bin)

Examples:
  # Default: auto-detect best method
  curl -fsSL ... | bash
  
  # Force Docker installation
  curl -fsSL ... | bash -s -- --docker
  
  # Install to custom directory
  INSTALL_DIR=/usr/local/bin curl -fsSL ... | bash
EOF
      exit 0
      ;;
  esac

  echo ""
  echo "  ╔══════════════════════════════════════════╗"
  echo "  ║         ZeroBuild Installer              ║"
  echo "  ║  Autonomous Software Factory Agent       ║"
  echo "  ╚══════════════════════════════════════════╝"
  echo ""

  # Auto-detect method
  if [[ "$method" == "auto" ]]; then
    if have_cmd docker || have_cmd podman; then
      method="docker"
      info "Docker detected, using Docker installation method"
    elif have_cmd cargo; then
      method="source"
      info "Cargo detected, building from source"
    else
      method="binary"
      info "Attempting binary installation"
    fi
  fi

  # Run installation
  case "$method" in
    docker)
      install_docker
      ;;
    binary)
      install_binary
      ;;
    source)
      install_source
      ;;
    *)
      error "Unknown installation method: $method"
      exit 1
      ;;
  esac

  # Verify installation
  if [[ -x "$INSTALL_DIR/zerobuild" ]]; then
    echo ""
    success "Installation complete!"
    echo ""
    info "Next steps:"
    echo "  1. Run: export PATH=\"$INSTALL_DIR:\$PATH\"  (if not in PATH)"
    echo "  2. Setup: zerobuild onboard"
    echo "  3. Usage: zerobuild --help"
    echo ""
    info "Documentation: https://github.com/$REPO"
    info "Support: https://github.com/$REPO/issues"
  fi
}

main "$@"
