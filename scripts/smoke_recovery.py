"""Kill a worker after a tool commits, then recover its receipt without replay.

Only loopback stubs are called. Requires a dedicated local PostgreSQL test DB.
"""
import http.server
import json
import os
import pathlib
import re
import subprocess
import tempfile
import threading
import urllib.request
import uuid

committed = threading.Event()
release = threading.Event()
model_started = threading.Event()
model_release = threading.Event()
receipts = {}
writes = []
model_calls = []


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def respond(self, value):
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        try:
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass  # The crash deliberately severs the pending tool response.

    def do_GET(self):
        operation = self.path.removeprefix('/receipts/')
        self.respond(receipts[operation])

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        if self.path == '/tool':
            operation = self.headers['Idempotency-Key']
            writes.append(operation)
            receipts[operation] = {'receipt': 'committed:' + operation, 'result': {'saved': True, 'value': request['value']}}
            committed.set()
            assert release.wait(20), 'test did not release committed tool'
            self.respond(receipts[operation]['result'])
        else:
            model_calls.append(request)
            if 'Interrupt model' in str(request['messages'][-1]['content']):
                model_started.set()
                assert model_release.wait(20), 'test did not release model'
                self.respond({'choices': [{'finish_reason': 'stop', 'message': {'content': 'unavailable'}}]})
                return
            if request['messages'][-1]['role'] == 'tool':
                self.respond({'choices': [{'finish_reason': 'stop', 'message': {'content': 'saved'}}]})
            else:
                self.respond({'choices': [{'finish_reason': 'tool_calls', 'message': {'tool_calls': [
                    {'id': 'write-1', 'type': 'function', 'function': {'name': 'save', 'arguments': '{"value":42}'}}
                ]}}]})


stub = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
thread = threading.Thread(target=stub.serve_forever, daemon=True)
thread.start()
worker = None
try:
    with tempfile.TemporaryDirectory() as directory:
        base = f'http://127.0.0.1:{stub.server_port}'
        config = pathlib.Path(directory) / 'agent.json'
        config.write_text(json.dumps({
            'name': 'writer', 'instructions': 'Save the value', 'endpoint': base + '/model',
            'http_tools': [{'name': 'save', 'description': 'Save a value', 'endpoint': base + '/tool',
                            'input_schema': {'type': 'object'}, 'effect': 'write',
                            'output_schema': {'type': 'object', 'required': ['saved', 'value'],
                                              'properties': {'saved': {'const': True}, 'value': {'const': 42}}}}]
        }))
        common = ['target/debug/hudson-worker', '--database',
                  os.environ.get('HUDSON_TEST_DATABASE', 'hudson_harness_test_20260921'),
                  '--namespace', 'recovery-' + str(uuid.uuid4())]
        env = os.environ.copy()
        env.pop('OPENAI_API_KEY', None)

        def command(*arguments, success=True):
            result = subprocess.run(common + list(arguments), env=env, capture_output=True, text=True, timeout=15)
            if not success:
                assert result.returncode != 0, 'invalid recovery was accepted'
                return
            assert result.returncode == 0, result.stderr
            return json.loads(result.stdout)

        worker = subprocess.Popen(common + ['--config', str(config), '--task', 'Save 42'],
                                  env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        assert committed.wait(10), 'write did not reach destination'
        worker.kill()
        _, stderr = worker.communicate(timeout=5)
        assert worker.returncode < 0, 'worker was not killed'
        run_id = re.search(r'Run: ([0-9a-f-]+)', stderr).group(1)
        release.set()
        operation = writes[0]
        observed = command('--inspect-operation', operation)
        assert observed['status'] == 'running' and len(observed['attempts']) == 1
        attempt = observed['attempts'][0]['id']
        # Restart cannot infer that Running means safe to repeat.
        command('--config', str(config), '--resume', run_id)
        assert writes == [operation] and len(model_calls) == 1
        command('--mark-interrupted', operation, '--attempt-id', str(uuid.uuid4()),
                '--evidence', 'worker SIGKILL confirmed', success=False)
        marked = command('--mark-interrupted', operation, '--attempt-id', attempt,
                         '--evidence', f'worker PID {worker.pid} reaped after SIGKILL')
        assert marked['status'] == 'unknown'
        waiting = command('--config', str(config), '--resume', run_id)
        assert waiting['status'] == 'waiting' and waiting['wait']['type'] == 'reconciliation'
        assert writes == [operation]

        # The operator reads evidence from the destination rather than inventing a result.
        with urllib.request.urlopen(base + '/receipts/' + operation, timeout=3) as response:
            receipt = json.load(response)
        result_file = pathlib.Path(directory) / 'receipt-result.json'
        result_file.write_text('{"saved":false}')
        command('--reconcile-operation', operation, '--result-file', str(result_file),
                '--receipt', receipt['receipt'], success=False)
        assert command('--inspect-operation', operation)['status'] == 'unknown'
        result_file.write_text(json.dumps(receipt['result']))
        reconciled = command('--reconcile-operation', operation, '--result-file', str(result_file),
                             '--receipt', receipt['receipt'])
        assert reconciled['status'] == 'succeeded'
        assert writes == [operation] and len(model_calls) == 1, 'management performed execution'
        final = command('--config', str(config), '--resume', run_id)
        assert final['status'] == 'completed' and final['result'] == 'saved'
        assert writes == [operation] and len(model_calls) == 2
        inspected = command('--inspect-run', run_id)
        assert receipt['receipt'] in json.dumps(inspected['events'])
        print('PASS: SIGKILL after destination commit, no replay on restart, exact attempt fencing, verified receipt/schema validation, resumed completion, one write total')

        worker = subprocess.Popen(common + ['--config', str(config), '--task', 'Interrupt model'],
                                  env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        assert model_started.wait(10)
        worker.kill()
        _, stderr = worker.communicate(timeout=5)
        model_run = re.search(r'Run: ([0-9a-f-]+)', stderr).group(1)
        model_release.set()
        state = command('--inspect-run', model_run)
        model_op = state['operations'][0]['id']
        assert state['run']['usage']['model_calls'] == 1
        observed = command('--inspect-operation', model_op)
        attempt = observed['attempts'][0]['id']
        command('--abandon-model', model_op, '--evidence', 'not yet fenced', success=False)
        command('--mark-interrupted', model_op, '--attempt-id', attempt,
                '--evidence', f'worker PID {worker.pid} reaped after SIGKILL')
        abandoned = command('--abandon-model', model_op, '--evidence', 'model response unavailable after worker exit')
        assert abandoned['status'] == 'failed'
        command('--config', str(config), '--resume', model_run, success=False)
        failed = command('--inspect-run', model_run)
        assert failed['run']['status'] == 'failed'
        assert failed['run']['usage']['model_calls'] == 1
        assert command('--inspect-operation', model_op)['attempts'][0]['status'] == 'unknown'
        assert len(model_calls) == 3 and writes == [operation]
        print('PASS: interrupted model response explicitly abandoned, no retry, run fails, uncertain attempt and admission charge preserved')
finally:
    release.set()
    model_release.set()
    if worker is not None and worker.poll() is None:
        worker.kill()
        worker.wait(timeout=5)
    stub.shutdown()
    stub.server_close()
    thread.join()
