#!/bin/sh
# scripts/release.sh — AgentUp Harness 发布一条龙
#
# 流程（校验先于写入，失败即中止；发布段中途失败会给出当前状态与补救命令）：
#   1. 预检：gh/cargo/pnpm 就位、main 分支、跟踪文件无改动、与 origin/main 同步
#      （未跟踪的本地文件不阻塞——本仓事实记录本就只存本地，也不会进安装包）
#   2. 版本一致：package.json == src-tauri/tauri.conf.json == src-tauri/Cargo.toml == 入参 X.Y.Z
#   3. 门禁：pnpm typecheck ＋ cargo test（以 cargo 退出码为准，非零即中止）
#   4. tag/Release 防重：v<版本> 的本地 tag、远端 tag、GitHub Release；远端不可达时中止而非放行
#   5. 构建：pnpm dist（.app ＋ .dmg，取文件名含入参版本的最新 DMG）
#   6. DMG 验证：只读挂载，须含 AgentUp Harness.app 与 Applications 软链；退出时自动清理挂载
#   7. 确认后：打 annotated tag → 推 tag → 建 GitHub Release → 上传 DMG
#
# 用法：
#   scripts/release.sh <版本号> [--notes-file <文件>] [--dry-run] [--yes]
#     <版本号>        X.Y.Z 三段数字，不带 v 前缀；版本号变更需先行改三处并提交
#     --notes-file    Release 说明文件；缺省则生成标准骨架（含实际 DMG 文件名）
#     --dry-run       跑完 1–6 步即止，不写 tag、不推送、不发布
#     --yes           跳过第 7 步前的人工确认（供 CI/无人值守，慎用）
set -eu

cd "$(dirname "$0")/.."
MNT=""
TMPD=$(mktemp -d "${TMPDIR:-/tmp}/agentup-release.XXXXXX")
TEST_LOG="$TMPD/test.log"; DIST_LOG="$TMPD/dist.log"
cleanup() { [ -z "$MNT" ] || hdiutil detach "$MNT" -quiet 2>/dev/null || true; rm -rf "$TMPD"; }
trap cleanup EXIT INT TERM

usage() { awk 'NR>1{ if ($0 !~ /^#/) exit; sub(/^# ?/,""); print }' "$0"; exit "${1:-0}"; }
say() { printf '\n==> %s\n' "$1"; }
die() { printf '✗ %s\n' "$1" >&2; exit 1; }

DRY_RUN=0; ASSUME_YES=0; NOTES_FILE=""; VERSION=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --yes) ASSUME_YES=1 ;;
    --notes-file) [ $# -ge 2 ] || { echo "✗ --notes-file 缺参数" >&2; exit 2; }; NOTES_FILE="$2"; shift ;;
    -h|--help) usage ;;
    -*) echo "✗ 未知参数：$1（--help 看用法）" >&2; exit 2 ;;
    *) [ -z "$VERSION" ] || { echo "✗ 版本号重复：$1" >&2; exit 2; }; VERSION="$1" ;;
  esac
  shift
done
[ -n "$VERSION" ] || { echo "✗ 用法：scripts/release.sh <版本号> [--notes-file <文件>] [--dry-run] [--yes]" >&2; exit 2; }
printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || die "版本号须为 X.Y.Z 三段数字（不带 v 前缀），如 1.0.1"
TAG="v$VERSION"

say "1/7 预检"
for cmd in git cargo pnpm gh hdiutil; do command -v "$cmd" >/dev/null 2>&1 || die "缺依赖命令：$cmd"; done
[ "$(git branch --show-current)" = "main" ] || die "须在 main 分支执行"
[ -z "$(git status --porcelain --untracked-files=no)" ] || die "跟踪文件有未提交改动——先提交或还原（版本号三处改动须已提交）"
git fetch origin main --quiet
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "本地 main 与 origin/main 不一致——先推送/拉齐"
say "预检通过（main @ $(git rev-parse --short HEAD)，与远端同步；未跟踪的本地文件不影响发布）"

say "2/7 版本一致性（${VERSION}）"
json_ver() { sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -1; }
P_J=$(json_ver package.json); T_J=$(json_ver src-tauri/tauri.conf.json)
C_T=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' src-tauri/Cargo.toml | head -1)
[ "$P_J" = "$VERSION" ] || die "package.json 版本为 ${P_J:-<未找到>}，应为 $VERSION"
[ "$T_J" = "$VERSION" ] || die "tauri.conf.json 版本为 ${T_J:-<未找到>}，应为 $VERSION"
[ "$C_T" = "$VERSION" ] || die "Cargo.toml 版本为 ${C_T:-<未找到>}，应为 $VERSION"
say "三处版本一致：$VERSION"

say "3/7 门禁（typecheck ＋ cargo test）"
pnpm typecheck || die "typecheck 未通过"
if ! cargo test --manifest-path src-tauri/Cargo.toml >"$TEST_LOG" 2>&1; then
  echo "---- cargo test 失败，末 40 行 ----" >&2
  tail -40 "$TEST_LOG" >&2
  die "cargo test 未通过（编译或测试失败，完整输出已在上方）"
fi
grep 'test result' "$TEST_LOG" | awk '{for(i=1;i<NF;i++){if($i=="passed;")p+=$(i-1);if($i=="failed;")f+=$(i-1)}} END{printf "    cargo test：%d 通过 / %d 失败\n",p,f}'

say "4/7 tag/Release 防重（${TAG}）"
SKIP_PUBLISH=0
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  if [ "$DRY_RUN" -eq 1 ]; then say "[dry-run] 本地 tag $TAG 已存在——dry-run 下视为通过，将跳过发布段"; SKIP_PUBLISH=1
  else die "本地 tag $TAG 已存在"; fi
