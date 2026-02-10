#!/usr/bin/env python3
"""
WebSocket Echo Server - 用于验证代理服务的 WebSocket 处理能力

启动方式:
    python3 ws_echo_server.py [port] [--ssl]
    python3 ws_echo_server.py 3020           # ws://
    python3 ws_echo_server.py 3021 --ssl     # wss://

功能:
    - 支持 ws:// 和 wss:// 连接
    - 详细打印连接信息、收到的消息
    - 消息回显功能
    - 返回连接时的 headers 信息
"""

import asyncio
import base64
import hashlib
import json
import os
import ssl
import struct
import sys
import tempfile
from datetime import datetime


WS_MAGIC_KEY = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


def log(message):
    timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S.%f')[:-3]
    print(f"[{timestamp}] {message}")


def generate_self_signed_cert():
    """生成临时自签名证书"""
    cert_dir = tempfile.mkdtemp(prefix='bifrost_ws_test_')
    cert_path = os.path.join(cert_dir, 'cert.pem')
    key_path = os.path.join(cert_dir, 'key.pem')

    try:
        from cryptography import x509
        from cryptography.x509.oid import NameOID
        from cryptography.hazmat.primitives import hashes, serialization
        from cryptography.hazmat.primitives.asymmetric import rsa
        from datetime import timedelta
        import ipaddress

        key = rsa.generate_private_key(public_exponent=65537, key_size=2048)

        subject = issuer = x509.Name([
            x509.NameAttribute(NameOID.COUNTRY_NAME, "CN"),
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "Bifrost Test"),
            x509.NameAttribute(NameOID.COMMON_NAME, "localhost"),
        ])

        cert = (
            x509.CertificateBuilder()
            .subject_name(subject)
            .issuer_name(issuer)
            .public_key(key.public_key())
            .serial_number(x509.random_serial_number())
            .not_valid_before(datetime.utcnow())
            .not_valid_after(datetime.utcnow() + timedelta(days=365))
            .add_extension(
                x509.SubjectAlternativeName([
                    x509.DNSName("localhost"),
                    x509.DNSName("*.local"),
                    x509.IPAddress(ipaddress.IPv4Address("127.0.0.1")),
                ]),
                critical=False,
            )
            .sign(key, hashes.SHA256())
        )

        with open(key_path, 'wb') as f:
            f.write(key.private_bytes(
                encoding=serialization.Encoding.PEM,
                format=serialization.PrivateFormat.TraditionalOpenSSL,
                encryption_algorithm=serialization.NoEncryption()
            ))

        with open(cert_path, 'wb') as f:
            f.write(cert.public_bytes(serialization.Encoding.PEM))

        return cert_path, key_path, cert_dir

    except ImportError:
        import subprocess
        subprocess.run([
            'openssl', 'req', '-x509', '-newkey', 'rsa:2048',
            '-keyout', key_path, '-out', cert_path,
            '-days', '365', '-nodes',
            '-subj', '/CN=localhost/O=Bifrost Test'
        ], capture_output=True)
        return cert_path, key_path, cert_dir


def parse_http_request(data):
    """解析 HTTP 请求头"""
    try:
        request_str = data.decode('utf-8', errors='ignore')
        lines = request_str.split('\r\n')

        if not lines:
            return None, None, None

        request_line = lines[0]
        parts = request_line.split(' ')
        method = parts[0] if len(parts) > 0 else ''
        path = parts[1] if len(parts) > 1 else '/'

        headers = {}
        for line in lines[1:]:
            if ':' in line:
                key, value = line.split(':', 1)
                headers[key.strip()] = value.strip()

        return method, path, headers
    except Exception as e:
        log(f"Error parsing request: {e}")
        return None, None, None


def compute_accept_key(key):
    """计算 WebSocket accept key"""
    sha1 = hashlib.sha1((key + WS_MAGIC_KEY).encode()).digest()
    return base64.b64encode(sha1).decode()


