# Patterns illustrated by the examples

These are original, simplified examples informed by the linked source profiles. They are not reconstructions of any complete upstream loop. All execution is local and mocked unless ordinary Rust threads are explicitly used.

## 1. A model proposes; a driver executes

Run [tool_cycle.rs](examples/tool_cycle.rs). A scripted model requests an order lookup. A separate driver checks the tool, supplies an observation, and asks for the next reply. A second request for an unauthorized tool never reaches execution.

The driver is where Hudson must enforce current identity, policy, approval, resource scope, and budget. A tool name allowlist in this example stands in for that larger contract.

References: [Rig](references/rig.md), [Codex](references/codex.md), [yoagent](references/yoagent.md).

## 2. Approval is a wait state, not a successful tool result

Run [approval_resume.rs](examples/approval_resume.rs). The mock checkpoint contains the exact operation and arguments. Changed arguments, expired approval, and revoked access all prevent dispatch. After a simulated lost acknowledgement, the operation becomes uncertain and cannot be blindly repeated. A trusted reconciliation supplies a terminal receipt.

Cloning the state simulates restoration; it does not prove persistence. Production requires transactional writes, authenticated approvers, operation identity, leases, and destination-specific reconciliation. A completed receipt is historical evidence and does not grant new authority.

References: [OpenAI Agents SDK](references/agents-sdk.md), [OpenHands](references/openhands.md), [LangGraph](references/langgraph.md). Unknown-outcome handling also follows Hudson's existing runtime and sandbox goals.

## 3. Parallelize explicitly independent operations

Run [parallel_tools.rs](examples/parallel_tools.rs). Two read operations run on scoped threads and results remain associated with their call IDs. A write or oversized batch is rejected before anything starts.

Real parallel admission must use trusted tool metadata and resource dependencies. Even read operations may depend on order or snapshot consistency. Validate and reserve resources before dispatch, and settle partial failures individually.

References: [Codex](references/codex.md), [Pi](references/pi.md), [OpenHands](references/openhands.md).

## 4. Delegate a task with restricted authority

Run [parent_children.rs](examples/parent_children.rs). Two child tasks receive a subset of the parent's permissions and separately reserved budget units. The parent collects bounded findings after both finish.

This illustrates child runs, not an agent handoff. A handoff changes the active agent; delegation preserves a parent that receives a result. Production still needs explicit child identity, context filtering, cancellation, partial-failure behavior, and aggregate cost settlement.

References: [Goose](references/goose.md), [Google ADK](references/adk.md), [Deep Agents](references/deepagents.md), [OpenAI Agents SDK](references/agents-sdk.md).

## 5. Foreground and background share the worker path

Run [foreground_background.rs](examples/foreground_background.rs). One caller waits immediately. Another receives a handle and reads the result later. Dropping a viewer leaves the worker intact.

The process is deliberately kept alive. Rust threads do not provide durable execution. Hudson must persist run state and dispatch work independently of a request handler to survive real disconnections, restarts, and host failures. Async code, parallel work, and durable background execution are different properties.

References: session/control boundaries in [Goose](references/goose.md) and [Claude Agent SDK](references/claude-sdk.md); persistence boundaries in [LangGraph](references/langgraph.md). The unified foreground/background rule is a Hudson proposal.

## 6. Build bounded model context from preserved evidence

Run [context_budget.rs](examples/context_budget.rs). It trims an old, large exchange while keeping a recent request/result pair together. Pending input and trusted instructions have reserved space. The full evidence history remains unchanged. Required context that cannot fit produces an error.

Character counts are only a teaching proxy. Real context management needs provider token accounting, complete protocol-valid tool exchanges, artifact references, and tested summaries. Summaries remain untrusted model input and cannot modify policy or erase pending operations.

References: [Codex](references/codex.md), [Claude Code](references/claude-code.md), [OpenCode](references/opencode.md), [Deep Agents](references/deepagents.md).

## 7. Verify, then repair within a bound

Run [verify_repair.rs](examples/verify_repair.rs). A candidate answer fails a deterministic check, receives feedback, and is corrected. A separate fixture exhausts the attempt allowance and fails rather than claiming success.

Production evaluators may be deterministic, integration-based, human, or model-assisted. Record which evaluator ran and what evidence it used. A model saying it checked its own work is not equivalent to an independent check. Repair attempts consume the same run budget and permissions.

References: [smolagents](references/smolagents.md), [Claude Code](references/claude-code.md), [AutoAgents](references/autoagents.md).

## 8. Accept steering and cancellation between bounded steps

Run [steering_cancel.rs](examples/steering_cancel.rs). A new instruction enters the next model context. A later cancellation stops further dispatch.

Production cancellation must also reach active model streams, tool workers, and sandbox operations. Record requested versus confirmed cancellation. Stopping new work cannot undo an already-completed external effect, and user steering cannot override policy simply by appearing in a prompt.

References: [Pi](references/pi.md), [yoagent](references/yoagent.md), [Codex](references/codex.md).

## Generated code as an action

[smolagents](references/smolagents.md) and [Deep Agents](references/deepagents.md) are useful references for agents that produce code or work with files. This collection intentionally does not execute generated code. In Hudson, such an action should request [Hudson Sandbox](https://github.com/hudson-infinity/hudson-sandbox) with explicit limits and scoped tool access, then consume a structured execution receipt.

No standalone example can establish that security boundary. It requires a real integration and adversarial isolation tests.
