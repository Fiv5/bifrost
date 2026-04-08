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

detect_libc() {
    if [ "$(detect_os)" != "linux" ]; then
        echo "gnu"
        return
    fi

    if command -v ldd &> /dev/null; then
        local ldd_output
        ldd_output=$(ldd --version 2>&1 || true)
        if echo "$ldd_output" | grep -qi "musl"; then
            echo "musl"
            return
        fi
        if echo "$ldd_output" | grep -qiE "GLIBC|GNU libc"; then
            echo "gnu"
            return
        fi
    fi

    if [ -f /lib/ld-musl-x86_64.so.1 ] || \
       [ -f /lib/ld-musl-aarch64.so.1 ] || \
       [ -f /lib/ld-musl-armhf.so.1 ]; then
        echo "musl"
        return
    fi

    echo "gnu"
}

MIN_GLIBC_VERSION="2.29"

get_glibc_version() {
    if [ "$(detect_os)" != "linux" ]; then
        return 1
    fi

    if ! command -v ldd >/dev/null 2>&1; then
        return 1
    fi

    local out version
    out=$(ldd --version 2>&1 || true)

    if ! echo "$out" | grep -qiE "GLIBC|GNU libc"; then
        return 1
    fi

    version=$(echo "$out" | head -1 | grep -oE '[0-9]+\.[0-9]+' | head -1)
    if [ -n "$version" ]; then
        echo "$version"
        return 0
    fi

    return 1
}

version_lt() {
    [ "$(printf '%s\n' "$1" "$2" | sort -V | head -n1)" != "$2" ]
}

verify_binary_runs() {
    local bin="$1"
    "$bin" --version >/dev/null 2>&1
}

get_linux_target() {
    local arch="$1"
    local libc="$2"
    local gnu_target musl_target

    case "$arch" in
        x86_64)
            gnu_target="x86_64-unknown-linux-gnu"
            musl_target="x86_64-unknown-linux-musl"
            ;;
        aarch64)
            gnu_target="aarch64-unknown-linux-gnu"
            musl_target="aarch64-unknown-linux-musl"
            ;;
        armv7)
            echo "armv7-unknown-linux-gnueabihf"
            return
            ;;
        *)
            return 1
            ;;
    esac

    if [ "$libc" = "musl" ]; then
        echo "$musl_target"
        return
    fi

    local glibc_ver
    glibc_ver=$(get_glibc_version || true)

    if [ -n "$glibc_ver" ]; then
        if version_lt "$glibc_ver" "$MIN_GLIBC_VERSION"; then
            print_warning "Detected glibc $glibc_ver (< $MIN_GLIBC_VERSION), falling back to musl build for compatibility"
            echo "$musl_target"
            return
        fi
        echo "$gnu_target"
    else
        print_warning "Could not detect glibc version, using musl build for maximum compatibility"
        echo "$musl_target"
    fi
}

get_target() {
    local os="$1"
    local arch="$2"

    case "$os" in
        linux)
            local libc
            libc=$(detect_libc)
            get_linux_target "$arch" "$libc"
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

github_api_request() {
    local url="$1"
    if command -v curl &> /dev/null; then
        if [[ -n "${GITHUB_TOKEN:-}" ]]; then
            curl -sL -H "Authorization: token ${GITHUB_TOKEN}" "$url"
        else
            curl -sL "$url"
        fi
    elif command -v wget &> /dev/null; then
        if [[ -n "${GITHUB_TOKEN:-}" ]]; then
            wget -qO- --header="Authorization: token ${GITHUB_TOKEN}" "$url"
        else
            wget -qO- "$url"
        fi
    else
        return 1
    fi
}

get_latest_version_via_redirect() {
    local redirect_url="https://github.com/${REPO}/releases/latest"
    local location

    if command -v curl &> /dev/null; then
        location=$(curl -sI -o /dev/null -w '%{url_effective}' -L "$redirect_url" 2>/dev/null)
    elif command -v wget &> /dev/null; then
        location=$(wget --spider -S --max-redirect=5 "$redirect_url" 2>&1 | grep -i 'Location:' | tail -1 | sed 's/.*Location:[[:space:]]*//' | sed 's/[[:space:]].*//' | tr -d '\r')
    fi

    if [[ -n "$location" ]]; then
        local version
        version=$(echo "$location" | sed -E 's|.*/tag/([^/?#]+).*|\1|')
        if [[ -n "$version" && "$version" != "$location" ]]; then
            echo "$version"
            return 0
        fi
    fi
    return 1
}

