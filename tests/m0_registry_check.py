#!/usr/bin/env python3
r"""治理登记面守卫：docs/agent/artifacts.yaml ↔ 仓库现实的一致性检查（秒级、只读、零依赖）。

Input: 仓库根目录（argv[1]，缺省取本文件位置上一级）。
Output: 逐条 FAIL 行（可定位）+ 结尾汇总行；发现任一失败 exit 1，全部通过 exit 0。
Pos: 非代理执行器——R-DP-007「登记与仓库文件清单可对照」的机器承载（2026-09-22 治理守卫批）；
     经 scripts/hooks/pre-commit 在每次提交时运行，并接入 npm test 链。行式解析按登记文件的
     机器格式契约（`  - id: ` 切块、四空格字段），格式漂移本身即失败（fail-closed）。

检查项：
  1. 每条目恰含十三字段（无缺失、无未知字段——含已退役的 status 回归）、id 唯一；
  2. 每条 path 在盘存在；
  3. 反向覆盖：docs/（issues/ 除外——票本体由检查 4/5 经索引承载，新代 JSON 票不逐票登记）、
     schemas/、scripts/ 三目录与根 AGENTS.md 下每个非隐藏文件均被登记；
  4. docs/issues/index.json：id 集合 == 票文件集合、一条目一行、status 取值合法；
  5. docs/issues/README.md 状态列锚点（`任务票 <NN>；\`状态\``）与索引逐票一致；
  6. docs/changes.jsonl（键序 date,kind,scope,decision,evidence_ref）与
     docs/agent/generation-manifest.jsonl 逐行可解析。
"""
import json
import re
import sys
from pathlib import Path

FIELDS = ["id", "path", "kind", "authority", "owner", "lifecycle", "trigger",
          "read_when", "sync_on", "depends_on", "generated_from", "platform", "update_policy"]
STATUS_RE = re.compile(r"^(ready|in_progress|blocked(:[ a-zA-Z0-9-]+)?|review_ready|review_pass|review_fail|done|superseded)$")
ENTRY_RE = re.compile(r"^  - id: (\S+)\s*$")
FIELD_RE = re.compile(r"^    ([a-z_]+): (.*)$")
CHANGE_KEYS = ["date", "kind", "scope", "decision", "evidence_ref"]
MANIFEST_KEYS = {"date", "target", "source", "confirmation", "version", "restore"}
TICKET_RE = re.compile(r"^\d{2,}-[a-z0-9-]+\.(?:md|json)$")
README_ROW_RE = re.compile(r"^\| `([0-9]{2,}-[a-z0-9-]+\.(?:md|json))` \| 任务票 ([0-9]{2})；`([^`]+)`")

fails = []


def fail(msg):
    fails.append(msg)


def parse_registry(text):
    entries, cur = [], None
    in_body = False
    for no, raw in enumerate(text.splitlines(), 1):
        if not in_body:
            if raw == "artifacts:":
                in_body = True
            continue
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        m = ENTRY_RE.match(raw)
        if m:
            cur = {"_line": no, "id": m.group(1)}
            entries.append(cur)
            continue
        m = FIELD_RE.match(raw)
        if m and cur is not None:
            key, val = m.group(1), m.group(2).strip()
            if key in cur:
                fail(f"artifacts.yaml:{no} 字段 {key} 在条目 {cur['id']} 中重复")
            else:
                cur[key] = val
            continue
        if cur is not None:
            fail(f"artifacts.yaml:{no} 不合机器格式契约（既非条目起始也非四空格字段）: {raw[:60]}")
    return entries


def check_entries(entries):
    ids = set()
    for e in entries:
        eid = e.get("id", f"<line {e['_line']}>")
        if eid in ids:
            fail(f"artifacts.yaml 条目 id 重复: {eid}")
        ids.add(eid)
        for k in FIELDS:
            if k not in e:
                fail(f"artifacts.yaml 条目 {eid} 缺字段 {k}（行 {e['_line']}）")
        for k in e:
            if k not in FIELDS and k != "_line":
                fail(f"artifacts.yaml 条目 {eid} 出现未知字段 {k}（十三字段之外的回归，含已退役 status）")
    return ids


def check_paths(root, entries):
    n = 0
    for e in entries:
        rel = e.get("path")
        if not rel:
            continue
        if not (root / rel).is_file():
            fail(f"登记 path 不在盘上: {rel}（条目 {e.get('id')}）")
        n += 1
    return n


def tracked_files(root):
    import subprocess
    r = subprocess.run(["git", "-C", str(root), "ls-files"], capture_output=True, text=True)
    if r.returncode != 0:
        fail("git ls-files 不可用，反向覆盖退化为全量（含在飞未跟踪件）")
        return None
    return set(r.stdout.splitlines())


