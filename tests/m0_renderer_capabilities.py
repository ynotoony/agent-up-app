#!/usr/bin/env python3
from pathlib import Path
import re
import sys

root = Path(__file__).resolve().parents[1]
src = root / "src"
forbidden = [
    r"@tauri-apps/plugin-fs",
    r"@tauri-apps/plugin-shell",
    r"@tauri-apps/plugin-http",
    r"@tauri-apps/plugin-process",
    r"@tauri-apps/plugin-stronghold",
    r"node:fs",
    r"node:child_process",
    r"node:net",
    r"from ['\"]fs['\"]",
    r"from ['\"]child_process['\"]",
    r"from ['\"]net['\"]",
    r"from ['\"]process['\"]",
]
files = list(src.rglob("*.ts")) + list(src.rglob("*.tsx")) + list(src.rglob("*.js"))
if not files:
    print("renderer source missing", file=sys.stderr)
    sys.exit(1)
violations = []
for path in files:
    text = path.read_text()
    for pattern in forbidden:
        if re.search(pattern, text):
            violations.append(f"{path}: {pattern}")

capability = (root / "src-tauri/capabilities/m0-default.json").read_text()
for needle in ["fs:", "shell:", "http:", "process:", "opener:", "core:os"]:
    if needle in capability:
        violations.append(f"capability grants {needle}")

permissions = (root / "src-tauri/permissions/m0.toml").read_text()
for command in [
    "scan_project",
    "preview_initialize",
    "initialize_project",
    "create_request",
    "load_project",
    "list_projects",
    "register_project",
    "rebind_project",
    "remove_agentup",
]:
    if command not in permissions:
        violations.append(f"missing command allow {command}")

if violations:
    print("capability violations:", file=sys.stderr)
    print("\n".join(violations), file=sys.stderr)
    sys.exit(1)
print("renderer capability static check ok")
print(f"scanned {len(files)} renderer files")
