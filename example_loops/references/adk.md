# Google ADK

Python agent and workflow framework; Apache-2.0. Reviewed 2026-09-20.

Repository: [google/adk-python](https://github.com/google/adk-python). Source snapshot: `d57c84f13baf53cfd910c2155449f8c4254e01c5`.

## What we inspected

The base LLM flow yields events from one step until a final response or termination condition. LoopAgent repeats child agents with iteration and escalation controls; ParallelAgent merges concurrent child execution.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
run one LLM step and yield events
→ process tools
→ continue until final response
optional orchestration: repeat children or run them in parallel
```

## What Hudson can learn

Keep the model/tool loop separate from a workflow that repeats agents. Child lifecycle and event routing deserve explicit contracts.

## Limits of this reference

This is a source reference, not an ADK adoption decision. Its agent/context types and Python runtime would add an additional framework and language if imported directly.

## Source pointers

- [LICENSE](https://github.com/google/adk-python/blob/d57c84f13baf53cfd910c2155449f8c4254e01c5/LICENSE)
- [src/google/adk/flows/llm_flows/base_llm_flow.py](https://github.com/google/adk-python/blob/d57c84f13baf53cfd910c2155449f8c4254e01c5/src/google/adk/flows/llm_flows/base_llm_flow.py)
- [src/google/adk/agents/loop_agent.py](https://github.com/google/adk-python/blob/d57c84f13baf53cfd910c2155449f8c4254e01c5/src/google/adk/agents/loop_agent.py)
- [src/google/adk/agents/parallel_agent.py](https://github.com/google/adk-python/blob/d57c84f13baf53cfd910c2155449f8c4254e01c5/src/google/adk/agents/parallel_agent.py)

## Related offline examples

- [parent_children.rs](../examples/parent_children.rs)
- [parallel_tools.rs](../examples/parallel_tools.rs)
- [verify_repair.rs](../examples/verify_repair.rs)

[Back to the collection](../README.md).
