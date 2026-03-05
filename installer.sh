#!/bin/sh
set -e

REPO="https://github.com/schneipp/y.git"
INSTALL_DIR="$HOME/.local/bin"

echo "Installing y editor..."

# Check for required tools
if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo not found. Install Rust first: https://rustup.rs"
    exit 1
fi

if ! command -v git >/dev/null 2>&1; then
    echo "Error: git not found."
    exit 1
fi

# Clone and build
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Cloning repository..."
git clone --depth 1 "$REPO" "$TMPDIR/y"

echo "Building (release)..."
cargo build --release --manifest-path "$TMPDIR/y/Cargo.toml"

# Install binary
mkdir -p "$INSTALL_DIR"
cp "$TMPDIR/y/target/release/y" "$INSTALL_DIR/y"
chmod +x "$INSTALL_DIR/y"

echo "Installed y to $INSTALL_DIR/y"

# Check if INSTALL_DIR is in PATH
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) echo "Note: $INSTALL_DIR is not in your PATH. Add it with:"
       echo "  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac

echo "Done. Run 'y' to start editing."