def create_ws_frame(payload, opcode=1, mask=False):
    """创建 WebSocket 帧"""
    if isinstance(payload, str):
        payload = payload.encode('utf-8')

    length = len(payload)
    header = bytes([0x80 | opcode])

    if length <= 125:
        header += bytes([length])
    elif length <= 65535:
        header += bytes([126]) + struct.pack('>H', length)
    else:
        header += bytes([127]) + struct.pack('>Q', length)

    return header + payload


def parse_ws_frame(data):
    """解析 WebSocket 帧"""
    if len(data) < 2:
        return None, None, data

    first_byte = data[0]
    second_byte = data[1]

    fin = (first_byte & 0x80) != 0
    opcode = first_byte & 0x0F
    masked = (second_byte & 0x80) != 0
    payload_len = second_byte & 0x7F

    offset = 2

    if payload_len == 126:
        if len(data) < 4:
            return None, None, data
        payload_len = struct.unpack('>H', data[2:4])[0]
        offset = 4
    elif payload_len == 127:
        if len(data) < 10:
            return None, None, data
        payload_len = struct.unpack('>Q', data[2:10])[0]
        offset = 10

    if masked:
        if len(data) < offset + 4 + payload_len:
            return None, None, data
        mask_key = data[offset:offset + 4]
        offset += 4
        payload = bytes(b ^ mask_key[i % 4] for i, b in enumerate(data[offset:offset + payload_len]))
    else:
        if len(data) < offset + payload_len:
            return None, None, data
        payload = data[offset:offset + payload_len]

    remaining = data[offset + payload_len:]

    return opcode, payload, remaining


class WebSocketConnection:
    def __init__(self, reader, writer, client_addr, use_ssl, server_port):
        self.reader = reader
        self.writer = writer
        self.client_addr = client_addr
        self.use_ssl = use_ssl
        self.server_port = server_port
        self.headers = {}
        self.path = '/'

    async def handle(self):
        try:
            data = await self.reader.read(4096)
            if not data:
                return

            method, path, headers = parse_http_request(data)
            self.headers = headers or {}
            self.path = path or '/'

            log(f"{'='*60}")
            log(f"New connection from {self.client_addr}")
            log(f"Request: {method} {path}")
            log(f"Headers:")
            for key, value in self.headers.items():
                log(f"  {key}: {value}")

            ws_key = self.headers.get('Sec-WebSocket-Key')
            if not ws_key:
                log("No Sec-WebSocket-Key found, not a WebSocket request")
                self.writer.close()
                return

            accept_key = compute_accept_key(ws_key)
            protocol = 'wss' if self.use_ssl else 'ws'

            response = (
                'HTTP/1.1 101 Switching Protocols\r\n'
                'Upgrade: websocket\r\n'
                'Connection: Upgrade\r\n'
                f'Sec-WebSocket-Accept: {accept_key}\r\n'
                'X-Echo-Server: bifrost-ws-test\r\n'
                f'X-Protocol: {protocol}\r\n'
                '\r\n'
            )
            self.writer.write(response.encode())
            await self.writer.drain()

            log(f"WebSocket handshake completed ({protocol}://)")

            welcome_msg = json.dumps({
                "type": "connection_info",
                "server": {
                    "type": "ws_echo_server",
                    "protocol": protocol,
                    "port": self.server_port,
                    "address": f"127.0.0.1:{self.server_port}"
                },
                "connection": {
                    "path": self.path,
                    "headers": self.headers,
                    "client_address": self.client_addr
                },
                "timestamp": datetime.now().isoformat()
            })

            self.writer.write(create_ws_frame(welcome_msg))
            await self.writer.drain()
            log(f"Sent welcome message with connection info")

            await self.message_loop()

        except Exception as e:
            log(f"Error handling connection: {e}")
        finally:
            self.writer.close()
            await self.writer.wait_closed()
            log(f"Connection closed: {self.client_addr}")

    async def message_loop(self):
        """消息处理循环"""
        buffer = b''

        while True:
            try:
                data = await asyncio.wait_for(self.reader.read(4096), timeout=300)
                if not data:
                    break

                buffer += data

                while buffer:
                    opcode, payload, buffer = parse_ws_frame(buffer)
                    if opcode is None:
                        break

                    if opcode == 8:
                        log("Received close frame")
                        self.writer.write(create_ws_frame(b'', opcode=8))
                        await self.writer.drain()
                        return

                    elif opcode == 9:
                        log("Received ping, sending pong")
                        self.writer.write(create_ws_frame(payload, opcode=10))
                        await self.writer.drain()

                    elif opcode == 10:
                        log("Received pong")

                    elif opcode in (1, 2):
                        if opcode == 1:
                            try:
                                message = payload.decode('utf-8')
                                log(f"Received text: {message[:200]}{'...' if len(message) > 200 else ''}")
                            except:
                                message = payload.hex()
                                log(f"Received binary (as hex): {message[:100]}")
                        else:
                            log(f"Received binary: {len(payload)} bytes")
                            message = payload.hex()

                        echo_response = json.dumps({
                            "type": "echo",
                            "opcode": opcode,
                            "message": message if opcode == 1 else None,
                            "binary_hex": payload.hex() if opcode == 2 else None,
                            "length": len(payload),
                            "timestamp": datetime.now().isoformat()
                        })

                        self.writer.write(create_ws_frame(echo_response))
                        await self.writer.drain()
                        log(f"Sent echo response")

            except asyncio.TimeoutError:
                log("Connection timeout, sending ping")
                self.writer.write(create_ws_frame(b'ping', opcode=9))
                await self.writer.drain()
            except Exception as e:
                log(f"Error in message loop: {e}")
                break


