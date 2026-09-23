# Customer agent and tool publication

Status: authenticated tool/agent publication, submission by revision, and dynamic
worker loading are implemented. See the [customer walkthrough](../examples/customer-api/README.md). This extends the authenticated
single-instance API. Existing startup configuration and operator commands remain
available. Hudson Sandbox remains a separate product; this work does not load
customer code or add a host shell.

## Customer flow

1. Define a tool's name, description and input/output schemas against an approved
   connection belonging to the customer's workspace.
2. Publish an agent revision with instructions, a permitted model profile, selected
   tool revisions and optional built-in capabilities.
3. Submit a task using that immutable revision. Hudson saves admission, schedules
   the run, executes tools, handles waits and records results.

A publication is configuration, not execution. Publishing never contacts a model
or tool service. Customers do not provide a loop, queue consumer or coordinator.
They can keep tool implementations in their existing HTTP/MCP service. A future
connected-worker transport can use the same definition and operation model.

## Authority boundary

Authentication supplies workspace and actor identity. Public request schemas must
not accept workspace/actor IDs, policy IDs, environment variable names, worker
queues, native registered-function keys, server filesystem paths or arbitrary
provider endpoints. Those fields in the existing trusted configuration format
must not become writable merely by exposing Configuration::load over HTTP.

Host-managed model profiles and connection records bind provider/transport,
credential references, allowed destinations, execution limits and minimum approval
requirements. A customer references a connection it owns. The host resolves it
before validation and pins its revision in the published definition. A connection
is not a way to borrow another workspace's secret or change a destination later.

Customer tool declarations can request stricter handling but cannot weaken a
connection's effect or approval policy. New externally implemented tools default
to write/approval handling unless a trusted binding establishes a read-only
contract. Labels and natural-language descriptions do not establish authority.
Built-in memory/context/skill capabilities derive their scope from authenticated
ownership. Customer limits may narrow host ceilings; they cannot increase them.

Customer-controlled HTTP destinations require dispatch-time egress enforcement,
including DNS rebinding protection and redirect/proxy behavior. Origin string
matching alone is insufficient. Until that is implemented, connection provisioning
remains a trusted installation operation; customer publication cannot introduce
arbitrary network destinations. Credentials stay in worker-side secret storage.

## Publication and versioning

Use dedicated public input types with unknown-field rejection. Do not deserialize
public bodies directly into the administrative Agent, Tool or Configuration types.
The public types select capabilities; the host constructs their effective policy.

A publication request carries a retry key. The service authenticates first,
validates size/schema/ownership and compiles the complete effective definition in
an isolated staging store. It then commits the immutable agent/tool definitions,
policies, transport/context/memory bindings, canonical configuration and publication
receipt in one transaction. A failed publication must leave no partial children,
tools or policy changes in the live store.

Identical retries return the original revision. A reused key with changed content
returns conflict. Existing revisions cannot be edited in place. A run pins the
agent, tool and connection revisions at admission; a later publication does not
change an active run. Canonical configuration contains references, never secrets.

The current configured builder registers definitions through several transactions.
It is suitable for trusted startup but cannot itself provide atomic HTTP publication.
Use its validation in staging, then add a narrow, validated bundle commit. The bundle
must exclude run state, operations, API credentials and unrelated workspace data.

## Execution handoff

The startup-configured Temporal worker only knows its original tree. Published
mode instead loads immutable revisions from storage. Storing agent rows alone is
insufficient; the reconstruction contract is:

- Persist a canonical effective configuration for each published root revision.
- Resolve it by the run's pinned reference under the authenticated workspace.
- Build the worker runtime with its own model/tool credentials and validate the
  same goal, shared budget, connection revision and capability bindings on recovery.
- Cache by immutable revision and scope, never by an unqualified agent name.
- Keep child scheduling, approvals and uncertain-effect reconciliation in the
  existing runtime/Temporal path. Customer configuration cannot pick another host.

The integration test executes a customer-defined tool through a separate restarted
worker after API restart. Existing startup-only operation remains supported.

## Acceptance coverage

- Publish a custom tool and an agent using it through authenticated HTTP; reject
  cross-workspace references and attempts to submit host-only fields.
- Invalid nested definitions leave the live store unchanged.
- Identical and concurrent retries converge on one immutable revision; changed
  content conflicts and old runs retain their original definitions.
- Publication succeeds without execution secrets and performs no external calls.
- A separate restarted worker reconstructs the revision, calls the customer tool
  through runtime authorization, and records the result exactly as existing runs do.
- Stricter customer limits apply; requests cannot weaken host approval or scope.
- Unknown external writes remain unresolved until evidence-based reconciliation.

Multi-workspace routing, public connection provisioning, browser sessions and a
self-service administration UI are separate follow-ups. They must not be implied
by successfully publishing metadata in a single configured workspace.

## Trusted storage foundation

`Store::publish_configuration` validates the complete admission tree without
execution credentials, checks it against a private snapshot of current state, and
commits definitions, bindings, a portable configuration, and a retry receipt in one
transaction. Existing budgets retain their usage. Failed nested definitions leave
no partial publication. `Store::published_configuration` restores the pinned tree
without reopening skill files; loaded skill instructions and package resources are
stored with the revision.

These methods are for trusted installation code. They do not accept HTTP requests
or make raw administrative configuration safe for customers. A revision that already
has legacy runs must use a new version for its first publication, because the old
runs did not pin the persisted configuration. Retries of an existing publication
remain valid after runs start.

Validation includes immutable retries, ownership, nested rollback, preservation of
live state, deleted skill sources, and concurrent PostgreSQL publication followed
by reconnection and run reconstruction. Dynamic worker reconstruction is covered separately by Temporal integration tests.
Public request validation and API/worker restart are exercised by
`crates/hudson-temporal/tests/publication_api.rs` and `scripts/smoke_publication.py`.

## Public API and host catalog

`--catalog` requires PostgreSQL, bearer authentication, and a Temporal queue.
`GET /capabilities` exposes approved model/connection references and policy limits,
without credential names or destinations. Customers publish tools with `POST /tools`,
then select those immutable tool revisions in `POST /agents`. Definition bodies
reject unknown fields recursively. `GET /tools/{name}/versions/{version}` and the
corresponding agent route return only the public definition.

Model and connection versions are installed atomically and cannot change in place.
A connection selects one exact HTTP endpoint or MCP method. Its effect and approval
rules form minimum requirements. A tool may impose stricter handling. New agent
publications can only select capabilities present in the current host catalog;
existing published agents retain their effective snapshot. Removing an entry from
the catalog does not revoke existing agents; explicit runtime policy revocation is
still an operator action.

`POST /runs` in catalog mode requires `agent_ref`, `request_key`, and `input`.
Each new root submission gets a distinct durable shared budget, derived from the
authenticated owner, agent revision and retry key. Children use that root budget.
Admission commits the budget and run together; invalid input leaves neither behind.
Retries retain counters, and workers reconstruct the same binding after restart.
Publication retries return the original effective defaults even if the operator
subsequently changes catalog limits or removes a connection. New revisions must
use the currently approved capabilities. Nested agents have their own immutable
name/version identities; changing a child also requires a new child version.
The execution cache is bounded to 64 root submissions.
