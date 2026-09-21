# Hudson data model

Status: proposed design, 2026-09-21. No runtime, database schema, or recovery guarantee is implemented by this document.

Hudson has five top-level models: **Agent, Tool, Run, Operation, Event**. Developers define agents and tools; Hudson manages runs, operations, and events. Coding, data, and support agents use these same models.

This is a logical schema. Nested values are typed structures, not additional public resources. Storage may normalize them internally. A database, orchestration engine, and loop dependency remain unselected.

## Relationships

```text
Agent (immutable version) ── references ──> Tool (immutable version)
         │
         └── Run (one task)
              ├── Operation (model call, tool call, or verification)
              │    ├── approval records
              │    └── execution attempts
              └── Event (ordered durable history)
```

An operation's events also belong to its run. All resources carry `workspace_id`; references must remain within that workspace. Caller-supplied IDs never establish access. Identity, workspace membership, policy evaluation, and secret storage are platform services outside these five execution models.

IDs are opaque, timestamps are UTC, and persisted envelopes carry `schema_version`. JSON Schema describes customer inputs and outputs. Payloads have size limits; larger files and results use access-controlled artifact references. Secrets are represented only by protected references.

## 1. Agent — what behavior to run

Published versions are immutable. `(workspace_id, id, version)` identifies a configuration; the stable `id` groups versions. A configuration change publishes a new version. Draft editing is outside this initial execution contract.

| Field | Shape | Meaning |
| --- | --- | --- |
| `id`, `workspace_id`, `version` | IDs and version | Published identity |
| `name`, `description` | Text | Human-facing purpose |
| `instructions` | Text | Trusted developer instructions |
| `model` | Object | Provider, model identifier, supported generation settings, connection reference |
| `tools` | List of `{tool_id, version, alias}` | Pinned capabilities; aliases must be unique in the agent |
| `context` | Object | Context-source references and context-selection configuration |
| `input_schema`, `output_schema` | Optional JSON Schema | Structured task and final-result contracts |
| `checks` | List of versioned check references | Completion criteria, with required versus advisory classification |
| `limits` | Object | Defaults for model turns, duration, usage/cost, concurrency, and repair attempts |
| `harness` | Object | Harness identifier, version, and supported configuration |
| `created_at` | Timestamp | Publication time |

No domain-specific agent subtype is needed. The model chooses actions using instructions, context, and tool descriptions. Schema-valid output is not automatically a successful business result.

## 2. Tool — a capability and its execution binding

Tools also use immutable `(workspace_id, id, version)` identities. The public tool resource contains two nested parts: a model-facing description and runtime-only execution configuration. Only the model-facing projection is advertised to the model.

| Field | Shape | Meaning |
| --- | --- | --- |
| `id`, `workspace_id`, `version` | IDs and version | Published identity |
| `name`, `description` | Text | Capability name and when to use it |
| `input_schema`, `output_schema` | JSON Schema; output optional | Request validation and optional result validation |
| `execution` | Tagged object | `http`, `worker`, or `sandbox`; target reference and versioned executable/contract details |
| `credential_ref` | Optional protected reference | Resolved by trusted execution services, never sent to the model |
| `policy_ref` | Reference | Current business permissions, resource restrictions, and approval requirements |
| `execution_rules` | Object | Timeout, output limit, trusted effect classification, retry/idempotency contract, concurrency restrictions |
| `created_at` | Timestamp | Publication time |

Execution variants need concrete adapters: an HTTP tool has an endpoint and protocol; a worker tool identifies a registered worker capability; a sandbox tool identifies a packaged executable and compatible environment. Describing a function does not deploy it. Hosted customer code follows the [external sandbox boundary](implementation-decisions/0002-hudson-sandbox.md).

Credential rotation and access revocation remain live through their references. Changing a tool's schema, target, or executable publishes a new version. Tool descriptions or model-supplied arguments cannot grant permissions or declare an action safe to retry.

## 3. Run — one task and its saved progress

| Field | Shape | Meaning |
| --- | --- | --- |
| `id`, `workspace_id` | IDs | Run identity |
| `agent_ref` | `{id, version}` | Exact published agent configuration |
| `actor_ref` | Principal reference | Authenticated initiating identity; later actions still require current authority |
| `request_key`, `input_digest` | Optional key and digest | Submission deduplication within workspace and actor scope |
| `input` | Typed content or structured payload | User task and attachments |
| `status`, `reason` | Enum and optional typed detail | Execution lifecycle and explanation |
| `wait` | Optional tagged object | Approval, user input, timer, external result, or reconciliation; includes correlation IDs |
| `state` | Versioned checkpoint envelope | Harness/library version, saved payload or reference, pending operation IDs, and consumed-input cursor |
| `limits`, `usage` | Objects | Effective admitted limits, reservations, and measured or estimated consumption |
| `result` | Optional object | Final content, structured output, and artifact references |
| `assessment` | Object | `pending`, `passed`, `failed`, `unavailable`, or `not_required`, plus check versions and evidence |
| `revision`, `lease` | Counter and optional object | Optimistic concurrency plus worker ownership, fencing token, and expiry |
| `created_at`, `updated_at`, `deadline_at` | Timestamps; deadline optional | Lifecycle timing |

