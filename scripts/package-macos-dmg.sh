#!/bin/bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <path-to-app-bundle> <output-dmg-path>" >&2
  exit 1
fi

APP_PATH="$1"
OUTPUT_DMG="$2"

if [[ ! -d "${APP_PATH}" ]]; then
  echo "App bundle not found: ${APP_PATH}" >&2
  exit 1
fi

APP_NAME="$(basename "${APP_PATH}")"
VOLUME_NAME="${APP_NAME%.app}"
OUTPUT_DIR_RAW="$(dirname "${OUTPUT_DMG}")"
OUTPUT_NAME="$(basename "${OUTPUT_DMG}")"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bifrost-dmg.XXXXXX")"
STAGING_DIR="${TMP_ROOT}/staging"
RW_DMG="${TMP_ROOT}/temp.dmg"

cleanup() {
  rm -rf "${TMP_ROOT}"
}
trap cleanup EXIT

retry_hdiutil() {
  local attempt=1
  local max_attempts=3

  while (( attempt <= max_attempts )); do
    if hdiutil "$@"; then
      return 0
    fi

    if (( attempt == max_attempts )); then
      return 1
    fi

    echo "hdiutil $1 failed on attempt ${attempt}/${max_attempts}; retrying after cleanup..." >&2
    cleanup
    mkdir -p "${STAGING_DIR}" "${OUTPUT_DIR_RAW}"
    rm -rf "${STAGING_DIR:?}"/*
    ditto "${APP_PATH}" "${STAGING_DIR}/${APP_NAME}"
    ln -s /Applications "${STAGING_DIR}/Applications"
    sleep $(( attempt * 2 ))
    attempt=$(( attempt + 1 ))
  done
}

mkdir -p "${STAGING_DIR}" "${OUTPUT_DIR_RAW}"
OUTPUT_DIR="$(cd "${OUTPUT_DIR_RAW}" && pwd)"
ditto "${APP_PATH}" "${STAGING_DIR}/${APP_NAME}"
ln -s /Applications "${STAGING_DIR}/Applications"

SOURCE_SIZE_KB="$(du -sk "${STAGING_DIR}" | awk '{print $1}')"
DMG_SIZE_MB="$(( SOURCE_SIZE_KB / 1024 + 32 ))"

retry_hdiutil create \
  -srcfolder "${STAGING_DIR}" \
  -volname "${VOLUME_NAME}" \
  -fs HFS+ \
  -format UDRW \
  -size "${DMG_SIZE_MB}m" \
  -ov \
  "${RW_DMG}" \
  >/dev/null

rm -f "${OUTPUT_DIR}/${OUTPUT_NAME}"
retry_hdiutil convert "${RW_DMG}" -format UDZO -imagekey zlib-level=9 -ov -o "${OUTPUT_DIR}/${OUTPUT_NAME}" >/dev/null

echo "Packaged DMG: ${OUTPUT_DIR}/${OUTPUT_NAME}"
