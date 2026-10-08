#!/usr/bin/env python3
from pathlib import Path
import json
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
    "preview_remove_agentup",
    "post_discussion",
    "add_attachment",
    "load_request_thread",
    "put_scope",
    "diff_scope",
    "put_task",
    "set_task_state",
    "load_board",
    "set_agent_recording",
    "start_implement",
    "start_review",
    "commit_agent_task",
    "publish_result",
    "load_results",
    "accept_result",
    "submit_result_feedback",
    "compare_results",
    "read_attachment",
    "read_attachment_thumbnail",
    "load_decisions",
    "search_requests",
    "read_logs",
    "decide",
    "put_understanding",
    "confirm_understanding",
    "load_understandings",
    "discover_agents",
    "list_agents",
    "upsert_agent",
    "remove_agent",
    "get_agent_settings",
    "put_agent_settings",
    "load_workspace",
    "import_run",
    "load_governance_tickets",
    "open_ticket_agent",
    "assess_governance",
    "install_governance_files",
    "init_empty_repo",
]:
    if command not in permissions:
        violations.append(f"missing command allow {command}")

# The renderer may only call commands that are granted in the capability set.
registry = json.loads((root / "schemas/command-registry.json").read_text())
granted = {entry["name"] for entry in registry["commands"]}
call = re.compile(r'\b(?:invoke|run)(?:<[^>]*>)?\(\s*"([a-z_]+)"')
for path in files:
    for name in call.findall(path.read_text()):
        if name not in granted:
            violations.append(f"{path}: calls ungranted command {name}")

if violations:
    print("capability violations:", file=sys.stderr)
    print("\n".join(violations), file=sys.stderr)
    sys.exit(1)
print("renderer capability static check ok")
print(f"scanned {len(files)} renderer files")
