#!/usr/bin/env python3
from pathlib import Path
import json
import sys

root = Path(__file__).resolve().parents[1]
schemas = sorted((root / "schemas").glob("*.json"))
if not schemas:
    print("no schema files found", file=sys.stderr)
    sys.exit(1)
for path in schemas:
    with path.open() as handle:
        json.load(handle)
    print(f"valid JSON: {path.relative_to(root)}")
print("schema parse ok")
