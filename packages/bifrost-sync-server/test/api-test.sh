#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR/.."
SYNC_PORT=18686
SYNC_DATA_DIR="/tmp/bifrost-sync-api-test-$$"
BASE="http://127.0.0.1:$SYNC_PORT"

PASS=0
FAIL=0
TOTAL=0

cleanup() {
  echo ""
  echo "[cleanup] stopping services..."
  if [ -n "${SYNC_PID:-}" ] && kill -0 "$SYNC_PID" 2>/dev/null; then
    kill "$SYNC_PID" 2>/dev/null || true
    wait "$SYNC_PID" 2>/dev/null || true
  fi
  rm -rf "$SYNC_DATA_DIR"
  echo "[cleanup] done"
}
trap cleanup EXIT

assert_eq() {
  local label="$1" expected="$2" actual="$3"
  TOTAL=$((TOTAL + 1))
  if [ "$expected" = "$actual" ]; then
    echo "  ✓ $label"
    PASS=$((PASS + 1))
  else
    echo "  ✗ $label (expected: $expected, got: $actual)"
    FAIL=$((FAIL + 1))
  fi
}

assert_contains() {
  local label="$1" needle="$2" haystack="$3"
  TOTAL=$((TOTAL + 1))
  if echo "$haystack" | grep -q "$needle"; then
    echo "  ✓ $label"
    PASS=$((PASS + 1))
  else
    echo "  ✗ $label (expected to contain: $needle)"
    echo "    actual: $haystack"
    FAIL=$((FAIL + 1))
  fi
}

assert_http_status() {
  local label="$1" expected="$2" actual="$3"
  TOTAL=$((TOTAL + 1))
  if [ "$expected" = "$actual" ]; then
    echo "  ✓ $label (HTTP $actual)"
    PASS=$((PASS + 1))
  else
    echo "  ✗ $label (expected HTTP $expected, got HTTP $actual)"
    FAIL=$((FAIL + 1))
  fi
}

json_field() {
  echo "$1" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d$2)" 2>/dev/null || echo ""
}

json_array_len() {
  echo "$1" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d$2))" 2>/dev/null || echo "0"
}

wait_for_service() {
  local url="$1" retries=30
  for i in $(seq 1 $retries); do
    if curl -sf -o /dev/null "$url" 2>/dev/null || curl -s -o /dev/null -w "%{http_code}" "$url" 2>/dev/null | grep -qv "000"; then
      return 0
    fi
    sleep 0.5
  done
  echo "FATAL: service $url did not start in time"
  exit 1
}

http_get() {
  local url="$1"
  shift
  curl -s "$@" "$url"
}

http_get_status() {
  local url="$1"
  shift
  curl -s -o /dev/null -w "%{http_code}" "$@" "$url"
}

http_post() {
  local url="$1" data="$2"
  shift 2
  curl -s -X POST -H "Content-Type: application/json" "$@" -d "$data" "$url"
}

http_post_status() {
  local url="$1" data="$2"
  shift 2
  curl -s -o /dev/null -w "%{http_code}" -X POST -H "Content-Type: application/json" "$@" -d "$data" "$url"
}

http_patch() {
  local url="$1" data="$2"
  shift 2
  curl -s -X PATCH -H "Content-Type: application/json" "$@" -d "$data" "$url"
}

http_delete() {
  local url="$1"
  shift
  curl -s -X DELETE "$@" "$url"
}

http_delete_status() {
  local url="$1"
  shift
  curl -s -o /dev/null -w "%{http_code}" -X DELETE "$@" "$url"
}

echo "╔════════════════════════════════════════════════════════════╗"
echo "║     Bifrost Sync Server — Full API Test Suite             ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# ── Start sync server ───────────────────────────────────────
echo "[setup] starting sync server on port $SYNC_PORT..."
mkdir -p "$SYNC_DATA_DIR"
cd "$PROJECT_DIR"
npx tsx src/cli.ts -p "$SYNC_PORT" -d "$SYNC_DATA_DIR" &
SYNC_PID=$!
wait_for_service "$BASE/v4/sso/login"
echo "[setup] sync server ready (PID: $SYNC_PID)"
echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 1: SSO — Registration
# ═══════════════════════════════════════════════════════════════
echo "── 1. Registration ─────────────────────────────────────────"

