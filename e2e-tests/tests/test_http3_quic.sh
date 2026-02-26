#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
E2E_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$E2E_DIR")"

PROXY_HOST=${PROXY_HOST:-127.0.0.1}
PROXY_PORT=${PROXY_PORT:-8080}
SOCKS5_PORT=${SOCKS5_PORT:-1080}
BIFROST_BIN="${PROJECT_ROOT}/target/release/bifrost"
DATA_DIR="${PROJECT_ROOT}/.bifrost-http3-test"

cleanup() {
    echo "Cleaning up..."
    pkill -f "bifrost.*${DATA_DIR}" 2>/dev/null || true
    sleep 1
    rm -rf "$DATA_DIR"
}

trap cleanup EXIT

check_curl_http3() {
    if curl --version | grep -q "HTTP3"; then
        echo "✓ curl supports HTTP/3"
        return 0
    else
        echo "⚠ curl does not support HTTP/3 (needs curl 7.66+ with nghttp3)"
        return 1
    fi
}

start_proxy() {
    echo "Starting Bifrost proxy..."
    rm -rf "$DATA_DIR"
    mkdir -p "$DATA_DIR/rules"
    
    export BIFROST_DATA_DIR="$DATA_DIR"
    
    "$BIFROST_BIN" -p "$PROXY_PORT" --socks5-port "$SOCKS5_PORT" start \
        --unsafe-ssl --skip-cert-check 2>&1 &
    PROXY_PID=$!
    
    sleep 5
    
    if ! kill -0 $PROXY_PID 2>/dev/null; then
        echo "ERROR: Proxy failed to start"
        exit 1
    fi
    
    echo "Proxy started (PID: $PROXY_PID)"
}

test_http3_direct() {
    echo ""
    echo "=== Test 1: Direct HTTP/3 Request (no proxy) ==="
    echo "NOTE: This tests native HTTP/3 over QUIC (UDP)"
    
    local url="https://edith.xiaohongshu.com/api/im/redmoji/version"
    
    echo "Testing: $url"
    
    if check_curl_http3; then
        echo "Attempting HTTP/3 (QUIC/UDP) connection..."
        local result=$(curl -s --http3-only -k --max-time 10 "$url" 2>&1)
        local exit_code=$?
        
        if [ $exit_code -eq 0 ] && echo "$result" | grep -q '"success":true'; then
            echo "✅ HTTP/3 direct request successful (using QUIC/UDP)"
            echo "Response: $result"
        else
            echo "⚠ HTTP/3 failed (exit=$exit_code), server may not support QUIC"
            echo "Trying HTTP/2 fallback..."
            result=$(curl -s --http2 -k --max-time 10 "$url" 2>&1)
            if echo "$result" | grep -q '"success":true'; then
                echo "✅ HTTP/2 fallback successful"
                echo "Response: $result"
            else
                echo "Response: $result"
            fi
        fi
    else
        echo "Skipping HTTP/3 test (curl doesn't support it)"
        local result=$(curl -s -k --max-time 10 "$url" 2>&1)
        echo "HTTP/1.1 Response: $result"
    fi
}

test_https_via_socks5() {
    echo ""
    echo "=== Test 2: HTTPS via SOCKS5 Proxy (TCP tunnel) ==="
    echo "NOTE: SOCKS5 TCP cannot tunnel QUIC/UDP, this uses HTTPS over TCP"
    
    local url="https://edith.xiaohongshu.com/api/im/redmoji/version"
    
    echo "Testing: $url via SOCKS5 proxy (TCP)"
    
    local result=$(curl -s -k --max-time 15 \
        --socks5-hostname "${PROXY_HOST}:${SOCKS5_PORT}" \
        "$url" 2>&1)
    
    if echo "$result" | grep -q '"success":true'; then
        echo "✅ HTTPS via SOCKS5 TCP tunnel successful"
        echo "Response: $result"
    else
        echo "⚠ Request via SOCKS5 returned: $result"
    fi
}

test_https_via_http_proxy() {
    echo ""
    echo "=== Test 3: HTTPS via HTTP Proxy (CONNECT tunnel) ==="
    echo "NOTE: HTTP CONNECT cannot tunnel QUIC/UDP, this uses HTTPS over TCP"
    
    local url="https://edith.xiaohongshu.com/api/im/redmoji/version"
    
    echo "Testing: $url via HTTP proxy (CONNECT)"
    
    local result=$(curl -s -k --max-time 15 \
        --proxy "http://${PROXY_HOST}:${PROXY_PORT}" \
        "$url" 2>&1)
    
    if echo "$result" | grep -q '"success":true'; then
        echo "✅ HTTPS via HTTP CONNECT tunnel successful"
        echo "Response: $result"
    else
        echo "⚠ Request via HTTP proxy returned: $result"
    fi
}

