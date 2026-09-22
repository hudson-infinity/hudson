"""Offline worker smoke test: parent -> child -> parent with one shared budget.
Build hudson-worker first; run this from the repository root. No credentials used.
"""
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
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        calls.append(request["model"])
        if request["model"] == "parent" and calls.count("parent") == 1:
            delegate = next(tool['function'] for tool in request['tools'] if tool['function']['name'] == 'delegate_specialist')
            assert delegate['parameters']['properties']['task']['type'] == 'object'
            assert delegate['parameters']['properties']['task']['properties']['values']['$ref'] == '#/$defs/numbers'
            message = {"tool_calls": [{"id": "delegate-1", "type": "function", "function": {
                "name": "delegate_specialist", "arguments": '{"task":{"values":[10,20,30]}}'}}]}
            reason = "tool_calls"
        elif request["model"] == "child":
            assert not request.get("tools"), "child unexpectedly inherited parent tools"
            assert json.loads(request["messages"][-1]["content"]) == {"values":[10,20,30]}
            message, reason = {"content": "specialist result"}, "stop"
        elif request["model"] == "parent" and calls.count("parent") == 2:
            result = json.loads(request["messages"][-1]["content"])["value"]
            message = {"tool_calls": [{"id": "join-1", "type": "function", "function": {
                "name": "join_delegate_specialist", "arguments": json.dumps({"run_id": result["child_run"]})}}]}
            reason = "tool_calls"
        else:
            result = json.loads(request["messages"][-1]["content"])["value"]
            assert result["status"] == "completed"
            assert result["result"] == "specialist result"
            assert result["usage"]["reported_model_calls"] == 1
            assert result["usage"]["reported_input_tokens"] == 10
            message, reason = {"content": "parent finished"}, "stop"
        response = json.dumps({"choices": [{"finish_reason": reason, "message": message}], "usage": {"prompt_tokens":10,"completion_tokens":2}}).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)


server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    endpoint = f"http://127.0.0.1:{server.server_port}/v1/chat/completions"
    definition = {
        "name": "coordinator", "instructions": "Delegate to the specialist", "model": "parent",
        "input_schema": {"type":"object","required":["values"],"properties":{"values":{"type":"array","items":{"type":"number"}}}},
        "endpoint": endpoint, "shared_model_budget": {"group": "smoke", "limit": 4},
        "subagents": [{"name": "specialist", "instructions": "Analyze the task", "model": "child", "endpoint": endpoint,
                       "input_schema": {"type":"object","required":["values"],"additionalProperties":False,
                                        "$defs":{"numbers":{"type":"array","items":{"type":"number"}}},
                                        "properties":{"values":{"$ref":"#/$defs/numbers"}}}}],
    }
    with tempfile.TemporaryDirectory() as directory:
        path = pathlib.Path(directory) / "agent.json"
        path.write_text(json.dumps(definition))
        env = os.environ.copy()
        env.pop("OPENAI_API_KEY", None)
        result = subprocess.run(
            ["target/debug/hudson-worker", "--config", str(path), "--input-file", "-"],
            input=json.dumps({"values":[10,20,30]}), env=env, capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, result.stderr
        view = json.loads(result.stdout)
        assert view["status"] == "completed", view
        assert view["result"] == "parent finished", view
        assert view["usage"]["reported_model_calls"] == 3
        assert view["usage"]["reported_input_tokens"] == 30
        assert view["usage"]["reported_output_tokens"] == 6
        assert calls == ["parent", "child", "parent", "parent"], calls
        print("PASS: configured delegation, isolated child tools, join without child replay, shared four-call budget, parent completion")
finally:
    server.shutdown()
    server.server_close()
    thread.join()
