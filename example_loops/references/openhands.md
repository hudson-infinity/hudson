# OpenHands Software Agent SDK

Python software-agent SDK; MIT. Reviewed 2026-09-20.

Repository: [OpenHands/software-agent-sdk](https://github.com/OpenHands/software-agent-sdk). Source snapshot: `856d99d48e4b11c70c5f1cab21e7830570dbc324`.

## What we inspected

The inspected Agent step handles pending actions before sampling new ones. Its execution paths partition blocked actions and correlate batch results with action identities. Synchronous and asynchronous execution paths share that lifecycle.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
inspect pending actions
→ resolve confirmation or blocking
→ execute eligible actions
→ append observations
→ sample new actions when ready
```

## What Hudson can learn

Preserve action identity across confirmation and result handling. Resume known pending work before asking the model to invent replacement actions.

## Limits of this reference

This review covers the pinned software-agent-sdk repository, not every OpenHands service or deployment. Confirmation state must still be connected to Hudson policy and its external operation receipts.

## Source pointers

- [LICENSE](https://github.com/OpenHands/software-agent-sdk/blob/856d99d48e4b11c70c5f1cab21e7830570dbc324/LICENSE)
- [openhands-sdk/openhands/sdk/agent/agent.py](https://github.com/OpenHands/software-agent-sdk/blob/856d99d48e4b11c70c5f1cab21e7830570dbc324/openhands-sdk/openhands/sdk/agent/agent.py)

## Related offline examples

- [approval_resume.rs](../examples/approval_resume.rs)
- [parallel_tools.rs](../examples/parallel_tools.rs)

[Back to the collection](../README.md).