get_latest_version_via_api() {
    local all_releases_url="https://api.github.com/repos/${REPO}/releases?per_page=10"
    local response version

    response=$(github_api_request "$all_releases_url" 2>/dev/null) || return 1

    if echo "$response" | grep -q '"message"[[:space:]]*:[[:space:]]*"API rate limit exceeded'; then
        return 1
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

    version=$(echo "$response" | grep '"tag_name":' | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
    if [[ -n "$version" ]]; then
        echo "$version"
        return 0
    fi

    return 1
}

get_latest_version() {
    local version

    version=$(get_latest_version_via_redirect 2>/dev/null) && {
        echo "$version"
        return 0
    }
    print_warning "Redirect-based version detection failed, falling back to GitHub API..."

    version=$(get_latest_version_via_api 2>/dev/null) && {
        echo "$version"
        return 0
    }

    print_error "Failed to detect latest version"
    echo "" >&2
    echo "Solutions:" >&2
    echo "  1. Specify a version manually:" >&2
    echo "     curl -fsSL ... | bash -s -- --version v0.2.0" >&2
    echo "  2. Download directly from:" >&2
    echo "     https://github.com/${REPO}/releases" >&2
    exit 1
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
            tar --no-same-owner -xzf "$archive" -C "$dest"
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
    echo "  --target <TRIPLE>     Override target triple (e.g., x86_64-unknown-linux-musl)"
    echo "  --libc <gnu|musl>     Override libc variant on Linux (auto-detected by default)"
    echo "  --help                Show this help message"
    echo ""
    echo "Environment variables:"
    echo "  BIFROST_INSTALL_DIR   Custom installation directory"
    echo ""
    echo "Examples:"
    echo "  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install-binary.sh | bash"
    echo "  curl -fsSL ... | bash -s -- --version v0.0.9-alpha"
    echo "  curl -fsSL ... | bash -s -- --dir /usr/local/bin"
    echo "  curl -fsSL ... | bash -s -- --libc musl"
    echo "  curl -fsSL ... | bash -s -- --target x86_64-unknown-linux-musl"
}

VERSION=""
FORCE_TARGET=""
FORCE_LIBC=""

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
        --target)
            FORCE_TARGET="$2"
            shift 2
            ;;
        --libc)
            FORCE_LIBC="$2"
            if [[ "$FORCE_LIBC" != "gnu" && "$FORCE_LIBC" != "musl" ]]; then
                print_error "Invalid --libc value: $FORCE_LIBC (must be 'gnu' or 'musl')"
                exit 1
            fi
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

install_binary_for_target() {
    local target="$1"
    local version="$2"
    local os="$3"
    local install_dir="$4"
    local tmpdir="$5"

    local ext
    ext=$(get_archive_ext "$os")

    local cli_archive="bifrost-${version}-${target}.${ext}"
    local cli_url="https://github.com/${REPO}/releases/download/${version}/${cli_archive}"
    local checksums_url="https://github.com/${REPO}/releases/download/${version}/bifrost-${version}-checksums.txt"

    download_file "$cli_url" "$tmpdir/$cli_archive"

    if [[ ! -f "$tmpdir/checksums.txt" ]]; then
        print_step "Downloading checksums..."
        download_file "$checksums_url" "$tmpdir/checksums.txt"
    fi

    local expected_checksum
    expected_checksum=$(grep "$cli_archive" "$tmpdir/checksums.txt" | awk '{print $1}')
    if [[ -n "$expected_checksum" ]]; then
        verify_checksum "$tmpdir/$cli_archive" "$expected_checksum"
    else
        print_warning "Checksum not found for $cli_archive, skipping verification"
    fi

    print_step "Extracting..."
    local extract_subdir="$tmpdir/extract_${target}"
    mkdir -p "$extract_subdir"
    extract_archive "$tmpdir/$cli_archive" "$extract_subdir" "$os"

    local extracted_dir="bifrost-${version}-${target}"
    local binary_name="bifrost"
    [[ "$os" == "windows" ]] && binary_name="bifrost.exe"

    if [[ -f "$extract_subdir/$extracted_dir/$binary_name" ]]; then
        cp "$extract_subdir/$extracted_dir/$binary_name" "$install_dir/$binary_name"
    elif [[ -f "$extract_subdir/$binary_name" ]]; then
        cp "$extract_subdir/$binary_name" "$install_dir/$binary_name"
    else
        print_error "Binary not found in archive"
        return 1
    fi

    chmod +x "$install_dir/$binary_name"
    clear_xattr "$install_dir/$binary_name"
    return 0
}

