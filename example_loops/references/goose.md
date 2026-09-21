# Goose

Rust-based agent product; Apache-2.0. Reviewed 2026-09-20.

Repository: [aaif-goose/goose](https://github.com/aaif-goose/goose). Source snapshot: `e629eea1dd37b611870cce249974da7849096bc1`.

## What we inspected

The inspected agent supports state-machine reply and resume paths, session configuration, permission handling, streaming, and subagent execution. This shows how a loop is embedded in a usable agent product.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
open session turn
→ prepare context
→ drive model and tool state transitions
→ handle confirmations and subagent results
→ emit session events
→ settle or resume later
```

## What Hudson can learn

Study the separation between session UX, tool confirmation, and the turn engine. Delegation needs recorded results and operational visibility.

## Limits of this reference

Its application configuration and session lifecycle are product choices. Reusing the whole application would require more adaptation than borrowing a loop contract. We have not validated its persistence semantics against Hudson failures.

## Source pointers

- [LICENSE](https://github.com/aaif-goose/goose/blob/e629eea1dd37b611870cce249974da7849096bc1/LICENSE)
- [crates/goose/src/agents/agent.rs](https://github.com/aaif-goose/goose/blob/e629eea1dd37b611870cce249974da7849096bc1/crates/goose/src/agents/agent.rs)
- [crates/goose/src/agents/subagent_handler.rs](https://github.com/aaif-goose/goose/blob/e629eea1dd37b611870cce249974da7849096bc1/crates/goose/src/agents/subagent_handler.rs)

## Related offline examples

- [approval_resume.rs](../examples/approval_resume.rs)
- [parent_children.rs](../examples/parent_children.rs)
- [foreground_background.rs](../examples/foreground_background.rs)

[Back to the collection](../README.md).
