#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

BIFROST_BIN="${PROJECT_DIR}/target/release/bifrost"
if [[ ! -x "$BIFROST_BIN" && -f "${BIFROST_BIN}.exe" ]]; then
    BIFROST_BIN="${BIFROST_BIN}.exe"
fi

TEST_DATA_DIR=""

cleanup() {
    if [[ -n "${TEST_DATA_DIR}" ]] && [[ -d "${TEST_DATA_DIR}" ]]; then
        rm -rf "${TEST_DATA_DIR}"
    fi
}
trap cleanup EXIT

build_bifrost() {
    if [[ -x "${BIFROST_BIN}" && "${SKIP_BUILD:-false}" == "true" ]]; then
        return 0
    fi

    if [[ ! -x "${BIFROST_BIN}" ]]; then
        cargo build --release --bin bifrost >/dev/null
    fi
}

assert_contains() {
    local haystack="$1"
    local needle="$2"
    local message="$3"

    if [[ "${haystack}" == *"${needle}"* ]]; then
        echo "PASS: ${message}"
    else
        echo "FAIL: ${message}"
        echo "Expected to find: ${needle}"
        echo "Actual output:"
        echo "${haystack}"
        exit 1
    fi
}

assert_not_contains() {
    local haystack="$1"
    local needle="$2"
    local message="$3"

    if [[ "${haystack}" == *"${needle}"* ]]; then
        echo "FAIL: ${message}"
        echo "Did not expect to find: ${needle}"
        echo "Actual output:"
        echo "${haystack}"
        exit 1
    else
        echo "PASS: ${message}"
    fi
}

main() {
    build_bifrost

    TEST_DATA_DIR="$(mktemp -d)"
    export BIFROST_DATA_DIR="${TEST_DATA_DIR}"

    "${BIFROST_BIN}" rule add valid --content "example.com host://127.0.0.1:3000" >/dev/null

    mkdir -p "${TEST_DATA_DIR}/rules"
    cat > "${TEST_DATA_DIR}/rules/broken.json" <<'EOF'
{"content":"broken.example.com host://127.0.0.1:4000","enabled":true}
EOF

    local output
    output="$("${BIFROST_BIN}" rule list 2>&1)"

    assert_contains "${output}" "Rules (1):" "rule list counts only valid local rules"
    assert_contains "${output}" "valid [enabled]" "rule list keeps valid local rule"
    assert_not_contains "${output}" "Error:" "rule list does not fail on broken legacy file"

    echo "All rule list legacy skip checks passed."
}

main "$@"