def check_coverage(root, registered):
    # 语义：进了 Git（tracked/staged）的治理文件必须已登记；untracked = 并行会话在飞产物，豁免
    # （收口提交时其登记行同批入 Git，届时自然转红点反向——登记与入 Git 原子）。
    tracked = tracked_files(root)
    for d in ("docs", "schemas", "scripts"):
        if d == "docs":
            it = [(root / p).relative_to(root) for p in []]
        for p in (root / d).rglob("*"):
            if d == "docs" and p.parent == root / "docs/issues" and p.name not in ("README.md", "index.json"):
                continue
            if not p.is_file() or p.name.startswith(".") or any(part.startswith(".") for part in p.relative_to(root).parts):
                continue
            rel = p.relative_to(root).as_posix()
            if rel in registered:
                continue
            if tracked is not None and rel not in tracked:
                continue
            fail(f"已入 Git 的治理文件未登记: {rel}（反向覆盖，R-DP-007）")
    if "AGENTS.md" not in registered:
        fail("根 AGENTS.md 未登记")


def load_index(root):
    idx_path = root / "docs/issues/index.json"
    try:
        raw = idx_path.read_text(encoding="utf-8")
    except OSError as e:
        fail(f"docs/issues/index.json 不可读: {e}")
        return None, set()
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as e:
        fail(f"docs/issues/index.json 不是合法 JSON: {e}")
        return None, set()
    issues = data.get("issues", [])
    entries = {}
    id_line_count = sum(1 for line in raw.splitlines() if '"id": "' in line)
    if id_line_count != len(issues):
        fail(f"docs/issues/index.json 一条目一行破坏：含 \"id\": \" 的行数 {id_line_count} != 条目数 {len(issues)}")
    for it in issues:
        eid = it.get("id", "")
        entries[eid] = it
        if not STATUS_RE.match(it.get("status", "")):
            fail(f"index.json 条目 {eid} status 非法: {it.get('status', '<缺失>')!r}")
    ticket_ids = {p.stem for p in (root / "docs/issues").iterdir() if TICKET_RE.match(p.name)}
    if set(entries) != ticket_ids:
        only_index = sorted(set(entries) - ticket_ids)
        only_disk = sorted(ticket_ids - set(entries))
        fail(f"index.json 与票文件集合不一致；仅索引有: {only_index}；仅盘上有: {only_disk}")
    return entries, set(entries)


def check_issues_readme(root, entries):
    if entries is None:
        return
    text = (root / "docs/issues/README.md").read_text(encoding="utf-8")
    seen = {}
    for no, line in enumerate(text.splitlines(), 1):
        m = README_ROW_RE.match(line)
        if m:
            seen[m.group(1)] = (m.group(3), no)
    seen_ids = {name.rsplit(".", 1)[0]: (tok, no) for name, (tok, no) in seen.items()}
    for eid, it in entries.items():
        if eid not in seen_ids:
            fail(f"docs/issues/README.md 缺 {eid} 的状态列行（ticket-ops 锚点格式）")
        elif seen_ids[eid][0] != it.get("status"):
            fail(f"docs/issues/README.md:{seen_ids[eid][1]} 状态列 `{seen_ids[eid][0]}` != 索引 `{it.get('status')}`（{eid}）")


def check_jsonl(root):
    for rel, keys, exact_order in (
        ("docs/changes.jsonl", CHANGE_KEYS, True),
        ("docs/agent/generation-manifest.jsonl", MANIFEST_KEYS, False),
    ):
        p = root / rel
        try:
            lines = p.read_text(encoding="utf-8").rstrip("\n").split("\n")
        except OSError as e:
            fail(f"{rel} 不可读: {e}")
            continue
        if not lines or lines == [""]:
            fail(f"{rel} 为空（登记/账本缺失）")
            continue
        for no, line in enumerate(lines, 1):
            try:
                obj = json.loads(line)
            except json.JSONDecodeError as e:
                fail(f"{rel}:{no} 非法 JSON 行: {e}")
                continue
            got = list(obj)
            if exact_order and got != keys:
                fail(f"{rel}:{no} 键序 {got} != 契约 {keys}")
            if not exact_order and set(got) != keys:
                fail(f"{rel}:{no} 字段集 {sorted(got)} != {sorted(keys)}")


def main():
    root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent
    try:
        text = (root / "docs/agent/artifacts.yaml").read_text(encoding="utf-8")
    except OSError as e:
        print(f"registry-check: FAIL: artifacts.yaml 不可读: {e}")
        return 1
    entries = parse_registry(text)
    check_entries(entries)
    n_paths = check_paths(root, entries)
    registered = {e["path"] for e in entries if "path" in e}
    check_coverage(root, registered)
    entries_by_id, _ = load_index(root)
    check_issues_readme(root, entries_by_id)
    check_jsonl(root)

    if fails:
        for f in fails:
            print("registry-check: " + f)
        print(f"registry-check: FAIL（{len(fails)} 项；条目 {len(entries)}，登记 path 在盘 {n_paths}）")
        return 1
    print(f"registry-check: OK（条目 {len(entries)}，登记 path 在盘 {n_paths}，票 {len(entries_by_id or {})}，JSONL/README 锚点一致）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
