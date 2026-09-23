# Local HTTP API

The machine-readable contract is [openapi.json](openapi.json). Each server also
serves it at `GET /openapi.json`. Credential-protected instances authenticate that
request through PostgreSQL; serving the schema makes no model calls. The live
schema declares bearer authentication as required on protected instances and
no authentication on unprotected local instances.
Import that document into an OpenAPI-capable client to inspect request and
response schemas. The API is currently a preview with unversioned paths.

Start with the [README server command](../README.md#use-the-http-api). This host
binds to loopback and uses one configured workspace/actor identity. Agent definitions
are loaded at startup. Credential issuance uses separate operator commands.

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

This remains a loopback API with one configured identity. Optional scoped bearer
authentication is described below; hosted multi-tenant deployment remains separate work.
Hudson Sandbox is a separate product and is not required for this mode. A future
adapter will connect its execution operations through Hudson's tool controls.

## Require scoped API credentials

For a credential-protected instance, run the API with `--require-api-token`,
`--database`, `--workspace-id` and `--actor-id`. Every route, including health and
OpenAPI, then requires exactly one `Authorization: Bearer <token>` header. The
stored credential must match the configured workspace and actor. Request bodies
cannot select an identity. The server remains loopback-only; terminate HTTPS at a
trusted reverse proxy when making it reachable from another host.

An installation operator with local database access issues a credential:

```sh
cargo run --locked -p hudson-server --bin hudson-credentials -- \
  --database hudson --namespace my-project \
  --workspace-id customer-one --actor-id backend \
  issue --label application-backend --ttl-seconds 86400
```

This prints the token once. Keep it in the calling backend's secret storage; only
its SHA-256 hash and ownership/expiry/revocation metadata are persisted. No public
HTTP endpoint can issue credentials. These commands require trusted database
access and are not agent tools.

Start the API and worker with matching identities:

```sh
cargo run --locked -p hudson-server -- --config examples/analyst.json \
  --database hudson --namespace my-project --temporal-task-queue my-agents \
  --workspace-id customer-one --actor-id backend --require-api-token
cargo run --locked -p hudson-temporal -- --config examples/analyst.json \
  --database hudson --namespace my-project --task-queue my-agents \
  --workspace-id customer-one --actor-id backend worker
```

Configure `HUDSON_API_TOKEN` in the calling process environment, then use:

```sh
cargo run --locked -p hudson-cli -- --api-token-env HUDSON_API_TOKEN \
  start --task 'Analyze these records' --request-key analysis-001
```

The CLI sends credentials only to HTTPS or loopback HTTP URLs, follows no
redirects, and marks the authorization header sensitive. Tokens are backend
credentials; this does not implement browser login or session cookies.

To rotate, issue a replacement for the same identity, update the caller, then
revoke the old token by the ID returned at issuance:

```sh
cargo run --locked -p hudson-server --bin hudson-credentials -- \
  --database hudson --namespace my-project \
  --workspace-id customer-one --actor-id backend revoke TOKEN_UUID
```

Authentication reads current persisted expiry/revocation on each request.
Revocation blocks later API requests; an already admitted run retains its identity
and continues. Use run cancellation when execution should stop. Tokens for this
identity can perform all the API's run controls, including approval decisions
permitted by runtime policy. There are no token-specific roles in this version.

The default unprotected mode remains a local development convenience. This is one
configured workspace/actor per server, not a hosted multi-tenant platform. Separate
customers need separate configured instances and storage namespaces. Hudson and
Hudson Sandbox use separate credentials and resource ownership; future integration
will use an explicit service adapter.
