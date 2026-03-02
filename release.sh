#!/bin/bash
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

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

show_help() {
    echo "Bifrost Release Script"
    echo ""
    echo "Usage: $0 [OPTIONS] [VERSION]"
    echo ""
    echo "Arguments:"
    echo "  VERSION               Version to release (e.g., 0.0.2-alpha, 1.0.0)"
    echo "                        If not provided, will prompt for input"
    echo ""
    echo "Options:"
    echo "  --patch               Bump patch version (0.0.1 -> 0.0.2)"
    echo "  --minor               Bump minor version (0.0.1 -> 0.1.0)"
    echo "  --major               Bump major version (0.0.1 -> 1.0.0)"
    echo "  --prerelease <id>     Add/update prerelease identifier (alpha, beta, rc)"
    echo "  --dry-run             Show what would be done without making changes"
    echo "  --no-push             Create tag locally but don't push"
    echo "  --help                Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 0.0.2-alpha        # Release version 0.0.2-alpha"
    echo "  $0 --patch            # Bump patch: 0.0.1-alpha -> 0.0.2-alpha"
    echo "  $0 --minor            # Bump minor: 0.0.1 -> 0.1.0"
    echo "  $0 --patch --prerelease beta  # 0.0.1-alpha -> 0.0.2-beta"
    echo "  $0 --dry-run 1.0.0    # Preview release 1.0.0"
}

get_current_version() {
    grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/'
}

parse_version() {
    local version="$1"
    local base_version="${version%%-*}"
    local prerelease=""
    
    if [[ "$version" == *-* ]]; then
        prerelease="${version#*-}"
    fi
    
    IFS='.' read -r MAJOR MINOR PATCH <<< "$base_version"
    MAJOR=${MAJOR:-0}
    MINOR=${MINOR:-0}
    PATCH=${PATCH:-0}
    
    echo "$MAJOR $MINOR $PATCH $prerelease"
}

bump_version() {
    local current="$1"
    local bump_type="$2"
    local new_prerelease="$3"
    
    read -r MAJOR MINOR PATCH PRERELEASE <<< "$(parse_version "$current")"
    
    case "$bump_type" in
        major)
            MAJOR=$((MAJOR + 1))
            MINOR=0
            PATCH=0
            ;;
        minor)
            MINOR=$((MINOR + 1))
            PATCH=0
            ;;
        patch)
            PATCH=$((PATCH + 1))
            ;;
    esac
    
    local new_version="${MAJOR}.${MINOR}.${PATCH}"
    
    if [[ -n "$new_prerelease" ]]; then
        new_version="${new_version}-${new_prerelease}"
    elif [[ -n "$PRERELEASE" && "$bump_type" == "patch" ]]; then
        new_version="${new_version}-${PRERELEASE}"
    fi
    
    echo "$new_version"
}

update_version_files() {
    local version="$1"
    local dry_run="$2"
    local tag="v${version}"
    
    if [[ "$dry_run" == "true" ]]; then
        print_step "Would update version files to ${version}"
        return
    fi
    
    print_step "Updating Cargo.toml files..."
    
    sed -i.bak "s/^version = \".*\"/version = \"${version}\"/" Cargo.toml
    rm -f Cargo.toml.bak
    print_success "Updated: Cargo.toml"
    
    for crate_dir in crates/*/; do
        if [[ -f "${crate_dir}Cargo.toml" ]]; then
            local crate_toml="${crate_dir}Cargo.toml"
            if grep -q '^version = "' "$crate_toml"; then
                sed -i.bak "s/^version = \".*\"/version = \"${version}\"/" "$crate_toml"
                rm -f "${crate_toml}.bak"
                print_success "Updated: ${crate_toml}"
            fi
        fi
    done
    
    print_step "Updating install scripts..."
    
    if [[ -f "install-binary.sh" ]]; then
        sed -i.bak "s/--version v[0-9]*\.[0-9]*\.[0-9]*[^\"']*/--version ${tag}/g" install-binary.sh
        rm -f install-binary.sh.bak
        print_success "Updated: install-binary.sh"
    fi
    
    if [[ -f "install-binary.ps1" ]]; then
        sed -i.bak "s/-Version v[0-9]*\.[0-9]*\.[0-9]*[^\"']*/-Version ${tag}/g" install-binary.ps1
        rm -f install-binary.ps1.bak
        print_success "Updated: install-binary.ps1"
    fi
    
    print_step "Updating Cargo.lock..."
    cargo check --quiet 2>/dev/null || cargo check
    print_success "Updated: Cargo.lock"
}

