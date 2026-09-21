# Pi

TypeScript agent loop and coding harness; MIT. Reviewed 2026-09-20.

Repository: [earendil-works/pi](https://github.com/earendil-works/pi). Source snapshot: `890f920884f6d21fc7617d236ef9e1cc5d7a0ef8`.

## What we inspected

The inspected agent loop has an inner tool/steering cycle and an outer follow-up cycle. It emits turn and message events and allows preparation before the next model turn. The coding-agent session adds higher-level lifecycle behavior.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
take steering messages
→ prepare the next turn
→ stream model output
→ execute requested tools
→ continue if tools or steering remain
→ check queued follow-ups before ending
```

## What Hudson can learn

Treat follow-up input, steering, and tool observations as different events. Keep the portable loop smaller than the surrounding coding application.

## Limits of this reference

This is a behavior reference for Rust, not a Rust dependency. Its source repository currently redirects from badlogic/pi-mono to the canonical repository pinned below.

## Source pointers

- [LICENSE](https://github.com/earendil-works/pi/blob/890f920884f6d21fc7617d236ef9e1cc5d7a0ef8/LICENSE)
- [packages/agent/src/agent-loop.ts](https://github.com/earendil-works/pi/blob/890f920884f6d21fc7617d236ef9e1cc5d7a0ef8/packages/agent/src/agent-loop.ts)
- [packages/coding-agent/src/core/agent-session.ts](https://github.com/earendil-works/pi/blob/890f920884f6d21fc7617d236ef9e1cc5d7a0ef8/packages/coding-agent/src/core/agent-session.ts)

## Related offline examples

- [steering_cancel.rs](../examples/steering_cancel.rs)
- [context_budget.rs](../examples/context_budget.rs)
- [tool_cycle.rs](../examples/tool_cycle.rs)

[Back to the collection](../README.md).
