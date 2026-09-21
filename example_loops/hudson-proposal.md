# Proposed composition for Hudson

Status: discussion draft informed by the reference collection. No loop library or new runtime dependency is accepted by this document.

## One agent definition, one execution model

An agent definition describes instructions, model configuration, tools, context, output requirements, and allowed delegation. A run is one invocation of a pinned definition. A subagent is another run with a parent and explicitly delegated scope.

Foreground and background share the run engine. The caller chooses whether to wait, stream, disconnect, or retrieve the result later. Scheduled and event-triggered work enters through the same run admission path.

The proposed default is that children belong to their parent: cancellation propagates, reserved child budgets count toward the parent, and the parent resolves outstanding children before terminal completion. Independent background work starts as a separate top-level run. This is a proposed default, not implemented behavior.

## A small harness boundary

The strongest common shape is a harness that advances from recorded observations and asks the runtime to perform effects:

```text
agent version + checkpoint + new observations
    → harness advances
    → request model / request tools / delegate / wait / propose completion
    → runtime validates and records the operation
    → authorized executor returns an observation
    → checkpoint and continue
```

Model and tool calls are external effects. A harness must not receive a bypass around policy, credential control, or budget admission. If a custom harness runs untrusted code, it belongs across the sandbox boundary too.

Rig's inspected state-machine interface is a concrete reference for externally driven model/tool steps. Codex and Goose show the surrounding harness responsibilities. Pi and yoagent show useful steering and continuation boundaries. Claude Code's documentation supplies a behavioral reference for context, action, and verification.

## What belongs to Hudson

| Responsibility | Owner |
| --- | --- |
| Agent versions, run identity, parent/child relationships | Hudson runtime |
| Scheduling, checkpoints, cancellation, recovery | Hudson runtime and its future durable execution integration |
| Business permissions, approvals, budget reservation, credential authority | Hudson trusted execution services |
| Model context and proposed next actions | Default or custom harness |
| Provider protocol normalization | Model adapters; evaluate library reuse |
| Authorized external business actions | Tool gateway and customer integrations |
| Hosted customer code, process limits, filesystem and network isolation | External Hudson Sandbox |
| Public progress, operation receipts, artifacts, success assessments | Hudson execution records and evaluators |

Temporal's use inside Hudson Sandbox does not choose the main runtime's orchestration engine. An upstream harness's sandbox implementation does not replace the accepted Hudson Sandbox boundary automatically.

## Suggested sequence for later implementation

1. Define versioned run, observation, command, and operation-receipt contracts.
2. Build a scripted-model conformance harness around those contracts.
3. Evaluate a pinned Rig release and a pinned yoagent release against the same failure cases.
4. Reuse a loop only if it exposes the required effect, pause, and recovery boundaries. Provider adapters can be reused separately if the loop is unsuitable.
5. Add the default loop, then approvals, persistence, recovery, and the real sandbox adapter as a complete execution flow.
6. Introduce bounded delegation and richer context strategies after the core lifecycle works.

The examples exercise isolated concepts. They do not replace the [integration checklist](evaluation-checklist.md) or implement these steps.

## Decisions to discuss before writing the runtime

- Which effects can the harness request, and which observations are sufficient to resume?
- What is persisted atomically before model, tool, and child-run dispatch?
- How are unknown outcomes reconciled, and which operations may be retried automatically?
- What does cancellation mean for a parent with running children and active sandbox commands?
- How are tool results and durable events bounded without losing evidence?
- Which success criteria can block completion, and which assessments run afterward?
- Can a released library meet these contracts without forking its core?

The result should be a focused implementation decision after a reproducible evaluation, rather than a choice based on stars, marketing claims, or a successful happy-path demo.
