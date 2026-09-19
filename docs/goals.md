# Hudson goals and feature scope

Status: Product and architecture design. Updated: 2026-09-19.

This document defines what Hudson aims to provide and how we will judge the initial product. Features described here are planned, not implemented. Accepted technical choices live in [implementation decisions](implementation-decisions/); this document does not select the remaining stack or define a release schedule.

## Purpose

Build one open-source-first platform for **building, running, securing, observing, and evaluating AI agents**.

Companies bring their tools, context, and instructions. Hudson supplies a general-purpose runtime and an extensible default harness, with explicit permissions, approvals, budgets, and success criteria.

An agent should be straightforward to start, safe to connect to useful capabilities, able to continue without an active user session, and inspectable when something goes wrong. Teams should be able to improve it using evidence from actual executions.

## Who it is for

- Developers integrating agents into existing applications and services.
- Small businesses and teams that need useful defaults and an approachable path to deployment.
- Platform and security teams that need control over access, execution, credentials, and operational evidence.
- Larger organizations that need shared infrastructure across multiple teams and use cases.

Broad applicability is the long-term goal. The initial product should prove a complete execution flow before adding specialized enterprise features or every possible integration.

## Product principles

1. **One product with shared state.** Runtime, security, observability, and evaluations use the same run identities, permissions, and execution evidence.
2. **Security outside the model.** Instructions and model output cannot grant access or bypass enforcement.
3. **Useful defaults with extension points.** Teams can start with the default harness and specialize planning, memory, context, and verification as needed.
4. **Durable execution.** Waiting, interruption, and recovery are normal parts of execution and have explicit behavior.
5. **Language-neutral access.** Customers can use their existing languages through documented interfaces.
6. **Useful self-hosting.** The public repository should support a working installation that demonstrates the core product without requiring Hudson's managed cloud.
7. **Evidence-based improvement.** Completion, business success, and evaluation results are recorded distinctly.

## Intended developer flow

**Create agent → connect tools and context → review permissions → test → deploy → monitor and improve.**

| Step | What the developer should be able to do |
| --- | --- |
| Create agent | Define instructions, inputs, harness configuration, limits, and success criteria. |
| Connect tools and context | Attach integrations, customer functions, and relevant information with explicit access scopes. |
| Review permissions | Understand which actions are permitted, which require approval, and which are blocked. |
| Test | Run examples and evaluation cases with fixtures or isolated integrations before enabling real actions. |
| Deploy | Publish a known agent version into an environment with configured connections and triggers. |
| Monitor and improve | Inspect progress and outcomes, resolve approval requests or failures, and turn findings into tests. |

## Planned capabilities

### Agent definitions and versions

- Define instructions, tool bindings, context sources, harness settings, and expected outputs.
- Publish immutable agent versions and record the version used for each run.
- Configure deployments with environment-specific connections, permissions, budgets, and triggers.
- Keep conversation history, individual executions, and persistent agent memory distinguishable, with explicit sharing rules.

### Agent runtime

- Own the execution lifecycle, persistent state, checkpoints, and final results.
- Support queued, running, waiting, and terminal outcomes with clear reasons for waits and failures.
- Resume after worker interruption without losing recorded progress.
- Support cancellation, deadlines, execution limits, and bounded retries.
- Track external operations and attempts so recovery can reuse known outcomes.
- Reconcile uncertain external writes before retrying actions that could duplicate effects.
- Separate execution completion from whether the run met its success criteria.

Cancellation should stop new work and interrupt active work where supported. It cannot undo an external action that has already completed. Recovery must not imply that every external system supports exactly-once effects.

### Default harness and extensions

- Provide a ready-to-use loop: build context, call a model, request tools, inspect results, and continue or finish.
- Support model-provider adapters through explicit interfaces.
- Manage bounded context and persist the state needed to resume the loop.
- Allow custom planning, memory retrieval, context selection, and verification.
- Keep provider-specific and integration-specific behavior behind adapters.
- Require custom harnesses to submit operations through the same runtime controls.

The runtime owns execution authority. A harness proposes actions within that authority, including when the harness is supplied by a customer.

### Tools, context, and connectivity

