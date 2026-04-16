#!/usr/bin/env bash
#
# Unit-test & integration-test coverage for the Bifrost workspace.
#
# Prerequisites:
#   cargo install cargo-llvm-cov
#   rustup component add llvm-tools-preview
#
# Usage:
#   bash scripts/ci/coverage.sh [options]
#
# Options:
#   --html            Generate HTML report and open in browser
#   --lcov            Generate LCOV report (lcov.info)
#   --json            Generate JSON summary to stdout
#   --fail-under PCT  Fail if line coverage < PCT% (default: 0, disabled)
#   --open            Open HTML report in browser (implies --html)
#   --output-dir DIR  Output directory (default: target/coverage)
#   -p, --package PKG Measure coverage for a single crate
#   -h, --help        Show this help

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

FORMAT="text"
FAIL_UNDER=0
OPEN_REPORT=0
OUTPUT_DIR="target/coverage"
PACKAGE=""
EXTRA_ARGS=()

usage() {
  sed -n '2,/^$/s/^# \?//p' "$0"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --html)       FORMAT="html"; shift ;;
    --lcov)       FORMAT="lcov"; shift ;;
    --json)       FORMAT="json"; shift ;;
    --fail-under) FAIL_UNDER="$2"; shift 2 ;;
    --open)       FORMAT="html"; OPEN_REPORT=1; shift ;;
    --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
    -p|--package) PACKAGE="$2"; shift 2 ;;
    -h|--help)    usage; exit 0 ;;
    --)           shift; EXTRA_ARGS+=("$@"); break ;;
    *)            echo -e "${RED}Unknown option: $1${NC}" >&2; usage; exit 1 ;;
  esac
done

ensure_cargo_llvm_cov() {
  if ! command -v cargo-llvm-cov &>/dev/null; then
    echo -e "${YELLOW}cargo-llvm-cov not found. Installing...${NC}"
    cargo install cargo-llvm-cov
  fi

  if ! rustup component list --installed 2>/dev/null | grep -q llvm-tools; then
    echo -e "${YELLOW}llvm-tools-preview not found. Installing...${NC}"
    rustup component add llvm-tools-preview
  fi
}

build_command() {
  local -a cmd=(cargo llvm-cov)

  if [[ -n "$PACKAGE" ]]; then
    cmd+=(--package "$PACKAGE")
  else
    cmd+=(--workspace)
  fi

  cmd+=(--all-features)

  if [[ "$FAIL_UNDER" -gt 0 ]]; then
    cmd+=(--fail-under-lines "$FAIL_UNDER")
  fi

  case "$FORMAT" in
    html)
      mkdir -p "$OUTPUT_DIR"
      cmd+=(--html --output-dir "$OUTPUT_DIR")
      if [[ "$OPEN_REPORT" -eq 1 ]]; then
        cmd+=(--open)
      fi
      ;;
    lcov)
      mkdir -p "$OUTPUT_DIR"
      cmd+=(--lcov --output-path "$OUTPUT_DIR/lcov.info")
      ;;
    json)
      cmd+=(--json --summary-only)
      ;;
    text)
      ;;
  esac

  if [[ ${#EXTRA_ARGS[@]} -gt 0 ]]; then
    cmd+=("--" "${EXTRA_ARGS[@]}")
  fi

  printf '%s\n' "${cmd[*]}"
}

main() {
  echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo -e "${BLUE}  Bifrost Unit-Test Coverage${NC}"
  echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo ""

  ensure_cargo_llvm_cov

  echo -e "${BLUE}Format     :${NC} $FORMAT"
  echo -e "${BLUE}Fail-under :${NC} ${FAIL_UNDER}%"
  echo -e "${BLUE}Output dir :${NC} $OUTPUT_DIR"
  [[ -n "$PACKAGE" ]] && echo -e "${BLUE}Package    :${NC} $PACKAGE"
  echo ""

  local cmd
  cmd="$(build_command)"
  echo -e "${BLUE}Running:${NC} $cmd"
  echo ""

  eval "$cmd"
  local exit_code=$?

  if [[ "$FORMAT" == "html" ]]; then
    echo ""
    echo -e "${GREEN}HTML report generated at: ${OUTPUT_DIR}/html/index.html${NC}"
  elif [[ "$FORMAT" == "lcov" ]]; then
    echo ""
    echo -e "${GREEN}LCOV report generated at: ${OUTPUT_DIR}/lcov.info${NC}"
  fi

  return $exit_code
}

main