# 1.1 Normal registration
RESP=$(http_post "$BASE/v4/sso/register" '{"user_id":"testuser","password":"test1234","nickname":"Test User","email":"test@example.com"}')
CODE=$(json_field "$RESP" "['code']")
assert_eq "register testuser → code 0" "0" "$CODE"
TOKEN_A=$(json_field "$RESP" "['data']['token']")
assert_contains "register returns token" "." "$TOKEN_A"
NICK=$(json_field "$RESP" "['data']['nickname']")
assert_eq "register returns nickname" "Test User" "$NICK"
EMAIL=$(json_field "$RESP" "['data']['email']")
assert_eq "register returns email" "test@example.com" "$EMAIL"

# 1.2 Duplicate registration
STATUS=$(http_post_status "$BASE/v4/sso/register" '{"user_id":"testuser","password":"other123"}')
assert_http_status "duplicate register → 409" "409" "$STATUS"

# 1.3 Register second user
RESP=$(http_post "$BASE/v4/sso/register" '{"user_id":"testuser2","password":"pass5678","nickname":"User Two"}')
CODE=$(json_field "$RESP" "['code']")
assert_eq "register testuser2 → code 0" "0" "$CODE"
TOKEN_B=$(json_field "$RESP" "['data']['token']")

# 1.4 Short password
STATUS=$(http_post_status "$BASE/v4/sso/register" '{"user_id":"shortpw","password":"12"}')
assert_http_status "short password → 400" "400" "$STATUS"

# 1.5 Invalid username
STATUS=$(http_post_status "$BASE/v4/sso/register" '{"user_id":"a","password":"validpass123"}')
assert_http_status "too short username → 400" "400" "$STATUS"

STATUS=$(http_post_status "$BASE/v4/sso/register" '{"user_id":"bad user!","password":"validpass123"}')
assert_http_status "invalid chars in username → 400" "400" "$STATUS"

# 1.6 Missing fields
STATUS=$(http_post_status "$BASE/v4/sso/register" '{"user_id":"onlyuser"}')
assert_http_status "missing password → 400" "400" "$STATUS"

STATUS=$(http_post_status "$BASE/v4/sso/register" '{"password":"onlypass"}')
assert_http_status "missing user_id → 400" "400" "$STATUS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 2: SSO — Login
# ═══════════════════════════════════════════════════════════════
echo "── 2. Login ────────────────────────────────────────────────"

# 2.1 Normal login
RESP=$(http_post "$BASE/v4/sso/login" '{"user_id":"testuser","password":"test1234"}')
CODE=$(json_field "$RESP" "['code']")
assert_eq "login testuser → code 0" "0" "$CODE"
TOKEN_A=$(json_field "$RESP" "['data']['token']")
assert_contains "login returns token" "." "$TOKEN_A"

# 2.2 Wrong password
STATUS=$(http_post_status "$BASE/v4/sso/login" '{"user_id":"testuser","password":"wrong"}')
assert_http_status "wrong password → 401" "401" "$STATUS"

# 2.3 Non-existent user
STATUS=$(http_post_status "$BASE/v4/sso/login" '{"user_id":"nouser","password":"test1234"}')
assert_http_status "non-existent user → 401" "401" "$STATUS"

# 2.4 Missing fields
STATUS=$(http_post_status "$BASE/v4/sso/login" '{}')
assert_http_status "empty login → 400" "400" "$STATUS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 3: SSO — Token Check
# ═══════════════════════════════════════════════════════════════
echo "── 3. Token Check ──────────────────────────────────────────"

# 3.1 Check via header
RESP=$(http_get "$BASE/v4/sso/check" -H "x-bifrost-token: $TOKEN_A")
CODE=$(json_field "$RESP" "['code']")
assert_eq "check via header → code 0" "0" "$CODE"
USER_ID=$(json_field "$RESP" "['data']['user_id']")
assert_eq "check returns user_id" "testuser" "$USER_ID"

