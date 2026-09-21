# OpenAI Agents SDK

Python agent orchestration library; MIT. Reviewed 2026-09-20.

Repository: [openai/openai-agents-python](https://github.com/openai/openai-agents-python). Source snapshot: `518b1f2d6393e4aca80b37b1e84a4a3256fc1b9f`.

## What we inspected

The inspected run loop distinguishes final output, handoff, interruption, and another turn. Turn resolution handles tool effects and approval requirements; approval helpers keep approval placeholders distinct from tool outputs.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
prepare agent turn
→ call model
→ resolve tools and approvals
→ final output / handoff / interruption / run again
```

## What Hudson can learn

Represent a handoff, a pending approval, and completion as distinct outcomes. An approval request must not be treated as a completed tool result.

## Limits of this reference

A handoff transfers the active agent; spawning a child and collecting its result is a different relationship. This Python SDK is a reference, not a new implementation language for Hudson.

## Source pointers

- [LICENSE](https://github.com/openai/openai-agents-python/blob/518b1f2d6393e4aca80b37b1e84a4a3256fc1b9f/LICENSE)
- [src/agents/run_internal/run_loop.py](https://github.com/openai/openai-agents-python/blob/518b1f2d6393e4aca80b37b1e84a4a3256fc1b9f/src/agents/run_internal/run_loop.py)
- [src/agents/run_internal/turn_resolution.py](https://github.com/openai/openai-agents-python/blob/518b1f2d6393e4aca80b37b1e84a4a3256fc1b9f/src/agents/run_internal/turn_resolution.py)
- [src/agents/run_internal/approvals.py](https://github.com/openai/openai-agents-python/blob/518b1f2d6393e4aca80b37b1e84a4a3256fc1b9f/src/agents/run_internal/approvals.py)

Public documentation is a moving source, accessed on the review date:

- [Official documentation 1](https://developers.openai.com/api/docs/guides/agents/running-agents)

## Related offline examples

- [approval_resume.rs](../examples/approval_resume.rs)
- [parent_children.rs](../examples/parent_children.rs)
- [tool_cycle.rs](../examples/tool_cycle.rs)

[Back to the collection](../README.md).
