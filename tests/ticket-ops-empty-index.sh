#!/bin/sh
# Input: Packaged ticket-ops and progress generator resources from the repository under test.
# Output: Isolated regressions for opening the first ticket from an empty index.
# Pos: tests/ targeted ticket-ops regression; run with sh tests/ticket-ops-empty-index.sh.

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp_root=$(mktemp -d "${TMPDIR:-/tmp}/agentup-ticket-ops.XXXXXX")
trap 'rm -rf "$tmp_root"' EXIT HUP INT TERM

fail() {
  printf 'ticket-ops regression: FAIL: %s\n' "$1" >&2
  exit 1
}

prepare_root() {
  fixture=$1
  index_body=$2
  mkdir -p "$fixture/rules/implementation/scripts" \
    "$fixture/facts/requirements/tickets" "$fixture/facts/project" \
    "$fixture/agent-up/references/schemas"
  cp "$repo_root/src-tauri/resources/agent-up-runtime/ticket-ops.sh" \
    "$fixture/rules/implementation/scripts/ticket-ops.sh"
  cp "$repo_root/src-tauri/resources/agent-up-runtime/generate-progress.sh" \
    "$fixture/rules/implementation/scripts/generate-progress.sh"
  printf '%s\n' "$index_body" > "$fixture/facts/requirements/tickets/index.json"
  cat > "$fixture/facts/requirements/tickets/README.md" <<'README'
# Tickets

| Name | Role | Purpose |
| --- | --- | --- |
| `README.md` | directory index | Fixture entry. |
README
  : > "$fixture/facts/project/changes.jsonl"
  cat > "$fixture/agent-up/references/schemas/ticket-record.schema.json" <<'SCHEMA'
{
  "required": ["id", "kind", "title", "created_at"],
  "properties": {"kind": {"enum": ["task", "request"]}},
  "allOf": [
    {
      "if": {"properties": {"kind": {"const": "task"}}},
      "then": {"required": ["complexity", "profile", "blocked_by", "complexity_reason", "priority_reason"]}
    }
  ]
}
SCHEMA
}

write_ticket() {
  fixture=$1
  ticket_id=$2
  cat > "$fixture/facts/requirements/tickets/$ticket_id.json" <<TICKET
{
  "id": "$ticket_id",
  "kind": "task",
  "title": "Regression ticket",
  "created_at": "2026-10-12",
  "complexity": "C0",
  "profile": "D:Required,B:Advisory,I:Required,U:Required,S:N/A,M:N/A,O:N/A",
  "blocked_by": [],
  "complexity_reason": "Regression fixture.",
  "priority_reason": "Regression fixture.",
  "goal": "Open a ticket from an empty index.",
  "scope": ["tests/"],
  "acceptance": ["The ticket is indexed."],
  "verification": "sh tests/ticket-ops-empty-index.sh"
}
TICKET
}

open_ticket() {
  fixture=$1
  ticket_id=$2
  title=$3
  if [ ! -f "$fixture/facts/requirements/tickets/$ticket_id.json" ]; then
    write_ticket "$fixture" "$ticket_id"
  fi
  AGENTUP_SKILL_ROOT="$fixture/agent-up" sh \
    "$fixture/rules/implementation/scripts/ticket-ops.sh" \
    "$fixture" open --id "$ticket_id" --complexity C0 --title "$title" \
    --ledger-line '{"date":"2026-10-12","kind":"change","scope":"fixture","decision":"test","evidence_ref":"fixture"}'
}

assert_projection_has() {
  fixture=$1
  expected=$2
  grep -F "$expected" "$fixture/facts/requirements/tickets/progress-current.md" >/dev/null || \
    fail "projection is missing $expected"
}

assert_registered_ids() {
  fixture=$1
  expected_ids=$2
  python3 - "$fixture/facts/requirements/tickets/index.json" "$expected_ids" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    index = json.load(handle)
ids = [entry["id"] for entry in index["issues"]]
expected = sys.argv[2].split()
if ids != expected:
    raise SystemExit(f"expected ids {expected}, got {ids}")
if any(entry.get("status") != "ready" for entry in index["issues"]):
    raise SystemExit(f"new tickets are not ready: {index['issues']}")
PY
}

assert_no_ticket_state_changes() {
  fixture=$1
  snapshot=$2
  for relative in \
    facts/requirements/tickets/index.json \
    facts/requirements/tickets/README.md \
    facts/requirements/tickets/progress-current.md \
    facts/project/changes.jsonl
  do
    if [ -f "$snapshot/$relative" ]; then
      [ -f "$fixture/$relative" ] && cmp -s "$snapshot/$relative" "$fixture/$relative" || \
        fail "rejected open changed $relative"
    else
      [ ! -e "$fixture/$relative" ] || fail "rejected open created $relative"
    fi
  done
}

