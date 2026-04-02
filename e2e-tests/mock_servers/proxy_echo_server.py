#!/usr/bin/env python3
import http.server
import json
import socketserver
import sys
import urllib.parse


class ProxyEchoHandler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, format, *args):
        sys.stdout.write("%s\n" % (format % args))
        sys.stdout.flush()

    def _write_json(self, status_code, payload):
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status_code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/health":
            self._write_json(200, {"status": "ok", "server": "proxy_echo"})
            return

        parsed = urllib.parse.urlparse(self.path)
        headers = {k.lower(): v for k, v in self.headers.items()}
        payload = {
            "server": {
                "type": "proxy_echo_server",
                "port": self.server.server_address[1],
            },
            "request": {
                "method": self.command,
                "raw_path": self.path,
                "scheme": parsed.scheme,
                "host": parsed.hostname,
                "port": parsed.port,
                "path": parsed.path,
                "query": parsed.query,
                "headers": headers,
            },
        }
        self._write_json(200, payload)


class ThreadingTCPServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 3999
    with ThreadingTCPServer(("127.0.0.1", port), ProxyEchoHandler) as httpd:
        print(f"proxy echo server listening on 127.0.0.1:{port}")
        sys.stdout.flush()
        httpd.serve_forever()


if __name__ == "__main__":
    main()
