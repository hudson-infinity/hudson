# Customer tools and portable skills

Hudson accepts customer MCP tools through its normal registered-tool boundary.
The model never connects directly to a server. Runtime policy admission, approval,
budget accounting and operation intent precede every actual tool invocation.

## MCP bindings

`hudson_core::adapters::mcp::McpServerConfig` describes an explicit HTTPS endpoint
(loopback HTTP is allowed for local development), an optional environment-variable
name for bearer authentication, and selected, pinned tools:

```json
{
  "endpoint": "https://tools.example.com/mcp",
  "bearer_env": "CUSTOMER_MCP_TOKEN",
  "timeout_seconds": 60,
  "max_response_bytes": 1048576,
  "tools": [{
    "name": "search_listings",
    "remote_name": "search",
    "description": "Search our property inventory",
    "input_schema": {
      "type": "object",
      "properties": {"city": {"type": "string"}},
      "required": ["city"],
      "additionalProperties": false
    },
    "effect": "read"
  }]
}
```

Call `discover_tools(&config)` during authoring to obtain server tool schemas.
The customer selects tools and assigns read/write effects. Server annotations
cannot grant permissions. Call `register_tools(&config, &mut registry, workspace,
policy)` at startup, then publish the returned definitions with the same APIs as
other tools. Registration is offline and never invokes tools.

Every admitted call initializes a session using the official Rust MCP SDK,
discovers the server's paginated catalog, verifies the exact selected input schema,
and calls the pinned remote name. A changed schema fails before tool dispatch.
Endpoint, environment-variable name, limits and bindings contribute to the immutable
execution key. Credential values remain outside definitions and error messages.

The adapter supports Streamable HTTP JSON and POST SSE responses, bounded before
parsing; discovery is limited to 16 pages/128 tools. Each connection/discovery and
call phase has a timeout. The adapter does not expose stdio, server-initiated
sampling, elicitation, prompts, resource reads, or unsolicited GET SSE streams.
Only explicitly configured tool calls are available to the model.

After dispatch, transport failures, timeouts, oversized results and remote tool
errors preserve an Unknown outcome. They are never automatically replayed, including
expired sessions. `hudson/operation_id` request metadata provides correlation;
it is not a claim that arbitrary MCP servers support idempotency. The existing
operator reconciliation workflow applies to uncertain writes.

## Portable skills

`SkillPackage::load(directory)` reads an Agent Skills package:

```text
property-analysis/
  SKILL.md
  references/metrics.md
  assets/report-template.md
  scripts/example.py
```

```markdown
---
name: property-analysis
description: Compare properties using the company's valuation rules.
license: MIT
---
Read references/metrics.md before calculating a comparison.
```

Combine packages with existing plain Markdown/inline skills using
`SkillCatalog::with_packages(skills, packages)`, then register the catalog.
The model initially sees only names/descriptions. `load_skill` loads instructions
and lists available resource paths. `load_skill` with `name` and optional `resource`
returns a specific frozen resource. Reads never access the filesystem during a run.

Package names must match their directory; SKILL.md is limited to 32 KiB, each
UTF-8 resource to 128 KiB, and each package to 128 resources/2 MiB. Symlinks and
nonregular resources are rejected, nesting is bounded, and traversal paths cannot
resolve. Scripts are readable source data only: Hudson does not execute them.
Optional frontmatter such as `allowed-tools` does not grant tool permissions.
Changing resource content changes the catalog digest, while existing loaded
catalogs continue serving the original bytes. Binary resources are not supported.

The package layout follows https://agentskills.io/specification and MCP uses
https://github.com/modelcontextprotocol/rust-sdk. No upstream project code is copied.
