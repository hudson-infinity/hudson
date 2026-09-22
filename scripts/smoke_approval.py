"""Offline model + HTTP tool approval across worker processes. Needs local test DB."""
import http.server
import json
import os
import pathlib
import subprocess
import tempfile
import threading
import uuid

executed = []


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        if self.path == '/tool':
            executed.append(self.headers['Idempotency-Key'])
            response = {'saved': True}
        elif request['messages'][-1]['role'] == 'tool':
            response = {'choices': [{'finish_reason': 'stop', 'message': {'content': 'saved'}}]}
        else:
            response = {'choices': [{'finish_reason': 'tool_calls', 'message': {'tool_calls': [
                {'id': 'write-1', 'type': 'function', 'function': {'name': 'save', 'arguments': '{"value":42}'}}
            ]}}]}
        body = json.dumps(response).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


server = http.server.HTTPServer(('127.0.0.1', 0), Handler)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    base = f'http://127.0.0.1:{server.server_port}'
    with tempfile.TemporaryDirectory() as directory:
        config = pathlib.Path(directory) / 'agent.json'
        config.write_text(json.dumps({
            'name': 'writer', 'instructions': 'Save the supplied value', 'endpoint': base + '/model',
            'http_tools': [{'name': 'save', 'description': 'Save a value', 'endpoint': base + '/tool',
                            'input_schema': {'type': 'object'}, 'effect': 'write', 'require_approval': True}]
        }))
        common = ['target/debug/hudson-worker', '--database', os.environ.get('HUDSON_TEST_DATABASE', 'hudson_harness_test_20260921'), '--namespace', 'approval-' + str(uuid.uuid4())]
        env = os.environ.copy()
        env.pop('OPENAI_API_KEY', None)

        def run(*arguments):
            result = subprocess.run(common + list(arguments), env=env, capture_output=True, text=True, timeout=30)
            assert result.returncode == 0, result.stderr
            return json.loads(result.stdout)

        waiting = run('--config', str(config), '--task', 'Save 42')
        assert waiting['status'] == 'waiting' and not executed
        operation = waiting['wait']['operation_id']
        preview = run('--inspect-operation', operation)
        assert preview['arguments'] == {'value': 42}
        original = config.read_text()
        changed = json.loads(original)
        changed['http_tools'][0]['require_approval'] = False
        config.write_text(json.dumps(changed))
        rejected = subprocess.run(common + ['--config', str(config), '--resume', waiting['id']],
                                  env=env, capture_output=True, text=True, timeout=30)
        assert rejected.returncode != 0 and not executed, 'resume removed approval gate'
        assert run('--inspect-operation', operation)['status'] == 'waiting_approval'
        config.write_text(original)
        run('--approve-operation', operation)
        assert not executed, 'approval command must not execute tools'
        finished = run('--config', str(config), '--resume', waiting['id'])
        assert finished['status'] == 'completed' and executed == [operation]
        inspection = run('--inspect-run', waiting['id'])
        assert inspection['run']['result'] == 'saved'
        print('PASS: PostgreSQL approval wait, exact argument preview, rejection of changed approval policy, approval without execution, resume executes once')
finally:
    server.shutdown()
    server.server_close()
    thread.join()
