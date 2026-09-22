# Recovering an interrupted local run

Use the same database and namespace as the worker or server. Management commands
construct no provider client and execute no tool. They use the local developer
identity; they are operator commands, not agent tools.

A killed worker can leave an operation `running`: its dispatch intent committed,
but its result did not. Restart never assumes this means the action did not happen.

1. Inspect the run with `--inspect-run <run_id>` to find the operation.
2. Independently confirm that the executor stopped. A long elapsed time is not
   evidence of a stopped process.
3. Inspect its latest attempt ID, then fence that exact attempt:

```sh
hudson-worker --database hudson --namespace my-project \
  --inspect-operation OPERATION_UUID
hudson-worker --database hudson --namespace my-project \
  --mark-interrupted OPERATION_UUID --attempt-id ATTEMPT_UUID \
  --evidence 'Worker process exited; recorded by the operator'
```

This changes `running` to `unknown` and records the evidence. A stale attempt ID
is rejected. A late executor result cannot replace the fenced state. This command
does not verify the operator's statement or prove what happened at the destination.

For a tool operation, obtain the result and a receipt from the destination using
the operation UUID (the HTTP tool's `Idempotency-Key`). Save the actual result as
JSON, then record it:

```sh
hudson-worker --database hudson --namespace my-project \
  --reconcile-operation OPERATION_UUID --result-file result.json \
  --receipt 'Destination receipt or audit reference'
hudson-worker --database hudson --namespace my-project \
  --config agent.json --resume RUN_UUID
```

The result must match the immutable tool's output schema and the run's payload
limit. The CLI also caps result files at 1 MiB. Reconciliation records the result
and operator identity; it does not execute the tool. Resume consumes that result
and continues the ordinary loop. The destination remains the source of truth.

If the destination outcome is unknown, leave it unresolved. There is no automatic
retry or command that declares an uncertain write safe merely because time passed.
Automatic process ownership/leases and automated receipt lookup are not
implemented. Cancellation also preserves uncertain effects.

## Unavailable model response

A model operation cannot use tool-receipt reconciliation. If its response cannot
be recovered, first mark its exact attempt interrupted as above, then explicitly
abandon the unavailable response:

```sh
hudson-worker --database hudson --namespace my-project \
  --abandon-model OPERATION_UUID --evidence 'Response unavailable after confirmed worker exit'
hudson-worker --database hudson --namespace my-project \
  --config agent.json --resume RUN_UUID
```

The operation becomes failed; the next tick fails the run or settles an already
requested cancellation. It never retries the model. The original attempt remains
`unknown`, and all admitted call counters remain charged: this makes no claim that
the provider failed to process or bill the request. A running attempt must be
fenced first. Tool operations cannot use this path to bypass receipt reconciliation.
Starting a fresh task afterward is a separate explicit submission.

## Reproduce the crash boundary

```sh
cargo build --locked -p hudson-worker
HUDSON_TEST_DATABASE=hudson_harness_test_20260921 python3 scripts/smoke_recovery.py
```

Create a dedicated local PostgreSQL database before running this command. The
script uses a unique namespace and loopback stubs. It commits a destination write,
kills and reaps the worker with SIGKILL before the tool response is recorded,
restarts, verifies no replay, fences the exact attempt, reads a destination receipt,
rejects an invalid result, and resumes to completion with one write in total.

The same smoke also kills a worker during a model call, rejects abandonment before
fencing, then abandons the unavailable response. It verifies a failed run without
another provider call, retaining the uncertain attempt and admission charge.