expect_rejected_without_writes() {
  fixture=$1
  ticket_id=$2
  title=$3
  snapshot="$tmp_root/snapshot-$(basename "$fixture")"
  mkdir -p "$snapshot/facts/requirements/tickets" "$snapshot/facts/project"
  for relative in \
    facts/requirements/tickets/index.json \
    facts/requirements/tickets/README.md \
    facts/project/changes.jsonl
  do
    cp "$fixture/$relative" "$snapshot/$relative"
  done
  if [ -f "$fixture/facts/requirements/tickets/progress-current.md" ]; then
    mkdir -p "$snapshot/facts/requirements/tickets"
    cp "$fixture/facts/requirements/tickets/progress-current.md" \
      "$snapshot/facts/requirements/tickets/progress-current.md"
  fi
  if open_ticket "$fixture" "$ticket_id" "$title" > "$tmp_root/rejected.log" 2>&1; then
    fail "open unexpectedly accepted $ticket_id"
  fi
  assert_no_ticket_state_changes "$fixture" "$snapshot"
}

run_empty_index_case() {
  name=$1
  index_body=$2
  fixture="$tmp_root/$name"
  prepare_root "$fixture" "$index_body"

  if ! open_ticket "$fixture" 01-first-ticket 'First ticket' > "$tmp_root/$name.log" 2>&1; then
    cat "$tmp_root/$name.log" >&2
    fail "first open failed for $name empty index"
  fi
  assert_registered_ids "$fixture" '01-first-ticket'
  grep -F '| `01-first-ticket.json` |' \
    "$fixture/facts/requirements/tickets/README.md" >/dev/null || \
    fail "README is missing the first ticket for $name"
  grep -F '"evidence_ref":"fixture"' \
    "$fixture/facts/project/changes.jsonl" >/dev/null || \
    fail "ledger is missing the first ticket record for $name"
  assert_projection_has "$fixture" '| 01-first-ticket | ready |'
  sh "$fixture/rules/implementation/scripts/generate-progress.sh" --check "$fixture" >/dev/null

  if ! open_ticket "$fixture" 02-second-ticket 'Second ticket' > "$tmp_root/$name-second.log" 2>&1; then
    cat "$tmp_root/$name-second.log" >&2
    fail "second open failed for $name index"
  fi
  assert_registered_ids "$fixture" '01-first-ticket 02-second-ticket'
  assert_projection_has "$fixture" '| 02-second-ticket | ready |'
  sh "$fixture/rules/implementation/scripts/generate-progress.sh" --check "$fixture" >/dev/null

  expect_rejected_without_writes "$fixture" 02-second-ticket 'Duplicate ticket'
}

run_empty_index_case inline '{"issues":[]}'
run_empty_index_case multiline '{
  "issues": [
  ]
}'

fixture="$tmp_root/metadata-index"
prepare_root "$fixture" '{
  "issues": [],
  "metadata": {"owner": "fixture", "labels": ["kept"]}
}'
if ! open_ticket "$fixture" 01-first-ticket 'First ticket' > "$tmp_root/metadata-index.log" 2>&1; then
  cat "$tmp_root/metadata-index.log" >&2
  fail "first open failed when the index had unrelated metadata"
fi
python3 - "$fixture/facts/requirements/tickets/index.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    index = json.load(handle)
if [entry["id"] for entry in index["issues"]] != ["01-first-ticket"]:
    raise SystemExit(f"ticket was not inserted into issues: {index}")
if index.get("metadata") != {"owner": "fixture", "labels": ["kept"]}:
    raise SystemExit(f"non-issues metadata changed: {index}")
PY
assert_projection_has "$fixture" '| 01-first-ticket | ready |'

fixture="$tmp_root/ambiguous-metadata-index"
prepare_root "$fixture" '{
  "issues": [],
  "extra": [
    {"id": "99-extra", "status": "ready", "complexity": "C0", "blocked_by": [], "updated_at": "2026-10-12T00:00"}
  ]
}'
expect_rejected_without_writes "$fixture" 01-first-ticket 'First ticket'

for missing_field in status updated_at
do
  fixture="$tmp_root/missing-$missing_field"
  if [ "$missing_field" = status ]; then
    index_body='{"issues":[{"id":"09-existing","complexity":"C0","updated_at":"2026-10-12T00:00"}]}'
  else
    index_body='{"issues":[{"id":"09-existing","status":"ready","complexity":"C0"}]}'
  fi
  prepare_root "$fixture" "$index_body"
  expect_rejected_without_writes "$fixture" 10-new-ticket 'New ticket'
done

fixture="$tmp_root/invalid-empty-index"
prepare_root "$fixture" '{"issues":null}'
expect_rejected_without_writes "$fixture" 01-first-ticket 'First ticket'

fixture="$tmp_root/duplicate-key-empty-index"
prepare_root "$fixture" '{"issues":[],"issues":[]}'
expect_rejected_without_writes "$fixture" 01-first-ticket 'First ticket'

printf 'ticket-ops regression: PASS\n'
