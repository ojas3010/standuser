"""Static file server that gzips text, JSON, JS and wasm responses, as Cloudflare Pages does. Usage: serve_gz.py PORT (serves the cwd)."""
import gzip
import http.server
import mimetypes
import os
import sys

TYPES = (".html", ".js", ".json", ".css", ".svg", ".wasm", ".txt")
mimetypes.add_type("application/wasm", ".wasm")


class H(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        path = self.translate_path(self.path.split("?")[0])
        if os.path.isdir(path):
            path = os.path.join(path, "index.html")
        if not os.path.isfile(path) or not path.endswith(TYPES) or "gzip" not in self.headers.get("Accept-Encoding", ""):
            return super().do_GET()
        body = gzip.compress(open(path, "rb").read(), 6)
        self.send_response(200)
        self.send_header("Content-Type", mimetypes.guess_type(path)[0] or "application/octet-stream")
        self.send_header("Content-Encoding", "gzip")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


if __name__ == "__main__":
    http.server.ThreadingHTTPServer(("0.0.0.0", int(sys.argv[1])), H).serve_forever()
