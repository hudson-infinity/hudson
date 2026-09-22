"""Opt-in paid provider smoke. Credentials come only from the process environment.

At most two model calls, each capped at 256 output tokens. Only synthetic arithmetic
is sent. No automatic retries. Build hudson-worker before invoking this script.
"""
import argparse
import http.server
import json
import os
import pathlib
import subprocess
import tempfile
import threading
import uuid

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--provider', required=True, choices=['openai', 'anthropic', 'gemini', 'ollama'])
parser.add_argument('--model', required=True)
parser.add_argument('--database', help='Optional dedicated local PostgreSQL database to retain evidence')
args = parser.parse_args()
executed = []


class Sum(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        values = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        assert values == {'left': 17, 'right': 25}
        executed.append(self.headers['Idempotency-Key'])
        body = json.dumps({'answer': values['left'] + values['right']}).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


stub = http.server.HTTPServer(('127.0.0.1', 0), Sum)
thread = threading.Thread(target=stub.serve_forever, daemon=True)
thread.start()
try:
    with tempfile.TemporaryDirectory() as directory:
        config = pathlib.Path(directory) / 'agent.json'
        config.write_text(json.dumps({
            'name': 'provider-check', 'provider': args.provider, 'model': args.model,
            'instructions': 'Call sum exactly once for the requested arithmetic. Then return only a bare JSON object with the integer answer from the tool, without Markdown. Never answer before calling the tool.',
            'max_output_tokens': 256, 'limits': {'max_model_calls': 2, 'max_operations': 6},
            'output_schema': {'const': {'answer': 42}},
            'http_tools': [{'name': 'sum', 'description': 'Add two integers.',
                            'endpoint': f'http://127.0.0.1:{stub.server_port}/sum', 'effect': 'read',
                            'input_schema': {'type': 'object', 'properties': {'left': {'type': 'integer'}, 'right': {'type': 'integer'}}, 'required': ['left', 'right']},
                            'output_schema': {'const': {'answer': 42}}}]
        }))
        common = ['target/debug/hudson-worker']
        namespace = 'live-' + str(uuid.uuid4())
        if args.database:
            common += ['--database', args.database, '--namespace', namespace]
        process = subprocess.run(common + ['--config', str(config), '--task', 'Use sum to add 17 and 25.'],
                                 env=os.environ.copy(), capture_output=True, text=True, timeout=300)
        if not process.stdout.strip():
            raise RuntimeError(process.stderr.strip())
        view = json.loads(process.stdout)
        print(json.dumps({'provider': args.provider, 'model': args.model, 'run_id': view['id'],
                          'namespace': namespace if args.database else None,
                          'status': view['status'], 'result': view['result'], 'usage': view['usage'],
                          'tool_executions': len(executed), 'assessment': view['assessment']}))
        if view['status'] != 'completed' and args.database:
            inspected = json.loads(subprocess.run(common + ['--inspect-run', view['id']], capture_output=True, text=True, check=True).stdout)
            for operation in inspected['operations']:
                details = json.loads(subprocess.run(common + ['--inspect-operation', operation['id']], capture_output=True, text=True, check=True).stdout)
                print(json.dumps({'kind': operation['kind'], 'attempts': details['attempts']}))
        assert process.returncode == 0 and view['status'] == 'completed'
        assert view['result'] == {'answer': 42} and len(executed) == 1
        assert view['usage']['model_calls'] <= 2 and view['assessment']['passed']
finally:
    stub.shutdown()
    stub.server_close()
    thread.join()
