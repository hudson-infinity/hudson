# Claude Code

Documented product behavior; proprietary runtime. Reviewed 2026-09-20.

Repository: [anthropics/claude-code](https://github.com/anthropics/claude-code). Source snapshot: `7974a70773fa229e4cc65aa1b356cc21f5c216c4`.

## What we inspected

Public documentation describes gathering context, acting, and verifying in a repeated loop. It also describes steering, context compaction, and separate subagent contexts. These are observable product contracts, not a recovered implementation.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
gather relevant context
→ choose and request an action
→ enforce permissions
→ inspect results and verify
→ continue, delegate, or finish
```

## What Hudson can learn

Use verification and controlled context sharing as design references. Foreground and background UX can share an execution model.

## Limits of this reference

The public repository license says all rights reserved and points to commercial terms. It does not provide an open-source implementation of the proprietary core loop. No Claude Code runtime code is copied or reconstructed here. The sketch is an original conceptual illustration.

## Source pointers

- [LICENSE.md](https://github.com/anthropics/claude-code/blob/7974a70773fa229e4cc65aa1b356cc21f5c216c4/LICENSE.md)
- [README.md](https://github.com/anthropics/claude-code/blob/7974a70773fa229e4cc65aa1b356cc21f5c216c4/README.md)

Public documentation is a moving source, accessed on the review date:

- [Official documentation 1](https://code.claude.com/docs/en/how-claude-code-works)
- [Official documentation 2](https://code.claude.com/docs/en/sub-agents)

## Related offline examples

- [verify_repair.rs](../examples/verify_repair.rs)
- [parent_children.rs](../examples/parent_children.rs)
- [context_budget.rs](../examples/context_budget.rs)

[Back to the collection](../README.md).
