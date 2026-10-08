#!/bin/sh
# scripts/release.sh — AgentUp Harness 发布一条龙
#
# 流程（校验先于写入，任一步失败即整体中止）：
#   1. 预检：gh/cargo/pnpm 就位、main 分支、工作区干净、与 origin/main 同步
#   2. 版本一致：package.json == src-tauri/tauri.conf.json == src-tauri/Cargo.toml == 入参版本
#   3. 门禁：pnpm typecheck ＋ cargo test 全绿
#   4. tag/Release 防重：v<版本> 的本地 tag、远端 tag、GitHub Release 均不存在
#   5. 构建：pnpm dist（.app ＋ .dmg）
#   6. DMG 验证：只读挂载，须含 AgentUp Harness.app 与 Applications 软链
#   7. 确认后：打 annotated tag → 推 tag → 建 GitHub Release → 上传 DMG
#
# 用法：
#   scripts/release.sh <版本号> [--notes-file <文件>] [--dry-run] [--yes]
#     <版本号>        不带 v 前缀，如 1.0.1；版本号变更需先行改三处并提交
#     --notes-file    Release 说明文件；缺省则生成标准骨架（含实际 DMG 文件名）
#     --dry-run       跑完 1–6 步即止，不写 tag、不推送、不发布
#     --yes           跳过第 7 步前的人工确认（供 CI/无人值守，慎用）
set -eu

usage() { sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'; exit "${1:-0}"; }

DRY_RUN=0; ASSUME_YES=0; NOTES_FILE=""; VERSION=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --yes) ASSUME_YES=1 ;;
    --notes-file) [ $# -ge 2 ] || { echo "✗ --notes-file 缺参数" >&2; exit 2; }; NOTES_FILE="$2"; shift ;;
    -h|--help) usage 0 ;;
    -*) echo "✗ 未知参数：$1" >&2; usage 2 ;;
    *) [ -z "$VERSION" ] || { echo "✗ 版本号重复：$1" >&2; exit 2; }; VERSION="$1" ;;
  esac
  shift
done
[ -n "$VERSION" ] || { echo "✗ 用法：scripts/release.sh <版本号> [--notes-file <文件>] [--dry-run] [--yes]" >&2; exit 2; }
case "$VERSION" in v*|*[!0-9.]*) echo "✗ 版本号须为不带 v 前缀的数字点位，如 1.0.1" >&2; exit 2 ;; esac

say()  { printf '\n==> %s\n' "$1"; }
die()  { printf '✗ %s\n' "$1" >&2; exit 1; }
TAG="v$VERSION"

say "1/7 预检"
for cmd in git cargo pnpm gh hdiutil; do command -v "$cmd" >/dev/null 2>&1 || die "缺依赖命令：$cmd"; done
[ "$(git branch --show-current)" = "main" ] || die "须在 main 分支执行"
[ -z "$(git status --porcelain)" ] || die "工作区不干净——先提交或清理（版本号三处改动须已提交）"
git fetch origin main --quiet
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "本地 main 与 origin/main 不一致——先推送/拉齐"
say "预检通过（main @ $(git rev-parse --short HEAD)，干净且与远端同步）"

say "2/7 版本一致性（$VERSION）"
json_ver() { sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -1; }
P_J=$(json_ver package.json); T_J=$(json_ver src-tauri/tauri.conf.json)
C_T=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' src-tauri/Cargo.toml | head -1)
[ "$P_J" = "$VERSION" ] || die "package.json 版本为 $P_J，应为 $VERSION"
[ "$T_J" = "$VERSION" ] || die "tauri.conf.json 版本为 $T_J，应为 $VERSION"
[ "$C_T" = "$VERSION" ] || die "Cargo.toml 版本为 $C_T，应为 $VERSION"
say "三处版本一致：$VERSION"

