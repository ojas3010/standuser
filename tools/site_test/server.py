"""Static file server for the browser tests, with a mock POST /api/tmdb that answers like api/tmdb.js."""
import json, sys, unicodedata
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer


def key(name, year):
    return "".join(c for c in unicodedata.normalize("NFKD", name) if c.isalnum()).lower() + f"|{year}"


class H(SimpleHTTPRequestHandler):
    def do_POST(self):
        if self.path != "/api/tmdb":
            self.send_error(404)
            return
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
        films = [[key(f["name"], f["year"]), "", 900000 + i, f["year"], 100, "fr", "FR|JP", "7001", "7002", None, None, None] for i, f in enumerate(body["films"])]
        out = json.dumps(dict(v=1, countries={"FR": "France", "JP": "Japan"}, people={"7001": "Mock Director", "7002": "Mock Composer"}, films=films, missing=[])).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


ThreadingHTTPServer(("0.0.0.0", int(sys.argv[1])), H).serve_forever()
