# Adaptive team coordination

Customers register specialists and tools. The lead model chooses which specialist to call and can add dependencies at run time; no hand-written workflow graph is required. Existing Temporal orchestration executes independent child workflows concurrently and waits before resuming the lead.

Delegation tools accept:

```json
{"task":"collect sales figures","task_key":"collect","max_model_calls":3}
```

A later call in the same batch may depend on it:

```json
{"task":"analyze the collected sales figures","task_key":"analyze","depends_on":["collect"]}
```

Task keys are unique within the parent. Submit prerequisites before their dependents in the tool-call batch. Dependencies must identify already submitted siblings; forward references, self references, duplicates and cross-parent references are rejected. This creates a DAG by construction. Dependencies apply equally to local runtime ticks and Temporal activities. A blocked child stays queued without consuming operations or model-call budget. On prerequisite completion the child receives the exact persisted result, assessment and source run ID as task data. When context management is enabled, large prerequisite sets become immutable artifacts scoped to the receiving child. The child reads exact results and assessments through the existing context reader; the pointer and artifact commit with its checkpoint. Disabled context management retains the explicit context-limit failure rather than silently truncating results.

A failed or cancelled prerequisite fails queued dependents before effects. Cancelling the parent propagates through the existing child tree, including blocked children. The lead receives current child statuses and failure reasons in its model context and can inspect durable results with the existing join tools; it can adapt its next task or stop. Child failure does not automatically fail the lead, allowing recovery. The runtime does not claim that matching output shapes prove semantic success.

`CoordinationPolicy` is pinned with `store.bind_coordination_policy(workspace, agent_ref, &policy)` before runs start. Defaults:

| Field | Default | Enforced scope |
|---|---:|---|
| `max_children` | 16 | Direct children and all descendants under the root |
| `max_repeated_task` | 2 | Same specialist, input and dependency set under one parent |
| `child_max_model_calls` | 8 | Each admitted child |
| `child_max_operations` | 32 | Each admitted child |
| `child_max_harness_steps` | 128 | Each admitted child |

Child limits are the minimum of the specialist's own limits and the parent's policy. An individual delegation can lower the model-call limit further. Shared workspace model budgets remain in force. Repeated identical tasks are rejected with feedback to inspect existing results instead; this is deterministic repetition detection, not a semantic estimate of progress. The total delegation limit also bounds attempts that keep changing their input.

Child creation, parent link, dependency graph, task key and narrowed budgets commit atomically. The parent operation is the idempotency identity; repeating its identical submission returns the same child, while changed arguments fail. Uncertain database commits remain unknown operations for reconciliation, not automatically retried external effects. PostgreSQL preserves the graph and limits through worker restarts.

Library hosts can use `subagents::register_deferred_with_join` to expose these tools without driving child effects inside the callback. The older explicit separate-store embedding remains available through ordinary registration, but dependencies and coordinated limits require the shared parent store. Updated delegation schemas have a new tool identity; publish a new agent version when replacing older saved delegation definitions.

Tests cover concurrent independent children, blocked dependent execution and result propagation, failed/cancelled prerequisites, parent cancellation, repetition/task-count limits, child model-call budgets, idempotency, PostgreSQL reconnect, and the existing Temporal team test extended with an actual dependent third child.

Review regressions additionally cover nested root budgets, actor isolation, stopped roots, unresolved provider outcomes, dependency artifact paging under a small context cap, and PostgreSQL reconnect before the receiving model dispatch. A failed engine advance commits no staged dependency artifacts.