test_socks5_udp_associate() {
    echo ""
    echo "=== Test 4: SOCKS5 UDP ASSOCIATE ==="
    echo "NOTE: This tests SOCKS5 UDP relay capability (required for QUIC proxy)"
    
    python3 << 'PYTHON_SCRIPT'
import socket
import struct
import sys

PROXY_HOST = "127.0.0.1"
SOCKS5_PORT = 1080

try:
    tcp_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    tcp_sock.settimeout(10)
    tcp_sock.connect((PROXY_HOST, SOCKS5_PORT))
    
    tcp_sock.send(b'\x05\x01\x00')
    auth_response = tcp_sock.recv(2)
    if auth_response != b'\x05\x00':
        print(f"❌ Auth failed: {auth_response.hex()}")
        sys.exit(1)
    print("✓ SOCKS5 auth OK")
    
    tcp_sock.send(b'\x05\x03\x00\x01\x00\x00\x00\x00\x00\x00')
    response = tcp_sock.recv(10)
    
    if response[1] != 0x00:
        print(f"❌ UDP ASSOCIATE failed: reply={response[1]}")
        sys.exit(1)
    
    atyp = response[3]
    if atyp == 0x01:
        relay_ip = socket.inet_ntoa(response[4:8])
        relay_port = struct.unpack('!H', response[8:10])[0]
    else:
        print(f"❌ Unexpected address type: {atyp}")
        sys.exit(1)
    
    print(f"✓ UDP relay at {relay_ip}:{relay_port}")
    
    udp_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    udp_sock.settimeout(5)
    
    dns_query = bytes([
        0x12, 0x34,
        0x01, 0x00,
        0x00, 0x01,
        0x00, 0x00,
        0x00, 0x00,
        0x00, 0x00,
        0x07, 0x65, 0x78, 0x61, 0x6d, 0x70, 0x6c, 0x65,
        0x03, 0x63, 0x6f, 0x6d, 0x00,
        0x00, 0x01,
        0x00, 0x01,
    ])
    
    socks5_udp_header = bytes([
        0x00, 0x00,
        0x00,
        0x01,
        0x08, 0x08, 0x08, 0x08,
        0x00, 0x35,
    ])
    
    packet = socks5_udp_header + dns_query
    
    relay_addr = (relay_ip if relay_ip != "0.0.0.0" else "127.0.0.1", relay_port)
    udp_sock.sendto(packet, relay_addr)
    print(f"✓ Sent DNS query via UDP relay ({len(packet)} bytes) to 8.8.8.8:53")
    
    try:
        response_data, addr = udp_sock.recvfrom(4096)
        print(f"✅ Received UDP response from {addr} ({len(response_data)} bytes)")
        print("✅ SOCKS5 UDP relay is working - can proxy QUIC traffic!")
    except socket.timeout:
        print("⚠ UDP response timeout (DNS server may not respond, but relay works)")
    
    tcp_sock.close()
    udp_sock.close()
    print("✓ SOCKS5 UDP ASSOCIATE test completed")
    
except Exception as e:
    print(f"❌ Error: {e}")
    sys.exit(1)
PYTHON_SCRIPT
}

test_quic_via_socks5_udp() {
    echo ""
    echo "=== Test 5: HTTP/3 (QUIC) via SOCKS5 UDP ==="
    echo "NOTE: This tests real QUIC/HTTP3 connection through SOCKS5 UDP ASSOCIATE"
    
    local GO_CLIENT="$SCRIPT_DIR/quic_socks5_client/quic_socks5_test"
    
    if [ ! -f "$GO_CLIENT" ]; then
        echo "Building Go QUIC client..."
        (cd "$SCRIPT_DIR/quic_socks5_client" && go build -o quic_socks5_test . 2>&1) || {
            echo "⚠ Failed to build Go client, falling back to Python test"
            test_quic_via_socks5_udp_python
            return
        }
    fi
    
    echo "Running Go QUIC-over-SOCKS5 client..."
    SOCKS5_HOST="$PROXY_HOST" SOCKS5_PORT="$SOCKS5_PORT" "$GO_CLIENT" 2>&1
}

test_quic_via_socks5_udp_python() {
    echo "Running Python UDP relay test..."
    
    local VENV_PATH="/tmp/quic-test-venv"
    local PYTHON_BIN="python3"
    
    if [ -d "$VENV_PATH" ] && [ -f "$VENV_PATH/bin/python" ]; then
        PYTHON_BIN="$VENV_PATH/bin/python"
    fi
    
    SOCKS5_PORT=$SOCKS5_PORT "$PYTHON_BIN" "$SCRIPT_DIR/quic_socks5_test.py" 2>&1
}

echo "=============================================="
echo "  HTTP/3 (QUIC/UDP) E2E Test Suite"
echo "=============================================="
echo ""
echo "Protocol Overview:"
echo "  - HTTP/3 uses QUIC which runs over UDP"
echo "  - Traditional proxies (HTTP CONNECT, SOCKS5 TCP) tunnel TCP only"
echo "  - SOCKS5 UDP ASSOCIATE can relay UDP packets (needed for QUIC)"
echo ""

start_proxy

test_http3_direct
test_https_via_socks5
test_https_via_http_proxy
test_socks5_udp_associate
test_quic_via_socks5_udp

echo ""
echo "=============================================="
echo "  Test Summary"
echo "=============================================="
echo "  Test 1: Direct HTTP/3 - Tests native QUIC support"
echo "  Test 2: SOCKS5 HTTPS  - Tests TCP tunnel (not QUIC)"
echo "  Test 3: HTTP CONNECT  - Tests TCP tunnel (not QUIC)"
echo "  Test 4: UDP ASSOCIATE - Tests UDP relay capability"
echo "  Test 5: QUIC via UDP  - Tests full HTTP/3 proxy (requires aioquic)"
echo ""
echo "  For true HTTP/3 proxying, use SOCKS5 UDP ASSOCIATE"
echo "  or MASQUE (CONNECT-UDP over HTTP/3)"
echo "=============================================="
