"""Full Harbor Docker plumbing smoke with scripted reference outputs, not an agent benchmark."""
import argparse
import asyncio
import json
import os
from pathlib import Path
import sys
import tempfile

from aiohttp import web


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--worker", required=True)
    parser.add_argument("--jobs-dir", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    async def model(request):
        body = await request.json()
        if any(m.get("role") == "tool" for m in body["messages"]):
            message = {"role": "assistant", "content": "Reference fixture deliverable written."}
            reason = "stop"
        else:
            text = json.dumps(body["messages"])
            task = "coding-intervals" if "intervals.py" in text else "data-sales" if "sales.csv" in text else "research-evidence"
            command = (root / "tasks" / task / "solution/solve.sh").read_text()
            message = {"role": "assistant", "content": None, "tool_calls": [{"id": "fixture-exec", "type": "function", "function": {"name": "harbor_exec", "arguments": json.dumps({"command": command, "timeout_sec": 30})}}]}
            reason = "tool_calls"
        return web.json_response({"choices": [{"message": message, "finish_reason": reason}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
    app = web.Application()
    app.router.add_post("/chat/completions", model)
    runner = web.AppRunner(app)
    await runner.setup()
    await web.TCPSite(runner, "127.0.0.1", 0).start()
    try:
        with tempfile.TemporaryDirectory() as temporary:
            config = Path(temporary) / "fixture.json"
            config.write_text(json.dumps({"name": "harbor-fixture", "provider": "openai", "model": "scripted-reference-fixture", "instructions": "Execute reference fixture commands.", "endpoint": f"http://127.0.0.1:{runner.addresses[0][1]}/chat/completions"}))
            existing = set(Path(args.jobs_dir).glob("*/result.json"))
            process = await asyncio.create_subprocess_exec(
                str(Path(sys.executable).parent / "harbor"), "run", "--path", str(root / "tasks"),
                "--agent", "hudson_harbor.agent:HudsonAgent", "--agent-kwarg", f"config_path={config}",
                "--agent-kwarg", f"worker_path={Path(args.worker).resolve()}",
                "--jobs-dir", args.jobs_dir,
                env={**os.environ, "PYTHONPATH": str(root)})
            code = await process.wait()
            if code:
                raise SystemExit(code)
            created = set(Path(args.jobs_dir).glob("*/result.json")) - existing
            if len(created) != 1:
                raise RuntimeError("expected one new Harbor job result")
            trials = list(next(iter(created)).parent.glob("*/result.json"))
            if len(trials) != 3 or any(json.loads(p.read_text()).get("verifier_result", {}).get("rewards", {}).get("reward") != 1 for p in trials):
                raise RuntimeError("reference smoke did not pass all three independent verifiers")
    finally:
        await runner.cleanup()


if __name__ == "__main__":
    asyncio.run(main())
