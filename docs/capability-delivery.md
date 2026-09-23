# Managed harness capability delivery

Customers provide instructions, model selection, domain tools or MCP bindings,
skill packages, optional memory scope, budgets, and success criteria. Hudson owns
run execution and orchestration. Sandbox implementation and Langfuse are outside
this delivery. In particular, no host-shell fallback is permitted.

## Acceptance matrix

A module or passing unit test alone does not complete a row. Integrated behavior
must run through normal runtime authorization and durable execution.

| Requirement | Tracking | Evidence required before completion |
| --- | --- | --- |
| Bounded context and original-output retrieval | #1 | Long real loop, exact archive retrieval, actor/run isolation, PostgreSQL reconnect |
| MCP and portable skills | #2 | Actual protocol handshake/list/call through approval-aware runtime, schema drift rejection, lazy immutable package resources |
| Scoped memory | #3 | Prior-run recall, bounded persisted snapshot, selected retention, correction/deletion, tenant isolation and restart |
| Adaptive teams | #4 | Independent children overlap, dependencies wait, failures propagate, per-child/root budgets and repeated-task bounds |
| Verifiable results and evaluation | #5 | Customer criteria repair loop, recorded evidence, held-out incorrect-answer rejection, coding/data/research cases, actual Harbor adapter |
| Customer configuration and complete execution | #6 | Foreground/background Temporal runs combine all capabilities; disabled capabilities absent; customer domain example requires no custom workflow |

## Current integration notes

The integration branch contains context archival/retrieval, scoped memory runtime
hooks, value criteria and operation evidence. MCP and portable package modules
are integrated; customer configuration wiring is in progress. Adaptive team and
Harbor work are in progress. The complete end-to-end acceptance run is pending.

Tests with fixture models prove lifecycle behavior, not model quality. Paid model
benchmarking and production hosting must not be inferred from local test results.
All issue rows remain open until their full acceptance evidence is recorded.
