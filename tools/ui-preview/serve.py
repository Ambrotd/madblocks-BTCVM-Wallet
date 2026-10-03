"""Serves the wallet's window (app/ui) with a stand-in for its Rust side, to
look at it and test it in a browser:

    python tools/ui-preview/serve.py [port]

then open http://127.0.0.1:8765/?state=wallet&lang=es (the states are listed
in tools/ui-preview/mock.js). Only for development: nothing here goes into
the exe, which loads app/ui alone.
"""

import http.server
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
UI = ROOT / "app" / "ui"
HERE = pathlib.Path(__file__).resolve().parent
APP_SCRIPT = '<script type="module" src="app.js"></script>'


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".js": "text/javascript",
        ".svg": "image/svg+xml",
        ".css": "text/css",
    }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(UI), **kwargs)

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def send(self, body: bytes, kind: str):
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path in ("/", "/index.html"):
            html = (UI / "index.html").read_text(encoding="utf-8")
            if APP_SCRIPT not in html:
                raise RuntimeError("app/ui/index.html no longer loads app.js as expected")
            html = html.replace(APP_SCRIPT, '<script src="/__preview/mock.js"></script>\n  ' + APP_SCRIPT)
            return self.send(html.encode("utf-8"), "text/html; charset=utf-8")
        if path == "/__preview/mock.js":
            return self.send((HERE / "mock.js").read_bytes(), "text/javascript; charset=utf-8")
        return super().do_GET()


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    print(f"the wallet's window, with a stand-in for its Rust side: http://127.0.0.1:{port}/?state=wallet&lang=es")
    server.serve_forever()


if __name__ == "__main__":
    main()
