# Claude Agent SDK

Python SDK wrapper; MIT for this repository. Reviewed 2026-09-20.

Repository: [anthropics/claude-agent-sdk-python](https://github.com/anthropics/claude-agent-sdk-python). Source snapshot: `f7547d7233527739ece8b12ed28c57be96c966b5`.

## What we inspected

The inspected transport finds and launches a Claude Code CLI subprocess and exchanges structured messages. The README describes bundling the CLI. This SDK exposes access to a harness, rather than publishing that harness as a Python loop.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
application configures SDK
→ SDK starts Claude Code subprocess
→ exchange structured requests and events
→ application handles controls and results
```

## What Hudson can learn

A language-neutral process boundary can keep application code separate from the agent runtime. Distinguish the client wrapper from the execution engine.

## Limits of this reference

The MIT license of the wrapper does not change the separately licensed Claude Code runtime. This is an integration-boundary reference, not a reusable open-source core loop.

## Source pointers

- [LICENSE](https://github.com/anthropics/claude-agent-sdk-python/blob/f7547d7233527739ece8b12ed28c57be96c966b5/LICENSE)
- [README.md](https://github.com/anthropics/claude-agent-sdk-python/blob/f7547d7233527739ece8b12ed28c57be96c966b5/README.md)
- [src/claude_agent_sdk/_internal/transport/subprocess_cli.py](https://github.com/anthropics/claude-agent-sdk-python/blob/f7547d7233527739ece8b12ed28c57be96c966b5/src/claude_agent_sdk/_internal/transport/subprocess_cli.py)

Public documentation is a moving source, accessed on the review date:

- [Official documentation 1](https://code.claude.com/docs/en/agent-sdk/overview)

## Related offline examples

- [foreground_background.rs](../examples/foreground_background.rs)
- [steering_cancel.rs](../examples/steering_cancel.rs)

[Back to the collection](../README.md).
