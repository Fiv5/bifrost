#!/bin/bash

set -e

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
REMOVE_PATH_CONFIG=false
FORCE=false

show_help() {
    echo "Bifrost Uninstallation Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --dir <path>      Custom installation directory (default: ~/.local/bin)"
    echo "  --clean-path      Also remove PATH configuration from shell rc files"
    echo "  --force           Skip confirmation prompt"
    echo "  --help            Show this help message"
    echo ""
    echo "Environment variables:"
    echo "  BIFROST_INSTALL_DIR    Custom installation directory"
    echo ""
    echo "Examples:"
    echo "  $0                     Uninstall bifrost binaries"
    echo "  $0 --clean-path        Uninstall and remove PATH configuration"
    echo "  $0 --dir /usr/local/bin"
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --clean-path)
            REMOVE_PATH_CONFIG=true
            shift
            ;;
        --force)
            FORCE=true
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

BIFROST_CLI="$INSTALL_DIR/bifrost"
BIFROST_GUI="$INSTALL_DIR/bifrost-gui"

CLI_EXISTS=false
GUI_EXISTS=false

if [ -f "$BIFROST_CLI" ]; then
    CLI_EXISTS=true
fi

if [ -f "$BIFROST_GUI" ]; then
    GUI_EXISTS=true
fi

if [ "$CLI_EXISTS" = false ] && [ "$GUI_EXISTS" = false ]; then
    print_warning "No Bifrost installation found in $INSTALL_DIR"
    exit 0
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "The following files will be removed:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

if [ "$CLI_EXISTS" = true ]; then
    echo "  $BIFROST_CLI"
fi
if [ "$GUI_EXISTS" = true ]; then
    echo "  $BIFROST_GUI"
fi

if [ "$REMOVE_PATH_CONFIG" = true ]; then
    echo ""
    echo "PATH configuration will also be removed from shell rc files."
fi

echo ""

if [ "$FORCE" = false ]; then
    read -p "Are you sure you want to uninstall? [y/N] " -n 1 -r
    echo ""
    
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_warning "Uninstallation cancelled"
        exit 0
    fi
fi

echo ""

if [ "$CLI_EXISTS" = true ]; then
    print_step "Removing bifrost CLI..."
    rm -f "$BIFROST_CLI"
    print_success "Removed $BIFROST_CLI"
fi

if [ "$GUI_EXISTS" = true ]; then
    print_step "Removing bifrost GUI..."
    rm -f "$BIFROST_GUI"
    print_success "Removed $BIFROST_GUI"
fi

remove_path_from_rc() {
    local rc_file="$1"
    
    if [ ! -f "$rc_file" ]; then
        return
    fi
    
    if grep -q "# Bifrost" "$rc_file" 2>/dev/null || grep -q "$INSTALL_DIR" "$rc_file" 2>/dev/null; then
        print_step "Cleaning PATH from $rc_file..."
        
        local temp_file=$(mktemp)
        local in_bifrost_block=false
        local removed=false
        
        while IFS= read -r line || [[ -n "$line" ]]; do
            if [[ "$line" == "# Bifrost" ]]; then
                in_bifrost_block=true
                removed=true
                continue
            fi
            
            if [ "$in_bifrost_block" = true ]; then
                if [[ "$line" == *"$INSTALL_DIR"* ]]; then
                    in_bifrost_block=false
                    continue
                fi
                in_bifrost_block=false
            fi
            
            echo "$line" >> "$temp_file"
        done < "$rc_file"
        
        if [ "$removed" = true ]; then
            sed -i.bak '/^$/N;/^\n$/d' "$temp_file" 2>/dev/null || sed -i '' '/^$/N;/^\n$/d' "$temp_file"
            mv "$temp_file" "$rc_file"
            rm -f "$rc_file.bak"
            print_success "Removed PATH configuration from $rc_file"
        else
            rm -f "$temp_file"
        fi
    fi
}

if [ "$REMOVE_PATH_CONFIG" = true ]; then
    echo ""
    print_step "Cleaning PATH configuration..."
    
    remove_path_from_rc "$HOME/.zshrc"
    remove_path_from_rc "$HOME/.bashrc"
    remove_path_from_rc "$HOME/.bash_profile"
    remove_path_from_rc "$HOME/.profile"
    remove_path_from_rc "$HOME/.config/fish/config.fish"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
print_success "Uninstallation completed!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

if [ "$REMOVE_PATH_CONFIG" = true ]; then
    print_warning "Please restart your terminal for PATH changes to take effect"
fi

echo ""