- Describe tools with versioned identities and structured input and output schemas.
- Connect authenticated HTTP tools and customer workers through documented contracts.
- Allow customer functions to run in their existing language and return structured results.
- Represent credentials through protected references, separate from ordinary agent context.
- Track the provenance and access scope of context used in a run.
- Provide examples and adapters that make common integration patterns easy to adopt.

Language-neutral interfaces do not mean arbitrary code can execute without an adapter or compatible worker. Hudson controls the requests it sends to customer-operated services; those services remain responsible for their own execution environment and access.

### Background processing

- Run agents without an active browser, client connection, or user session.
- Start runs from direct requests, schedules, and external events.
- Wait for approvals, user input, timers, or external results without occupying an execution worker for the entire wait.
- Define trigger deduplication, worker ownership, concurrency limits, and retry behavior.
- Use the same run model and security checks for interactive and background work.

### Security and access control

- Authenticate callers and enforce workspace ownership on reads and actions.
- Deny actions unless the applicable policy grants access.
- Validate tool arguments and resource scope before dispatch.
- Bind approvals to specific operations, arguments, authorized approvers, and expiry.
- Apply current revocations when work resumes; restoring a checkpoint must not restore revoked access.
- Scope credentials to the required capabilities and keep secrets out of model context and ordinary logs.
- Isolate hosted customer code through [Hudson Sandbox](https://github.com/hudson-infinity/hudson-sandbox), with enforced filesystem, network, CPU, memory, and execution-time restrictions.
- Enforce budgets and concurrency limits across parallel operations.
- Treat retrieved content and tool responses as untrusted data that cannot change authority.

Budget behavior must distinguish enforceable limits from estimates when provider usage cannot be bounded precisely in advance. Rust supports the implementation; authorization and isolation still require explicit design and testing.

Hudson retains business permissions, approvals, credential authority, and budgets. The separate Hudson Sandbox project owns isolated environments, command execution, enforcement of sandbox limits, and cleanup. These are planned responsibilities, not validated security guarantees; see [decision 0002](implementation-decisions/0002-hudson-sandbox.md).

### Observability and operational controls

- Provide a run timeline containing model and tool activity, costs, failures, approvals, waits, and results.
- Link records to the relevant agent version, operation, attempt, and artifacts.
- Expose current state and live progress, with a way to reconnect and retrieve missed durable events.
- Explain blocked or waiting runs and the action needed to move them forward.
- Support inspecting, approving, denying, cancelling, and explicitly retrying eligible work through authorized interfaces.
- Apply access control, redaction, and retention rules to execution records and artifacts.
- Present public execution evidence without depending on private model reasoning.

### Custom evaluation tooling

- Let customers define test cases, expected outcomes, and evaluators.
- Evaluate results, tool behavior, policy compliance, cost, and other customer-defined criteria.
- Compare agent versions against the same versioned cases and relevant context or tool fixtures.
- Run repeatable tests with controlled responses and integration tests against isolated resources.
- Assess production evidence without automatically repeating production side effects.
- Turn a production failure into a sanitized regression case.
- Link evaluation results to the agent versions and runs they assessed.

Evaluation execution should use the same runtime and permission enforcement as ordinary runs. The platform must distinguish fixture-based results, integration results, and assessments of production evidence.

### API, SDKs, CLI, and console

- Provide a versioned API for agents, deployments, runs, approvals, results, and evaluations.
- Describe public contracts so applications in different languages can integrate directly.
- Offer thin SDKs that simplify integration while keeping execution and authorization logic in Hudson.
- Provide a CLI for local development and routine operations.
- Provide a console for setup, permission review, testing, run inspection, approvals, and evaluation comparisons.
- Apply consistent authorization across every interface.

The accepted implementation language for the core is **Rust**. The initial API direction is HTTP and JSON with OpenAPI. SDK language choices and the console implementation remain open; see [decision 0001](implementation-decisions/0001-rust.md).

### Deployment and open-source distribution

- Support local development, self-hosting, and a managed cloud offering using the same public contracts.
- Document setup, configuration, required dependencies, upgrades, and recovery procedures.
- Include a useful self-hosted path through the core execution, security, observability, and evaluation flow.
- Make the distinction between development conveniences and production security guarantees explicit.
- Keep managed cloud operations separate where necessary without making the core product depend on private infrastructure.
- Document compatible Hudson Sandbox versions, configuration, and host requirements as part of the self-hosted execution path. Its current design targets Firecracker on Linux with KVM; development on other hosts must expose any execution limitations explicitly.

## Repository scope

Start with one public monorepo containing the runtime, default harness, security, observability, evaluations, SDKs, API, workers, console, CLI, documentation, integrations, and examples.

These are logical ownership boundaries. They do not require one service, process, package, or Rust crate per feature. The implementation layout should follow actual dependency and isolation needs.

Sandbox infrastructure lives outside this monorepo in [hudson-infinity/hudson-sandbox](https://github.com/hudson-infinity/hudson-sandbox). Hudson owns the integration adapter, authorization decisions, and mapping of sandbox operations and results into agent runs. The sandbox repository owns its execution service, host supervision, guest execution, and isolation design.

The README introduces the product, this document defines its goals and feature scope, and implementation decision records explain accepted technical choices. Detailed architecture documents should define the contracts and failure behavior needed to implement these goals.

## First milestone: one complete, recoverable agent flow

Use a concrete example such as investigating a support ticket, reading an order, and proposing a write that requires approval. The first milestone should demonstrate that:

1. A developer can start a local installation using the public repository's documentation.
2. An application in a language other than Rust can create a run through the public API.
3. A versioned agent uses the default harness and a connected tool with structured input and output.
4. An unauthorized operation is blocked and the denial is visible in the run record.
5. A permitted operation requiring approval pauses and resumes only after a valid decision.
6. The client can disconnect while the run continues or waits durably.
7. A worker can stop and restart without losing recorded progress or blindly repeating an uncertain write.
8. Cancellation and configured limits have observable outcomes.
9. The final record shows the actions, approvals, costs where available, results, and success assessment.
10. A failure can become a regression case that compares two agent versions through the same runtime.

This milestone establishes the shared execution model. It does not require every integration, deployment option, or advanced customization described above.

## Later expansion and scope boundaries

- **Multi-agent systems:** add delegation and coordination after the core execution model is reliable, with explicit permission and budget inheritance.
- **Research:** initially focus on runtime reliability, security, harness behavior, and evaluation quality. Broader research can follow demonstrated product needs.
- **Ecosystem:** grow through integrations, examples, reusable extensions, and contributors.
- **Enterprise features:** expand administration and organizational controls as concrete requirements emerge.

The initial scope does not include training foundation models, building a general business application for each customer, or developing a custom operating system or hypervisor. Sandbox implementation is maintained in the separate Hudson Sandbox repository. Its [current design](https://github.com/hudson-infinity/hudson-sandbox/blob/main/docs/implementation.md) selects Rust, Temporal, and Firecracker; integration details and validation remain outstanding.

Existing work from the separate `general-harness` project may offer a starting point, but it has not been assessed for Hudson. These goals do not assume that its implementation satisfies any requirement here.

## How we will measure progress

Track time to first useful run, permission and isolation test outcomes, recovery behavior under injected failures, completeness of execution evidence, evaluation reproducibility, and the effort required to self-host and upgrade.

Measure latency, throughput, memory, and execution cost against explicit workloads. Performance targets and service guarantees should follow those measurements; this document makes no untested scale or availability promises.

## Decisions still needed

- Core entities, lifecycle transitions, and harness extension contracts
- Durable execution, storage, scheduling, and recovery design
- Identity, policy, credential, and Hudson Sandbox integration contracts
- Model and tool adapters, worker protocol, and API compatibility policy
- Event schemas, artifact storage, retention, and evaluation contracts
- Initial SDKs, console technology, and packaging
- Open-source license and managed-cloud feature boundaries
- Final product and company naming

The accepted [Rust decision](implementation-decisions/0001-rust.md) and [external sandbox decision](implementation-decisions/0002-hudson-sandbox.md) are the starting points. Remaining choices should be recorded as they are made, without treating proposed features as shipped behavior.
