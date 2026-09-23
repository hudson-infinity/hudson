"""Exercise the customer-owned real-estate example with a local model fixture."""
import http.server
import json
import pathlib
import runpy
import subprocess
import tempfile
import threading

calls, failures = [], []
expected = {'matches': [{'id': 'demo-1', 'reason': 'Austin property within the requested budget'}],
            'memory_note': 'Buyer prefers Austin properties under 450000 dollars'}


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        try:
            calls.append(body)
            if len(calls) == 1:
                assert 'search_properties' in [t['function']['name'] for t in body['tools']]
                message = {'tool_calls': [{'id': 'search', 'type': 'function', 'function': {
                    'name': 'search_properties', 'arguments': '{"city":"Austin","max_price":450000}'}}]}
                reason = 'tool_calls'
            else:
                assert len(calls) == 2
                receipt = json.loads(body['messages'][-1]['content'])
                assert receipt['status'] == 'success'
                assert [p['id'] for p in receipt['value']['properties']] == ['demo-1']
                message, reason = {'content': json.dumps(expected)}, 'stop'
            status, response = 200, {'choices': [{'message': message, 'finish_reason': reason}]}
        except Exception as error:
            failures.append(repr(error))
            status, response = 500, {}
        payload = json.dumps(response).encode()
        self.send_response(status)
        self.send_header('Content-Length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


handler = runpy.run_path('examples/real-estate/tools.py')['Handler']
service = http.server.HTTPServer(('127.0.0.1', 0), handler)
model = http.server.HTTPServer(('127.0.0.1', 0), Model)
for server in (service, model):
    threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with tempfile.TemporaryDirectory() as directory:
        config = json.loads(pathlib.Path('examples/real-estate/agent.json').read_text())
        config['endpoint'] = f'http://127.0.0.1:{model.server_port}/model'
        config['http_tools'][0]['endpoint'] = f'http://127.0.0.1:{service.server_port}/search'
        path = pathlib.Path(directory) / 'agent.json'
        path.write_text(json.dumps(config))
        run = subprocess.run(['target/debug/hudson-worker', '--config', str(path), '--task',
                              'Find Austin properties under 450000 dollars'], capture_output=True, text=True, timeout=20)
        assert run.returncode == 0, (run.stdout, run.stderr, failures)
        result = json.loads(run.stdout)
        assert result['status'] == 'completed' and result['result'] == expected
        assert result['assessment']['passed']
        assert result['assessment']['evidence'][0]['tool_name'] == 'search_properties'
        assert len(calls) == 2 and not failures, failures
    print('PASS: real-estate customer tool, filtered inventory, shared loop and recorded evidence')
finally:
    for server in (service, model):
        server.shutdown()
        server.server_close()
