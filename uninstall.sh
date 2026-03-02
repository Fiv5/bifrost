#!/bin/bash
set -e

BINARY_NAME="bifrost"
DEFAULT_INSTALL_DIR="$HOME/.local/bin"
DEFAULT_DATA_DIR="$HOME/.bifrost"
DEFAULT_CONFIG_DIR="$HOME/.config/bifrost"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

print_banner() {
    printf "%b" "${CYAN}"
    echo "╔═══════════════════════════════════════════════════════════╗"
    echo "║                                                           ║"
    echo "║   Bifrost Uninstaller                                     ║"
    echo "║                                                           ║"
    echo "╚═══════════════════════════════════════════════════════════╝"
    printf "%b\n" "${NC}"
}

print_step() {
    printf "%b==>%b %s\n" "${BLUE}" "${NC}" "$1"
}

print_success() {
    printf "%b✓%b %s\n" "${GREEN}" "${NC}" "$1"
}

print_warning() {
    printf "%b!%b %s\n" "${YELLOW}" "${NC}" "$1"
}

print_error() {
    printf "%b✗%b %s\n" "${RED}" "${NC}" "$1"
}

detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "darwin" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *)       echo "unknown" ;;
    esac
}

show_help() {
    echo "Bifrost Uninstallation Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --dir <PATH>          Installation directory (default: ~/.local/bin)"
    echo "  --purge               Also remove configuration and data files"
    echo "  --yes, -y             Skip confirmation prompts"
    echo "  --help                Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    # Interactive uninstall"
    echo "  $0 --purge            # Remove binary and all data"
    echo "  $0 --yes --purge      # Non-interactive full removal"
}

INSTALL_DIR="$DEFAULT_INSTALL_DIR"
PURGE=false
YES=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --purge)
            PURGE=true
            shift
            ;;
        --yes|-y)
            YES=true
            shift
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

confirm() {
    local prompt="$1"
    if [[ "$YES" == "true" ]]; then
        return 0
    fi
    
    printf "%s [y/N] " "$prompt"
    read -r response
    case "$response" in
        [yY][eE][sS]|[yY]) return 0 ;;
        *) return 1 ;;
    esac
}

main() {
    print_banner

    local os binary_path
    os=$(detect_os)
    
    local binary_name="$BINARY_NAME"
    [[ "$os" == "windows" ]] && binary_name="$BINARY_NAME.exe"
    
    binary_path="$INSTALL_DIR/$binary_name"

    print_step "Checking installation..."

    local found_binary=false
    local found_data=false
    local found_config=false

    if [[ -f "$binary_path" ]]; then
        found_binary=true
        echo "  Binary:  $binary_path"
    fi

    if [[ -d "$DEFAULT_DATA_DIR" ]]; then
        found_data=true
        echo "  Data:    $DEFAULT_DATA_DIR"
    fi

    if [[ -d "$DEFAULT_CONFIG_DIR" ]]; then
        found_config=true
        echo "  Config:  $DEFAULT_CONFIG_DIR"
    fi

    if [[ "$found_binary" == "false" && "$found_data" == "false" && "$found_config" == "false" ]]; then
        print_warning "Bifrost is not installed or already uninstalled"
        echo ""
        echo "Checked locations:"
        echo "  Binary: $binary_path"
        echo "  Data:   $DEFAULT_DATA_DIR"
        echo "  Config: $DEFAULT_CONFIG_DIR"
        exit 0
    fi

    echo ""

    if [[ "$PURGE" == "true" ]]; then
        print_warning "This will remove the binary and ALL data/configuration files!"
    fi

    if ! confirm "Do you want to proceed with uninstallation?"; then
        echo "Uninstallation cancelled."
        exit 0
    fi

    echo ""

    if [[ "$found_binary" == "true" ]]; then
        print_step "Removing binary..."
        rm -f "$binary_path"
        print_success "Removed: $binary_path"
    fi

    if [[ "$PURGE" == "true" ]]; then
        if [[ "$found_data" == "true" ]]; then
            print_step "Removing data directory..."
            rm -rf "$DEFAULT_DATA_DIR"
            print_success "Removed: $DEFAULT_DATA_DIR"
        fi

        if [[ "$found_config" == "true" ]]; then
            print_step "Removing config directory..."
            rm -rf "$DEFAULT_CONFIG_DIR"
            print_success "Removed: $DEFAULT_CONFIG_DIR"
        fi
    else
        if [[ "$found_data" == "true" || "$found_config" == "true" ]]; then
            echo ""
            print_warning "Data and config files were preserved."
            echo "  To remove them, run: $0 --purge"
            if [[ "$found_data" == "true" ]]; then
                echo "  Data:   $DEFAULT_DATA_DIR"
            fi
            if [[ "$found_config" == "true" ]]; then
                echo "  Config: $DEFAULT_CONFIG_DIR"
            fi
        fi
    fi

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print_success "Uninstallation completed!"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    if [[ ":$PATH:" == *":$INSTALL_DIR:"* ]]; then
        print_warning "You may want to remove $INSTALL_DIR from your PATH"
        echo ""
        echo "Edit your shell configuration file and remove the PATH entry:"
        case "$SHELL" in
            */fish)
                echo "  ~/.config/fish/config.fish"
                ;;
            */zsh)
                echo "  ~/.zshrc"
                ;;
            *)
                echo "  ~/.bashrc"
                ;;
        esac
    fi

    echo ""
}

main "$@"