Run status: `queued`, `running`, `waiting`, `cancelling`, `completed`, `failed`, `cancelled`.

```text
queued → running ↔ waiting
             ├── completed
             └── failed
nonterminal → cancelling → cancelled
```

Cancellation records both requested and confirmed outcomes. A cancelling run may still receive the receipt of an already-started operation. Unresolved external effects keep the run nonterminal with a reconciliation reason; cancellation cannot reverse a completed effect.

Conversation items are typed payloads in durable Events, selected into model requests. The checkpoint carries the harness's working view or references to it; it is not a second independently editable conversation history. Context compaction changes the working view without erasing evidence. Library checkpoint formats are versioned and migration must be supported explicitly or resume rejected clearly.

One run covers a task including its approval and clarification waits. Separate tasks create new runs. Session grouping, persistent cross-run memory, and parent/child runs are later additions with explicit sharing and lifecycle rules.

## 4. Operation — one intended effect

| Field | Shape | Meaning |
| --- | --- | --- |
| `id`, `workspace_id`, `run_id` | IDs | Stable execution identity |
| `step_index`, `request_index` | Integers | Stable request position within a persisted harness transition |
| `kind` | `model`, `tool`, `verify` | Typed request and result variant |
| `request`, `request_digest` | Typed payload and digest | Immutable execution intent and its canonical digest |
| `source` | Optional object | Producing model operation and provider tool-call ID, for conversation correlation |
| `status`, `reason` | Enum and optional typed detail | Current operation outcome |
| `approval` | Optional object | Exact binding, authorized decision records, expiry, and current disposition |
| `attempts` | List of typed records | Every dispatch attempt, receipt, error, usage, and reconciliation evidence |
| `result` | Optional typed payload | Authoritative settled result consumed by the harness |
| `revision` | Counter | Concurrent-update protection |
| `created_at`, `updated_at` | Timestamps | Lifecycle timing |

Request payloads:

- **Model:** resolved provider/model, supported settings, ordered context content/references, exact advertised tool versions and schemas, output contract, and required provider continuation metadata. A broken stream does not authorize execution of partial tool arguments.
- **Tool:** pinned tool reference, alias, final validated arguments, and target/resource scope used for authorization. Keep the provider's call ID for message matching; use Hudson's operation ID for execution.
- **Verify:** versioned check reference, candidate result, and evidence references. Model-assisted or tool-backed checks dispatch through the same controls and record linked operations rather than performing hidden effects.

Operation status: `pending`, `waiting_approval`, `running`, `unknown`, `succeeded`, `failed`, `denied`, `cancelled`.

An attempt contains `{id, number, executor_ref, fencing_token, idempotency_key?, started_at, finished_at?, status, receipt?, error?, usage?}`. Request identity stays stable across eligible retries; attempt identity changes. Destination idempotency keys stay stable across retries of the same effect when supported. Unknown outcomes require destination-specific reconciliation or a proven idempotency contract; timeout alone is not evidence of failure.

Approval binds workspace, operation ID, tool version, target scope, and canonical request digest. Record requester, eligible approver rule, authenticated decisions, decision times, and expiry. Expired approvals may be renewed for an unchanged pending request with preserved history. Changed arguments or target require a new operation and a new approval decision. Recheck current policy and revocation immediately before dispatch.

`succeeded` means execution produced a valid result, not that the overall agent met its goal. A required check that returns `passed: false` is a successful verification operation with a failed assessment.

## 5. Event — ordered execution evidence

| Field | Shape | Meaning |
| --- | --- | --- |
| `id`, `workspace_id`, `run_id` | IDs | Event ownership |
| `operation_id` | Optional ID | Associated operation, if any |
| `sequence` | Increasing integer within a run | Stable ordering and reconnect cursor |
| `type`, `schema_version` | Tag and version | Versioned event contract |
| `actor_ref` | Optional principal/service reference | Who caused the transition |
| `payload` | Typed, bounded object | Relevant data or references to larger payloads |
| `created_at` | Timestamp | Record time |

