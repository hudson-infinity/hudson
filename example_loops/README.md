# Agent loop reference collection

Reviewed: 2026-09-20. Status: research and offline examples, before Hudson implementation.

This collection studies 15 relevant projects and product interfaces, then illustrates reusable behaviors in small Rust programs. It covers reusable libraries, complete coding-agent harnesses, and workflow engines. It is a broad selection, not a claim to inventory every open-source agent or establish a benchmark winner.

**No production Hudson loop or dependency has been selected here.** The programs use scripted model responses and mock tools. They do not call providers, execute customer code, or integrate with Hudson Sandbox.

## Start here

1. Read the comparison below and the [source profiles](#source-profiles).
2. Run the [offline Rust examples](#run-the-examples) to see individual behaviors.
3. Read [patterns and their limits](patterns.md).
4. Discuss the [proposed Hudson composition](hudson-proposal.md).
5. Use the [implementation evaluation checklist](evaluation-checklist.md) before selecting a dependency or building the runtime.

## What is a loop?

```text
task + relevant context
    → model proposes an answer or action
    → runtime checks policy, approval, and limits
    → authorized tool produces an observation
    → observation returns to the model
    → repeat, wait, or finish
```

A model/tool loop is one part of a harness. Session management, durable scheduling, tenant authorization, credentials, sandboxing, and evaluation require additional contracts. A local checkpoint or serialized object does not establish safe recovery of external side effects.

## Source profiles

The descriptions below summarize the inspected snapshots. Each profile links to exact commit versions of the files used. Scope labels are assessments for Hudson, not claims of measured performance or security.

| Reference | Implementation / source license | Strongest reason to study it | Hudson fit |
| --- | --- | --- | --- |
| [Codex](references/codex.md) | Rust / Apache-2.0 | Turn lifecycle, tool orchestration, approvals, compaction, agent control | Full harness reference |
| [Claude Code](references/claude-code.md) | Proprietary core; public behavior documentation | Context → action → verification, steering, subagent context | Behavioral reference only |
| [Rig](references/rig.md) | Rust / MIT | Externally driven, serializable model/tool state machine | Candidate reusable core |
| [yoagent](references/yoagent.md) | Rust / MIT | Compact tool loop, middleware, streaming, steering | Candidate reusable core |
| [Goose](references/goose.md) | Rust-based / Apache-2.0 | Sessions, confirmations, state-machine execution, delegation | Full harness reference |
| [AutoAgents](references/autoagents.md) | Rust / MIT OR Apache-2.0 | Typed turn results and ReAct executor boundaries | Candidate framework; assess overlap |
| [Pi](references/pi.md) | TypeScript / MIT | Inner tool cycle and outer follow-up cycle | Loop and interaction reference |
| [OpenCode](references/opencode.md) | TypeScript / MIT | Stream state, repeated-action detection, delegation, compaction | Full harness reference |
| [OpenAI Agents SDK](references/agents-sdk.md) | Python / MIT | Handoffs, interruptions, approvals, continuation outcomes | Orchestration reference |
| [LangGraph](references/langgraph.md) | Python / MIT | Checkpoints, pending work, interrupts, workflow steps | Durability reference |
| [smolagents](references/smolagents.md) | Python / Apache-2.0 | Bounded action loops and final-answer checks | Tool/code-action reference |
| [Google ADK](references/adk.md) | Python / Apache-2.0 | LLM flow versus repeating or parallel child workflows | Composition reference |
| [OpenHands SDK](references/openhands.md) | Python / MIT | Pending actions, confirmations, action/result correlation | Execution lifecycle reference |
| [Deep Agents](references/deepagents.md) | Python / MIT | Subagent context and filesystem/summarization middleware | Long-task harness reference |
| [Claude Agent SDK](references/claude-sdk.md) | Python wrapper / MIT; runtime separately licensed | Client-to-harness process boundary | Integration reference, not open core |

Claude Code's public repository explicitly reserves rights and refers to commercial terms. The open-source Claude Agent SDK wrapper does not make its bundled runtime open source. This collection uses public documentation and wrapper source to describe the boundary; it includes no proprietary loop implementation.

Other language implementations are references for Rust design. Adding a profile does not introduce that language or framework into Hudson's implementation.

## Run the examples

Only an installed Rust toolchain is needed. The example package has no external dependencies and makes no network requests. From the repository root:

```sh
cargo run --offline --manifest-path example_loops/Cargo.toml --example tool_cycle
```

Replace `tool_cycle` with another example name:

| Example | What it demonstrates | What it does not implement |
| --- | --- | --- |
| [tool_cycle](examples/tool_cycle.rs) | Model request → permission gate → result → final answer | Real model calls or complete authorization |
| [approval_resume](examples/approval_resume.rs) | Bound approvals, changed/expired/revoked rejection, uncertain outcome, receipt reuse | Durable storage or actual process-restart recovery |
| [parallel_tools](examples/parallel_tools.rs) | Bounded independent reads with stable result correlation | Production concurrency admission or write scheduling |
| [parent_children](examples/parent_children.rs) | Child scope restriction, up-front budget reservation, parent join | Distributed agents, cancellation propagation, or real billing |
| [foreground_background](examples/foreground_background.rs) | One worker path with immediate wait or later observation | Survival of process or machine failure |
| [context_budget](examples/context_budget.rs) | Whole tool exchanges and preserved pending input | Token accounting, summarization, or provider-specific message rules |
| [verify_repair](examples/verify_repair.rs) | Separate candidate generation and bounded verification | Live-model evaluation or reliable general-purpose judging |
| [steering_cancel](examples/steering_cancel.rs) | Steering and cancellation at step boundaries | Interrupting active external operations |

Each program prints a short trace and includes assertions for its intended behavior. These assertions validate the example, not an upstream project or a Hudson integration.

To format, lint, and run the collection:

```sh
cargo fmt --manifest-path example_loops/Cargo.toml -- --check
cargo clippy --offline --manifest-path example_loops/Cargo.toml --all-targets -- -D warnings
for example in tool_cycle approval_resume parallel_tools parent_children foreground_background context_budget verify_repair steering_cancel; do
  cargo run --quiet --offline --manifest-path example_loops/Cargo.toml --example "$example" || exit 1
done
```

## Evidence and reuse

Local validation on 2026-09-20 used Rust and Cargo 1.92.0: formatting checks and Clippy with warnings denied passed, and all eight examples ran offline with their assertions passing. Local Markdown links and all 51 fetched source-file hashes were also checked. These are local results for this collection, not hosted CI or upstream integration results.

- [sources.json](sources.json) records canonical repositories, commit SHAs, inspected paths, source hashes, and documentation URLs.
- Source review was targeted at control flow and selected boundaries. It is not a full code or dependency audit.
- Upstream projects were not built, deployed, benchmarked, or tested with live providers for this collection.
- Snapshots from default branches may contain behavior not yet available in a published release.
- Examples and conceptual sketches are original teaching material, not copied or drop-in versions of upstream implementations.
- No third-party source is vendored here. Follow pinned upstream license and notice requirements if implementation work later incorporates code. This collection does not select Hudson's own license.

## Relationship to Hudson

The [product goals](../docs/goals.md), [Rust decision](../docs/implementation-decisions/0001-rust.md), and [external sandbox decision](../docs/implementation-decisions/0002-hudson-sandbox.md) remain authoritative.

Main agent and subagent describe task relationships. Foreground and background describe how callers wait for and observe work. The proposed composition uses one execution model for all of them; detailed behavior remains a design proposal.
