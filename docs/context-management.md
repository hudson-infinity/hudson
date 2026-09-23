# Bounded context and source artifacts

Context management is opt-in per immutable agent version. Register the context reader using `hudson_core::context::register`, publish it and grant its read policy, bind it under `read_context_artifact`, then call `store.bind_context_policy(workspace, agent_ref, Some(&policy))`. Bind `None` when disabled. Changing a binding requires a new agent version.

`ContextPolicy` defaults:

| Field | Default | Meaning |
|---|---:|---|
| `offload_bytes` | 8192 | Successful tool values larger than this become source artifacts |
| `recent_exchanges` | 2 | Minimum number of complete recent exchanges to retain |
| `excerpt_bytes` | 1024 | Maximum verbatim preview/index size |
| `page_bytes` | 1024 | Source reader page size |

The runtime preserves the original task and instructions. When a checkpoint or model request would exceed the configured byte budget, it archives oldest complete exchanges until the request fits. A parallel tool-call batch and its results stay together. Retained calls keep their provider continuation metadata unchanged. A pending exchange is never split. If the fixed instructions/tools, original task, pending arguments, or minimum recent context cannot fit, the run still fails explicitly rather than silently discarding them.

Large successful tool outputs and large prerequisite-result sets become immutable digest-addressed artifacts before reaching the model. The model receives the artifact ID, original size and explicitly partial verbatim preview. Original operation results remain intact. Older history gets an extractive index containing verbatim argument/result/text excerpts; it is not a semantic summary, and does not claim omitted details have been preserved in the preview. The original exchange and its link to previous archived history are retrievable.

`read_context_artifact({"artifact_id":"...","offset":0})` returns JSON text pages. Concatenate `text` pages using `next_offset`; offsets are UTF-8 byte offsets. Conversation artifacts contain `messages` and `previous_artifact`; tool-output and dependency-result artifacts contain `value`. Dependency artifacts are scoped to the receiving run and preserve source run IDs and assessments. Follow the previous-artifact chain for earlier exchanges. The agent should retrieve sources before relying on omitted details. Retrieval itself goes through the normal tool permissions, operation, usage and persistence path.

Artifacts and the checkpoint referencing them commit in the same transaction. PostgreSQL reconnect preserves both. The reader derives workspace, actor and run from the persisted invoking operation; model arguments cannot select another run or workspace. Sibling runs cannot read each other's context artifacts. Artifacts are retained alongside the existing run/operation document, so this implementation does not reduce database storage or introduce an external object store.

Validation: focused tests drive a long run under a 6500-byte request limit, retrieve and reconstruct a 14KB result through ordinary tool calls, verify source metadata and complete archive chains, reject access from another workspace/actor/run, and reconnect PostgreSQL midway through execution. No paid model calls or hidden summarization calls are used.
