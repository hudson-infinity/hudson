# Hudson × Harbor

This is a real `harbor.agents.base.BaseAgent` subclass. It implements Harbor's `setup(environment)` and `run(instruction, environment, AgentContext)` contract. It starts the normal Rust `hudson-worker` and registers `harbor_exec` as a policy-controlled HTTP tool. Commands execute **only** through Harbor's supplied `environment.exec`; Hudson does not implement an environment backend or execute task shell commands on the host.

The adapter uses a random bearer token, loopback listener, operation-ID receipts, command/response bounds, fixed model/tool budgets, subprocess timeout, and cleanup. An interrupted environment execution remains uncertain and cannot be retried under the same identity. The adapter does not claim resume or trajectory capabilities. Harbor owns environment isolation and task verification.

## Install and run

From the Hudson checkout, build the worker and install the integration in an isolated environment:

```sh
cargo build -p hudson-worker
uv venv .venv-harbor --python 3.13
uv pip install --python .venv-harbor/bin/python ./integrations/harbor
```

The package pins Harbor's official source at `15da91c18580a25489f5bdf2ee71029f3ff3bb2e` (0.23.0), whose actual BaseAgent and Task models are used in tests. Python 3.13 is recommended for the pinned Harbor revision.

Create an ordinary Hudson agent config with provider, model, endpoint/key environment variable, and instructions. The adapter adds its environment tool and fixes budgets. Provider credentials stay in the worker environment; the bridge token is never written into the config. Existing explicitly configured HTTP tools remain available. Avoid unrelated external tools in reproducible benchmark comparisons.

```sh
.venv-harbor/bin/harbor run \
  --path integrations/harbor/tasks \
  --agent hudson_harbor.agent:HudsonAgent \
  --agent-kwarg config_path=/absolute/path/agent.json \
  --agent-kwarg worker_path=/absolute/path/target/debug/hudson-worker \
  --agent-kwarg max_model_calls=16 \
  --agent-kwarg max_operations=64
```

Harbor requires an available execution backend, such as its Docker backend. A real provider run consumes that provider's tokens. Do not pass `--model` unless intentionally overriding the exact model identifier in the Hudson config. The first adapter supports a single Hudson agent; team configurations and Harbor task-supplied MCP/skills are rejected rather than silently ignored. Hudson-configured capabilities can be benchmarked where supported by the worker.

## Tasks and verification

Three runnable Harbor tasks are included:

- Coding: merge integer intervals without mutation; verifier includes 100 independently generated cases.
- Data: calculate regional sales with cancellation/deduplication rules and exact cents.
- Research: resolve conflicting dated sources and return supporting citations.

Task environment images copy only input files. Harbor supplies verifier files separately after agent execution. Verifier reward is independent of the agent's claimed success. Reference solutions are for Harbor's oracle mode, not agent input. These are small regression cases, not evidence of broad superiority or production task quality.

## Compare results

Run baseline and candidate jobs on the same task set with identical budgets and repetitions, then:

```sh
.venv-harbor/bin/python -m hudson_harbor.report jobs/baseline jobs/candidate
```

The report uses Harbor verifier rewards for correctness and Hudson's reported usage and elapsed time. Missing rewards remain explicit. Dollar cost remains unknown; the adapter does not invent pricing. Context, run output, and worker stderr are saved under Harbor's agent logs. Be mindful that these benchmark logs contain task data and model output.

## Local validation without paid calls

```sh
PYTHONPATH=integrations/harbor HUDSON_WORKER="$PWD/target/debug/hudson-worker" \
  .venv-harbor/bin/python -m unittest discover -s integrations/harbor/tests -v
```

The integration test uses the real installed Harbor classes and Rust worker, a fixture model HTTP server, and an autospecced Harbor environment. It verifies the full model → HTTP tool → async environment → tool result → model cycle and reported usage. Separate tests parse every task with Harbor and prove each verifier accepts correct artifacts and rejects incorrect ones. This validation uses no paid model, performs no live Harbor environment deployment, and is not a benchmark score.

Primary contract references: [BaseAgent](https://github.com/harbor-framework/harbor/blob/15da91c18580a25489f5bdf2ee71029f3ff3bb2e/src/harbor/agents/base.py), [BaseEnvironment](https://github.com/harbor-framework/harbor/blob/15da91c18580a25489f5bdf2ee71029f3ff3bb2e/src/harbor/environments/base.py), [AgentContext](https://github.com/harbor-framework/harbor/blob/15da91c18580a25489f5bdf2ee71029f3ff3bb2e/src/harbor/models/agent/context.py).

A full Docker plumbing smoke is also available:

```sh
.venv-harbor/bin/python integrations/harbor/scripts/smoke.py \
  --worker "$PWD/target/debug/hudson-worker" --jobs-dir /tmp/hudson-harbor-smoke
```

This script deliberately returns reference-solution commands from a fixture model. It exercises actual Harbor Docker environments, the Rust loop, bridge, and independent verifier. It must never be reported as model benchmark performance. Validation on 2026-09-23 completed all three Docker trials with verifier reward 1 and no exceptions. Fixture usage counters are synthetic; no paid provider was called.

For estimated dollar costs, pass `--pricing rates.json` to the comparison reporter.
Supply `baseline` and `candidate` objects, each containing `input_per_million` and
`output_per_million` USD rates for that run's model. Costs remain null when any
model call lacks reported usage. These are estimates from supplied rates, not
billing receipts; omitted pricing never becomes a zero-cost claim.
