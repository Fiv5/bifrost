#!/bin/bash
set -e

REPO="bifrost-proxy/bifrost"
BINARY_NAME="bifrost"
INSTALL_DIR="${BIFROST_INSTALL_DIR:-$HOME/.local/bin}"

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
    echo "║   ____  _  __                _                            ║"
    echo "║  |  _ \(_)/ _|_ __ ___  ___| |_                           ║"
    echo "║  | |_) | | |_| '__/ _ \/ __| __|                          ║"
    echo "║  |  _ <| |  _| | | (_) \__ \ |_                           ║"
    echo "║  |_| \_\_|_| |_|  \___/|___/\__|                          ║"
    echo "║                                                           ║"
    echo "║   High-performance HTTP/HTTPS/SOCKS5 Proxy Server         ║"
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
    printf "%b!%b %s\n" "${YELLOW}" "${NC}" "$1" >&2
}

print_error() {
    printf "%b✗%b %s\n" "${RED}" "${NC}" "$1" >&2
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

detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)     echo "x86_64" ;;
        aarch64|arm64)    echo "aarch64" ;;
        armv7l|armv7)     echo "armv7" ;;
        *)                echo "unknown" ;;
    esac
}

get_target() {
    local os="$1"
    local arch="$2"

    case "$os" in
        linux)
            case "$arch" in
                x86_64)  echo "x86_64-unknown-linux-gnu" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
                armv7)   echo "armv7-unknown-linux-gnueabihf" ;;
                *)       return 1 ;;
            esac
            ;;
        darwin)
            case "$arch" in
                x86_64)  echo "x86_64-apple-darwin" ;;
                aarch64) echo "aarch64-apple-darwin" ;;
                *)       return 1 ;;
            esac
            ;;
        windows)
            case "$arch" in
                x86_64)  echo "x86_64-pc-windows-msvc" ;;
                aarch64) echo "aarch64-pc-windows-msvc" ;;
                *)       return 1 ;;
            esac
            ;;
        *)
            return 1
            ;;
    esac
}

get_archive_ext() {
    local os="$1"
    case "$os" in
        windows) echo "zip" ;;
        *)       echo "tar.gz" ;;
    esac
}

get_latest_version() {
    local all_releases_url="https://api.github.com/repos/${REPO}/releases?per_page=10"
    local response version is_prerelease

    if command -v curl &> /dev/null; then
        response=$(curl -sL "$all_releases_url")
    elif command -v wget &> /dev/null; then
        response=$(wget -qO- "$all_releases_url")
    else
        print_error "Neither curl nor wget found. Please install one of them." >&2
        exit 1
    fi

    if echo "$response" | grep -q '"message"[[:space:]]*:[[:space:]]*"API rate limit exceeded'; then
        print_error "GitHub API rate limit exceeded"
        print_warning "Please try again later or specify a version manually:"
        echo "  curl -fsSL ... | bash -s -- --version v0.0.4-alpha" >&2
        exit 1
    fi

    if echo "$response" | grep -q '"message"[[:space:]]*:[[:space:]]*"Not Found"'; then
        print_error "Repository not found: ${REPO}"
        exit 1
    fi

    version=$(echo "$response" | grep -B5 '"prerelease"[[:space:]]*:[[:space:]]*false' | grep '"tag_name":' | head -1 | sed -E 's/.*"([^"]+)".*/\1/')

    if [[ -n "$version" ]]; then
        echo "$version"
        return 0
    fi

    print_warning "No stable release found, checking for pre-releases..."
    version=$(echo "$response" | grep '"tag_name":' | head -1 | sed -E 's/.*"([^"]+)".*/\1/')

    if [[ -z "$version" ]]; then
        print_error "No releases found for ${REPO}"
        print_warning "The project may not have published any releases yet."
        echo "" >&2
        echo "You can build from source instead:" >&2
        echo "  git clone https://github.com/${REPO}.git" >&2
        echo "  cd bifrost && ./install.sh" >&2
        exit 1
    fi

    echo "$version"
}

download_file() {
    local url="$1"
    local output="$2"

    print_step "Downloading from: $url"

    if command -v curl &> /dev/null; then
        curl -fsSL "$url" -o "$output"
    elif command -v wget &> /dev/null; then
        wget -q "$url" -O "$output"
    else
        print_error "Neither curl nor wget found"
        exit 1
    fi
}

verify_checksum() {
    local file="$1"
    local expected="$2"
    local actual

    if command -v sha256sum &> /dev/null; then
        actual=$(sha256sum "$file" | awk '{print $1}')
    elif command -v shasum &> /dev/null; then
        actual=$(shasum -a 256 "$file" | awk '{print $1}')
    else
        print_warning "sha256sum/shasum not found, skipping checksum verification"
        return 0
    fi

    if [[ "$actual" != "$expected" ]]; then
        print_error "Checksum verification failed!"
        print_error "Expected: $expected"
        print_error "Actual:   $actual"
        return 1
    fi

    print_success "Checksum verified"
    return 0
}

