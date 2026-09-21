# Rig

Rust library; MIT. Reviewed 2026-09-20.

Repository: [0xPlaygrounds/rig](https://github.com/0xPlaygrounds/rig). Source snapshot: `53cb5e164ac83ae514c4a298e62f89f79ee106a6`.

## What we inspected

The inspected AgentRunStep enum exposes CallModel, CallTools, and Done. AgentRun is a serializable state machine, while the runner drives effects and hooks. This is a concrete candidate for separating decisions from Hudson-owned execution.

## Conceptual shape

Original reading aid, not copied upstream code or an exact reconstruction:

```text
advance run state
→ CallModel: driver supplies a model response
→ CallTools: driver supplies correlated tool results
→ advance again
→ Done
```

## What Hudson can learn

Evaluate the externally driven state machine as well as the convenience runner. Hudson could persist state and control effects at these boundaries.

## Limits of this reference

Source on main is not proof of an available stable release or migration compatibility. Serialization alone does not provide durable scheduling, atomic receipts, or safe retry of unknown effects. Pin and test a released version before choosing this dependency.

## Source pointers

- [LICENSE](https://github.com/0xPlaygrounds/rig/blob/53cb5e164ac83ae514c4a298e62f89f79ee106a6/LICENSE)
- [crates/rig-agent/src/agent/run.rs](https://github.com/0xPlaygrounds/rig/blob/53cb5e164ac83ae514c4a298e62f89f79ee106a6/crates/rig-agent/src/agent/run.rs)
- [crates/rig-agent/src/agent/hook.rs](https://github.com/0xPlaygrounds/rig/blob/53cb5e164ac83ae514c4a298e62f89f79ee106a6/crates/rig-agent/src/agent/hook.rs)
- [crates/rig-agent/src/run/mod.rs](https://github.com/0xPlaygrounds/rig/blob/53cb5e164ac83ae514c4a298e62f89f79ee106a6/crates/rig-agent/src/run/mod.rs)
- [crates/rig-agent/src/agent/runner.rs](https://github.com/0xPlaygrounds/rig/blob/53cb5e164ac83ae514c4a298e62f89f79ee106a6/crates/rig-agent/src/agent/runner.rs)

## Related offline examples

- [tool_cycle.rs](../examples/tool_cycle.rs)
- [approval_resume.rs](../examples/approval_resume.rs)

[Back to the collection](../README.md).
