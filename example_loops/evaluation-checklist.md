# Evaluation before implementation

This checklist describes work still to do against real candidate integrations. Running the offline examples does not satisfy it.

Use a named upstream release or commit, a pinned toolchain, a recorded agent version, and fixed scripted model responses first. Add live-model tests separately, with provider and usage details, because their variability answers different questions.

## Required integration cases

| Case | Evidence required |
| --- | --- |
| Ordinary tool cycle | Correct tool-call/result IDs, bounded continuation, and explicit final outcome |
| Malformed or unknown tool | No dispatch; structured failure that preserves a valid model conversation |
| Tool authorization | Denied tool and resource access never reach any executor, including parallel and child paths |
| Bound approval | Altered arguments, wrong workspace, unauthorized approver, expiry, and revocation cannot reuse approval |
| Real approval suspension | Worker exits; a different process loads the checkpoint and resumes the same pending operation |
| Lost acknowledgement | Known completion is reused; an uncertain non-idempotent action is reconciled, not blindly retried |
| Concurrent admission | Parallel operations and child runs cannot each reserve the same remaining budget |
| Cancellation | New work stops; active executor cancellation is acknowledged or remains visibly unresolved |
| Client disconnect | The run continues independently and a new client can retrieve ordered durable evidence |
| Child execution | Filtered context, restricted permissions, explicit budget, linked receipts, and defined parent cancellation |
| Partial batch failure | Each call has an outcome; retries do not repeat known completed writes |
| Context exhaustion | Pending actions and required instructions survive; oversized required context fails explicitly |
| Partial model stream | A broken stream does not cause execution of incomplete arguments or silently duplicate a call |
| Repeated ineffective actions | Turn, cost, time, and retry limits terminate without falsely reporting success |
| Run-version migration | Persisted checkpoints from a supported prior version can be resumed or rejected clearly |
| Sandbox integration | Hosted code cannot reach credentials, other tenants, or privileged control paths directly |
| Success assessment | Execution completion is distinct from a passed, failed, or unavailable evaluator |

## Compare candidates fairly

Record setup effort, adapter complexity, required forks, exposed control points, state migration burden, and failure outcomes. Measure local overhead separately from model and tool latency. Do not invent a throughput or cost ranking before running the same workload under comparable conditions.

For a Rust dependency, check whether the reviewed API is released, its dependency licenses and advisories, feature selection, cancellation behavior, and maintenance cost. Python and TypeScript projects remain design references unless a separate implementation decision changes Hudson's language boundary.

## Evidence produced by this collection

- Pinned source paths and hashes: [sources.json](sources.json).
- Targeted source-reading notes: [references](references/).
- Original dependency-free examples with local assertions: [examples](examples/).
- No upstream build results, live-provider results, distributed recovery proof, security certification, or benchmark ranking.

Acceptance of a library should cite the actual integration results and remaining limitations in a new implementation decision record.