async def start_server(host, port, use_ssl=False):
    ssl_context = None
    cert_dir = None

    if use_ssl:
        log("Generating self-signed certificate...")
        cert_path, key_path, cert_dir = generate_self_signed_cert()
        ssl_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ssl_context.load_cert_chain(cert_path, key_path)
        log(f"Certificate loaded: {cert_path}")

    protocol = 'wss' if use_ssl else 'ws'

    async def client_handler(reader, writer):
        addr = writer.get_extra_info('peername')
        client_addr = f"{addr[0]}:{addr[1]}" if addr else "unknown"
        conn = WebSocketConnection(reader, writer, client_addr, use_ssl, port)
        await conn.handle()

    server = await asyncio.start_server(
        client_handler,
        host,
        port,
        ssl=ssl_context
    )

    print(f"""
╔══════════════════════════════════════════════════════════════╗
║         WebSocket Echo Server - Bifrost E2E Testing          ║
╠══════════════════════════════════════════════════════════════╣
║  Address: {protocol}://{host}:{port:<5}                               ║
║  Protocol: {protocol.upper():<4}                                           ║
║  Purpose: Test WebSocket forwarding                          ║
║                                                              ║
║  Features:                                                   ║
║    - Echo all received messages                              ║
║    - Return connection headers info                          ║
║    - Ping/Pong support                                       ║
╚══════════════════════════════════════════════════════════════╝
""")

    log(f"Starting WebSocket Echo Server on {protocol}://{host}:{port}...")
    log("Press Ctrl+C to stop\n")

    try:
        async with server:
            await server.serve_forever()
    except KeyboardInterrupt:
        log("Shutting down server...")
    finally:
        if cert_dir:
            import shutil
            shutil.rmtree(cert_dir, ignore_errors=True)


def main():
    port = 3020
    use_ssl = False

    args = sys.argv[1:]
    for arg in args:
        if arg == '--ssl':
            use_ssl = True
        elif arg.isdigit():
            port = int(arg)

    asyncio.run(start_server('127.0.0.1', port, use_ssl))


if __name__ == '__main__':
    main()
