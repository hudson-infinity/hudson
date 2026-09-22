"""Compare two immutable agent versions through the real loop with a local model."""
import http.server
import json
import os
import pathlib
import subprocess
import tempfile
import threading

calls = []


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        assert 'EVALUATOR_ONLY' not in json.dumps(request), 'expected answers leaked into model context'
        calls.append(request)
        fixed = 'corrected' in request['messages'][0]['content']
        response = {'choices': [{'finish_reason': 'stop', 'message': {'content': 'ok' if fixed else 'wrong'}}]}
        body = json.dumps(response).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


stub = http.server.HTTPServer(('127.0.0.1', 0), Handler)
thread = threading.Thread(target=stub.serve_forever, daemon=True)
thread.start()
try:
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        agent = {'name': 'labeler', 'instructions': 'Return a label', 'version': 1,
                 'endpoint': f'http://127.0.0.1:{stub.server_port}/model', 'limits': {'max_model_calls': 1}}
        baseline = root / 'baseline.json'
        baseline.write_text(json.dumps(agent))
        agent.update(version=2, instructions='Return a corrected label')
        candidate = root / 'candidate.json'
        candidate.write_text(json.dumps(agent))
        suite = root / 'cases.json'
        suite.write_text(json.dumps([{'name': 'label-regression', 'input': 'Label this example',
                                     'expected_schema': {'const': 'ok', 'description': 'EVALUATOR_ONLY'}}]))
        env = os.environ.copy()
        env.pop('OPENAI_API_KEY', None)
        command = ['target/debug/hudson-worker', '--config', str(baseline), '--evaluate', str(suite)]
        bad = subprocess.run(command, env=env, capture_output=True, text=True, timeout=15)
        assert bad.returncode != 0 and json.loads(bad.stdout)['passed'] is False, bad.stderr
        comparison = subprocess.run(command + ['--compare-config', str(candidate)], env=env,
                                    capture_output=True, text=True, timeout=15)
        assert comparison.returncode == 0, comparison.stderr
        report = json.loads(comparison.stdout)
        assert report['passed'] and len(report['reports']) == 2
        before, after = report['reports']
        assert before['suite_digest'] == after['suite_digest']
        assert before['agent']['version'] == 1 and not before['passed']
        assert after['agent']['version'] == 2 and after['passed']
        assert before['cases'][0]['run']['status'] == after['cases'][0]['run']['status'] == 'completed'
        assert before['cases'][0]['run']['result'] == 'wrong'
        assert after['cases'][0]['run']['result'] == 'ok'
        assert before['cases'][0]['run']['id'] != after['cases'][0]['run']['id']
        assert after['cases'][0]['run']['usage']['model_calls'] == 1
        assert len(calls) == 3
        suite.write_text('[{"name":"invalid","input":"task","expected_schema":{"type":"invalid"}}]')
        invalid = subprocess.run(command, env=env, capture_output=True, text=True, timeout=15)
        assert invalid.returncode != 0 and len(calls) == 3
        print('PASS: failing baseline, passing candidate, distinct versioned runs, original verification/usage, private expected schema, malformed suite rejected before IO')
finally:
    stub.shutdown()
    stub.server_close()
    thread.join()