# 3.2 Check via query param
RESP=$(http_get "$BASE/v4/sso/check?token=$TOKEN_A")
CODE=$(json_field "$RESP" "['code']")
assert_eq "check via query param → code 0" "0" "$CODE"

# 3.3 Check with invalid token
STATUS=$(http_get_status "$BASE/v4/sso/check" -H "x-bifrost-token: invalid-token-xxx")
assert_http_status "invalid token → 401" "401" "$STATUS"

# 3.4 Check without token
STATUS=$(http_get_status "$BASE/v4/sso/check")
assert_http_status "no token → 401" "401" "$STATUS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 4: SSO — User Info
# ═══════════════════════════════════════════════════════════════
echo "── 4. User Info ────────────────────────────────────────────"

# 4.1 Get user info
RESP=$(http_get "$BASE/v4/sso/info" -H "x-bifrost-token: $TOKEN_A")
CODE=$(json_field "$RESP" "['code']")
assert_eq "info → code 0" "0" "$CODE"
NICK=$(json_field "$RESP" "['data']['nickname']")
assert_eq "info returns nickname" "Test User" "$NICK"
EMAIL=$(json_field "$RESP" "['data']['email']")
assert_eq "info returns email" "test@example.com" "$EMAIL"

# 4.2 Info without token
STATUS=$(http_get_status "$BASE/v4/sso/info")
assert_http_status "info without token → 401" "401" "$STATUS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 5: SSO — HTML Pages
# ═══════════════════════════════════════════════════════════════
echo "── 5. HTML Pages ───────────────────────────────────────────"

# 5.1 Login page
RESP=$(curl -s -o /dev/null -w "%{http_code}|%{content_type}" "$BASE/v4/sso/login")
STATUS=$(echo "$RESP" | cut -d'|' -f1)
CTYPE=$(echo "$RESP" | cut -d'|' -f2)
assert_http_status "login page → 200" "200" "$STATUS"
assert_contains "login page content-type" "text/html" "$CTYPE"

# 5.2 Login page has CSP header
CSP=$(curl -s -D - -o /dev/null "$BASE/v4/sso/login" | grep -i "content-security-policy" | head -1)
assert_contains "login page has CSP header" "script-src" "$CSP"

# 5.3 Register page
RESP=$(curl -s -o /dev/null -w "%{http_code}|%{content_type}" "$BASE/v4/sso/register-page")
STATUS=$(echo "$RESP" | cut -d'|' -f1)
CTYPE=$(echo "$RESP" | cut -d'|' -f2)
assert_http_status "register page → 200" "200" "$STATUS"
assert_contains "register page content-type" "text/html" "$CTYPE"

# 5.4 Register page has CSP header
CSP=$(curl -s -D - -o /dev/null "$BASE/v4/sso/register-page" | grep -i "content-security-policy" | head -1)
assert_contains "register page has CSP header" "script-src" "$CSP"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 6: Env CRUD
# ═══════════════════════════════════════════════════════════════
echo "── 6. Env CRUD ─────────────────────────────────────────────"
AUTH_A="-H x-bifrost-token:$TOKEN_A"
AUTH_B="-H x-bifrost-token:$TOKEN_B"

