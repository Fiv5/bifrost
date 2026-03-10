#!/usr/bin/env python3
import argparse
import base64
import hashlib
import os
import socket
import struct
import time


WS_MAGIC_KEY = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


def _recv_exact(sock: socket.socket, n: int) -> bytes:
    buf = bytearray()
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("unexpected EOF")
        buf.extend(chunk)
    return bytes(buf)


def _read_http_headers(sock: socket.socket, timeout_s: float) -> bytes:
    sock.settimeout(timeout_s)
    data = bytearray()
    while b"\r\n\r\n" not in data:
        chunk = sock.recv(4096)
        if not chunk:
            break
        data.extend(chunk)
        if len(data) > 64 * 1024:
            raise ValueError("http header too large")
    return bytes(data)


def _parse_http_response(data: bytes):
    head = data.split(b"\r\n\r\n", 1)[0].decode("utf-8", errors="replace")
    lines = head.split("\r\n")
    status_line = lines[0]
    parts = status_line.split(" ", 2)
    code = int(parts[1]) if len(parts) > 1 and parts[1].isdigit() else 0
    headers = {}
    for line in lines[1:]:
        if ":" in line:
            k, v = line.split(":", 1)
            headers[k.strip().lower()] = v.strip()
    return code, headers


def _ws_make_client_text_frame(payload: bytes) -> bytes:
    fin_opcode = 0x81
    mask_bit = 0x80
    length = len(payload)
    hdr = bytearray([fin_opcode])
    if length <= 125:
        hdr.append(mask_bit | length)
    elif length <= 65535:
        hdr.append(mask_bit | 126)
        hdr.extend(struct.pack(">H", length))
    else:
        hdr.append(mask_bit | 127)
        hdr.extend(struct.pack(">Q", length))
    mask_key = os.urandom(4)
    hdr.extend(mask_key)
    masked = bytes(b ^ mask_key[i % 4] for i, b in enumerate(payload))
    return bytes(hdr) + masked


def _ws_make_client_close_frame(code: int = 1000) -> bytes:
    payload = struct.pack(">H", code)
    fin_opcode = 0x88
    mask_bit = 0x80
    hdr = bytearray([fin_opcode, mask_bit | len(payload)])
    mask_key = os.urandom(4)
    hdr.extend(mask_key)
    masked = bytes(b ^ mask_key[i % 4] for i, b in enumerate(payload))
    return bytes(hdr) + masked


def _ws_read_frame(sock: socket.socket, timeout_s: float):
    sock.settimeout(timeout_s)
    b1, b2 = _recv_exact(sock, 2)
    fin = (b1 & 0x80) != 0
    opcode = b1 & 0x0F
    masked = (b2 & 0x80) != 0
    length = b2 & 0x7F
    if length == 126:
        length = struct.unpack(">H", _recv_exact(sock, 2))[0]
    elif length == 127:
        length = struct.unpack(">Q", _recv_exact(sock, 8))[0]
    mask_key = b""
    if masked:
        mask_key = _recv_exact(sock, 4)
    payload = _recv_exact(sock, length) if length else b""
    if masked and payload:
        payload = bytes(b ^ mask_key[i % 4] for i, b in enumerate(payload))
    return fin, opcode, payload


def run(
    proxy_host: str,
    proxy_port: int,
    request_host_header: str,
    path: str,
    message_text: str,
    messages: int,
    timeout_s: float,
    expect_binary: bool,
):
    key = base64.b64encode(os.urandom(16)).decode("ascii")
    expected_accept = base64.b64encode(
        hashlib.sha1((key + WS_MAGIC_KEY).encode("utf-8")).digest()
    ).decode("ascii")

    sock = socket.create_connection((proxy_host, proxy_port), timeout=timeout_s)
    try:
        req = (
            f"GET {path} HTTP/1.1\r\n"
            f"Host: {request_host_header}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            "Sec-WebSocket-Version: 13\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "\r\n"
        ).encode("utf-8")
        sock.sendall(req)
        resp = _read_http_headers(sock, timeout_s)
        code, headers = _parse_http_response(resp)
        if code != 101:
            raise RuntimeError(f"handshake failed: status={code}")
        accept = headers.get("sec-websocket-accept", "")
        if accept != expected_accept:
            raise RuntimeError("handshake failed: bad sec-websocket-accept")

        payload = message_text.encode("utf-8")
        for _ in range(messages):
            sock.sendall(_ws_make_client_text_frame(payload))

        recv_ok = 0
        start = time.time()
        expected_opcode = 2 if expect_binary else 1
        while recv_ok < messages and (time.time() - start) < timeout_s:
            _, opcode, _payload = _ws_read_frame(sock, timeout_s)
            if opcode == expected_opcode:
                recv_ok += 1
            elif opcode == 8:
                break
            elif opcode == 9:
                pong = bytearray([0x8A, 0x80 | len(_payload)])
                mask_key = os.urandom(4)
                pong.extend(mask_key)
                pong_payload = bytes(b ^ mask_key[i % 4] for i, b in enumerate(_payload))
                sock.sendall(bytes(pong) + pong_payload)

        sock.sendall(_ws_make_client_close_frame(1000))
        if recv_ok < messages:
            raise RuntimeError(f"expected {messages} messages, got {recv_ok}")
    finally:
        try:
            sock.close()
        except Exception:
            pass


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--proxy-host", required=True)
    ap.add_argument("--proxy-port", type=int, required=True)
    ap.add_argument("--host-header", required=True)
    ap.add_argument("--path", default="/ws")
    ap.add_argument("--message", default='{"type":"ping"}')
    ap.add_argument("--messages", type=int, default=1000)
    ap.add_argument("--timeout", type=float, default=30.0)
    ap.add_argument("--expect-binary", action="store_true")
    args = ap.parse_args()

    run(
        proxy_host=args.proxy_host,
        proxy_port=args.proxy_port,
        request_host_header=args.host_header,
        path=args.path,
        message_text=args.message,
        messages=args.messages,
        timeout_s=args.timeout,
        expect_binary=args.expect_binary,
    )


if __name__ == "__main__":
    main()