say "3/7 门禁（typecheck ＋ cargo test）"
pnpm typecheck || die "typecheck 未通过"
cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tee /tmp/agentup-release-test.log | grep -E 'test result' | awk -F'[ ;]' '{p+=$4; f+=$6} END {printf "    cargo test：%d 通过 / %d 失败\n", p, f; exit (f > 0)}' || die "cargo test 存在失败"

say "4/7 tag/Release 防重（$TAG）"
SKIP_PUBLISH=0
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  if [ "$DRY_RUN" -eq 1 ]; then say "[dry-run] 本地 tag $TAG 已存在——dry-run 下视为通过，将跳过发布段"; SKIP_PUBLISH=1
  else die "本地 tag $TAG 已存在"; fi
fi
if git ls-remote --tags origin "refs/tags/$TAG" | grep -q .; then
  if [ "$DRY_RUN" -eq 1 ]; then say "[dry-run] 远端 tag $TAG 已存在——将跳过发布段"; SKIP_PUBLISH=1
  else die "远端 tag $TAG 已存在"; fi
fi
if gh release view "$TAG" --json tagName >/dev/null 2>&1; then
  if [ "$DRY_RUN" -eq 1 ]; then say "[dry-run] GitHub Release $TAG 已存在——将跳过发布段"; SKIP_PUBLISH=1
  else die "GitHub Release $TAG 已存在"; fi
fi
[ "$SKIP_PUBLISH" -eq 1 ] || say "$TAG 无冲突"

say "5/7 构建（.app ＋ .dmg）"
pnpm dist >/tmp/agentup-release-dist.log 2>&1 || { tail -20 /tmp/agentup-release-dist.log; die "构建失败（详见 /tmp/agentup-release-dist.log）"; }
DMG=$(ls -t src-tauri/target/release/bundle/dmg/*.dmg 2>/dev/null | head -1)
[ -n "$DMG" ] || { tail -20 /tmp/agentup-release-dist.log; die "未找到 DMG 产物——确认 tauri.conf.json bundle targets 含 dmg"; }
say "构建完成：$(ls -lh "$DMG" | awk '{print $5, $NF}')"

say "6/7 DMG 挂载验证"
MNT=$(mktemp -d)
hdiutil attach -nobrowse -readonly "$DMG" -mountpoint "$MNT" >/dev/null || die "DMG 无法挂载"
[ -d "$MNT/AgentUp Harness.app" ] || { hdiutil detach "$MNT" -quiet || true; die "DMG 内无 AgentUp Harness.app"; }
[ -L "$MNT/Applications" ] || { hdiutil detach "$MNT" -quiet || true; die "DMG 内无 Applications 软链"; }
hdiutil detach "$MNT" -quiet || die "DMG 卸载失败"
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

if [ -n "$NOTES_FILE" ]; then
  [ -f "$NOTES_FILE" ] || die "说明文件不存在：$NOTES_FILE"
  REL_NOTES="$NOTES_FILE"
else
  REL_NOTES=$(mktemp)
  {
    printf '# AgentUp Harness %s\n\n' "$TAG"
    printf '安装：下载下方 DMG（Apple Silicon），拖入 Applications。应用未做开发者签名，首次启动若被 Gatekeeper 拦截：右键 → 打开。\n\n'
    printf '## 验证\n\n- cargo test 全绿、pnpm typecheck 通过（发布前由 scripts/release.sh 强制执行）\n\n'
    printf '> 治理流程、票单与产品路线等事实记录仅存维护者本地。\n'
  } >"$REL_NOTES"
fi

git tag -a "$TAG" -m "AgentUp Harness $TAG" || die "打 tag 失败"
git push origin "$TAG" || die "推送 tag 失败（tag 已留在本地，处理后可手动重推）"
gh release create "$TAG" --title "AgentUp Harness $TAG" --notes-file "$REL_NOTES" || die "创建 Release 失败"
gh release upload "$TAG" "$DMG" || die "上传 DMG 失败（Release 已建，可手动 gh release upload $TAG 补传）"

say "发布完成：$(gh release view "$TAG" --json url -q .url)"