Initial event families: `run.created`, `input.received`, `message.recorded`, `operation.requested`, `approval.requested`, `approval.decided`, `operation.started`, `operation.settled`, `operation.unknown`, `run.waiting`, `run.resumed`, `run.cancellation_requested`, `run.finished`, `assessment.recorded`.

Conversation items use content blocks for text, images, structured values, tool calls/results, and artifact references. Their source and trust classification are assigned by the runtime, not accepted from an untrusted payload. Preserve necessary provider protocol metadata in protected adapter data, without making private reasoning a public observability contract.

Events are append-only during their retention period. Authoritative status lives in Run and Operation; events describe committed transitions and are not an independent command queue. A tool result is stored once as the operation result and may be referenced by its conversation event. Durable input events carry deduplication keys so delivery can be retried safely.

Transient token deltas may be streamed separately. Their loss must not lose completed messages or execution outcomes. Public event projections apply workspace authorization and redaction. Retention must not remove records still needed to resume or reconcile nonterminal runs.

## Persistence and consistency rules

1. **Pin definitions.** Run creation resolves an immutable Agent version and its Tool versions. Current policy and revocations are checked independently of those pins. Run-specific limits cannot enlarge the initiating principal's permitted budget.
2. **Deduplicate submissions.** For `(workspace, actor, request_key)`, identical input returns the existing run; changed input is rejected. Publishing a new version does not change the agent selected for a duplicate submission.
3. **Commit intent before effects.** Atomically save the next checkpoint, new pending operations, budget reservations, and transition events before dispatch. Uniqueness on `(run_id, step_index, request_index)` prevents a repeated transition from creating new operation identities. A recoverable dispatcher scans committed pending operations or uses a transactional outbox.
4. **Fence workers.** Only the current lease owner may advance Run state. Claims and operation-attempt admission require transactional concurrency control. Fencing protects Hudson's writes; it does not retract an external request already sent by an old worker.
5. **Commit outcomes consistently.** Settle an attempt and operation, account for its usage, and append result events atomically. Advance the harness checkpoint and consumed-input cursor transactionally; a crash between result persistence and consumption must reuse the result. Pending members of a parallel batch remain tracked individually.
6. **Preserve uncertainty.** Never translate a lost acknowledgement into success or an automatically retryable failure. Completed receipts are evidence, not permission to perform another action.
7. **Bound data and cost.** Reserve shared limits before parallel dispatch, reconcile reservations with measured usage, and distinguish estimates and unavailable usage from zero. Keep payloads bounded and place large artifacts behind references. The number and size of attempts are bounded by run policy.
8. **Finish deliberately.** Propose completion only after required operations settle; required checks can return feedback for bounded repair. Exhausted repair becomes a failed run. Advisory or later assessments do not rewrite historical execution outcomes.

Suggested storage constraints/indexes: unique Agent and Tool version identities; unique submission keys when present; workspace-scoped foreign references; unique operation request positions; unique `(run_id, sequence)`; indexes on run/operation status and lease expiry. These are requirements for a later storage design, not a choice of database.

## Example: an order refund

1. Publish Agent `support@1` with Tool `lookup_order@1` and Tool `refund_order@1`.
2. Create Run `R1` for the customer's request, pinning those versions.
3. Model Operation `M1` requests a lookup; Tool Operation `T1` returns the order evidence.
4. Model Operation `M2` requests a $30 refund. Tool Operation `T2` stores that exact request and enters `waiting_approval`. Run `R1` enters `waiting`.
5. An eligible user approves `T2`. Hudson records the decision, rechecks authority, records dispatch intent, and executes it.
6. If the response is lost, `T2` becomes `unknown`. Hudson reconciles using destination evidence before considering another dispatch. The harness remains waiting for a settled outcome.
7. A confirmed refund receipt becomes `T2.result`. A later model operation produces a candidate answer; configured verification checks the evidence.
8. Run `R1` completes with its answer and assessment. Events provide the ordered history throughout.

## Scope of the first implementation

Prove one recoverable run with model/tool operations, one bound approval, recorded events, budget admission, and a completion check. Use scripted responses for conformance tests before live providers. Keep delegation, session-sharing rules, scheduling definitions, and a broader evaluator registry as later designs; the five models do not pretend to describe the entire administration platform.

Required acceptance cases: cross-workspace reference rejection, pinned versions after publication changes, submission deduplication, two-worker contention, changed/expired/revoked approval rejection, restart during an approval wait, lost write acknowledgement, partial parallel results, repeated input delivery, cancellation during an active effect, incompatible checkpoint versions, and required-check failure.

The [loop composition proposal](../example_loops/hudson-proposal.md), [integration checklist](../example_loops/evaluation-checklist.md), and [product goals](goals.md) provide the surrounding requirements. This document does not replace them or choose a loop library.