get_musl_fallback_target() {
    local target="$1"
    case "$target" in
        x86_64-unknown-linux-gnu)  echo "x86_64-unknown-linux-musl" ;;
        aarch64-unknown-linux-gnu) echo "aarch64-unknown-linux-musl" ;;
        *) echo "" ;;
    esac
}

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

    if [[ -n "$FORCE_TARGET" ]]; then
        target="$FORCE_TARGET"
        print_step "Using user-specified target: $target"
    elif [[ -n "$FORCE_LIBC" && "$os" == "linux" ]]; then
        case "$arch" in
            x86_64)  target="${arch}-unknown-linux-${FORCE_LIBC}" ;;
            aarch64) target="${arch}-unknown-linux-${FORCE_LIBC}" ;;
            armv7)   target="armv7-unknown-linux-gnueabihf" ;;
            *)
                print_error "No pre-built binary available for $os-$arch"
                exit 1
                ;;
        esac
        print_step "Using user-specified libc: $FORCE_LIBC -> $target"
    else
        target=$(get_target "$os" "$arch")
    fi

    if [[ -z "$target" ]]; then
        print_error "No pre-built binary available for $os-$arch"
        print_warning "You can build from source instead:"
        echo "  git clone https://github.com/${REPO}.git"
        echo "  cd bifrost && ./install.sh"
        exit 1
    fi

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

    local binary_name="bifrost"
    [[ "$os" == "windows" ]] && binary_name="bifrost.exe"

    print_step "Installing CLI..."
    install_binary_for_target "$target" "$VERSION" "$os" "$INSTALL_DIR" "$tmpdir"

    if ! verify_binary_runs "$INSTALL_DIR/$binary_name"; then
        local musl_target
        musl_target=$(get_musl_fallback_target "$target")

        if [[ -n "$musl_target" ]]; then
            print_warning "Installed binary (${target}) failed to run — likely a glibc version mismatch"
            print_step "Retrying with musl build: $musl_target"
            target="$musl_target"
            install_binary_for_target "$target" "$VERSION" "$os" "$INSTALL_DIR" "$tmpdir"

            if ! verify_binary_runs "$INSTALL_DIR/$binary_name"; then
                print_error "Fallback musl binary also failed to run"
                exit 1
            fi
            print_success "musl fallback succeeded"
        else
            print_error "Installed binary failed to run"
            print_warning "Try reinstalling with: --libc musl  or  --target <triple>"
            exit 1
        fi
    fi

    print_success "CLI installed: $INSTALL_DIR/$binary_name ($target)"

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    print_success "Installation completed!"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        print_warning "$INSTALL_DIR is not in your PATH"
        echo ""
        if [[ "$os" == "windows" ]]; then
            local win_install_dir="$INSTALL_DIR"
            if command -v cygpath >/dev/null 2>&1; then
                win_install_dir=$(cygpath -w "$INSTALL_DIR")
            fi
            echo "Add it to your Git Bash shell configuration:"
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
            echo "Or add it to Windows PATH (PowerShell):"
            echo ""
            echo "  \$currentPath = [Environment]::GetEnvironmentVariable('Path', 'User')"
            echo "  [Environment]::SetEnvironmentVariable('Path', \"\$currentPath;$win_install_dir\", 'User')"
            echo ""
            echo "Then restart your terminal or run:"
            echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        else
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
