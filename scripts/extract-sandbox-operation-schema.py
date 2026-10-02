#!/usr/bin/env python3
"""Extract the receipt schema from sandbox aa5cef5's api/openapi.json.

Usage: python3 scripts/extract-sandbox-operation-schema.py /path/to/api/openapi.json
The caller must verify the input revision against docs/sandbox-adapter-contract.md.
"""
import json
import pathlib
import sys

source = json.loads(pathlib.Path(sys.argv[1]).read_text())
schemas = source["components"]["schemas"]
needed = set()


def collect(value):
    if isinstance(value, dict):
        if "$ref" in value:
            key = value["$ref"].split("/")[-1]
            if key not in needed:
                needed.add(key)
                collect(schemas[key])
        for child in value.values():
            collect(child)
    elif isinstance(value, list):
        for child in value:
            collect(child)


collect({"$ref": "#/components/schemas/OperationBody"})
collect({"$ref": "#/components/schemas/AdmittedResponse"})
output = {"$ref": "#/components/schemas/OperationBody",
          "components": {"schemas": {key: schemas[key] for key in sorted(needed)}}}
path = pathlib.Path(__file__).resolve().parents[1] / "crates/hudson-core/tests/fixtures/sandbox-operation-schema.json"
path.write_text(json.dumps(output, indent=2))
