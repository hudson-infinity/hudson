# Codex

Rust coding-agent harness; Apache-2.0. Reviewed 2026-09-20.

Repository: [openai/codex](https://github.com/openai/codex). Source snapshot: `57567b8d9e5a158ddce2a8bb5f75ac455b1e9b83`.

## What we inspected

The inspected turn driver combines model continuation with pending user input. Separate modules handle approval-aware tool execution, parallel admission, compaction, and agent control. Its execution machinery is broader than a portable model/tool loop.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
receive input
→ prepare context and compact if needed
→ stream a model response
→ authorize and dispatch requested tools
→ record results and consume pending input
→ continue or settle the turn
```

## What Hudson can learn

Keep the turn driver, tool authorization, execution routing, and public events distinct. Parallel tool support should be explicit metadata, rather than assuming every requested batch is safe.

## Limits of this reference

Treat this as a source reference for a full harness. Codex CLI source does not establish every behavior of hosted Codex products. Its sandbox and session assumptions need adaptation to Hudson Sandbox and workspace authorization.

## Source pointers

- [LICENSE](https://github.com/openai/codex/blob/57567b8d9e5a158ddce2a8bb5f75ac455b1e9b83/LICENSE)
- [codex-rs/core/src/session/turn.rs](https://github.com/openai/codex/blob/57567b8d9e5a158ddce2a8bb5f75ac455b1e9b83/codex-rs/core/src/session/turn.rs)
- [codex-rs/core/src/tools/orchestrator.rs](https://github.com/openai/codex/blob/57567b8d9e5a158ddce2a8bb5f75ac455b1e9b83/codex-rs/core/src/tools/orchestrator.rs)
- [codex-rs/core/src/tools/parallel.rs](https://github.com/openai/codex/blob/57567b8d9e5a158ddce2a8bb5f75ac455b1e9b83/codex-rs/core/src/tools/parallel.rs)
- [codex-rs/core/src/agent/control.rs](https://github.com/openai/codex/blob/57567b8d9e5a158ddce2a8bb5f75ac455b1e9b83/codex-rs/core/src/agent/control.rs)
- [codex-rs/core/src/compact.rs](https://github.com/openai/codex/blob/57567b8d9e5a158ddce2a8bb5f75ac455b1e9b83/codex-rs/core/src/compact.rs)

Public documentation is a moving source, accessed on the review date:

- [Official documentation 1](https://developers.openai.com/blog/codex-as-a-platform)
- [Official documentation 2](https://developers.openai.com/blog/run-long-horizon-tasks-with-codex)

## Related offline examples

- [tool_cycle.rs](../examples/tool_cycle.rs)
- [parallel_tools.rs](../examples/parallel_tools.rs)
- [steering_cancel.rs](../examples/steering_cancel.rs)
- [context_budget.rs](../examples/context_budget.rs)

[Back to the collection](../README.md).
