# Public sandbox SDK qualification

Contributes to Hudson #17 using `sandbox-client` 0.0.0 pinned to public sandbox
commit `aa5cef5c51b7c735e6ede82d2c4f6e90f4f4f119`. This crate does not dispatch
mutations, enable Sandbox tools, execute a VM, or attach completion to Hudson runs.

`PreparedBody` serializes the SDK's typed create/execute/destroy models. Persist
`canonical_json()` before any future authorized send. `from_canonical_json()`
rejects serialization changes, including omitted defaults, whitespace, field
order, and unknown fields; a future SDK send must reproduce the persisted bytes.
Absolute execute deadlines are preserved. Command/image/policy selection must
come from an operator-approved mapping, never inferred from a package name.

`QualificationObserver::inspect_unverified()` makes one authenticated HTTPS GET
for a persisted original operation ID. It rejects unbound admission plans and
limits the serialized observation to the caller's bound (at most 64 KiB).
Mismatched operation/sandbox/kind, unknown and expired receipts require
reconciliation and suppress result/error release. The probe never resubmits,
cancels or cleans up. The caller must treat backend values as sensitive and
workload controlled; probe types intentionally lack Debug.

**Probe decisions are non-authoritative.** The pinned SDK does not expose its
actual canonical origin and project, so this crate cannot compare the constructed
client identity with `Binding.profile_fingerprint`. `observe_binding()` fails
closed before any network request. Do not write probe results as Store completion
or cleanup evidence. A future public SDK identity accessor and a durable verified
profile comparison are prerequisites for authoritative observation or dispatch.
SDK configuration retains its private-file, HTTPS/trusted-CA, Bearer credential,
no-proxy, no-redirect and no-retry checks; there is no insecure TLS switch.

Tests use the real SDK against a local HTTPS server and generated trusted test CA.
They qualify exact mutation request bytes/keys/auth headers and 202 shapes, plus
read-only receipt semantics. Direct SDK mutation calls exist only in fixtures;
202 acceptance is not completion. No test demonstrates runtime authorization,
network policy enforcement, guest execution, Temporal integration or Linux/KVM.

Run `cargo test --locked -p hudson-sandbox-transport` from the repository root.
