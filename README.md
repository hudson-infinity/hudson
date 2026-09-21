# Hudson

**One platform for building, running, securing, observing, and evaluating AI agents.**

Hudson is an open-source-first AI infrastructure platform being designed for teams ranging from small businesses to large enterprises. Companies bring their tools, context, and instructions. Hudson provides the runtime and an extensible default harness to execute agents, with explicit permissions, approvals, budgets, and success criteria.

> **Status:** Hudson is in product and architecture design. This repository currently documents the intended product; the capabilities below are planned, not implemented.

See [goals and feature scope](docs/goals.md) for the detailed product goals, planned capabilities, and first milestone.

See [agent loop references and offline Rust examples](example_loops/README.md) for source-backed research to inform the default harness. These are teaching examples, not an implemented Hudson runtime.

See the proposed [five-model data design](docs/data-model.md) for Agent, Tool, Run, Operation, and Event, including versioning, approvals, and recovery boundaries.

## Developer experience

**Create agent → connect tools and context → review permissions → test → deploy → monitor and improve.**

The goal is to make a useful agent straightforward to build while giving teams control over what it can access, what it can do, and how its success is measured.

An agent should be able to run interactively or continue in the background, with the same execution model, security controls, and inspectable history.

## What Hudson will include

| Capability | Intended responsibility |
| --- | --- |
| **Agent runtime** | Execution lifecycle, persistent state, cancellation, resource limits, and recovery. |
| **Default harness** | A ready-to-use agent loop for model calls, tool execution, and context management, with extension points for custom behavior. |
| **Background processing** | Long-running, scheduled, and event-triggered agents that continue without an active user session. |
| **Security** | Enforced tool permissions, scoped credentials, isolation, approval workflows, and budgets. |
| **Observability** | Inspectable records of actions, tool calls, costs, failures, approvals, and results. |
| **Custom evaluations** | Customer-defined test cases and evaluators, version comparisons, and production failures turned into regression tests. |
| **Deployment** | Local development, a useful self-hosted installation, and a managed cloud offering. |

These capabilities should work as one product, sharing execution state, permissions, and evidence.

## Architecture direction

### A durable run at the center

A run represents one execution of an agent. Model calls, tool operations, approvals, checkpoints, costs, and results connect back to that run.

Runs should survive worker interruptions, wait for approvals or external events without holding a worker, and resume from persisted state. Recovery must account for external actions that may already have happened: an uncertain write should be reconciled before a retry that could duplicate its effects.

### A general-purpose runtime and extensible harness

The runtime owns execution, persistence, scheduling, permissions, budgets, cancellation, and recovery. The harness decides what to do next through an interface controlled by the runtime.

The default harness provides a simple loop:

**Build context → call model → request tools → inspect results → continue or finish.**

Teams should be able to specialize planning, memory, context management, and verification while retaining the runtime's execution and security guarantees.

### Security enforced outside the model

Permissions and approvals must be enforced by the execution system. Instructions, model output, retrieved documents, and tool responses cannot grant authority.

The design calls for explicit access scopes, argument-level tool checks, approvals bound to specific actions, isolated execution, and credentials kept out of model context and ordinary execution records. Custom harnesses and tools must operate within these boundaries too.

Isolated execution will be provided by [Hudson Sandbox](https://github.com/hudson-infinity/hudson-sandbox), a separate repository in the `hudson-infinity` organization. Hudson owns agent behavior, business permissions, approvals, credential authority, and budgets; Hudson Sandbox owns sandbox environments, command execution, resource and network enforcement, and cleanup. Its [implementation design](https://github.com/hudson-infinity/hudson-sandbox/blob/main/docs/implementation.md) selects Rust, Temporal, and Firecracker. Both projects are currently design-only.

### Evidence shared across monitoring and evaluation

The same execution records should support debugging, operational monitoring, success checks, and regression testing. Completing an execution and satisfying its success criteria are separate outcomes.

Evaluations should exercise the same runtime used in production, with controlled fixtures or isolated integrations where appropriate. Teams should be able to investigate a production failure, turn it into a test, and compare agent versions against it.

## Repository direction

Hudson will start as one public monorepo containing:

- Runtime and default harness
- Security, observability, and evaluation modules
- SDK, API, workers, and CLI
- Console
- Documentation, integrations, and examples

These are logical module boundaries, not a commitment to separate services or packages. The public repository is intended to support a useful self-hosted installation. A separate private repository for managed cloud operations can be introduced later if needed.

Sandbox infrastructure is maintained in the separate [hudson-sandbox repository](https://github.com/hudson-infinity/hudson-sandbox). This repository owns its integration with the agent runtime and must document the compatible sandbox dependency for self-hosting.

## Initial focus

The first milestone is a complete execution flow: define an agent, connect a tool, enforce permissions, pause for approval, recover after a worker interruption, finish with an inspectable result, and turn a failure into a regression test.

Multi-agent and swarm capabilities come after the core execution model. Early research will focus on runtime reliability, security, harness behavior, and evaluations. The open-source ecosystem will grow through integrations, examples, and contributors.

## Current work

The next design work is to define the core entities, execution lifecycle, harness interface, durable background processing, security boundaries, observability, and evaluation integration.

Accepted implementation decisions are recorded in [`docs/implementation-decisions`](docs/implementation-decisions/):

- [0001: Rust implementation with language-neutral interfaces](docs/implementation-decisions/0001-rust.md)
- [0002: External sandbox execution through Hudson Sandbox](docs/implementation-decisions/0002-hudson-sandbox.md)

The guiding principle is simple: **keep the developer experience approachable and make security foundational.**