commit_and_tag() {
    local version="$1"
    local tag="v${version}"
    local dry_run="$2"
    local no_push="$3"
    
    if [[ "$dry_run" == "true" ]]; then
        print_step "Would commit changes with message: chore: bump version to ${version}"
        print_step "Would create tag: ${tag}"
        if [[ "$no_push" != "true" ]]; then
            print_step "Would push to origin"
        fi
        return
    fi
    
    print_step "Committing changes..."
    git add -A
    if ! git diff --cached --quiet; then
        git commit -m "chore: bump version to ${version}"
        print_success "Committed: chore: bump version to ${version}"
    else
        print_warning "No changes to commit"
    fi
    
    print_step "Creating tag ${tag}..."
    if git tag -l "$tag" | grep -q "$tag"; then
        print_error "Tag ${tag} already exists!"
        echo ""
        echo "To delete and recreate:"
        echo "  git tag -d ${tag}"
        echo "  git push origin :refs/tags/${tag}"
        exit 1
    fi
    
    git tag -a "$tag" -m "Release ${tag}"
    print_success "Created tag: ${tag}"
    
    if [[ "$no_push" != "true" ]]; then
        print_step "Pushing to origin..."
        git push origin HEAD
        git push origin "$tag"
        print_success "Pushed to origin"
    else
        print_warning "Skipped push (--no-push)"
        echo ""
        echo "To push manually:"
        echo "  git push origin HEAD"
        echo "  git push origin ${tag}"
    fi
}

main() {
    local version=""
    local bump_type=""
    local prerelease=""
    local dry_run="false"
    local no_push="false"
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --patch)
                bump_type="patch"
                shift
                ;;
            --minor)
                bump_type="minor"
                shift
                ;;
            --major)
                bump_type="major"
                shift
                ;;
            --prerelease)
                prerelease="$2"
                shift 2
                ;;
            --dry-run)
                dry_run="true"
                shift
                ;;
            --no-push)
                no_push="true"
                shift
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            -*)
                print_error "Unknown option: $1"
                show_help
                exit 1
                ;;
            *)
                version="$1"
                shift
                ;;
        esac
    done
    
    cd "$(dirname "$0")"
    
    if [[ ! -f "Cargo.toml" ]]; then
        print_error "Cargo.toml not found. Are you in the project root?"
        exit 1
    fi
    
    if [[ -n "$(git status --porcelain)" ]]; then
        print_warning "Working directory has uncommitted changes"
        git status --short
        echo ""
        read -p "Continue anyway? [y/N] " -r
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
    
    local current_version
    current_version=$(get_current_version)
    
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  Bifrost Release"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "  Current version: ${current_version}"
    echo ""
    
    if [[ -n "$bump_type" ]]; then
        version=$(bump_version "$current_version" "$bump_type" "$prerelease")
    elif [[ -z "$version" ]]; then
        local suggested
        suggested=$(bump_version "$current_version" "patch" "$prerelease")
        
        read -p "  Enter new version [${suggested}]: " -r input_version
        version="${input_version:-$suggested}"
    fi
    
    if [[ -n "$prerelease" && "$version" != *-* ]]; then
        version="${version}-${prerelease}"
    fi
    
    version="${version#v}"
    
    if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+)?$ ]]; then
        print_error "Invalid version format: ${version}"
        echo "Expected format: X.Y.Z or X.Y.Z-prerelease"
        exit 1
    fi
    
    echo "  New version:     ${version}"
    echo "  Tag:             v${version}"
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    
    if [[ "$dry_run" == "true" ]]; then
        print_warning "DRY RUN - No changes will be made"
        echo ""
    fi
    
    if [[ "$dry_run" != "true" ]]; then
        read -p "Proceed with release? [y/N] " -r
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo "Aborted."
            exit 0
        fi
        echo ""
    fi
    
    update_version_files "$version" "$dry_run"
    echo ""
    commit_and_tag "$version" "$dry_run" "$no_push"
    
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    if [[ "$dry_run" == "true" ]]; then
        print_warning "DRY RUN completed"
    else
        print_success "Release v${version} completed!"
        echo ""
        echo "  GitHub Actions will now build and publish the release."
        echo "  Monitor progress at:"
        echo "  https://github.com/bifrost-proxy/bifrost/actions"
    fi
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
}

main "$@"
