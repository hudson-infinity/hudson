# Sandbox adapter recovery contract

Tracked by Hudson #17 and hudson-sandbox #112. Public HTTP schema baseline:
`hudson-infinity/hudson-sandbox` commit `aa5cef5c51b7c735e6ede82d2c4f6e90f4f4f119`,
`api/openapi.json` and generated `crates/sandbox-client/src/models.rs`.

The backend already has an authenticated HTTP API and generated Rust client.
Hudson must reuse that transport rather than invent another guest RPC protocol.
Create, execute, destroy and cancellation are independent admitted operations.
`Idempotency-Key` identifies a request; it does not prove exactly-once guest effects.

`adapters::sandbox_recovery` is a transport-independent decision primitive for a
future persisted activity, not enabled sandbox execution. Existing dispatch still
rejects Sandbox tools. The primitive does not establish isolation or authority.

An operator explicitly binds a package name to a pinned image and fixed command,
working directory and output limit. Agent arguments must never become an executable,
shell string or arbitrary argv. Resource/network admission and credential scope must
be enforced by the backend before enabling this mapping. Current create schema has
image/resources/correlation fields and no client network-policy field; do not claim
per-run network isolation from that schema alone.

Persist run/workspace identity, immutable request body, idempotency key and admission
intent before network dispatch. Persist attempted=true before sending. Save the
returned sandbox and operation identities atomically with run state. After restart,
inspect the original operation. A lost admission response without an operation ID
requires reconciliation using backend admission evidence; this primitive deliberately
refuses to resubmit. Never rotate a key to recover an uncertain execute.

Receipts must match the recorded operation, sandbox and operation kind. Unknown,
expired, missing or unrecognized outcomes require explicit reconciliation. A successful
execute receipt is command evidence, not proof of agent task success. Retrieve bounded
outputs on the same operation, preserve truncation/simulated/guest-reported evidence,
and authorize result ownership using the run's workspace.

Cancellation must inspect both its own receipt and target execution evidence; an
accepted cancel is not termination proof. Destroy follows cancellation/reconciliation
and retains its own stable key and receipt until cleanup is confirmed. Neither retry
nor timeout may silently create a new sandbox or host process.

Remaining integration: durable store/activity wiring, pinned SDK dependency, secret
redaction/auth setup, explicit operator registration and enforced policy checks,
output/result attachment, cancellation targets, cleanup evidence, and local mock
acceptance followed by real Linux/KVM validation. The JSON roundtrip tests represent serialized recovery plans, not implemented durable storage. The unit tests cover restart,
lost response, mismatched receipts and expired/unknown status without VM claims.

The receipt's omitted `response_expired` means false in the pinned OpenAPI schema;
this matches the generated SDK default. The receipt cannot bind a Hudson workspace
or run because those fields are not in the public operation response. The future
store must enforce that binding and immutable request ownership; public Rust plan
fields are serialization data, not an authority boundary. Structural validation
rejects empty identities/keys and execute/destroy plans without a sandbox identity.
Reproduce the selected schema with `scripts/extract-sandbox-operation-schema.py`
against the verified pinned repository's `api/openapi.json`.

Public v1 mutation keys are project-wide, 16–128 ASCII alphanumeric or `._-`.
A 202 AdmittedResponse binds operation identities only; its status never substitutes
for a completion receipt. A 409 conflict is not admission. A 410 expired response
requires reconciliation, even when it retains an operation ID. The admission binder
rejects other HTTP statuses and different existing identities/status URLs. Persist
exact request bytes including the absolute command deadline before dispatch.
