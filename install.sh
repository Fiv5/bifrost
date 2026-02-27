#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_step() {
    echo -e "${BLUE}==>${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}!${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

INSTALL_DIR="${BIFROST_INSTALL_DIR:-$HOME/.local/bin}"

show_help() {
    echo "Bifrost Installation Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --dir <path>      Custom installation directory (default: ~/.local/bin)"
    echo "  --help            Show this help message"
    echo ""
    echo "Environment variables:"
    echo "  BIFROST_INSTALL_DIR    Custom installation directory"
    echo ""
    echo "Examples:"
    echo "  $0                     Build and install bifrost CLI"
    echo "  $0 --dir /usr/local/bin"
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --help)
            show_help
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

if ! command -v cargo &> /dev/null; then
    print_error "Rust toolchain not found. Please install Rust first:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

print_step "Install directory: $INSTALL_DIR"

mkdir -p "$INSTALL_DIR"

clear_xattr() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        xattr -c "$1" 2>/dev/null || true
        xattr -d com.apple.provenance "$1" 2>/dev/null || true
        xattr -d com.apple.quarantine "$1" 2>/dev/null || true
    fi
}

print_step "Ensuring frontend is built..."
WEB_DIR="$SCRIPT_DIR/web"
DIST_DIR="$WEB_DIR/dist"

if [ -d "$WEB_DIR" ]; then
    if [ ! -d "$WEB_DIR/node_modules" ]; then
        print_step "Installing frontend dependencies..."
        (cd "$WEB_DIR" && pnpm install)
    fi
    
    if [ ! -f "$DIST_DIR/index.html" ] || [ ! -d "$DIST_DIR/assets" ]; then
        print_step "Building frontend..."
        (cd "$WEB_DIR" && pnpm run build)
    fi
    
    if [ ! -f "$DIST_DIR/index.html" ] || [ ! -d "$DIST_DIR/assets" ]; then
        print_error "Frontend build failed or incomplete"
        exit 1
    fi
    print_success "Frontend ready"
else
    print_warning "Web directory not found, skipping frontend build"
fi

print_step "Building and installing bifrost CLI (release mode)..."
touch "$SCRIPT_DIR/crates/bifrost-admin/build.rs"
cargo install --path crates/bifrost-cli --root "$INSTALL_DIR" --force
clear_xattr "$INSTALL_DIR/bin/bifrost"
mv "$INSTALL_DIR/bin/bifrost" "$INSTALL_DIR/bifrost"
rmdir "$INSTALL_DIR/bin" 2>/dev/null || true
rm -f "$INSTALL_DIR/.crates.toml" "$INSTALL_DIR/.crates2.json" 2>/dev/null || true
print_success "bifrost CLI installed successfully"

check_path_configured() {
    case "$SHELL" in
        */zsh)
            SHELL_RC="$HOME/.zshrc"
            ;;
        */bash)
            if [ -f "$HOME/.bash_profile" ]; then
                SHELL_RC="$HOME/.bash_profile"
            else
                SHELL_RC="$HOME/.bashrc"
            fi
            ;;
        */fish)
            SHELL_RC="$HOME/.config/fish/config.fish"
            ;;
        *)
            SHELL_RC="$HOME/.profile"
            ;;
    esac

    if [[ ":$PATH:" == *":$INSTALL_DIR:"* ]]; then
        return 0
    else
        return 1
    fi
}

add_to_path() {
    local shell_rc="$1"
    local path_line=""
    
    case "$SHELL" in
        */fish)
            path_line="set -gx PATH \"$INSTALL_DIR\" \$PATH"
            ;;
        *)
            path_line="export PATH=\"$INSTALL_DIR:\$PATH\""
            ;;
    esac
    
    if [ -f "$shell_rc" ] && grep -q "$INSTALL_DIR" "$shell_rc" 2>/dev/null; then
        print_warning "PATH already configured in $shell_rc"
        return
    fi
    
    echo "" >> "$shell_rc"
    echo "# Bifrost" >> "$shell_rc"
    echo "$path_line" >> "$shell_rc"
    print_success "Added $INSTALL_DIR to PATH in $shell_rc"
}

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
print_success "Installation completed!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "  bifrost:     $INSTALL_DIR/bifrost"

echo ""

if ! check_path_configured; then
    print_warning "$INSTALL_DIR is not in your PATH"
    echo ""
    
    read -p "Would you like to add it to your shell configuration? [Y/n] " -n 1 -r
    echo ""
    
    if [[ $REPLY =~ ^[Yy]$ ]] || [[ -z $REPLY ]]; then
        add_to_path "$SHELL_RC"
        echo ""
        print_warning "Please restart your terminal or run:"
        echo "  source $SHELL_RC"
    else
        echo ""
        echo "To add manually, add this line to your shell configuration file:"
        echo ""
        case "$SHELL" in
            */fish)
                echo "  set -gx PATH \"$INSTALL_DIR\" \$PATH"
                ;;
            *)
                echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
                ;;
        esac
    fi
else
    print_success "$INSTALL_DIR is already in your PATH"
    echo ""
    echo "You can now run:"
    echo "  bifrost --help"
fi

echo ""
