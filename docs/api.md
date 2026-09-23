# Local HTTP API

The machine-readable contract is [openapi.json](openapi.json). Each server also
serves it at `GET /openapi.json`, without accessing PostgreSQL or model providers.
Import that document into an OpenAPI-capable client to inspect request and
response schemas. The API is currently a preview with unversioned paths.

Start with the [README server command](../README.md#use-the-http-api). This host
binds to loopback and uses one local identity. Agent definitions and credentials
are loaded from the startup configuration, not submitted over this API.

| Request | Purpose |
| --- | --- |
| `GET /health` | Read configured/fixture mode and persistence status |
| `POST /runs` | Submit JSON input to the root agent; optionally deduplicate with `request_key` |
| `GET /runs/{id}` | Read status, question/approval wait, usage and result |
| `GET /runs/{id}/children` | Discover direct children and their statuses |
| `GET /runs/{id}/events?after=0` | Poll events after an exclusive sequence cursor |
| `POST /runs/{id}/resume` | Schedule continuation with the configured executor |
| `POST /runs/{id}/input` | Record an answer for `question_id` and schedule continuation |
| `POST /runs/{id}/cancel` | Request cancellation of the run and known children |
| `GET /operations/{id}` | Inspect exact arguments, status and approval details |
| `POST /operations/{id}/approval` | Submit `{"approved":true}` or `{"approved":false}` |

Submission returns `202` with a `run_id`; it does not promise completion. Poll the
run until it waits or reaches a terminal status. Approval, reply, and resume
schedule work and return the run handle. Inspect the saved outcome afterward.
Resume, reply, and approval reject a changed saved budget binding with `409`
before accepting the control action. Restore the original budget group to continue
a nonterminal run; starting a new batch requires a new run.

After restarting the server, use the same configuration/database/namespace and
explicitly resume interrupted runs. Unknown external effects need the separate
[recovery workflow](recovery.md).

The body limit is 64 KiB. Runtime errors use `{"error":"..."}`; malformed JSON,
UUIDs, content types, and oversized bodies can receive plain-text framework
errors. Handle status before decoding the body. Event payloads and run results
are arbitrary JSON. Responses expose no executable checkpoint or credentials.

A reply must include `input`, `request_key`, and the `question_id` copied from
`run.wait.question_id`. Reuse all three when retrying the same answer. A changed
answer under the same key or an answer for the wrong pending question conflicts.

Provider usage may be absent. A zero `reported_model_calls` count means no usable
provider reports were recorded, not that the task cost nothing. Parent usage
covers the parent only; child usage belongs to each child run.

Server tests compile the schemas and validate HTTP responses against this
contract. The standard `scripts/check.py` command includes those tests.

## Separate API admission from Temporal execution

Start the API with durable storage and a task queue:

```sh
cargo run --locked -p hudson-server -- --config examples/analyst.json \
  --database hudson --namespace my-project --temporal-task-queue my-agents
```

Run the matching worker in another process:

```sh
cargo run --locked -p hudson-temporal -- --config examples/analyst.json \
  --database hudson --namespace my-project --task-queue my-agents worker
```

The existing `POST /runs` payload and run inspection/control routes remain the
same. The API commits the run and scheduling intent together, then returns `202`.
Acceptance means the request is saved, not that a worker has started or completed
it. The API makes no model or tool calls in this mode; it does not need a Temporal
connection or the configured model/HTTP-tool credential environment variables.
It publishes the same immutable definitions using an admission-only builder with
disabled executors. Execution credentials belong in the worker environment. A matching worker publishes saved requests and owns execution. An API
exit after acceptance does not discard the work. Reuse `request_key` for retries.

`GET /health` reports `mode: "temporal"`. `--temporal-task-queue` requires
`--database`. Both hosts must use the same configuration, storage namespace and
task queue. Controls validate the root scheduling target, including for delegated
runs. Approvals and replies are saved in PostgreSQL; the worker observes them on
its next tick. This mode does not adopt runs created by the local execution mode.

This remains a loopback development API with a fixed local identity. Hosted
identity, token authentication and multi-tenant deployment are separate work.
Hudson Sandbox is a separate product and is not required for this mode. A future
adapter will connect its execution operations through Hudson's tool controls.
