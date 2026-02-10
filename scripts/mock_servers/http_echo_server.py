#!/usr/bin/env python3
"""
HTTP Echo Server - 用于验证代理服务对请求的处理

启动方式:
    python3 http_echo_server.py [port]
    python3 http_echo_server.py 3000

功能:
    - 详细回显所有收到的请求信息
    - 返回 JSON 格式，便于断言验证
    - 支持所有 HTTP 方法
    - 支持自定义响应头和状态码
"""

import http.server
import json
import socketserver
import sys
import urllib.parse
from datetime import datetime


class EchoHandler(http.server.BaseHTTPRequestHandler):
    """回显所有请求信息的 Handler"""

    protocol_version = 'HTTP/1.1'

    def log_message(self, format, *args):
        """自定义日志格式，包含更多调试信息"""
        timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S.%f')[:-3]
        print(f"[{timestamp}] {self.client_address[0]}:{self.client_address[1]} - {format % args}")

    def _build_response(self, body_content=None):
        """构建统一的响应数据结构"""
        headers_dict = {}
        headers_list = []
        for key, value in self.headers.items():
            headers_dict[key] = value
            headers_list.append(f"{key}: {value}")

        parsed_path = urllib.parse.urlparse(self.path)
        query_params = urllib.parse.parse_qs(parsed_path.query)

        response_data = {
            "timestamp": datetime.now().isoformat(),
            "server": {
                "type": "http_echo_server",
                "protocol": "http",
                "port": self.server.server_address[1],
                "address": f"{self.server.server_address[0]}:{self.server.server_address[1]}"
            },
            "request": {
                "method": self.command,
                "path": self.path,
                "parsed_path": parsed_path.path,
                "query_string": parsed_path.query,
                "query_params": query_params,
                "http_version": self.request_version,
                "headers": headers_dict,
                "headers_raw": headers_list,
                "client_address": f"{self.client_address[0]}:{self.client_address[1]}"
            }
        }

        if body_content is not None:
            try:
                response_data["request"]["body"] = body_content.decode('utf-8')
                response_data["request"]["body_raw"] = body_content.hex()
            except UnicodeDecodeError:
                response_data["request"]["body"] = None
                response_data["request"]["body_raw"] = body_content.hex()
                response_data["request"]["body_encoding_error"] = True

        return response_data

    def _get_content_type_and_body(self, response_data):
        """根据请求路径确定 Content-Type 和响应体"""
        path = self.path.lower()
        
        if path.endswith('.html') or path.endswith('.htm'):
            body = f"""<!DOCTYPE html>
<html>
<head><title>Echo Response</title></head>
<body>
<h1>Echo Response</h1>
<pre>{json.dumps(response_data, indent=2, ensure_ascii=False)}</pre>
</body>
</html>"""
            return 'text/html; charset=utf-8', body
        elif path.endswith('.js'):
            body = f"""// Echo Response
var echoData = {json.dumps(response_data, ensure_ascii=False)};
console.log('Echo Server Response:', echoData);"""
            return 'application/javascript; charset=utf-8', body
        elif path.endswith('.css'):
            body = f"""/* Echo Response */
/* Request Info: {response_data.get('request', {}).get('path', '')} */
body {{ color: #333; }}
.echo-data {{ display: block; }}"""
            return 'text/css; charset=utf-8', body
        elif path.endswith('.xml'):
            body = f"""<?xml version="1.0" encoding="UTF-8"?>
<echo>
<request>
<method>{response_data.get('request', {}).get('method', '')}</method>
<path>{response_data.get('request', {}).get('path', '')}</path>
</request>
</echo>"""
            return 'application/xml; charset=utf-8', body
        elif path.endswith('.txt'):
            body = f"Echo Response\n\n{json.dumps(response_data, indent=2, ensure_ascii=False)}"
            return 'text/plain; charset=utf-8', body
        else:
            return 'application/json; charset=utf-8', json.dumps(response_data, indent=2, ensure_ascii=False)

    def _send_response(self, status_code=200, extra_headers=None, body_content=None):
        """发送响应"""
        response_data = self._build_response(body_content)
        content_type, response_body = self._get_content_type_and_body(response_data)

        self.send_response(status_code)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(response_body.encode('utf-8'))))
        self.send_header('X-Echo-Server', 'bifrost-test')
        self.send_header('X-Request-Method', self.command)
        self.send_header('X-Request-Path', self.path)
        self.send_header('Connection', 'keep-alive')

        if extra_headers:
            for key, value in extra_headers.items():
                self.send_header(key, value)

        self.end_headers()
        self.wfile.write(response_body.encode('utf-8'))

        print(f"  -> Response: {status_code}, Content-Type: {content_type.split(';')[0]}, Body size: {len(response_body)} bytes")

    def _read_body(self):
        """读取请求体"""
        content_length = self.headers.get('Content-Length')
        if content_length:
            try:
                length = int(content_length)
                return self.rfile.read(length)
            except (ValueError, Exception) as e:
                print(f"  -> Error reading body: {e}")
                return None
        return None

    def do_GET(self):
        """处理 GET 请求"""
        print(f"\n{'='*60}")
        print(f"GET {self.path}")
        print(f"Headers:")
        for key, value in self.headers.items():
            print(f"  {key}: {value}")
        self._send_response()

    def do_POST(self):
        """处理 POST 请求"""
        print(f"\n{'='*60}")
        print(f"POST {self.path}")
        print(f"Headers:")
        for key, value in self.headers.items():
            print(f"  {key}: {value}")

        body = self._read_body()
        if body:
            print(f"Body ({len(body)} bytes):")
            try:
                print(f"  {body.decode('utf-8')[:500]}...")
            except UnicodeDecodeError:
                print(f"  [Binary data: {body.hex()[:100]}...]")

        self._send_response(body_content=body)

    def do_PUT(self):
        """处理 PUT 请求"""
        print(f"\n{'='*60}")
        print(f"PUT {self.path}")
        print(f"Headers:")
        for key, value in self.headers.items():
            print(f"  {key}: {value}")

        body = self._read_body()
        self._send_response(body_content=body)

    def do_DELETE(self):
        """处理 DELETE 请求"""
        print(f"\n{'='*60}")
        print(f"DELETE {self.path}")
        print(f"Headers:")
        for key, value in self.headers.items():
            print(f"  {key}: {value}")
        self._send_response()

    def do_PATCH(self):
        """处理 PATCH 请求"""
        print(f"\n{'='*60}")
        print(f"PATCH {self.path}")
        body = self._read_body()
        self._send_response(body_content=body)

    def do_HEAD(self):
        """处理 HEAD 请求"""
        print(f"\n{'='*60}")
        print(f"HEAD {self.path}")
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('X-Echo-Server', 'bifrost-test')
        self.end_headers()

    def do_OPTIONS(self):
        """处理 OPTIONS 请求，支持 CORS preflight"""
        print(f"\n{'='*60}")
        print(f"OPTIONS {self.path}")
        print(f"Headers:")
        for key, value in self.headers.items():
            print(f"  {key}: {value}")

        self.send_response(200)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, PATCH, OPTIONS, HEAD')
        self.send_header('Access-Control-Allow-Headers', '*')
        self.send_header('Access-Control-Max-Age', '86400')
        self.send_header('Content-Length', '0')
        self.end_headers()


class ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    """支持多线程的 HTTP Server"""
    allow_reuse_address = True
    daemon_threads = True


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 3000
    host = '127.0.0.1'

    print(f"""
╔══════════════════════════════════════════════════════════════╗
║           HTTP Echo Server - Bifrost E2E Testing             ║
╠══════════════════════════════════════════════════════════════╣
║  Address: http://{host}:{port:<5}                              ║
║  Purpose: Echo all request details for verification          ║
║                                                              ║
║  Response format: JSON with complete request info            ║
║  Supported methods: GET, POST, PUT, DELETE, PATCH, OPTIONS   ║
╚══════════════════════════════════════════════════════════════╝
""")

    with ThreadedHTTPServer((host, port), EchoHandler) as httpd:
        print(f"Starting HTTP Echo Server on {host}:{port}...")
        print("Press Ctrl+C to stop\n")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down server...")
            httpd.shutdown()


if __name__ == '__main__':
    main()
