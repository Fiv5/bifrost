#!/bin/bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <path-to-app-bundle>" >&2
  exit 1
fi

APP_PATH="$1"

if [[ ! -d "${APP_PATH}" ]]; then
  echo "App bundle not found: ${APP_PATH}" >&2
  exit 1
fi

IDENTITY="${APPLE_SIGNING_IDENTITY:-}"

if [[ -z "${IDENTITY}" ]]; then
  if [[ -n "${APPLE_CERTIFICATE:-}" ]]; then
    IDENTITY="$(security find-identity -v -p codesigning | awk -F '"' '/Developer ID Application/ { print $2; exit }')"
    if [[ -z "${IDENTITY}" ]]; then
      echo "Unable to detect a Developer ID Application signing identity." >&2
      exit 1
    fi
  else
    IDENTITY="-"
  fi
fi

sign_args=(--force --sign "${IDENTITY}")
if [[ "${IDENTITY}" != "-" ]]; then
  sign_args+=(--options runtime)
fi

sign_executables_in_dir() {
  local dir="$1"
  if [[ ! -d "${dir}" ]]; then
    return 0
  fi

  while IFS= read -r -d '' file; do
    codesign "${sign_args[@]}" "${file}"
  done < <(find "${dir}" -type f -perm -111 -print0)
}

sign_executables_in_dir "${APP_PATH}/Contents/MacOS"
sign_executables_in_dir "${APP_PATH}/Contents/Resources/resources/bin"

codesign "${sign_args[@]}" --deep "${APP_PATH}"
codesign --verify --deep --strict --verbose=4 "${APP_PATH}"

echo "Re-signed macOS app bundle: ${APP_PATH}"
