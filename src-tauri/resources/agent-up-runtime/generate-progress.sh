#!/bin/sh
# Input: docs/issues/index.json（票状态索引真相源，单文件一条目一行，票 33 终裁 T2）
#        与可选参数：--check 开关、仓库根目录（缺省取脚本所在目录的上一级）。
# Output: docs/progress-current.md 现役状态投影——Derived 四标注头部（generated_from、
#         生成时间、覆盖范围、失效条件）＋每票一行 `| id | status | checkpoint_ref | updated_at |`
#         状态表，按 id 升序；--check 模式为 dry-run：生成结果与既有投影各自规范化
#         generated_at 行后逐字节比对，一致 exit 0，不一致 exit 1，不落盘不写任何文件。
# Pos: 本仓记录层投影生成器（票 33 草案 §5.1.4 生成规则 / 票 35 落位）；生成器独占写
#      docs/progress-current.md（development-process §12.5 派生载体独占写），agent 不手写。
#      POSIX sh、零外部依赖（仅 POSIX 标准工具与内建，无 jq/python）；fail-closed：索引缺失、
#      不可读或条目行不合预期即停止报告，不写投影。投影与索引不一致时以索引为准。

# 用法与退出码见同目录 README.md。

# ---- 路径常量区（票 113 波④：值翻 rules/facts 新形态，映射见 rules-facts-layout-spec §2 R13/R15/R19）----
issues_index_rel='facts/requirements/tickets/index.json'
progress_current_rel='facts/requirements/tickets/progress-current.md'
gen_script_rel='rules/implementation/scripts/generate-progress.sh'

set -eu
set -f  # 关闭文件名展开：脚本不依赖 glob

usage() {
  cat <<'USAGE'
用法: sh rules/implementation/scripts/generate-progress.sh [--check] [repo-root]
参数:
  --check       dry-run 模式：与既有 facts/requirements/tickets/progress-current.md 比对（规范化 generated_at
                行后逐字节比对），一致 exit 0，不一致 exit 1；不写任何文件。
  repo-root     仓库根目录；缺省取脚本所在目录的上一级。
  -h / --help   打印本用法。
退出码: 0 生成成功或 --check 一致；1 --check 不一致（投影 stale）或既有投影缺失；
        2 用法错误、索引文件缺失/不可读或条目行不合预期（fail-closed，不写投影）。
USAGE
}

die1() {
  printf 'generate-progress: FAIL: %s\n' "$1" >&2
  exit 1
}

die2() {
  printf 'generate-progress: %s\n' "$1" >&2
  exit 2
}

check_mode=0
repo_root=''
for arg in "$@"; do
  case "$arg" in
    -h|--help) usage; exit 0 ;;
    --check) check_mode=1 ;;
    --*) die2 "未知参数: $arg" ;;
    -*) usage >&2; exit 2 ;;
    *) if [ -n "$repo_root" ]; then die2 "多余参数: ${arg}（只接受一个仓库根）"; fi; repo_root=$arg ;;
  esac
done

if [ -z "$repo_root" ]; then
  repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
fi

index_file="$repo_root/$issues_index_rel"
out_file="$repo_root/$progress_current_rel"

[ -f "$index_file" ] || die2 "索引文件不存在: ${index_file}（现役状态投影真相源缺失，fail-closed 不生成）"
[ -r "$index_file" ] || die2 "索引文件不可读: $index_file"

# 条目提取依赖一条目一行排版（T2）：行含 `"id": "<数字开头 id>"` 锚点；
# 无命中即排版或内容不合预期，fail-closed。
entries=$(grep '"id": "[0-9]' "$index_file") || {
  printf 'generate-progress: FAIL: %s\n' '索引中未找到条目行（"id": 数字锚点零命中）——确认一条目一行排版' >&2
  exit 2
}

# 每条目行投影一行状态表；必备字段（id/status/complexity/updated_at）值均非空（schema minLength），
# 故空值即判缺失；checkpoint_ref 可选（省略投影为空单元格）。
rows=$(printf '%s\n' "$entries" | awk '
function jval(line, key,    i, rest, j) {
  i = index(line, "\"" key "\": \"")
  if (i == 0) return ""
  rest = substr(line, i + length(key) + 5)
  j = index(rest, "\"")
  if (j == 0) return ""
  return substr(rest, 1, j - 1)
}
{
  id = jval($0, "id"); st = jval($0, "status"); ua = jval($0, "updated_at")
  if (id == "" || st == "" || ua == "") {
    printf "generate-progress: FAIL: 条目行缺必备字段或不合预期: %s\n", $0 > "/dev/stderr"
    exit 2
  }
  printf "| %s | %s | %s | %s |\n", id, st, jval($0, "checkpoint_ref"), ua
}
')

t_dir=${TMPDIR:-/tmp}
tmp_out=$(mktemp "${t_dir%/}/genprogress.XXXXXX")
tmp_a=$(mktemp "${t_dir%/}/genprogress.XXXXXX")
tmp_b=$(mktemp "${t_dir%/}/genprogress.XXXXXX")
cleanup() {
  rm -f "$tmp_out" "$tmp_a" "$tmp_b"
}
trap cleanup EXIT HUP INT TERM

generated_at=$(date -u '+%Y-%m-%dT%H:%M:%SZ')

{
  printf '<!-- generated_from: %s + %s -->\n' "$issues_index_rel" "$gen_script_rel"
  printf '<!-- generated_at: %s -->\n' "$generated_at"
  printf '<!-- coverage: %s 登记的全部票，每票一行，按 id 升序 -->\n' "$issues_index_rel"
  printf '<!-- invalidation: 本文件为 Derived 投影，可由生成器整文件重建；与 %s 不一致时以索引为准 -->\n' "$issues_index_rel"
  printf '\n'
  printf '# 现役状态投影（Derived）\n'
  printf '\n'
  printf '| id | status | checkpoint_ref | updated_at |\n'
  printf '| --- | --- | --- | --- |\n'
  printf '%s\n' "$rows" | LC_ALL=C sort
} > "$tmp_out"

if [ "$check_mode" -eq 1 ]; then
  # --check：规范化两侧 generated_at 行后逐字节比对（该行随生成时间漂移，不参与一致性判定）。
  [ -f "$out_file" ] || die1 "投影文件不存在: ${out_file}（先运行生成器落盘）"
  sed 's|^<!-- generated_at: .*-->$|<!-- generated_at: NORMALIZED -->|' "$tmp_out" > "$tmp_a"
  sed 's|^<!-- generated_at: .*-->$|<!-- generated_at: NORMALIZED -->|' "$out_file" > "$tmp_b"
  if cmp -s "$tmp_a" "$tmp_b"; then
    printf 'generate-progress: CHECK: PASS — 投影与索引一致: %s\n' "$out_file"
    exit 0
  fi
  printf 'generate-progress: FAIL: 投影与索引不一致（stale）: %s\n' "$out_file" >&2
  diff -u "$tmp_b" "$tmp_a" >&2 || true
  exit 1
fi

mv -f "$tmp_out" "$out_file"
printf 'generate-progress: OK: 已生成 %s\n' "$out_file"
