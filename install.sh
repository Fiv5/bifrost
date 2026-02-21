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
BUILD_MODE="release"
INSTALL_CLI=true
INSTALL_GUI=false

show_help() {
    echo "Bifrost Installation Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --debug           Build in debug mode (default: release)"
    echo "  --gui             Also build and install bifrost-gui"
    echo "  --gui-only        Only build and install bifrost-gui"
    echo "  --dir <path>      Custom installation directory (default: ~/.local/bin)"
    echo "  --help            Show this help message"
    echo ""
    echo "Environment variables:"
    echo "  BIFROST_INSTALL_DIR    Custom installation directory"
    echo ""
    echo "Examples:"
    echo "  $0                     Build and install bifrost CLI"
    echo "  $0 --gui               Build and install both CLI and GUI"
    echo "  $0 --gui-only          Only build and install GUI"
    echo "  $0 --dir /usr/local/bin"
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --debug)
            BUILD_MODE="debug"
            shift
            ;;
        --gui)
            INSTALL_GUI=true
            shift
            ;;
        --gui-only)
            INSTALL_CLI=false
            INSTALL_GUI=true
            shift
            ;;
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

print_step "Build mode: $BUILD_MODE"
print_step "Install directory: $INSTALL_DIR"

mkdir -p "$INSTALL_DIR"

if [ "$BUILD_MODE" = "release" ]; then
    CARGO_BUILD_FLAGS="--release"
    TARGET_DIR="target/release"
else
    CARGO_BUILD_FLAGS=""
    TARGET_DIR="target/debug"
fi

if [ "$INSTALL_CLI" = true ]; then
    print_step "Building bifrost CLI..."
    cargo build --bin bifrost $CARGO_BUILD_FLAGS

    print_step "Installing bifrost to $INSTALL_DIR..."
    cp "$TARGET_DIR/bifrost" "$INSTALL_DIR/bifrost"
    chmod +x "$INSTALL_DIR/bifrost"
    print_success "bifrost CLI installed successfully"
fi

if [ "$INSTALL_GUI" = true ]; then
    print_step "Building bifrost GUI..."
    cargo build --bin bifrost-gui $CARGO_BUILD_FLAGS

    print_step "Installing bifrost-gui to $INSTALL_DIR..."
    cp "$TARGET_DIR/bifrost-gui" "$INSTALL_DIR/bifrost-gui"
    chmod +x "$INSTALL_DIR/bifrost-gui"
    print_success "bifrost-gui installed successfully"
fi

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

if [ "$INSTALL_CLI" = true ]; then
    echo "  bifrost:     $INSTALL_DIR/bifrost"
fi
if [ "$INSTALL_GUI" = true ]; then
    echo "  bifrost-gui: $INSTALL_DIR/bifrost-gui"
fi

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
    if [ "$INSTALL_CLI" = true ]; then
        echo "  bifrost --help"
    fi
    if [ "$INSTALL_GUI" = true ]; then
        echo "  bifrost-gui"
    fi
fi

echo ""
