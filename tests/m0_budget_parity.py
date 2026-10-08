#!/usr/bin/env python3
# EXEC-001 §4 / development-plan §23: the budget table and the numbers the code
# enforces must be the same numbers. This check exists because a budget that
# lives only in prose is a budget nobody enforces - which is exactly what
# happened to two rows of this table before W7-22.
from pathlib import Path
import re
import sys

root = Path(__file__).resolve().parents[1]
plan = (root / "docs/execution-plan.md").read_text()
budget = (root / "src-tauri/src/resource_budget.rs").read_text()

# (name in Rust, expected expression, prose the plan must contain)
rows = [
    ("MAX_SCAN_FILES", "50_000", "| 扫描文件数 | 50_000 |"),
    ("MAX_ATTACHMENT_BYTES", "10 * 1024 * 1024", "| 附件单文件 | 10 MiB |"),
    (
        "MAX_REQUEST_ATTACHMENT_BYTES",
        "100 * 1024 * 1024",
        "| 单需求附件总大小 | 100 MiB |",
    ),
    ("MAX_DISCUSSIONS_PER_REQUEST", "500", "| 单需求评论数 | 500 |"),
    ("SCAN_TARGET_MS", "5_000", "首次扫描（1 万文件） | 目标 ≤ 5s"),
    ("SCAN_HARD_MS", "30_000", "硬超时 30s"),
    ("AGENTUP_WARN_BYTES", "2 * 1024 * 1024 * 1024", "`.agentup/` 体积 | 警告 2 GiB"),
]

problems = []
for name, expression, prose in rows:
    match = re.search(rf"pub const {name}: \w+ = {re.escape(expression)};", budget)
    if not match:
        problems.append(f"{name} is not `{expression}` in resource_budget.rs")
    if prose not in plan:
        problems.append(f"execution-plan.md no longer says: {prose}")

if problems:
    print("budget parity problems:", file=sys.stderr)
    print("\n".join(problems), file=sys.stderr)
    sys.exit(1)
print(f"budget parity ok: {len(rows)} budgets match the plan table")
