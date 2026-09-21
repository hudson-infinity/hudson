# AutoAgents

Rust agent framework; MIT OR Apache-2.0. Reviewed 2026-09-20.

Repository: [liquidos-ai/AutoAgents](https://github.com/liquidos-ai/AutoAgents). Source snapshot: `6301004371aa3fe274a309e493cdd3bfb07592db`.

## What we inspected

The ReAct executor creates a TurnEngine, advances it within a turn limit, and handles Finish versus Continue results. Separate execution and streaming paths provide a useful executor boundary.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
create task context
→ run one bounded turn
→ inspect Finish or Continue
→ collect tool results
→ repeat within configured limit
```

## What Hudson can learn

Give the loop a small typed outcome and explicit stopping conditions. Separate task orchestration from a single turn.

## Limits of this reference

The framework has its own context and lifecycle model. Verify how limit exhaustion is represented before mapping upstream output to Hudson success; a returned answer is not automatically a verified outcome.

## Source pointers

- [MIT_LICENSE](https://github.com/liquidos-ai/AutoAgents/blob/6301004371aa3fe274a309e493cdd3bfb07592db/MIT_LICENSE)
- [APACHE_LICENSE](https://github.com/liquidos-ai/AutoAgents/blob/6301004371aa3fe274a309e493cdd3bfb07592db/APACHE_LICENSE)
- [crates/autoagents-core/src/agent/prebuilt/executor/react.rs](https://github.com/liquidos-ai/AutoAgents/blob/6301004371aa3fe274a309e493cdd3bfb07592db/crates/autoagents-core/src/agent/prebuilt/executor/react.rs)
- [crates/autoagents-core/src/agent/executor/turn_engine.rs](https://github.com/liquidos-ai/AutoAgents/blob/6301004371aa3fe274a309e493cdd3bfb07592db/crates/autoagents-core/src/agent/executor/turn_engine.rs)

## Related offline examples

- [tool_cycle.rs](../examples/tool_cycle.rs)
- [verify_repair.rs](../examples/verify_repair.rs)

[Back to the collection](../README.md).
