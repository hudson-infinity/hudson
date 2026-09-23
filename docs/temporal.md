# Temporal execution

Hudson uses the same AgentLoop, tools, permission checks, budgets, and Postgres
records in both execution modes. Foreground waits for a Temporal workflow result;
background returns the run ID while a separate worker continues the workflow.

A configuration with no `subagents` defines one agent. A configuration with
`subagents` defines a lead-managed team. Each child retains its own instructions,
model, and tools. Temporal starts all children in a completed delegation batch
before joining them. The lead then uses its join tools to read their results.
Peer-to-peer swarms are not implemented.

## Local usage

Start a Temporal server separately (`temporal server start-dev`) and a Postgres
server. Configure the Temporal connection through the official SDK's environment
configuration. Use a dedicated task queue for each immutable configured tree and
Hudson database namespace; every worker on that queue must use that same config.

```sh
cargo run -p hudson-temporal -- \
  --config examples/team.json --database hudson --namespace temporal-team \
  --task-queue temporal-team worker

cargo run -p hudson-temporal -- \
  --config examples/team.json --database hudson --namespace temporal-team \
  --task-queue temporal-team run --task "Research this task" --request-key task-1

# Same execution, return immediately after Temporal accepts it:
cargo run -p hudson-temporal -- \
  --config examples/team.json --database hudson --namespace temporal-team \
  --task-queue temporal-team run --task "Research this task" --request-key task-2 --background
```

`run --input-file task.json` submits structured JSON (use `-` for stdin).

`run --resume <run-id>` starts or attaches to the stable workflow ID. Repeating
submission with the same request key repairs an interruption between committing
the run to Postgres and starting Temporal. There is not yet an automatic outbox
repair process. The current Postgres connection uses the local `/tmp` socket.

Use the existing worker control commands to approve operations, answer questions,
cancel runs, and reconcile unknown outcomes in the same database/namespace.
Temporal observes those changes using durable timers. Do not use the old server's
execution driver or the synchronous worker to execute these runs concurrently.
Temporal team definitions have different tool identities; use a new namespace or
increment the agent versions when migrating an existing synchronous tree.

## Durability boundary

Workflow code does no provider or tool IO. Activities call the existing runtime,
which commits operation intent before effects. If an activity dies during an
external call, its retry does not repeat a Running or Unknown operation. A trusted
operator must reconcile uncertain effects with evidence through existing controls.
Temporal retries are not proof that a remote write failed.

Activities heartbeat every two seconds, with a ten-second heartbeat timeout.
Worker loss therefore triggers a Temporal retry while persisted operation state
continues to prevent replay of uncertain effects.

Workflow histories continue as new every 500 driver iterations. Child workflows
finish before that rollover. Waiting approvals/input and unresolved operations do
not consume model calls. Different specialist executors can run concurrently;
runs using the same configured agent executor are serialized within one worker.

## Verification

```sh
python3 scripts/check.py --database hudson_harness_test_20260921 --temporal
```

The Temporal tests start real isolated local servers and use local fake model
endpoints; they do not call paid model APIs. On first use the test SDK downloads
the Temporal CLI. Coverage includes cancellation, concurrent team members and
joined results, foreground/background CLI processes, request-key deduplication,
a question resumed from PostgreSQL after a worker restart, approval enforcement,
and SIGKILL during a write followed by a Temporal retry and receipt reconciliation.
No hosted deployment or paid provider calls are part of these checks.

## Embedding execution in a Rust service

`hudson_temporal::ExecutionClient` exposes the same start/attach and result logic
used by the CLI. Build a configured tree and submit with its core runtime to pin
instructions, tools, goal, and budget, then call `execution.start(run_id).await`
to obtain a serializable `RunReceipt`. Return that receipt for background work,
or call `execution.result(run_id).await` for foreground work. A separate worker
runs `RunActivities` from that same configured tree.

Construct the client with the official Temporal `Client`, Hudson storage namespace,
and worker task queue. A repeated start attaches to the stable workflow identity;
completed workflows are never executed again. Repeating submission with the same
request key repairs the storage-to-Temporal submission gap. An application exposing
this SDK over a network must authenticate its own callers and supply their trusted
workspace/actor identity; the existing loopback HTTP development server still uses
its local driver and does not schedule Temporal runs.
