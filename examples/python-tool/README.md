# Bring a Python tool to Hudson

This example connects a customer-owned Python function to the ordinary Hudson
loop. Python calculates the statistics; the model selects the tool and reports
its result. There are no Python package dependencies.

Start the tool service in one terminal:

```sh
python3 examples/python-tool/server.py
```

With `OPENAI_API_KEY` set, run the agent from another terminal:

```sh
cargo run --locked -p hudson-worker -- --config examples/python-tool/agent.json \
  --input-file examples/data-task.json
```

For `[10, 20, 30]`, the expected output is
`{"count":3,"mean":20.0,"minimum":10,"maximum":30}`. The example caps the run at
two model calls and each response at 256 output tokens. The schema checks the
output shape; it does not independently prove the model copied the tool correctly.

To use another provider, set `provider`, `model`, and its credential environment
variable in the configuration as described in the development guide. To keep the
run across processes, add the normal `--database` and `--namespace` options.

The tool contract is a POST of its argument JSON to a configured endpoint,
returning result JSON with a successful HTTP status. Hudson sends a stable
`Idempotency-Key` identifying the operation. This tool is read-only; a service
that performs writes must persist its own deduplication records and receipts.

The service binds only to loopback and runs in your Python process. It is an
example local integration, not a sandbox or hosted endpoint. Replace the
`statistics` function with your business logic and update its schemas; the
Hudson loop does not change.
