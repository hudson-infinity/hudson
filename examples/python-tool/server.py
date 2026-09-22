"""A customer-owned, read-only HTTP tool using only Python's standard library."""
import argparse
import json
import math
from http.server import BaseHTTPRequestHandler, HTTPServer


def statistics(arguments):
    if not isinstance(arguments, dict) or set(arguments) != {'values'}:
        raise ValueError('provide only a values array')
    values = arguments['values']
    if not isinstance(values, list) or not 1 <= len(values) <= 1000:
        raise ValueError('provide between 1 and 1000 values')
    if any(type(value) not in (int, float) or not math.isfinite(value) for value in values):
        raise ValueError('values must be finite numbers')
    return {'count': len(values), 'mean': math.fsum(value / len(values) for value in values),
            'minimum': min(values), 'maximum': max(values)}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        if self.path != '/statistics':
            return self.respond(404, {'error': 'unknown tool'})
        try:
            size = int(self.headers.get('Content-Length', '0'))
            if not 0 < size <= 1024 * 1024:
                return self.respond(413, {'error': 'body must be at most 1 MiB'})
            self.connection.settimeout(5)
            result = statistics(json.loads(self.rfile.read(size)))
        except (ValueError, OverflowError, OSError):
            return self.respond(400, {'error': 'expected 1 to 1000 finite numbers in values'})
        self.respond(200, result)

    def respond(self, status, result):
        body = json.dumps(result, allow_nan=False).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=9001)
    args = parser.parse_args()
    with HTTPServer(('127.0.0.1', args.port), Handler) as server:
        print(f'http://127.0.0.1:{server.server_port}/statistics', flush=True)
        server.serve_forever()
