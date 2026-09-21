# OpenCode

TypeScript coding-agent harness; MIT. Reviewed 2026-09-20.

Repository: [anomalyco/opencode](https://github.com/anomalyco/opencode). Source snapshot: `0e3dfd17694471b55f1cc0db578bff8920341e2d`.

## What we inspected

The processor consumes model stream events and tracks tool calls and results. It exposes compact, stop, and continue outcomes and includes repeated-tool-call detection. Separate task and compaction modules illustrate delegation and context handling.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
consume provider events
→ update tool-call state
→ apply permission checks
→ collect results or errors
→ compact, continue, or stop
```

## What Hudson can learn

Make repeated ineffective actions visible and bounded. Separate partial stream events from completed operations and final results.

## Limits of this reference

Its session, permission, and Effect-based application architecture are not Hudson contracts. Repeated-call detection is a diagnostic and control mechanism, not proof that an action is safe.

## Source pointers

- [LICENSE](https://github.com/anomalyco/opencode/blob/0e3dfd17694471b55f1cc0db578bff8920341e2d/LICENSE)
- [packages/opencode/src/session/processor.ts](https://github.com/anomalyco/opencode/blob/0e3dfd17694471b55f1cc0db578bff8920341e2d/packages/opencode/src/session/processor.ts)
- [packages/opencode/src/tool/task.ts](https://github.com/anomalyco/opencode/blob/0e3dfd17694471b55f1cc0db578bff8920341e2d/packages/opencode/src/tool/task.ts)
- [packages/opencode/src/session/compaction.ts](https://github.com/anomalyco/opencode/blob/0e3dfd17694471b55f1cc0db578bff8920341e2d/packages/opencode/src/session/compaction.ts)

## Related offline examples

- [steering_cancel.rs](../examples/steering_cancel.rs)
- [context_budget.rs](../examples/context_budget.rs)
- [parent_children.rs](../examples/parent_children.rs)

[Back to the collection](../README.md).
