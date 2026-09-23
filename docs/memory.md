# Scoped persistent memory

Memory is opt-in and belongs to `(workspace, actor, scope)`. The host chooses the scope and actor when registering tools; model arguments cannot widen them. Two agents using the same actor and scope can share records across runs. Different actors, workspaces, or scopes cannot retrieve, replace, or delete each other's records.

`memory::register_tools(registry, store, actor, scope, read_policy, write_policy)` returns three ordinary Tool definitions: `recall_memory` (Read), `retain_memory` (Write), and `delete_memory` (Write). Publish these definitions and grant their policies like any other tool. Write policy may require approval. Operation UUIDs provide retain idempotency. Ambiguous database commit errors are reported as unknown writes.

Records include creation/update timestamps, provenance references, optional owned run and operation IDs, and an explicit kind: `user_fact`, `inference`, or `successful_outcome`. A successful outcome requires an owned completed run. Completion does not establish the semantic truth of its content. Treat all retrieved content as untrusted data, never as system instructions. External references are attribution supplied by the caller; they are not automatically fetched or verified.

Retain with `supersedes` to atomically replace an active record. Recall excludes replaced and deleted records. Delete tombstones the record and erases its stored text and sources; it does not erase prior run transcripts, audit logs, or database backups. Retention is selected explicitly rather than saving every final answer.

Recall ranks overlapping normalized words using inverse document frequency and document length normalization. It handles case, punctuation, stop words, and basic plural normalization. It is a useful lexical baseline, not embeddings or semantic synonym search. Queries are at most 8 KiB; responses contain at most 20 records and 32 KiB of serialized records. Facts contain at most 8 KiB and 16 source references. Namespace storage uses Hudson's existing transactional PostgreSQL document; it is not a large-scale search index.

Runtime integration: call `store.recall_memory(actor, scope, input_text, limit)` before constructing initial context, retain full provenance and kind labels, and include only what fits the run context budget. For selected completed outputs, call `retain_memory` with a deterministic run-based request key and `SuccessfulOutcome` provenance. All tool-mediated writes continue through ordinary runtime admission. Direct Store methods are trusted host APIs, like other Store configuration methods.

Existing database documents deserialize with an empty memory collection. In-memory and PostgreSQL stores share the same validation and transaction code. Focused tests cover corrections, actor/workspace/scope isolation, registration effects, legacy serialization, reconnect persistence, and retrieval by a later run of a completed prior run's outcome.

## Automatic runtime integration

Before submitting runs, call `store.bind_memory(workspace, agent_ref, Some(MemoryConfig { scope, recall_limit, retain_pointer }))`. Binding `None` disables memory. Bindings cannot change for an existing agent version, and memory cannot be enabled after that version already has runs. Publish a new version to change behavior.

The shared Runtime snapshots relevant records on first execution, persists the snapshot, and includes it with an explicit untrusted-data label in every request's instructions. The snapshot takes no more than a quarter of the run's context byte budget; normal context validation still applies. Corrections affect later runs, while an in-flight run's snapshot remains stable across restart.

`retain_pointer: Some("/memory")` explicitly selects a string in the completed JSON output. Completion and retention commit atomically. Missing, blank, non-string, or oversized selections are omitted. `None` never retains final output automatically; the root pointer is forbidden. This host configuration authorizes the selected automatic memory write, independently of the approval policy for model-invoked memory tools. Retained outcomes include source run provenance and remain unverified claims.

Memory tool invocation also verifies the persisted operation is currently running, matches the exact tool and arguments, and belongs to the configured actor. Granting a memory tool's policy to another actor cannot expose the original actor's private scope. Unadmitted, altered, or terminal-operation invocations are rejected.
