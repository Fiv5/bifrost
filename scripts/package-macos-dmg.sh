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
MOUNT_DIR="${TMP_ROOT}/mount"

cleanup() {
  if mount | grep -q "on ${MOUNT_DIR} "; then
    hdiutil detach "${MOUNT_DIR}" -quiet || true
  fi
  rm -rf "${TMP_ROOT}"
}
trap cleanup EXIT

mkdir -p "${STAGING_DIR}" "${MOUNT_DIR}" "${OUTPUT_DIR_RAW}"
OUTPUT_DIR="$(cd "${OUTPUT_DIR_RAW}" && pwd)"
cp -R "${APP_PATH}" "${STAGING_DIR}/"
ln -s /Applications "${STAGING_DIR}/Applications"

SOURCE_SIZE_KB="$(du -sk "${STAGING_DIR}" | awk '{print $1}')"
DMG_SIZE_MB="$(( SOURCE_SIZE_KB / 1024 + 32 ))"

hdiutil create \
  -srcfolder "${STAGING_DIR}" \
  -volname "${VOLUME_NAME}" \
  -fs HFS+ \
  -format UDRW \
  -size "${DMG_SIZE_MB}m" \
  "${RW_DMG}" \
  >/dev/null

hdiutil attach "${RW_DMG}" -mountpoint "${MOUNT_DIR}" -noverify -nobrowse -quiet
sync
hdiutil detach "${MOUNT_DIR}" -quiet

rm -f "${OUTPUT_DIR}/${OUTPUT_NAME}"
hdiutil convert "${RW_DMG}" -format UDZO -imagekey zlib-level=9 -o "${OUTPUT_DIR}/${OUTPUT_NAME}" >/dev/null

echo "Packaged DMG: ${OUTPUT_DIR}/${OUTPUT_NAME}"
