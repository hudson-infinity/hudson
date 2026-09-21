# Deep Agents

Python harness built on LangChain/LangGraph; MIT. Reviewed 2026-09-20.

Repository: [langchain-ai/deepagents](https://github.com/langchain-ai/deepagents). Source snapshot: `a764619aa8c850bc75e2e916cf53a587637d8c81`.

## What we inspected

The inspected graph builder assembles filesystem, subagent, summarization, and other middleware. Subagent middleware prepares child state and returns a result correlated to the calling tool.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
assemble agent middleware
→ call model with available tools
→ delegate or execute through configured backends
→ summarize context when needed
→ return child results into the parent conversation
```

## What Hudson can learn

Study bounded child context and middleware composition. Filesystem state and summaries can support longer tasks when their provenance is preserved.

## Limits of this reference

This is a higher-level harness with its own dependencies and state model. Use it as a behavior reference; do not silently introduce its Python stack or equate a configured backend with Hudson Sandbox isolation.

## Source pointers

- [LICENSE](https://github.com/langchain-ai/deepagents/blob/a764619aa8c850bc75e2e916cf53a587637d8c81/LICENSE)
- [libs/deepagents/deepagents/graph.py](https://github.com/langchain-ai/deepagents/blob/a764619aa8c850bc75e2e916cf53a587637d8c81/libs/deepagents/deepagents/graph.py)
- [libs/deepagents/deepagents/middleware/subagents.py](https://github.com/langchain-ai/deepagents/blob/a764619aa8c850bc75e2e916cf53a587637d8c81/libs/deepagents/deepagents/middleware/subagents.py)

## Related offline examples

- [parent_children.rs](../examples/parent_children.rs)
- [context_budget.rs](../examples/context_budget.rs)

[Back to the collection](../README.md).