# 6.1 Create env
RESP=$(http_post "$BASE/v4/env" '{"user_id":"testuser","name":"my-rule-1","rule":"example.com host://127.0.0.1:3000"}' $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "create env → code 0" "0" "$CODE"
ENV_ID_1=$(json_field "$RESP" "['data']['id']")
ENV_NAME=$(json_field "$RESP" "['data']['name']")
assert_eq "create env name" "my-rule-1" "$ENV_NAME"
ENV_RULE=$(json_field "$RESP" "['data']['rule']")
assert_eq "create env rule" "example.com host://127.0.0.1:3000" "$ENV_RULE"

# 6.2 Create second env
RESP=$(http_post "$BASE/v4/env" '{"user_id":"testuser","name":"my-rule-2","rule":"test.com reject"}' $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "create env 2 → code 0" "0" "$CODE"
ENV_ID_2=$(json_field "$RESP" "['data']['id']")

# 6.3 Create env for user B
RESP=$(http_post "$BASE/v4/env" '{"user_id":"testuser2","name":"user2-rule","rule":"other.com proxy"}' $AUTH_B)
CODE=$(json_field "$RESP" "['code']")
assert_eq "create env for user B → code 0" "0" "$CODE"
ENV_ID_3=$(json_field "$RESP" "['data']['id']")

# 6.4 Duplicate create returns existing
RESP=$(http_post "$BASE/v4/env" '{"user_id":"testuser","name":"my-rule-1"}' $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "duplicate create → code 0 (returns existing)" "0" "$CODE"
DUP_ID=$(json_field "$RESP" "['data']['id']")
assert_eq "duplicate create returns same id" "$ENV_ID_1" "$DUP_ID"

# 6.5 Create without auth
STATUS=$(http_post_status "$BASE/v4/env" '{"user_id":"testuser","name":"no-auth"}')
assert_http_status "create without auth → 401" "401" "$STATUS"

# 6.6 Create with missing fields
STATUS=$(http_post_status "$BASE/v4/env" '{"user_id":"testuser"}' $AUTH_A)
assert_http_status "create missing name → 400" "400" "$STATUS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 7: Env Read
# ═══════════════════════════════════════════════════════════════
echo "── 7. Env Read ─────────────────────────────────────────────"

# 7.1 Read by id
RESP=$(http_get "$BASE/v4/env/$ENV_ID_1" $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "read env → code 0" "0" "$CODE"
READ_NAME=$(json_field "$RESP" "['data']['name']")
assert_eq "read env name" "my-rule-1" "$READ_NAME"

# 7.2 Read non-existent
STATUS=$(http_get_status "$BASE/v4/env/non-existent-id" $AUTH_A)
assert_http_status "read non-existent → 404" "404" "$STATUS"

# 7.3 Read without auth
STATUS=$(http_get_status "$BASE/v4/env/$ENV_ID_1")
assert_http_status "read without auth → 401" "401" "$STATUS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 8: Env Update
# ═══════════════════════════════════════════════════════════════
echo "── 8. Env Update ───────────────────────────────────────────"

# 8.1 Update rule
RESP=$(http_patch "$BASE/v4/env/$ENV_ID_1" '{"rule":"example.com host://127.0.0.1:4000"}' $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "update env → code 0" "0" "$CODE"
UPDATED_RULE=$(json_field "$RESP" "['data']['rule']")
assert_eq "updated rule" "example.com host://127.0.0.1:4000" "$UPDATED_RULE"

# 8.2 Verify update persisted
RESP=$(http_get "$BASE/v4/env/$ENV_ID_1" $AUTH_A)
PERSISTED_RULE=$(json_field "$RESP" "['data']['rule']")
assert_eq "update persisted" "example.com host://127.0.0.1:4000" "$PERSISTED_RULE"

# 8.3 Update non-existent
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X PATCH -H "Content-Type: application/json" -H "x-bifrost-token: $TOKEN_A" -d '{"rule":"x"}' "$BASE/v4/env/non-existent-id")
assert_http_status "update non-existent → 404" "404" "$STATUS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 9: Env Search
# ═══════════════════════════════════════════════════════════════
echo "── 9. Env Search ───────────────────────────────────────────"

# 9.1 Search all by user
RESP=$(http_get "$BASE/v4/env?user_id=testuser" $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "search by user → code 0" "0" "$CODE"
COUNT=$(json_array_len "$RESP" "['data']['list']")
assert_eq "search testuser returns 2 envs" "2" "$COUNT"

# 9.2 Search all (no filter)
RESP=$(http_get "$BASE/v4/env" $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "search all → code 0" "0" "$CODE"
COUNT=$(json_array_len "$RESP" "['data']['list']")
assert_eq "search all returns 3 envs" "3" "$COUNT"

# 9.3 Search with keyword
RESP=$(http_get "$BASE/v4/env?keyword=rule-1" $AUTH_A)
COUNT=$(json_array_len "$RESP" "['data']['list']")
assert_eq "search keyword 'rule-1' returns 1" "1" "$COUNT"

# 9.4 Search with limit/offset
RESP=$(http_get "$BASE/v4/env?user_id=testuser&limit=1&offset=0" $AUTH_A)
COUNT=$(json_array_len "$RESP" "['data']['list']")
assert_eq "search with limit=1 returns 1" "1" "$COUNT"

# 9.5 Search by name endpoint
RESP=$(http_get "$BASE/v4/env_search_name?keyword=testuser/my-rule" $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "search_by_name → code 0" "0" "$CODE"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 10: Env Delete
# ═══════════════════════════════════════════════════════════════
echo "── 10. Env Delete ──────────────────────────────────────────"

# 10.1 Delete env
RESP=$(http_delete "$BASE/v4/env/$ENV_ID_2" $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "delete env → code 0" "0" "$CODE"

# 10.2 Confirm deletion
STATUS=$(http_get_status "$BASE/v4/env/$ENV_ID_2" $AUTH_A)
assert_http_status "deleted env → 404" "404" "$STATUS"

# 10.3 Delete non-existent
STATUS=$(http_delete_status "$BASE/v4/env/non-existent-id" $AUTH_A)
assert_http_status "delete non-existent → 404" "404" "$STATUS"

# 10.4 Verify remaining count
RESP=$(http_get "$BASE/v4/env?user_id=testuser" $AUTH_A)
COUNT=$(json_array_len "$RESP" "['data']['list']")
assert_eq "after delete, testuser has 1 env" "1" "$COUNT"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 11: Env Sync (batch)
# ═══════════════════════════════════════════════════════════════
echo "── 11. Env Sync (batch protocol) ──────────────────────────"

# 11.1 Sync — push a new env
SYNC_BODY=$(cat <<EOF
{
  "user_ids": ["testuser"],
  "check_list": [],
  "update_list": [
    {
      "user_id": "testuser",
      "id": "sync-new-env-001",
      "name": "synced-rule",
      "rule": "synced.example.com proxy",
      "update_time": "2026-01-01T00:00:00.000Z"
    }
  ],
  "delete_list": []
}
EOF
)
RESP=$(http_post "$BASE/v4/env/sync" "$SYNC_BODY" $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "sync push → code 0" "0" "$CODE"

# 11.2 Verify synced env exists
RESP=$(http_get "$BASE/v4/env?user_id=testuser" $AUTH_A)
COUNT=$(json_array_len "$RESP" "['data']['list']")
assert_eq "after sync push, testuser has 2 envs" "2" "$COUNT"

# 11.3 Sync — check_list detects update_time difference
SYNC_CHECK=$(cat <<EOF
{
  "user_ids": ["testuser"],
  "check_list": [
    {"id": "$ENV_ID_1", "user_id": "testuser", "update_time": "1999-01-01T00:00:00.000Z", "hash": "old"}
  ],
  "update_list": [],
  "delete_list": []
}
EOF
)
RESP=$(http_post "$BASE/v4/env/sync" "$SYNC_CHECK" $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "sync check → code 0" "0" "$CODE"
LOCAL_UPD_COUNT=$(json_array_len "$RESP" "['data']['local_update_list']")
assert_eq "sync check detects time diff → local_update_list=1" "1" "$LOCAL_UPD_COUNT"

# 11.4 Sync — check_list with non-existent id → local_delete_list
SYNC_DEL=$(cat <<EOF
{
  "user_ids": ["testuser"],
  "check_list": [
    {"id": "does-not-exist", "user_id": "testuser", "update_time": "2026-01-01", "hash": "x"}
  ],
  "update_list": [],
  "delete_list": []
}
EOF
)
RESP=$(http_post "$BASE/v4/env/sync" "$SYNC_DEL" $AUTH_A)
LOCAL_DEL_COUNT=$(json_array_len "$RESP" "['data']['local_delete_list']")
assert_eq "sync check non-existent → local_delete_list=1" "1" "$LOCAL_DEL_COUNT"

# 11.5 Sync — delete via sync
SYNC_DELETE_BODY=$(cat <<EOF
{
  "user_ids": ["testuser"],
  "check_list": [],
  "update_list": [],
  "delete_list": [
    {"user_id": "testuser", "id": "sync-new-env-001", "delete_time": "2026-01-02T00:00:00.000Z"}
  ]
}
EOF
)
RESP=$(http_post "$BASE/v4/env/sync" "$SYNC_DELETE_BODY" $AUTH_A)
CODE=$(json_field "$RESP" "['code']")
assert_eq "sync delete → code 0" "0" "$CODE"

# 11.6 Verify sync-deleted env is gone
STATUS=$(http_get_status "$BASE/v4/env/sync-new-env-001" $AUTH_A)
assert_http_status "sync-deleted env → 404" "404" "$STATUS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 12: SSO — Logout
# ═══════════════════════════════════════════════════════════════
echo "── 12. Logout ──────────────────────────────────────────────"

# 12.1 Logout
RESP=$(http_get "$BASE/v4/sso/logout" -H "x-bifrost-token: $TOKEN_A")
CODE=$(json_field "$RESP" "['code']")
assert_eq "logout → code 0" "0" "$CODE"

# 12.2 Token invalidated after logout
STATUS=$(http_get_status "$BASE/v4/sso/check" -H "x-bifrost-token: $TOKEN_A")
assert_http_status "token invalid after logout → 401" "401" "$STATUS"

# 12.3 Env access fails with invalidated token
STATUS=$(http_get_status "$BASE/v4/env" -H "x-bifrost-token: $TOKEN_A")
assert_http_status "env access after logout → 401" "401" "$STATUS"

# 12.4 User B still works
STATUS=$(http_get_status "$BASE/v4/sso/check" -H "x-bifrost-token: $TOKEN_B")
assert_http_status "user B token still valid → 200" "200" "$STATUS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 13: Security Headers
# ═══════════════════════════════════════════════════════════════
echo "── 13. Security Headers ────────────────────────────────────"

HEADERS=$(curl -s -D - -o /dev/null "$BASE/v4/sso/check")
assert_contains "X-Content-Type-Options" "nosniff" "$HEADERS"
assert_contains "X-Frame-Options" "DENY" "$HEADERS"
assert_contains "X-XSS-Protection" "1; mode=block" "$HEADERS"
assert_contains "Cache-Control" "no-store" "$HEADERS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SECTION 14: Error handling / Edge cases
# ═══════════════════════════════════════════════════════════════
echo "── 14. Error Handling ──────────────────────────────────────"

# 14.1 404 for unknown route
STATUS=$(http_get_status "$BASE/v4/unknown-route")
assert_http_status "unknown route → 404" "404" "$STATUS"

# 14.2 OPTIONS preflight
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X OPTIONS "$BASE/v4/sso/login")
assert_http_status "OPTIONS preflight → 204" "204" "$STATUS"

# 14.3 CORS headers
CORS=$(curl -s -D - -o /dev/null -X OPTIONS "$BASE/v4/sso/login" | grep -i "access-control-allow-origin")
assert_contains "CORS Allow-Origin" "*" "$CORS"

echo ""

# ═══════════════════════════════════════════════════════════════
# SUMMARY
# ═══════════════════════════════════════════════════════════════
echo ""
echo "╔════════════════════════════════════════════════════════════╗"
if [ "$FAIL" -eq 0 ]; then
  echo "║   ALL $TOTAL API TESTS PASSED ✓                            ║"
else
  printf "║   %d / %d PASSED, %d FAILED ✗                            ║\n" "$PASS" "$TOTAL" "$FAIL"
fi
echo "╠════════════════════════════════════════════════════════════╣"
echo "║   SSO: register, login, check, info, logout, pages       ║"
echo "║   ENV: create, read, update, delete, search, sync        ║"
echo "║   Security: headers, CORS, auth, validation              ║"
echo "╚════════════════════════════════════════════════════════════╝"

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
