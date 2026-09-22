"""Run the documented customer-owned Python tool through Hudson, without paid IO."""
import http.server
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import threading
import urllib.error
import urllib.request

expected = {'count': 3, 'mean': 20.0, 'minimum': 10, 'maximum': 30}
calls = []
failures = []


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        try:
            calls.append(body)
            assert [tool['function']['name'] for tool in body['tools']] == ['statistics']
            if len(calls) == 1:
                assert json.loads(body['messages'][-1]['content']) == {'values': [10, 20, 30]}
                response = {'choices': [{'finish_reason': 'tool_calls', 'message': {'tool_calls': [
                    {'id': 'stats-1', 'type': 'function', 'function': {'name': 'statistics', 'arguments': '{"values":[10,20,30]}'}}
                ]}}]}
            else:
                assert len(calls) == 2
                result = body['messages'][-1]
                assert result['role'] == 'tool' and result['tool_call_id'] == 'stats-1'
                assert json.loads(result['content']) == {'status': 'success', 'value': expected}
                response = {'choices': [{'finish_reason': 'stop', 'message': {'content': json.dumps(expected)}}]}
            self.send_response(200)
        except Exception as error:
            failures.append(repr(error))
            response = {}
            self.send_response(500)
        data = json.dumps(response).encode()
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)


model = http.server.HTTPServer(('127.0.0.1', 0), Model)
threading.Thread(target=model.serve_forever, daemon=True).start()
service = subprocess.Popen([sys.executable, 'examples/python-tool/server.py', '--port', '0'], stdout=subprocess.PIPE, text=True)
try:
    endpoint = service.stdout.readline().strip()
    assert endpoint.startswith('http://127.0.0.1:')
    for values in ([], [True], ['20'], [float('inf')]):
        req = urllib.request.Request(endpoint, data=json.dumps({'values': values}).encode(), headers={'Content-Type': 'application/json'})
        try:
            urllib.request.urlopen(req, timeout=3)
            raise AssertionError('invalid values accepted')
        except urllib.error.HTTPError as error:
            assert error.code == 400
    with tempfile.TemporaryDirectory() as directory:
        config = json.loads(pathlib.Path('examples/python-tool/agent.json').read_text())
        config['endpoint'] = f'http://127.0.0.1:{model.server_port}/model'
        config['http_tools'][0]['endpoint'] = endpoint
        path = pathlib.Path(directory) / 'agent.json'
        path.write_text(json.dumps(config))
        env = os.environ.copy()
        env.pop('OPENAI_API_KEY', None)
        run = subprocess.run(['target/debug/hudson-worker', '--config', str(path), '--input-file', 'examples/data-task.json'], env=env, capture_output=True, text=True, timeout=20)
        assert run.returncode == 0, (run.stderr, run.stdout, failures)
        result = json.loads(run.stdout)
        assert result['status'] == 'completed' and result['result'] == expected
        assert result['usage']['model_calls'] == 2 and result['usage']['tool_calls'] == 1
        assert result['assessment']['passed'] and not failures
    print('PASS: real Python tool service, invalid argument rejection, structured input, two model calls, one HTTP tool call, verified output')
finally:
    service.terminate()
    service.wait(timeout=5)
    model.shutdown()
    model.server_close()
