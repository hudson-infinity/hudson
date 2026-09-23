import asyncio
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import create_autospec

from aiohttp import web
from harbor.agents.base import BaseAgent
from harbor.environments.base import BaseEnvironment, ExecResult
from harbor.models.agent.context import AgentContext
from harbor.models.task.task import Task
from hudson_harbor.agent import HudsonAgent
from hudson_harbor.report import compare


class AdapterTest(unittest.IsolatedAsyncioTestCase):
    async def test_real_worker_model_and_environment_tool_cycle(self):
        worker = os.environ.get("HUDSON_WORKER")
        if not worker:
            self.skipTest("set HUDSON_WORKER to built worker executable")
        calls = []
        async def model(request):
            body = await request.json()
            calls.append(body)
            if len(calls) == 1:
                message = {"role": "assistant", "content": None, "tool_calls": [{"id": "exec1", "type": "function", "function": {"name": "harbor_exec", "arguments": json.dumps({"command": "printf fixture", "timeout_sec": 5})}}]}
            else:
                self.assertTrue(any(m.get("role") == "tool" and "fixture output" in m.get("content", "") for m in body["messages"]))
                message = {"role": "assistant", "content": "Task finished with environment evidence."}
            return web.json_response({"choices": [{"message": message, "finish_reason": "tool_calls" if len(calls) == 1 else "stop"}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}})
        app = web.Application()
        app.router.add_post("/chat/completions", model)
        runner = web.AppRunner(app)
        await runner.setup()
        await web.TCPSite(runner, "127.0.0.1", 0).start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                directory = Path(directory)
                package = directory / "analysis"
                package.mkdir()
                (package / "SKILL.md").write_text("---\nname: analysis\ndescription: Analyze evidence\n---\nCheck the evidence before finishing.")
                config = directory / "agent.json"
                config.write_text(json.dumps({"name": "fixture", "model": "fixture", "provider": "openai", "skill_packages": ["analysis"], "instructions": "Complete the task.", "endpoint": f"http://127.0.0.1:{runner.addresses[0][1]}/chat/completions"}))
                agent = HudsonAgent(logs_dir=directory / "logs", config_path=str(config), worker_path=worker)
                self.assertIsInstance(agent, BaseAgent)
                environment = create_autospec(BaseEnvironment, instance=True)
                environment.exec.return_value = ExecResult(stdout="fixture output", stderr="", return_code=0)
                context = AgentContext()
                await agent.setup(environment)
                await agent.run("Use the environment tool once.", environment, context)
                environment.exec.assert_awaited_once_with(command="printf fixture", timeout_sec=5)
                self.assertEqual(context.n_input_tokens, 20)
                self.assertEqual(context.n_output_tokens, 10)
                self.assertIsNone(context.cost_usd)
                self.assertEqual(context.metadata["hudson"]["status"], "completed")
                self.assertEqual(context.metadata["hudson"]["tool_executions"], 1)
                self.assertTrue((directory / "logs/hudson-run.json").exists())
                rewritten = json.loads((directory / "logs/hudson-config.json").read_text())
                self.assertEqual(rewritten["skill_packages"], [str(package.resolve())])
        finally:
            await runner.cleanup()

    def test_tasks_parse_with_actual_harbor(self):
        root = Path(__file__).parents[1] / "tasks"
        for path in root.iterdir():
            task = Task(path)
            self.assertTrue(task.instruction)
            self.assertEqual(task.config.agent.timeout_sec, 300)

    def test_comparison_rejects_unequal_budgets(self):
        trial = {"reward": 1, "status": "completed", "elapsed_ms": 10, "usage": {"model_calls": 2}, "budgets": {"max_model_calls": 16}}
        self.assertEqual(compare({"task": [trial]}, {"task": [trial]})["tasks"][0]["candidate"]["verified_successes"], 1)
        other = {**trial, "budgets": {"max_model_calls": 32}}
        with self.assertRaises(ValueError):
            compare({"task": [trial]}, {"task": [other]})