extract_archive() {
    local archive="$1"
    local dest="$2"
    local os="$3"

    case "$os" in
        windows)
            if command -v unzip &> /dev/null; then
                unzip -q "$archive" -d "$dest"
            elif command -v powershell.exe &> /dev/null; then
                powershell.exe -Command "Expand-Archive -Path '$archive' -DestinationPath '$dest' -Force"
            elif command -v pwsh &> /dev/null; then
                pwsh -Command "Expand-Archive -Path '$archive' -DestinationPath '$dest' -Force"
            else
                print_error "Neither unzip nor PowerShell found"
                print_warning "Please install unzip or use the PowerShell installer:"
                echo "  irm https://raw.githubusercontent.com/${REPO}/main/install-binary.ps1 | iex"
                exit 1
            fi
            ;;
        *)
            tar -xzf "$archive" -C "$dest"
            ;;
    esac
}

clear_xattr() {
    local file="$1"
    if [[ "$(detect_os)" == "darwin" ]]; then
        xattr -c "$file" 2>/dev/null || true
        xattr -d com.apple.provenance "$file" 2>/dev/null || true
        xattr -d com.apple.quarantine "$file" 2>/dev/null || true
    fi
}

show_help() {
    echo "Bifrost Installation Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --version <VERSION>   Install a specific version (e.g., v0.1.0)"
    echo "  --dir <PATH>          Installation directory (default: ~/.local/bin)"
    echo "  --help                Show this help message"
    echo ""
    echo "Environment variables:"
    echo "  BIFROST_INSTALL_DIR   Custom installation directory"
    echo ""
    echo "Examples:"
    echo "  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install-binary.sh | bash"
    echo "  curl -fsSL ... | bash -s -- --version v0.0.4-alpha"
    echo "  curl -fsSL ... | bash -s -- --dir /usr/local/bin"
}

VERSION=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
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

main() {
    print_banner

    local os arch target ext

    os=$(detect_os)
    arch=$(detect_arch)

    print_step "Detecting system..."
    echo "  OS:           $os"
    echo "  Architecture: $arch"

    if [[ "$os" == "unknown" ]]; then
        print_error "Unsupported operating system"
        exit 1
    fi

    if [[ "$arch" == "unknown" ]]; then
        print_error "Unsupported architecture"
        exit 1
    fi

    target=$(get_target "$os" "$arch")
    if [[ -z "$target" ]]; then
        print_error "No pre-built binary available for $os-$arch"
        print_warning "You can build from source instead:"
        echo "  git clone https://github.com/${REPO}.git"
        echo "  cd bifrost && ./install.sh"
        exit 1
    fi

    ext=$(get_archive_ext "$os")

    if [[ -z "$VERSION" ]]; then
        print_step "Fetching latest version..."
        VERSION=$(get_latest_version)
    fi

    print_success "Installing version: $VERSION"
    echo "  Target: $target"

    mkdir -p "$INSTALL_DIR"

    local tmpdir
    tmpdir=$(mktemp -d)
    trap "rm -rf '$tmpdir'" EXIT

    print_step "Installing CLI..."

    local cli_archive="bifrost-${VERSION}-${target}.${ext}"
    local cli_url="https://github.com/${REPO}/releases/download/${VERSION}/${cli_archive}"
    local checksums_url="https://github.com/${REPO}/releases/download/${VERSION}/bifrost-${VERSION}-checksums.txt"

    download_file "$cli_url" "$tmpdir/$cli_archive"

    print_step "Downloading checksums..."
    download_file "$checksums_url" "$tmpdir/checksums.txt"

    local expected_checksum
    expected_checksum=$(grep "$cli_archive" "$tmpdir/checksums.txt" | awk '{print $1}')
    if [[ -n "$expected_checksum" ]]; then
        verify_checksum "$tmpdir/$cli_archive" "$expected_checksum"
    else
        print_warning "Checksum not found for $cli_archive, skipping verification"
    fi

    print_step "Extracting..."
    extract_archive "$tmpdir/$cli_archive" "$tmpdir" "$os"

    local extracted_dir="bifrost-${VERSION}-${target}"
    local binary_name="bifrost"
    [[ "$os" == "windows" ]] && binary_name="bifrost.exe"

    if [[ -f "$tmpdir/$extracted_dir/$binary_name" ]]; then
        cp "$tmpdir/$extracted_dir/$binary_name" "$INSTALL_DIR/$binary_name"
    elif [[ -f "$tmpdir/$binary_name" ]]; then
        cp "$tmpdir/$binary_name" "$INSTALL_DIR/$binary_name"
    else
        print_error "Binary not found in archive"
        exit 1
    fi

    chmod +x "$INSTALL_DIR/$binary_name"
    clear_xattr "$INSTALL_DIR/$binary_name"

    print_success "CLI installed: $INSTALL_DIR/$binary_name"

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print_success "Installation completed!"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        print_warning "$INSTALL_DIR is not in your PATH"
        echo ""
        echo "Add it to your shell configuration:"
        echo ""
        case "$SHELL" in
            */fish)
                echo "  echo 'set -gx PATH \"$INSTALL_DIR\" \$PATH' >> ~/.config/fish/config.fish"
                ;;
            */zsh)
                echo "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.zshrc"
                ;;
            *)
                echo "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.bashrc"
                ;;
        esac
        echo ""
        echo "Then restart your terminal or run:"
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    fi

    echo ""
    echo "Getting started:"
    echo ""
    echo "  # Start proxy server"
    echo "  bifrost start"
    echo ""
    echo "  # Start with custom port"
    echo "  bifrost -p 8080 start"
    echo ""
    echo "  # Show help"
    echo "  bifrost --help"
    echo ""
    echo "Documentation: https://github.com/${REPO}"
    echo ""
}

main "$@"
