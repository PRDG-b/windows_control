"""LAN-only disposable test page and independent oracle; no third-party packages."""
import argparse
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path


def make_server(host, port):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path == "/oracle":
                body, mime = json.dumps(self.server.state).encode(), "application/json"
            elif self.path.split("?", 1)[0] in ("/", "/index.html"):
                body, mime = Path(__file__).with_name("index.html").read_bytes(), "text/html; charset=utf-8"
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", mime)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            if self.path != "/events":
                self.send_error(404)
                return
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= 1024:
                    raise ValueError("invalid length")
                state = json.loads(self.rfile.read(length))
                if not isinstance(state, dict) or set(state) != {"phase", "clicks", "scenario"}:
                    raise ValueError("invalid fields")
                if state["phase"] not in ("ready", "waiting", "completed", "failed", "confirm"):
                    raise ValueError("invalid phase")
                if state["scenario"] not in ("success", "failure", "duplicate", "popup"):
                    raise ValueError("invalid scenario")
                if type(state["clicks"]) is not int or state["clicks"] < 0:
                    raise ValueError("invalid click count")
            except (ValueError, TypeError):
                self.send_error(400)
                return
            self.server.state = state
            self.send_response(204)
            self.end_headers()

    server = HTTPServer((host, port), Handler)
    server.state = {}
    return server


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    with make_server(args.host, args.port) as server:
        print(f"Test page: http://{args.host}:{server.server_port}/", flush=True)
        server.serve_forever()
