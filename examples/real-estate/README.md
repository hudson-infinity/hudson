# Bring a domain tool; Hudson runs the agent

`tools.py` is the customer's code: a `search_properties` function exposed over
HTTP. Its three property records are fictional. Replace that function with your
own inventory integration. No workflow or agent loop is implemented in this file.

`agent.json` supplies instructions, the tool schema, optional context/memory, and
budgets. Hudson chooses tool calls, validates them, checks permissions, manages
execution, and verifies the output contract. The contract checks shape; it does
not independently prove that every property claim is correct.

Start the tool with `python3 examples/real-estate/tools.py`. With PostgreSQL and
Temporal running as described in `docs/temporal.md`, start a worker:

```sh
cargo run -p hudson-temporal -- --config examples/real-estate/agent.json \
  --database hudson --namespace properties --task-queue properties worker
```

Then submit a foreground run:

```sh
cargo run -p hudson-temporal -- --config examples/real-estate/agent.json \
  --database hudson --namespace properties --task-queue properties \
  run --task "Find Austin properties under 450000 dollars" --request-key search-1
```

Add `--background` to receive a run ID immediately. Configure your model provider
credentials before starting. The example uses the default OpenAI provider; the
same tool works with an explicitly configured Anthropic or Gemini model.

Removing `context` disables archival tools. Removing `memory` disables recall and
retention. With memory enabled, only the selected `memory_note` string is retained
on successful completion, labeled as a model-produced outcome with run provenance.
Model-requested memory edits require approval. Scope is additionally isolated by
workspace and actor; use a separate actor or scope for each customer identity.
