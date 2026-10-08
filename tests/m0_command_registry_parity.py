#!/usr/bin/env python3
"""The command surface lives in nine places; keep them equal.

An external review (2026-09-16) found `COMMANDS` in runtime.rs missing `decide`
while both schemas, permissions and the registry had it, and no automated check
existed - so it had been wrong for several commits while the commit message
claimed all seven registries were checked. This is that check.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def registry_names() -> list[str]:
    data = json.loads((ROOT / "schemas/command-registry.json").read_text())
    return [entry["name"] for entry in data["commands"]]


def dotted(data: dict, pointer: str):
    node = data
    for part in pointer.split("."):
        node = node[part] if isinstance(node, dict) else node
    return node


def permission_allowlist() -> list[str]:
    text = (ROOT / "src-tauri/permissions/m0.toml").read_text()
    return re.findall(r'^\s+"([a-z_]+)",?$', text, re.M)


def runtime_commands() -> list[str]:
    text = (ROOT / "src-tauri/src/runtime.rs").read_text()
    match = re.search(r"const COMMANDS: \[&str; \d+\] = \[(.*?)\];", text, re.S)
    return re.findall(r'"([a-z_]+)"', match.group(1)) if match else []


def handler_names() -> list[str]:
    text = (ROOT / "src-tauri/src/lib.rs").read_text()
    block = re.search(r"generate_handler!\[(.*?)\]", text, re.S)
    return re.findall(r"\b([a-z_]+)\b", block.group(1)) if block else []


def python_check_list() -> list[str]:
    text = (ROOT / "tests/m0_renderer_capabilities.py").read_text()
    block = text.split("for command in [", 1)[1].split("]:", 1)[0]
    return re.findall(r'"([a-z_]+)"', block)


def registry_schema() -> dict:
    return json.loads((ROOT / "schemas/command-registry.schema.json").read_text())


def main() -> int:
    names = set(registry_names())
    schema = registry_schema()
    sources = {
        "command-registry.schema.json[name enum]": set(
            schema["$defs"]["command"]["properties"]["name"]["enum"]
        ),
        "command-registry.schema.json[contains]": {
            block["contains"]["properties"]["name"]["const"]
            for block in schema["properties"]["commands"]["allOf"]
        },
        "command-registry.schema.json[if/then]": {
            block["if"]["properties"]["name"]["const"]
            for block in schema["$defs"]["command"]["allOf"]
        },
        "command-result.schema.json[enum]": set(
            json.loads((ROOT / "schemas/command-result.schema.json").read_text())[
                "properties"
            ]["command"]["enum"]
        ),
        "command-error.schema.json[enum]": set(
            json.loads((ROOT / "schemas/command-error.schema.json").read_text())[
                "properties"
            ]["command"]["enum"]
        ),
        "permissions/m0.toml": set(permission_allowlist()),
        "runtime.rs COMMANDS": set(runtime_commands()),
        "lib.rs handlers": set(handler_names()),
        "m0_renderer_capabilities.py": set(python_check_list()),
    }
    problems = []
    for label, values in sources.items():
        missing = sorted(names - values)
        extra = sorted(values - names)
        if missing:
            problems.append(f"{label}: missing {missing}")
        if extra:
            problems.append(f"{label}: unexpected {extra}")
    if problems:
        print("command registry parity problems:", file=sys.stderr)
        print("\n".join(problems), file=sys.stderr)
        return 1
    print(
        f"command registry parity ok: {len(names)} commands match across "
        f"{len(sources)} places"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