fi
REMOTE_TAG=$(git ls-remote --tags origin "refs/tags/$TAG")
if [ -n "$REMOTE_TAG" ]; then
  if [ "$DRY_RUN" -eq 1 ]; then say "[dry-run] 远端 tag $TAG 已存在——将跳过发布段"; SKIP_PUBLISH=1
  else die "远端 tag $TAG 已存在"; fi
fi
if gh release view "$TAG" >/dev/null 2>&1; then
  if [ "$DRY_RUN" -eq 1 ]; then say "[dry-run] GitHub Release $TAG 已存在——将跳过发布段"; SKIP_PUBLISH=1
  else die "GitHub Release $TAG 已存在"; fi
elif ! gh api user --jq .login >/dev/null 2>&1; then
  die "GitHub API 不可达或未认证——无法安全执行 Release 防重检查，中止"
fi
[ "$SKIP_PUBLISH" -eq 1 ] || say "$TAG 无冲突"

say "5/7 构建（.app ＋ .dmg）"
if ! pnpm exec tauri build --bundles app >"$DIST_LOG" 2>&1; then
  tail -20 "$DIST_LOG" >&2; die "构建失败"
fi
DMG=""
for ATTEMPT in 1 2 3; do
  if pnpm exec tauri build --bundles dmg >>"$DIST_LOG" 2>&1; then
    DMG=$(ls -t src-tauri/target/release/bundle/dmg/*"$VERSION"*.dmg 2>/dev/null | head -1 || true)
    [ -n "$DMG" ] && break
  fi
  say "  第 ${ATTEMPT} 次 DMG 打包未成功（bundle_dmg.sh 的 Finder AppleScript 存在挂载竞态），5 秒后重试…"
  sleep 5
done
[ -n "$DMG" ] || { tail -20 "$DIST_LOG" >&2; die "DMG 打包连续 3 次失败——稍后系统空闲时单独执行 pnpm exec tauri build --bundles dmg，成功后再跑本脚本（tag/Release 段未开始，无半成品）"; }
say "构建完成：$(basename "${DMG}")（$(du -h "${DMG}" | cut -f1)）"

say "6/7 DMG 挂载验证"
MNT=$(mktemp -d "${TMPDIR:-/tmp}/agentup-mnt.XXXXXX")
hdiutil attach -nobrowse -readonly "$DMG" -mountpoint "$MNT" >/dev/null || die "DMG 无法挂载"
if [ ! -d "$MNT/AgentUp Harness.app" ] || [ ! -L "$MNT/Applications" ]; then
  die "DMG 内容不齐（缺 AgentUp Harness.app 或 Applications 软链）；挂载点 $MNT 将随退出自动卸载清理"
fi
hdiutil detach "$MNT" -quiet || die "DMG 卸载失败——请手动执行：hdiutil detach $MNT"
MNT=""
say "DMG 验证通过：App ＋ Applications 软链齐全"

if [ "$DRY_RUN" -eq 1 ]; then
  say "dry-run 结束：1–6 步全部通过，未写 tag、未推送、未发布"
  exit 0
fi
[ "$SKIP_PUBLISH" -eq 0 ] || die "tag/Release 已存在（见第 4 步），中止发布段——如需重发请先清理"

say "7/7 发布（tag ＋ GitHub Release）"
if [ "$ASSUME_YES" -ne 1 ]; then
  printf '即将：打 tag %s 并推送 → 创建 GitHub Release 并上传 %s\n确认继续？[y/N] ' "$TAG" "$(basename "$DMG")"
  read -r ANSWER || die "未读到确认（stdin 已关闭）——已取消，未做任何写入"
  case "$ANSWER" in y|Y|yes|YES) ;; *) die "已取消（未做任何写入）" ;; esac
fi

git tag -a "$TAG" -m "AgentUp Harness $TAG" || die "打 tag 失败（本地 tag 已删除或请检查 git 状态）"
git push origin "$TAG" || die "推送 tag 失败——tag $TAG 已留在本地；处理后可手动 git push origin $TAG 重试"

if [ -n "$NOTES_FILE" ]; then
  [ -f "$NOTES_FILE" ] || die "说明文件不存在：$NOTES_FILE"
  REL_NOTES="$NOTES_FILE"
else
  REL_NOTES="$TMPD/notes.md"
  {
    printf '# AgentUp Harness %s\n\n' "$TAG"
    printf '安装：下载下方 DMG（Apple Silicon），拖入 Applications。应用未做开发者签名，首次启动若被 Gatekeeper 拦截：右键 → 打开。\n\n'
    printf '## 验证\n\n- cargo test 全绿、pnpm typecheck 通过（发布前由 scripts/release.sh 强制执行）\n\n'
    printf '> 治理流程、票单与产品路线等事实记录仅存维护者本地。\n'
  } >"$REL_NOTES"
fi

gh release create "$TAG" --title "AgentUp Harness $TAG" --notes-file "$REL_NOTES" \
  || die "创建 Release 失败——注意：tag $TAG 已推送远端；可修复后手动 gh release create $TAG --title ... --notes-file ... 补建，或 git push origin :$TAG 删远端 tag 后重跑本脚本"
gh release upload "$TAG" "$DMG" \
  || die "上传 DMG 失败——Release $TAG 已创建；可手动补传：gh release upload $TAG \"$DMG\""

REL_URL=$(gh release view "$TAG" --json url -q .url 2>/dev/null || echo "（查询 URL 失败，请到 GitHub Releases 页查看）")
say "发布完成：$REL_URL"
