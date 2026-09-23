# Managed harness capability delivery

The customer-defined capability goal is implemented and locally verified on
2026-09-23. Customers provide instructions, model selection, domain tools or MCP
bindings, skill packages, memory scope, budgets, and success criteria. Hudson owns
the execution loop and Temporal orchestration. Sandbox implementation and Langfuse
are excluded. No host-shell fallback was added.

## Requirement-by-requirement evidence

| Requirement | Issue | Authoritative implementation and exercised acceptance |
| --- | --- | --- |
| Bounded context and source retrieval | #1 | `context.rs`, harness context preparation, and `tests/context.rs`: a long actual loop completes under its request cap, reconstructs original output, preserves complete exchanges/provider metadata, rejects cross-actor/workspace/run reads, and reconnects to PostgreSQL. `tests/dependency_context.rs` additionally proves bounded prerequisite evidence, impossible-budget failure, and no artifact publication on stale/failed transitions. |
| MCP and portable skills | #2 | `adapters/mcp.rs`, `skills.rs`, `configured.rs`, `tests/mcp.rs`, `tests/skills.rs`: real JSON/SSE MCP initialization/discovery/call, pinned schema drift rejection, approval before remote calls, unknown writes not replayed, frozen package references with UTF-8 paging and containment. Credentials stay outside model requests/results. |
| Scoped persistent memory | #3 | `memory.rs`, `tests/memory.rs`, `tests/memory_runtime.rs`, `scripts/smoke_memory.py`: ranked bounded recall, provenance, corrections/deletion, immutable per-run snapshots, selected retention, actor validation even under permissive tool policy, PostgreSQL reconnect, and recall across separate worker processes with another scope excluded. |
| Adaptive teams | #4 | `coordination.rs`, `subagents.rs`, `tests/coordination.rs`, `tests/dependency_context.rs`, Temporal `tests/local_server.rs`: independent children overlap, a third waits for prerequisites, results/evidence persist, failed/cancelled dependencies stop before effects, parent cancellation propagates, repeated delegation and nested/root/child budgets are bounded. |
| Verified completion and evaluation | #5 | `verification.rs`, `dispatch.rs`, `evaluation.rs`, `tests/goals.rs`, `tests/evaluation.rs`: wrong values and unsupported claims fail; repair can complete; held-out criteria stay out of prompts; verifier-tool checks can require freshness; assessments reference recorded operation digests. `integrations/harbor` implements the actual pinned Harbor BaseAgent contract, three independently verified task types, matched-budget comparison and optional explicit-rate cost estimates. |
| Customer configuration and complete execution | #6 | `configured.rs`, Temporal `ExecutionClient`, `tests/capabilities.rs`: one real Temporal/PostgreSQL run exercises memory, lazy skills/resources, MCP, archival/retrieval, child delegation/join, failed-then-passed verification, evidence, retention and reopen without duplicate MCP execution. Existing process tests prove foreground/background attachment. Disabled capabilities and legacy single-agent rebuild have regressions. Real-estate and Python data examples execute customer HTTP tools with no customer-written orchestration. |

## Final validation

- `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 python3 scripts/check.py --database hudson_harness_test_20260921 --temporal` passed after the runtime review fixes: formatting, workspace tests including opt-in PostgreSQL cases, all six real Temporal tests, strict workspace Clippy, core without default features, binary builds, six example configs, and ten CLI/API/provider/recovery smoke scripts.
- The subsequently added `python3 scripts/smoke_real_estate.py` passed and is now included in the aggregate runner as its eleventh smoke. No Rust implementation changed after the aggregate pass.
- Seven Harbor adapter/verifier/report tests passed against the freshly built worker and actual installed Harbor classes.
- A final actual Harbor Docker smoke passed all three independent verifiers with no exceptions. Matched-budget comparison successfully consumed the earlier and final trial reports.
- Independent review fixes cover memory invocation identity, bounded skill resource reads, relative Harbor package paths, atomic large dependency artifacts, terminal-root delegation, and legacy single-agent migration. The added regressions passed in the aggregate run.

The Temporal and Harbor models were local protocol fixtures. Harbor's Docker smoke
deliberately uses reference-solution commands and synthetic usage counters. These
results establish integration and verifier behavior, not model quality or superiority.
Dollar estimates require supplied model rates and complete usage; unavailable cost
is null, not zero. No paid provider call, production deployment, hosted multi-tenant
service, or hosted CI result is claimed.

The loopback HTTP development server retains its local driver. Services using
Temporal embed the Rust execution client or use the Temporal CLI and worker.
Harbor supplies its own execution environment; its adapter currently benchmarks
one Hudson agent. The managed Hudson runtime separately supports configured teams.
