"""Customer-owned sample inventory tool. All inventory here is fictional."""
import json
import math
from http.server import BaseHTTPRequestHandler, HTTPServer

INVENTORY = [
    {"id": "demo-1", "city": "Austin", "price": 400000, "bedrooms": 3},
    {"id": "demo-2", "city": "Austin", "price": 550000, "bedrooms": 4},
    {"id": "demo-3", "city": "Dallas", "price": 350000, "bedrooms": 2},
]


def search_properties(city, max_price):
    if not isinstance(city, str) or type(max_price) not in (int, float):
        raise ValueError("city must be text and max_price must be a number")
    if not math.isfinite(max_price) or max_price < 0:
        raise ValueError("max_price must be finite and nonnegative")
    return {"properties": [dict(p) for p in INVENTORY
                           if p["city"].casefold() == city.casefold()
                           and p["price"] <= max_price]}


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        status, result = 200, None
        try:
            if self.path != "/search":
                status, result = 404, {"error": "unknown tool"}
            else:
                self.connection.settimeout(5)
                size = int(self.headers.get("Content-Length", "0"))
                if not 0 < size <= 8192:
                    raise ValueError("body must be between 1 and 8192 bytes")
                result = search_properties(**json.loads(self.rfile.read(size)))
        except (ValueError, TypeError, OSError):
            status, result = 400, {"error": "provide city and nonnegative max_price"}
        body = json.dumps(result, allow_nan=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


if __name__ == "__main__":
    with HTTPServer(("127.0.0.1", 8099), Handler) as server:
        print("Fictional inventory tool: http://127.0.0.1:8099/search", flush=True)
        server.serve_forever()
