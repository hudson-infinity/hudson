"""Run Hudson's agent loop against a Harbor-owned execution environment."""
import asyncio
import copy
import json
import os
from pathlib import Path
import secrets
import time

from aiohttp import web
from harbor.agents.base import BaseAgent
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext


class HudsonAgent(BaseAgent):
    @staticmethod
    def name() -> str:
        return "hudson"

    def version(self) -> str:
        return "0.1.0"

    def __init__(self, logs_dir: Path, config_path: str, worker_path: str,
                 max_model_calls: int = 16, max_operations: int = 64,
                 run_timeout_sec: int = 300, max_output_tokens: int = 1024, **kwargs):
        super().__init__(logs_dir=logs_dir, **kwargs)
        self.config_path = Path(config_path).resolve(strict=True)
        self.worker_path = Path(worker_path).resolve(strict=True)
        if not os.access(self.worker_path, os.X_OK):
            raise ValueError("worker_path must be an executable Hudson worker")
        if not 1 <= int(max_model_calls) <= 100 or not 1 <= int(max_operations) <= 500:
            raise ValueError("benchmark budgets out of range")
        if not 1 <= int(run_timeout_sec) <= 3600:
            raise ValueError("run timeout out of range")
        self.max_model_calls = int(max_model_calls)
        self.max_operations = int(max_operations)
        self.run_timeout_sec = int(run_timeout_sec)
        if not 1 <= int(max_output_tokens) <= 32768:
            raise ValueError("output token budget out of range")
        self.max_output_tokens = int(max_output_tokens)
        self._config = json.loads(self.config_path.read_text())
        # Task-specific servers/skills must never be silently dropped.
        if self.mcp_servers or self.skills_dir:
            raise ValueError("Harbor task MCP servers/skills need explicit Hudson config mapping")
        if self._config.get("subagents") or self._config.get("team"):
            raise ValueError("this benchmark adapter accepts a single Hudson agent")
        self.logs_dir.mkdir(parents=True, exist_ok=True)

    async def setup(self, environment: BaseEnvironment) -> None:
        # Harbor owns environment creation and lifetime. No host shell fallback.
        if not callable(getattr(environment, "exec", None)):
            raise TypeError("Harbor environment must implement async exec")

    async def run(self, instruction: str, environment: BaseEnvironment,
                  context: AgentContext) -> None:
        token = secrets.token_urlsafe(32)
        receipts = {}
        lock = asyncio.Lock()
        executions = 0

        async def execute(request):
            nonlocal executions
            if request.headers.get("Authorization") != f"Bearer {token}":
                raise web.HTTPUnauthorized()
            operation = request.headers.get("Idempotency-Key")
            if not operation or len(operation) > 128:
                raise web.HTTPBadRequest(text="operation identity required")
            body = await request.json()
            if set(body) - {"command", "timeout_sec"} or not isinstance(body.get("command"), str) or not 1 <= len(body["command"]) <= 16384:
                raise web.HTTPBadRequest(text="bounded command required")
            timeout = body.get("timeout_sec", 30)
            if type(timeout) is not int or not 1 <= timeout <= 45:
                raise web.HTTPBadRequest(text="timeout must be 1 to 45 seconds")
            async with lock:
                if operation in receipts:
                    old_body, receipt = receipts[operation]
                    if old_body != body:
                        raise web.HTTPConflict(text="operation identity reused")
                    if receipt is None:
                        raise web.HTTPConflict(text="previous execution outcome unknown")
                    return web.json_response(receipt)
                if executions >= self.max_operations:
                    raise web.HTTPTooManyRequests(text="tool budget exhausted")
                executions += 1
                # The environment's configured user applies; never execute on the host.
                receipts[operation] = (body, None)
                result = await environment.exec(command=body["command"], timeout_sec=timeout)
                receipt = {"stdout": (result.stdout or "")[:16384],
                           "stderr": (result.stderr or "")[:8192],
                           "exit_code": result.return_code,
                           "output_truncated": len(result.stdout or "") > 16384 or len(result.stderr or "") > 8192}
                receipts[operation] = (body, receipt)
                return web.json_response(receipt)

        app = web.Application(client_max_size=32768)
        app.router.add_post("/execute", execute)
        runner = web.AppRunner(app, access_log=None)
        await runner.setup()
        site = web.TCPSite(runner, "127.0.0.1", 0)
        await site.start()
        port = runner.addresses[0][1]
        config = copy.deepcopy(self._config)
        config["name"] = "harbor-trial"
        if self.model_name:
            config["model"] = self.model_name
        config["instructions"] = config.get("instructions", "") + (
            "\nComplete the task inside the provided Harbor environment using harbor_exec. "
            "Write requested deliverables in that environment. Do not access /tests, /solution, "
            "or verifier artifacts. Report what you changed; do not claim success without evidence.")
        for skill in config.get("skill_files", []):
            skill["path"] = str((self.config_path.parent / skill["path"]).resolve())
        config["skill_packages"] = [str((self.config_path.parent / path).resolve())
                                    for path in config.get("skill_packages", [])]
        # Enforce identical budgets across compared configurations.
        config["max_output_tokens"] = self.max_output_tokens
        config["limits"] = {"max_model_calls": self.max_model_calls,
                            "max_operations": self.max_operations,
                            "max_harness_steps": self.max_operations * 8,
                            "max_batch_size": 4, "max_context_bytes": 65536,
                            "max_payload_bytes": 65536}
        config.setdefault("http_tools", []).append({
            "name": "harbor_exec", "description": "Execute a command inside the Harbor task environment.",
            "endpoint": f"http://127.0.0.1:{port}/execute", "token_env": "HUDSON_HARBOR_TOKEN",
            "effect": "write", "require_approval": False,
            "input_schema": {"type": "object", "properties": {
                "command": {"type": "string", "minLength": 1, "maxLength": 16384},
                "timeout_sec": {"type": "integer", "minimum": 1, "maximum": 45}},
                "required": ["command"], "additionalProperties": False}})
        config_file = self.logs_dir / "hudson-config.json"
        input_file = self.logs_dir / "hudson-input.json"
        config_file.write_text(json.dumps(config))
        input_file.write_text(json.dumps(instruction))
        started = time.monotonic()
        process = None
        context.metadata = {"hudson": {"budgets": {**config["limits"], "max_output_tokens": self.max_output_tokens, "run_timeout_sec": self.run_timeout_sec}, "status": "running"}}
        try:
            process = await asyncio.create_subprocess_exec(
                str(self.worker_path), "--config", str(config_file), "--input-file", str(input_file),
                stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE,
                env={**os.environ, **self.extra_env, "HUDSON_HARBOR_TOKEN": token})
            stdout, stderr = await asyncio.wait_for(process.communicate(), self.run_timeout_sec)
            (self.logs_dir / "hudson-stderr.txt").write_bytes(stderr)
            view = json.loads(stdout)
            (self.logs_dir / "hudson-run.json").write_text(json.dumps(view, indent=2))
            usage = view.get("usage", {})
            if usage.get("reported_model_calls", 0) > 0:
                context.n_input_tokens = usage["reported_input_tokens"]
                context.n_output_tokens = usage["reported_output_tokens"]
            context.metadata["hudson"].update({"status": view["status"], "run_id": view["id"],
                                               "usage": usage, "tool_executions": executions})
            if process.returncode or view["status"] != "completed":
                raise RuntimeError(f"Hudson did not complete: {view['status']}")
        except BaseException:
            context.metadata["hudson"]["status"] = "failed_or_interrupted"
            raise
        finally:
            if process is not None and process.returncode is None:
                process.kill()
                await process.wait()
            context.metadata["hudson"]["elapsed_ms"] = round((time.monotonic() - started) * 1000)
            (self.logs_dir / "hudson-context.json").write_text(context.model_dump_json(indent=2))
            await runner.cleanup()
