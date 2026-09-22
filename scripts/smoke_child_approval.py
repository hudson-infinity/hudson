"""Child approval after host restart must use the child's provider and tool registry."""
import http.server
import json
import os
import pathlib
import re
import subprocess
import tempfile
import threading
import time
import urllib.request
import urllib.error
import uuid

calls = []
writes = []
failures = []


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        try:
            if self.path == '/write':
                assert body == {'value': 42}
                writes.append(self.headers['Idempotency-Key'])
                response = {'saved': True}
            elif self.path == '/parent':
                assert body['model'] == 'parent', 'child routed through parent provider'
                calls.append('parent')
                if body['messages'][-1]['role'] == 'tool':
                    result = json.loads(body['messages'][-1]['content'])['value']
                    assert result['status'] == 'waiting'
                    response = {'choices': [{'finish_reason': 'stop', 'message': {'content': json.dumps({'child_run': result['child_run']})}}]}
                else:
                    response = {'choices': [{'finish_reason': 'tool_calls', 'message': {'tool_calls': [
                        {'id': 'delegate-1', 'type': 'function', 'function': {'name': 'delegate_writer', 'arguments': '{"task":"Save 42"}'}}
                    ]}}]}
            else:
                assert self.path == '/child' and body['model'] == 'child'
                assert 'system' in body and self.headers['anthropic-version'] == '2023-06-01'
                calls.append('child')
                if body['messages'][-1]['content'][0]['type'] == 'tool_result':
                    response = {'stop_reason': 'end_turn', 'content': [{'type': 'text', 'text': '{"saved":true}'}]}
                else:
                    response = {'stop_reason': 'tool_use', 'content': [{'type': 'tool_use', 'id': 'save-1', 'name': 'save', 'input': {'value': 42}}]}
            self.send_response(200)
        except Exception as error:
            failures.append(repr(error))
            self.send_response(500)
            response = {}
        data = json.dumps(response).encode()
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)


stub = http.server.HTTPServer(('127.0.0.1', 0), Handler)
thread = threading.Thread(target=stub.serve_forever, daemon=True)
thread.start()
process = None
try:
    with tempfile.TemporaryDirectory() as directory:
        base = f'http://127.0.0.1:{stub.server_port}'
        config = pathlib.Path(directory) / 'team.json'
        config.write_text(json.dumps({
            'name': 'coordinator', 'instructions': 'Delegate and report the returned child handle.', 'model': 'parent', 'endpoint': base + '/parent',
            'shared_model_budget': {'group': 'team', 'limit': 4},
            'subagents': [{'name': 'writer', 'instructions': 'Save the value.', 'model': 'child', 'provider': 'anthropic', 'endpoint': base + '/child',
                          'output_schema': {'const': {'saved': True}},
                          'http_tools': [{'name': 'save', 'description': 'Save value', 'endpoint': base + '/write', 'input_schema': {'type': 'object'},
                                          'effect': 'write', 'require_approval': True}]}]
        }))
        database = os.environ.get('HUDSON_TEST_DATABASE', 'hudson_harness_test_20260921')
        namespace = 'child-approval-' + str(uuid.uuid4())
        common = ['--config', str(config), '--database', database, '--namespace', namespace]
        env = os.environ.copy()
        env.pop('OPENAI_API_KEY', None)
        env.pop('ANTHROPIC_API_KEY', None)

        def start():
            global process
            process = subprocess.Popen(['target/debug/hudson-server'] + common + ['--port', '0'], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            line = process.stderr.readline()
            match = re.search(r'http://127.0.0.1:\d+', line)
            assert match, line
            return match.group()

        api = start()

        def request(method, path, body=None):
            data = json.dumps(body).encode() if body is not None else None
            req = urllib.request.Request(api + path, data=data, method=method, headers={'Content-Type': 'application/json'})
            with urllib.request.urlopen(req, timeout=3) as response:
                return json.load(response)

        def until(run, status):
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                view = request('GET', '/runs/' + run)
                if view['status'] == status:
                    return view
                time.sleep(.02)
            raise AssertionError(view)

        parent = request('POST', '/runs', {'input': 'Save 42'})['run_id']
        finished = until(parent, 'completed')
        children = request('GET', '/runs/' + parent + '/children')
        assert len(children) == 1 and children[0]['id'] == finished['result']['child_run']
        child = children[0]
        assert child['agent_ref'] == {'id': 'writer', 'version': 1} and child['status'] == 'waiting'
        operation = child['wait']['operation_id']
        assert not writes and calls == ['parent', 'child', 'parent']
        process.terminate()
        process.wait(timeout=5)
        original_config = config.read_text()
        changed = json.loads(original_config)
        changed['shared_model_budget']['group'] = 'replacement-group'
        config.write_text(json.dumps(changed))
        api = start()
        for path, body in [('/runs/' + child['id'] + '/resume', None),
                           ('/operations/' + operation + '/approval', {'approved': True})]:
            try:
                request('POST', path, body)
                raise AssertionError('changed execution budget accepted')
            except urllib.error.HTTPError as error:
                assert error.code == 409
                assert 'budget' in json.load(error)['error']
        assert request('GET', '/runs/' + child['id'])['wait'] == child['wait']
        assert request('GET', '/operations/' + operation)['approval']['decisions'] == []
        assert not writes and len(calls) == 3
        process.terminate()
        process.wait(timeout=5)
        config.write_text(original_config)
        api = start()
        request('POST', '/operations/' + operation + '/approval', {'approved': True})
        assert until(child['id'], 'completed')['result'] == {'saved': True}
        assert writes == [operation] and calls == ['parent', 'child', 'parent', 'child']
        request('POST', '/runs/' + child['id'] + '/resume')
        resumed = subprocess.run(['target/debug/hudson-worker'] + common + ['--resume', child['id']], env=env, capture_output=True, text=True, timeout=15)
        assert resumed.returncode == 0, resumed.stderr
        assert json.loads(resumed.stdout)['agent_ref']['id'] == 'writer'
        assert writes == [operation] and len(calls) == 4 and not failures
        print('PASS: child discovery, own provider/tools after approval and server restart, changed-budget controls rejected before decisions, child API/worker resume, one write and shared four-call budget')
finally:
    if process is not None and process.poll() is None:
        process.terminate()
        process.wait(timeout=5)
    stub.shutdown()
    stub.server_close()
    thread.join()
