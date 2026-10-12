#!/bin/sh
# Input: Release script and temporary Git repositories with stubbed release tools.
# Output: Regression proof that invalid inputs and untracked deliverables stop before build or publish.
# Pos: Focused release preflight test; never tags, pushes, builds or publishes.
set -eu

REPO_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/agentup-release-preflight.XXXXXX")
trap 'rm -rf "$TMP_ROOT"' EXIT HUP INT TERM

FIXTURE="$TMP_ROOT/repo"
BIN="$TMP_ROOT/bin"
LOG="$TMP_ROOT/tools.log"
REAL_GIT=$(command -v git)
mkdir -p "$FIXTURE/scripts" "$FIXTURE/src-tauri" "$BIN"
cp "$REPO_ROOT/scripts/release.sh" "$FIXTURE/scripts/release.sh"

cat >"$BIN/tool" <<'STUB'
#!/bin/sh
name=${0##*/}
printf '%s %s\n' "$name" "$*" >>"$RELEASE_PREFLIGHT_TEST_LOG"
if [ "$name" = git ]; then
  exec "$RELEASE_PREFLIGHT_TEST_REAL_GIT" "$@"
fi
exit 97
STUB
chmod +x "$BIN/tool"
for tool in git cargo pnpm gh hdiutil; do
  ln -s tool "$BIN/$tool"
done
TEST_PATH="$BIN:$PATH"

git -C "$FIXTURE" init -q -b main
git -C "$FIXTURE" config user.name "Release preflight test"
git -C "$FIXTURE" config user.email "release-preflight@example.invalid"
git -C "$TMP_ROOT" init --bare -q origin.git
git -C "$FIXTURE" remote add origin "$TMP_ROOT/origin.git"
printf 'facts/\n' >"$FIXTURE/.gitignore"
printf '{"version":"1.1.0"}\n' >"$FIXTURE/package.json"
printf '{"version":"1.1.0"}\n' >"$FIXTURE/src-tauri/tauri.conf.json"
printf '[package]\nname = "release-preflight-fixture"\nversion = "1.1.0"\n' >"$FIXTURE/src-tauri/Cargo.toml"
git -C "$FIXTURE" add .gitignore package.json src-tauri scripts/release.sh
git -C "$FIXTURE" commit -qm "fixture"

export PATH="$TEST_PATH"
export RELEASE_PREFLIGHT_TEST_LOG="$LOG"
export RELEASE_PREFLIGHT_TEST_REAL_GIT="$REAL_GIT"

: >"$LOG"
if OUTPUT=$("$FIXTURE/scripts/release.sh" 1.1.0 --notes-file "$FIXTURE/missing-notes.md" --dry-run 2>&1); then
  printf 'FAIL: missing notes file unexpectedly passed\n%s\n' "$OUTPUT" >&2
  exit 1
fi
printf '%s\n' "$OUTPUT" | grep -F '说明文件不存在' >/dev/null || {
  printf 'FAIL: missing notes error was not reported\n%s\n' "$OUTPUT" >&2
  exit 1
}
[ ! -s "$LOG" ] || {
  printf 'FAIL: a release/build tool ran before notes validation\n' >&2
  cat "$LOG" >&2
  exit 1
}

: >"$LOG"
mkdir -p "$FIXTURE/src/assets"
printf 'asset\n' >"$FIXTURE/src/assets/new-image.png"
if OUTPUT=$("$FIXTURE/scripts/release.sh" 1.1.0 --dry-run 2>&1); then
  printf 'FAIL: non-ignored untracked asset unexpectedly passed\n%s\n' "$OUTPUT" >&2
  exit 1
fi
printf '%s\n' "$OUTPUT" | grep -F 'src/assets/new-image.png' >/dev/null || {
  printf 'FAIL: untracked asset was not named in the preflight error\n%s\n' "$OUTPUT" >&2
  exit 1
}
grep -F 'git ls-files --others --exclude-standard' "$LOG" >/dev/null || {
  printf 'FAIL: preflight did not use Git ignore-aware untracked listing\n' >&2
  cat "$LOG" >&2
  exit 1
}
if grep -Eq '^(cargo|pnpm|gh|hdiutil) ' "$LOG"; then
  printf 'FAIL: build or publishing tool ran after untracked file detection\n' >&2
  cat "$LOG" >&2
  exit 1
fi

rm -rf "$FIXTURE/src/assets"
mkdir -p "$FIXTURE/facts/project"
printf 'ledger baseline\n' >"$FIXTURE/facts/project/changes.jsonl"
git -C "$FIXTURE" add -f facts/project/changes.jsonl
git -C "$FIXTURE" commit -qm "track local ledger fixture"
printf '{"kind":"change","decision":"local only"}\n' >>"$FIXTURE/facts/project/changes.jsonl"
printf 'ignored local fact\n' >"$FIXTURE/facts/project/CONTEXT.md"
: >"$LOG"
if OUTPUT=$("$FIXTURE/scripts/release.sh" 1.2.0 --dry-run 2>&1); then
  printf 'FAIL: version mismatch fixture unexpectedly passed\n%s\n' "$OUTPUT" >&2
  exit 1
fi
printf '%s\n' "$OUTPUT" | grep -F 'package.json 版本为 1.1.0' >/dev/null || {
  printf 'FAIL: ignored facts blocked before version validation\n%s\n' "$OUTPUT" >&2
  exit 1
}
grep -F 'git ls-files --others --exclude-standard' "$LOG" >/dev/null || {
  printf 'FAIL: ignored facts check did not complete clean preflight\n' >&2
  cat "$LOG" >&2
  exit 1
}
grep -F "git status --porcelain --untracked-files=no -- . :(top,exclude,literal)facts/project/changes.jsonl" "$LOG" >/dev/null || {
  printf 'FAIL: release preflight did not exclude only the exact local ledger path\n' >&2
  cat "$LOG" >&2
  exit 1
}
if printf '%s\n' "$OUTPUT" | grep -F 'changes.jsonl' >/dev/null; then
  printf 'FAIL: ignored local facts appeared as release blockers\n%s\n' "$OUTPUT" >&2
  exit 1
fi
if grep -Eq '^(cargo|pnpm|gh|hdiutil) ' "$LOG"; then
  printf 'FAIL: build or publishing tool ran before version validation\n' >&2
  cat "$LOG" >&2
  exit 1
fi

printf 'source change\n' >>"$FIXTURE/scripts/release.sh"
: >"$LOG"
if OUTPUT=$("$FIXTURE/scripts/release.sh" 1.1.0 --dry-run 2>&1); then
  printf 'FAIL: tracked source modification unexpectedly passed\n%s\n' "$OUTPUT" >&2
  exit 1
fi
printf '%s\n' "$OUTPUT" | grep -F '跟踪文件有未提交改动' >/dev/null || {
  printf 'FAIL: tracked source modification was not rejected by preflight\n%s\n' "$OUTPUT" >&2
  exit 1
}
if grep -Eq '^(cargo|pnpm|gh|hdiutil) ' "$LOG"; then
  printf 'FAIL: release/build tool ran despite a tracked source modification\n' >&2
  cat "$LOG" >&2
  exit 1
fi

printf 'PASS: notes, untracked tree, local ledger exemption, ignored facts and tracked source guard\n'
