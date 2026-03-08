#!/bin/bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../test_utils/admin_client.sh"
source "$SCRIPT_DIR/../test_utils/assert.sh"
source "$SCRIPT_DIR/../test_utils/http_client.sh"

HTTP_PORT="${HTTP_PORT:-3000}"
PROXY_PORT="${PROXY_PORT:-9900}"
ADMIN_PORT="${ADMIN_PORT:-9900}"
ADMIN_PATH_PREFIX="${ADMIN_PATH_PREFIX:-/_bifrost}"
export ADMIN_PATH_PREFIX
TEST_ID=""

python3 "$SCRIPT_DIR/../mock_servers/http_echo_server.py" "$HTTP_PORT" &
server_pid=$!
trap 'kill "$server_pid" 2>/dev/null || true' EXIT

payload=$(python3 - <<'PY'
print("a" * 32768)
PY
)

curl -s -X PUT -H "Content-Type: application/json" \
  -d '{"max_db_size_bytes":262144,"max_body_memory_size":1024}' \
  "http://127.0.0.1:${ADMIN_PORT}${ADMIN_PATH_PREFIX}/api/config/performance" >/dev/null

for i in $(seq 1 150); do
  http_post "http://127.0.0.1:${HTTP_PORT}/echo" "$payload"
done

traffic_response=$(admin_get "/api/traffic?limit=200")
record_count=$(echo "$traffic_response" | jq -r '.records | length')

if [ "$record_count" -lt 150 ]; then
  _log_pass "total size cleanup removed oldest records (count $record_count)"
else
  _log_fail "total size cleanup removed oldest records" "< 150" "$record_count"
fi

echo ""
echo "Results: $PASSED_ASSERTIONS passed, $FAILED_ASSERTIONS failed"
if [ "$FAILED_ASSERTIONS" -gt 0 ]; then
  exit 1
fi
