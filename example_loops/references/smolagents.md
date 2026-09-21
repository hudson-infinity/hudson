# smolagents

Python agent loops; Apache-2.0. Reviewed 2026-09-20.

Repository: [huggingface/smolagents](https://github.com/huggingface/smolagents). Source snapshot: `30bb1161095dbae2271e6bc3cc4c219cc3897a57`.

## What we inspected

The shared agent loop is bounded by a step limit, can emit planning steps, processes action observations, and can validate a final answer. Tool-calling and code-oriented agents specialize the action step.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
optionally plan
→ generate a tool call or code action
→ execute in the selected executor
→ record observation
→ validate final answer or continue within limit
```

## What Hudson can learn

Keep action generation distinct from execution. Explicit final-answer checks are useful for a bounded verification loop.

## Limits of this reference

Generated code is a workload for an isolated executor, not trusted runtime code. Any Hudson adaptation routes it through Hudson Sandbox and scoped tool access; this review makes no security claim about an upstream executor.

## Source pointers

- [LICENSE](https://github.com/huggingface/smolagents/blob/30bb1161095dbae2271e6bc3cc4c219cc3897a57/LICENSE)
- [src/smolagents/agents.py](https://github.com/huggingface/smolagents/blob/30bb1161095dbae2271e6bc3cc4c219cc3897a57/src/smolagents/agents.py)

## Related offline examples

- [tool_cycle.rs](../examples/tool_cycle.rs)
- [verify_repair.rs](../examples/verify_repair.rs)

[Back to the collection](../README.md).
