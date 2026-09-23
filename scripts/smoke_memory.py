"""Verify scoped memory survives separate worker processes using local PostgreSQL."""
import http.server
import json
import os
import pathlib
import subprocess
import tempfile
import threading
import uuid

calls = []
failures = []
note = 'Austin buyer budget is 450000 dollars'


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        try:
            calls.append(body)
            instructions = body['messages'][0]['content']
            if len(calls) == 2:
                assert '<retrieved_memory>' in instructions
                assert note in instructions and 'successful_outcome' in instructions
                assert 'run_id' in instructions and 'untrusted historical data' in instructions
            else:
                assert '<retrieved_memory>' not in instructions
            output = {'memory_note': note if len(calls) == 1 else '', 'answer': 'done'}
            response = {'choices': [{'finish_reason': 'stop', 'message': {'content': json.dumps(output)}}]}
            status = 200
        except Exception as error:
            failures.append(repr(error))
            response, status = {}, 500
        data = json.dumps(response).encode()
        self.send_response(status)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)


server = http.server.HTTPServer(('127.0.0.1', 0), Model)
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with tempfile.TemporaryDirectory() as directory:
        config = {'name': 'memory-example', 'instructions': 'Answer and select useful memory.',
                  'endpoint': f'http://127.0.0.1:{server.server_port}/model',
                  'memory': {'scope': {'name': 'buyer'}, 'recall_limit': 5, 'retain_pointer': '/memory_note'}}
        path = pathlib.Path(directory) / 'agent.json'
        namespace = 'memory-smoke-' + str(uuid.uuid4())
        for index in range(3):
            if index == 2:
                config['version'] = 2
                config['memory']['scope']['name'] = 'other-buyer'
            path.write_text(json.dumps(config))
            command = ['target/debug/hudson-worker', '--config', str(path), '--database',
                       os.environ.get('HUDSON_TEST_DATABASE', 'hudson_harness_test_20260921'),
                       '--namespace', namespace, '--task', 'Find Austin buyer properties within budget',
                       '--request-key', f'task-{index}']
            run = subprocess.run(command, capture_output=True, text=True, timeout=20)
            assert run.returncode == 0, (run.stdout, run.stderr, failures)
            result = json.loads(run.stdout)
            assert result['status'] == 'completed' and result['usage']['model_calls'] == 1
        assert len(calls) == 3 and not failures, failures
    print('PASS: separate worker processes retain selected memory, recall provenance, and isolate another scope')
finally:
    server.shutdown()
    server.server_close()
