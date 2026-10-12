#!/bin/sh
# Input: 子命令与参数（[repo-root] open/take/flip/ledger + 选项）与三个票务面数据载体：
#        facts/requirements/tickets/index.json（票状态真相源，单文件一条目一行；open 只改唯一
#        顶层 issues 数组，保留其他元数据，拒绝含投影 id 锚点的元数据，票 33 终裁 T2）、
#        facts/requirements/tickets/README.md 目录清单状态列（`任务票 <NN>；` 后首个反引号状态 token，
#        人工登记投影，票 37 口径）、facts/project/changes.jsonl 记录账本（键序
#        date,kind,scope,decision,evidence_ref，五键必备＋按 kind 条件键：lesson 必带
#        第六键 promoted_to、replan 必带 replan_original/replan_replacement/replan_undone
#        ——票 81，一行一事实，禁裸换行）。
# Output: 协调层票务面写入——open＝新票本体 schema 校验＋理由非空断言（票 79，只 open
#         时点生效，存量零回扫）＋校验＋索引新增条目（status=ready）＋README 追加目录
#         清单行＋账本追加行；take/flip＝索引 id 锚点整行替换（status+updated_at）＋README
#         状态 token 替换＋账本追加行，flip 另做 task 票 actual_time 收口硬拦（票 100：
#         票 JSON 本体 actual_time 须已填且非空，缺项即 exit 1 指名票 id 与缺项，request
#         不校验）＋账本追加行；ledger＝无票账本行入口（票 100：不依赖 docs/issues/index.json
#         条目 id，账本行形状校验先于写入，仅账本落行，不触索引/README/投影）；
#         open/take/flip 收尾调 scripts/generate-progress.sh 再生
#         facts/requirements/tickets/progress-current.md
#         现役状态投影并 --check 核对（ledger 无索引面改动，不做投影再生）。
# Pos: 协调层票务运维单入口脚本（票 41，用户 2026-09-20 裁决 2.1）；行为基线＝
#      development-process §6 零写入三段式第二段与 §12.5 票务脚本承载注记。POSIX sh、
#      零外部依赖（仅 POSIX 标准工具与内建，无 jq；票 79 起 open 本体 schema 校验另用
#      python3 标准库）；fail-closed：校验先于写入，
#      open 结构化定位 issues 数组，按其原始值生成一条目一行的索引副本；元数据含投影 id
#      锚点、issues 条目排版破坏、README 行缺失或锚点不唯一、账本行不合键序、同 NN 异 slug 撞号
#      （票 77）即停止不写；收尾生成器
#      缺失或 --check 不过即整体失败退出非零，已写部分如实报告。职责边界：只做票务面
#      写入，不执行门禁核对与 Git 提交，不调用 check-gates.sh 与 lane-commit.sh（投影
#      再生调用 generate-progress.sh 不在此限）。

# 用法、数据契约与退出码见同目录 README.md。

# ---- 路径常量区（票 113 波④：值翻 rules/facts 新形态，映射见 rules-facts-layout-spec §2 R11/R13/R15/R19）----
issues_index_rel='facts/requirements/tickets/index.json'
issues_readme_rel='facts/requirements/tickets/README.md'
issues_dir_rel='facts/requirements/tickets'
changes_ledger_rel='facts/project/changes.jsonl'
progress_current_rel='facts/requirements/tickets/progress-current.md'
scripts_dir_rel='rules/implementation/scripts'

set -eu
set -f  # 关闭文件名展开：脚本不依赖 glob

NL='
'
TAB=$(printf '\t')

usage() {
  cat <<'USAGE'
用法: sh rules/implementation/scripts/ticket-ops.sh [repo-root] <command> [options]
参数:
  repo-root      仓库根目录；缺省取脚本所在目录的上一级。
命令:
  open           开票：先过新票本体校验（facts/requirements/tickets/<id>.json 须已落位并过
                 ticket-record.schema.json 校验＋定级/优先级理由非空断言，票 79；缺本体
                 或校验不过即 fail-closed 零写入），再写索引条目（status=ready）＋
                 issues-README 目录清单追加行＋账本追加行，收尾投影再生＋--check。
                 选项（前四必选）:
                   --id <NN-slug>            票 id（^[0-9]{2,}-[a-z0-9-]+$；须不在索引中且 NN 段未被
                             占用——同 NN 异 slug 撞号拒开，票 77）
                   --complexity <C0-C3>      复杂度档位
                   --title <text>            票标题（用作 README 行功能列文本；不得含
                                             竖线/反引号/换行/制表符）
                   --ledger-line <json>      完整 JSONL 账本行（键序 date,kind,scope,
                                             decision,evidence_ref 五键必备＋按 kind 条件
                                             键，票 81：lesson 必带第六键 promoted_to（枚
                                             举 新票/rules/sop/validator/test/none），replan
                                             必带 replan_original/replan_replacement/
                                             replan_undone（布尔）；紧凑或带空格 JSONL
                                             均可；脚本只校验后追加，不代生成散文；字符
                                             串值不得内嵌引号或反斜杠）
                   [--blocked-by <id,id>]    逗号分隔 blocker id（缺省空数组）
  take           领取：--id --status --ledger-line 三项必选；--status 须为 in_progress。
  flip           收口/状态翻转：--id --status --ledger-line 三项必选；--status 为状态机
                 任意合法值（ready|in_progress|blocked|review_ready|review_pass|
                 review_fail|done|superseded）。task 票收口硬拦（票 100）：本票 JSON
                 本体（facts/requirements/tickets/<id>.json）须含 actual_time 字段且非空（净工时口径，
                 会话段累计减中断段），缺项或空白即 exit 1 指名票 id 与缺项（fail-closed
                 零写入）；request 票不校验。
                 take/flip 语义: 索引 id 锚点整行替换（仅改 status/updated_at 两值）→
                 README 状态行锚定 `任务票 <NN>；` 后首个反引号状态 token 替换（行内其余
                 文本不动）→ 账本追加行 → 投影再生＋--check。索引或 README 行缺失、锚点
                 不唯一、账本行非法即停止（fail-closed，写入前预检）。
  ledger         无票账本行入口（票 100）：--ledger-line 必选；不依赖 facts/requirements/tickets/index.json
                 条目 id，仅账本形状校验（键序五键必备＋按 kind 条件键，同上）先于写入，
                 通过后向 facts/project/changes.jsonl 追加该行；不触索引/README/票文件/投影
                 （C0 修正类免建票收口经此落账本行，决策 4）。
  -h / --help    打印本用法。
退出码: 0 全部完成；1 fail-closed（校验/锚点/收尾核对不过，已写部分如实报告；open 条目
        已存在即停，非幂等；take/flip 重跑会重复落账本行，核对后处理）；2 用法或环境错误。
USAGE
}

die1() {
  printf 'ticket-ops: FAIL: %s\n' "$1" >&2
  exit 1
}

die2() {
  printf 'ticket-ops: %s\n' "$1" >&2
  exit 2
}

tmp_idx=''
tmp_readme=''
cleanup() {
  if [ -n "${tmp_idx}" ]; then tmp_idx=''; rm -f "${tmp_idx}"; fi
  if [ -n "${tmp_readme}" ]; then tmp_readme=''; rm -f "${tmp_readme}"; fi
}
trap cleanup EXIT HUP INT TERM

[ $# -ge 1 ] || { usage >&2; exit 2; }
case $1 in
  -h|--help) usage; exit 0 ;;
  open|take|flip|ledger)
    cmd=$1
    shift
    repo_root=''
    ;;
  *)
    repo_root=$1
    shift
    [ $# -ge 1 ] || { usage >&2; exit 2; }
    case $1 in
      open|take|flip|ledger) cmd=$1; shift ;;
      *) usage >&2; exit 2 ;;
    esac
    ;;
esac

if [ -z "${repo_root}" ]; then
  repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
fi
[ -d "${repo_root}" ] || die2 "仓库根不存在: ${repo_root}"

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# ---- 选项解析 ----

id=''
complexity=''
title=''
ledger_line=''
blocked_by=''
status=''

while [ $# -gt 0 ]; do
  case $1 in
    --id) [ $# -ge 2 ] || die2 '--id 缺值'; id=$2; shift 2 ;;
    --complexity) [ $# -ge 2 ] || die2 '--complexity 缺值'; complexity=$2; shift 2 ;;
    --title) [ $# -ge 2 ] || die2 '--title 缺值'; title=$2; shift 2 ;;
    --ledger-line) [ $# -ge 2 ] || die2 '--ledger-line 缺值'; ledger_line=$2; shift 2 ;;
    --blocked-by) [ $# -ge 2 ] || die2 '--blocked-by 缺值'; blocked_by=$2; shift 2 ;;
    --status) [ $# -ge 2 ] || die2 '--status 缺值'; status=$2; shift 2 ;;
    *) die2 "未知参数: $1" ;;
  esac
done

# ---- 公共校验（fail-closed，先于任何写入） ----

# 票锚定命令（open/take/flip）才要求 --id；ledger 无票面不依赖 id（票 100）
case ${cmd} in
  open|take|flip)
    case ${id} in
      '') die1 "缺少 --id" ;;
    esac
    printf '%s' "${id}" | LC_ALL=C grep -Eq '^[0-9]{2,}-[a-z0-9-]+$' || \
      die1 "id 不合口径（^[0-9]{2,}-[a-z0-9-]+$）: ${id}"
    ;;
esac
case ${ledger_line} in
  '') die1 "缺少 --ledger-line" ;;
esac
case ${ledger_line} in
  *"${NL}"*|*"${TAB}"*) die1 '账本行含裸换行或制表符（一行一事实，禁裸换行）' ;;
esac
# 账本行形状校验（票 81 行型扩展，仅向前生效）：五键必备＋按 kind 条件键＋promoted_to
# 枚举。非 lesson/replan 维持恰五键收口（既有合法面不放宽不收窄）；lesson 恰六键——第六键
# promoted_to 枚举＝新票/rules/sop/validator/test/none（填 none 亦算显式拍板，防教训悬空；
# 越枚举不静默收编）；replan 恰八键——三条件键须为布尔字面量 true/false。条件键一律紧跟
# evidence_ref 之后按固定次序排布（键序稳定，键序破坏形态同拒）。
if printf '%s' "${ledger_line}" | LC_ALL=C grep -Eq '^\{"date": ?"[0-9]{4}-[0-9]{2}-[0-9]{2}", ?"kind": ?"lesson", ?"scope": ?"[^"\\]*", ?"decision": ?"[^"\\]*", ?"evidence_ref": ?"[^"\\]*", ?"promoted_to": ?"(新票|rules|sop|validator|test|none)"\}$'; then
  :
elif printf '%s' "${ledger_line}" | LC_ALL=C grep -Eq '^\{"date": ?"[0-9]{4}-[0-9]{2}-[0-9]{2}", ?"kind": ?"replan", ?"scope": ?"[^"\\]*", ?"decision": ?"[^"\\]*", ?"evidence_ref": ?"[^"\\]*", ?"replan_original": ?(true|false), ?"replan_replacement": ?(true|false), ?"replan_undone": ?(true|false)\}$'; then
  :
elif printf '%s' "${ledger_line}" | LC_ALL=C grep -Eq '^\{"date": ?"[0-9]{4}-[0-9]{2}-[0-9]{2}", ?"kind": ?"[^"\\]*", ?"scope": ?"[^"\\]*", ?"decision": ?"[^"\\]*", ?"evidence_ref": ?"[^"\\]*"\}$'; then
  # 恰五键行：kind=lesson/replan 即缺条件键，指名报因；其余 kind 维持既有合法面
  ledger_kind=$(printf '%s' "${ledger_line}" | sed -n 's/^{"date": \{0,1\}"[0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}", \{0,1\}"kind": \{0,1\}"\([^"\\]*\)".*/\1/p')
  case ${ledger_kind} in
    lesson) die1 'lesson 行缺第六键 promoted_to（枚举 新票/rules/sop/validator/test/none，填 none 亦算显式拍板——票 81）' ;;
    replan) die1 'replan 行缺条件键 replan_original/replan_replacement/replan_undone（布尔 true/false——票 81）' ;;
  esac
else
  # 形状不符：可定位行型条件键形态的指名报因，否则通用报因（fail-closed 口径不放宽）
  if printf '%s' "${ledger_line}" | LC_ALL=C grep -Eq '^\{"date": ?"[0-9]{4}-[0-9]{2}-[0-9]{2}", ?"kind": ?"lesson", ?"scope": ?"[^"\\]*", ?"decision": ?"[^"\\]*", ?"evidence_ref": ?"[^"\\]*", ?"promoted_to": ?"[^"\\]*"\}$'; then
    promoted_to_val=$(printf '%s' "${ledger_line}" | sed -n 's/.*"promoted_to": \{0,1\}"\([^"\\]*\)".*/\1/p')
    die1 "lesson 行 promoted_to 越枚举（枚举＝新票/rules/sop/validator/test/none，不静默收编——票 81）: ${promoted_to_val}"
  fi
  if printf '%s' "${ledger_line}" | LC_ALL=C grep -Eq '^\{"date": ?"[0-9]{4}-[0-9]{2}-[0-9]{2}", ?"kind": ?"replan", ?"scope": ?"[^"\\]*", ?"decision": ?"[^"\\]*", ?"evidence_ref": ?"[^"\\]*", ?"replan_original": ?[^,]*, ?"replan_replacement": ?[^,]*, ?"replan_undone": ?[^}]*\}$'; then
    die1 'replan 行条件键须为布尔字面量 true/false 且按 replan_original→replan_replacement→replan_undone 固定键序（票 81）'
  fi
  die1 '账本行不合键序或形状（键序 date,kind,scope,decision,evidence_ref；五键必备＋按 kind 条件键：lesson 必带第六键 promoted_to（枚举 新票/rules/sop/validator/test/none）、replan 必带 replan_original/replan_replacement/replan_undone（布尔）——票 81；紧凑或带空格 JSONL 均可；字符串值不得内嵌引号或反斜杠）'
fi

case ${cmd} in
  open)
    case ${complexity} in
      '') die1 "缺少 --complexity" ;;
    esac
    printf '%s' "${complexity}" | LC_ALL=C grep -Eq '^C[0-3]$' || \
      die1 "complexity 不合口径（C0-C3）: ${complexity}"
    case ${title} in
      '') die1 "缺少 --title" ;;
    esac
    case ${title} in
      *"|"*|*"\`"*|*"${NL}"*|*"${TAB}"*) die1 'title 不得含竖线、反引号、换行或制表符（README 表格单元安全）' ;;
    esac
    blocked_json=''
    if [ -n "${blocked_by}" ]; then
      rest=${blocked_by}
      while [ -n "${rest}" ]; do
        entry=${rest%%,*}
        case ${rest} in
          *,*) rest=${rest#*,} ;;
          *) rest='' ;;
        esac
        entry=$(printf '%s' "${entry}" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        printf '%s' "${entry}" | LC_ALL=C grep -Eq '^[0-9]{2,}-[a-z0-9-]+$' || \
          die1 "blocked_by 条目不合 id 口径: ${entry}"
        if [ -z "${blocked_json}" ]; then
          blocked_json="\"${entry}\""
        else
          blocked_json="${blocked_json}, \"${entry}\""
        fi
      done
    fi
    [ -z "${status}" ] || die2 'open 不得携带 --status（开票状态固定 ready）'
    ;;
  take|flip)
    case ${status} in
      '') die1 "缺少 --status" ;;
    esac
    printf '%s' "${status}" | LC_ALL=C grep -Eq '^(ready|in_progress|blocked|review_ready|review_pass|review_fail|done|superseded)$' || \
      die1 "status 不合状态机取值（裸值）: ${status}"
    if [ "${cmd}" = 'take' ] && [ "${status}" != 'in_progress' ]; then
      die1 "take 的 --status 须为 in_progress（领取语义；其他翻转用 flip）: ${status}"
    fi
    [ -z "${complexity}" ] || die2 'take/flip 不得携带 --complexity'
    [ -z "${title}" ] || die2 'take/flip 不得携带 --title'
    [ -z "${blocked_by}" ] || die2 'take/flip 不得携带 --blocked-by'
    ;;
  ledger)
    [ -z "${id}" ] || die2 'ledger 不得携带 --id（无票账本行入口不依赖票 id，票 100）'
    [ -z "${status}" ] || die2 'ledger 不得携带 --status（无票面状态可翻，票 100）'
    [ -z "${complexity}" ] || die2 'ledger 不得携带 --complexity'
    [ -z "${title}" ] || die2 'ledger 不得携带 --title'
    [ -z "${blocked_by}" ] || die2 'ledger 不得携带 --blocked-by'
    ;;
esac

index_file="${repo_root}/${issues_index_rel}"
readme_file="${repo_root}/${issues_readme_rel}"
ledger_file="${repo_root}/${changes_ledger_rel}"
# 票 79：open 本体落盘位与 schema 权威（development-process §5.2 开票形态注记）
ticket_file="${repo_root}/${issues_dir_rel}/${id}.json"
# Agent Up is an external skill package, not a project subdirectory. Resolve its
# installed schema from the known local skill root; fail closed when unavailable.
schema_file="${AGENTUP_SKILL_ROOT:-/Users/bic/.agents/skills/agent-up}/references/schemas/ticket-record.schema.json"

# 票 100：flip 收口硬拦载体定位（actual_time 校验用）；ledger 无票面，不要求票本体。
case ${cmd} in
  ledger) ;;
  *)
    [ -f "${index_file}" ] || die1 "索引文件不存在: ${index_file}（票状态真相源缺失，fail-closed）"
    [ -r "${index_file}" ] || die1 "索引文件不可读: ${index_file}"
    [ -f "${readme_file}" ] || die1 "issues-README 不存在: ${readme_file}"
    [ -r "${readme_file}" ] || die1 "issues-README 不可读: ${readme_file}"
    ;;
esac

nn=${id%%-*}

# ---- actual_time 收口硬拦（票 100，flip 专属） ----

actual_time_check() {
  # flip 收口硬拦（票 100，决策 2 fail-closed）：task 票 JSON 本体须含 actual_time 字段
  # 且必须是真实测量值（净工时口径；strip 后非空，N/A + reason 占位也拒绝），缺项、空白或占位值即 exit 1 指名
  # 票 id 与缺项，零写入。request 票不校验（open 时点已过 schema 校验的 kind 为准；
  # 本体缺失不属本断言语义——由 open 时点保证存在，此处缺本体即报缺项指名票 id）。
  command -v python3 >/dev/null 2>&1 || \
    die1 'python3 不可用——flip actual_time 收口校验无法执行（fail-closed；票 100 起为 flip 校验依赖）'
  [ -r "${ticket_file}" ] || \
    die1 "票 ${id} 收口缺 actual_time：票本体不可读: ${ticket_file}（flip 校验须先读本票本体核 actual_time——票 100）"
  python3 - "${ticket_file}" "${id}" <<'PYEOF'
import json
import sys

ticket_path, tid = sys.argv[1], sys.argv[2]

try:
    with open(ticket_path, "r", encoding="utf-8") as fh:
        body = json.load(fh)
except (OSError, json.JSONDecodeError) as exc:
    sys.stderr.write(f"ticket-ops: FAIL: 票 {tid} 收口缺 actual_time：票本体不可读或非合法 JSON: {ticket_path}（{exc}）——票 100\n")
    sys.exit(1)

if not isinstance(body, dict):
    sys.stderr.write(f"ticket-ops: FAIL: 票 {tid} 收口缺 actual_time：票本体顶层须为 JSON object——票 100\n")
    sys.exit(1)

if body.get("kind") != "task":
    sys.exit(0)  # request 票不校验（票 100 AC2）

value = body.get("actual_time")
if not isinstance(value, str) or not value.strip() or value.strip().lower().startswith("n/a"):
    sys.stderr.write(f"ticket-ops: FAIL: 票 {tid} 收口缺 actual_time：task 票收口须填 actual_time（净工时口径，实际完成时间；会话段累计减中断段，测量基础随票 Checkpoint 落盘）——票 100\n")
    sys.exit(1)

sys.exit(0)
PYEOF
}

# ---- 索引与 README 锚点预检（只扫描不写入；任何不合规在写入前停止） ----

index_check() {
  # open parses and edits only the top-level issues array. Keep non-issues metadata
  # byte-for-byte intact, and prepare the replacement before any project file is written.
  if [ "$1" = 'open' ]; then
    t_dir=${TMPDIR:-/tmp}
    tmp_idx=$(mktemp "${t_dir%/}/tix-idx.XXXXXX")
    if ! IDX_ID=${id} IDX_NN=${nn} IDX_COMPLEXITY=${complexity} \
      IDX_BLOCKED="[${blocked_json}]" IDX_UA=${now_ua} \
      python3 - "${index_file}" "${tmp_idx}" <<'PYEOF'
import json
import os
import re
import sys


def fail(message):
    sys.stderr.write(f"ticket-ops: FAIL: {message}\n")
    sys.exit(1)


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"重复 JSON key: {key}")
        value[key] = item
    return value


def skip_space(text, position):
    while position < len(text) and text[position] in " \t\r\n":
        position += 1
    return position


try:
    with open(sys.argv[1], "r", encoding="utf-8") as handle:
        text = handle.read()
    decoder = json.JSONDecoder(object_pairs_hook=unique_object)
    index = json.loads(text, object_pairs_hook=unique_object)
except (OSError, UnicodeError, ValueError) as exc:
    fail(f"索引不是无重复键的合法 JSON（{exc}）")

if not isinstance(index, dict):
    fail("任务票索引顶层须为 JSON object")
issues = index.get("issues")
if not isinstance(issues, list):
    fail("正式任务索引顶层 issues 须为数组")

# Locate the exact source span of the unique top-level issues value. Other top-level
# values may contain arbitrary objects (including id fields) and are never insertion targets.
position = skip_space(text, 0)
if position >= len(text) or text[position] != "{":
    fail("任务票索引顶层须为 JSON object")
position += 1
issues_start = None
issues_end = None
top_keys = set()
metadata_spans = []
while True:
    position = skip_space(text, position)
    if position < len(text) and text[position] == "}":
        position += 1
        break
    try:
        key, key_end = decoder.raw_decode(text, position)
    except ValueError as exc:
        fail(f"索引顶层 key 无法解析（{exc}）")
    if not isinstance(key, str) or key in top_keys:
        fail("索引顶层 key 无效或重复")
    top_keys.add(key)
    position = skip_space(text, key_end)
    if position >= len(text) or text[position] != ":":
        fail("索引顶层 key 缺少冒号")
    value_start = skip_space(text, position + 1)
    try:
        value, value_end = decoder.raw_decode(text, value_start)
    except ValueError as exc:
        fail(f"索引顶层 value 无法解析（{exc}）")
    if key == "issues":
        issues_start, issues_end = value_start, value_end
    else:
        metadata_spans.append((key, value_start, value_end))
    position = skip_space(text, value_end)
    if position < len(text) and text[position] == ",":
        position += 1
        continue
    if position < len(text) and text[position] == "}":
        position += 1
        break
    fail("索引顶层对象分隔符不合预期")
if skip_space(text, position) != len(text) or issues_start is None:
    fail("索引缺少唯一顶层 issues 数组或含尾随数据")
for key, start, end in metadata_spans:
    if re.search(r'"id": "[0-9]', text[start:end]):
        fail(f"顶层元数据 {key} 含任务索引锚点，会污染现有投影；拒绝开票")

position = issues_start + 1
item_spans = []
while True:
    position = skip_space(text, position)
    if position >= issues_end or text[position] == "]":
        break
    try:
        item, item_end = decoder.raw_decode(text, position)
    except ValueError as exc:
        fail(f"issues 数组条目无法解析（{exc}）")
    if not isinstance(item, dict):
        fail("issues 数组条目须为 object")
    raw_item = text[position:item_end]
    if "\n" in raw_item or "\r" in raw_item:
        fail("issues 条目必须保持一条目一行排版")
    if any(anchor not in raw_item for anchor in ('"id": "', '"status": "', '"updated_at": "')):
        fail("issues 条目缺同行的 id/status/updated_at 字段，拒绝单写")
    if not all(isinstance(item.get(field), str) and item[field] for field in ("id", "status", "updated_at")):
        fail("issues 条目的 id/status/updated_at 须为非空字符串")
    line_number = text.count("\n", 0, position)
    if item_spans and item_spans[-1][2] == line_number:
        fail("issues 每条任务须独占一行")
    item_spans.append((item, raw_item, line_number))
    position = skip_space(text, item_end)
    if position < issues_end and text[position] == ",":
        position += 1
        continue
    if position < issues_end and text[position] == "]":
        break
    fail("issues 数组分隔符不合预期")

ticket_id = os.environ["IDX_ID"]
ticket_nn = os.environ["IDX_NN"]
hits = 0
occupied = []
for item, _, _ in item_spans:
    existing_id = item["id"]
    if existing_id == ticket_id:
        hits += 1
    if re.match(r"^[0-9][0-9]+-", existing_id):
        existing_nn = existing_id.split("-", 1)[0]
        if existing_nn == ticket_nn:
            occupied.append(existing_id)
if hits:
    fail(f"索引条目 id 已存在（open 非幂等，命中 {hits} 条）: {ticket_id}")
if occupied:
    fail(f"票号段已被占用（同 NN 异 slug 撞号，open 拒开）: NN {ticket_nn} 已占 id → {', '.join(occupied)}")

entry = {
    "id": ticket_id,
    "status": "ready",
    "complexity": os.environ["IDX_COMPLEXITY"],
    "blocked_by": json.loads(os.environ["IDX_BLOCKED"]),
    "updated_at": os.environ["IDX_UA"],
}
new_row = json.dumps(entry, ensure_ascii=False, separators=(",", ": "))
close_line = text.rfind("\n", 0, issues_end - 1) + 1
close_indent = text[close_line:issues_end - 1]
if not close_indent.isspace() and close_indent != "":
    close_indent = ""
if not item_spans:
    entry_indent = close_indent + "  "
    rows = [new_row]
else:
    entry_indent = "    "
    rows = [raw.strip() for _, raw, _ in item_spans] + [new_row]
replacement = "[\n" + entry_indent + (",\n" + entry_indent).join(rows) + "\n" + close_indent + "]"
updated = text[:issues_start] + replacement + text[issues_end:]
try:
    with open(sys.argv[2], "w", encoding="utf-8", newline="") as handle:
        handle.write(updated)
except OSError as exc:
    fail(f"无法准备 issues 数组替换文件（{exc}）")

PYEOF
    then
      rm -f "${tmp_idx}"; tmp_idx=''
      return 1
    fi
    return 0
  fi

  IDX_MODE=$1 IDX_ID=${id} awk '
    BEGIN {
      mode = ENVIRON["IDX_MODE"]
      id = ENVIRON["IDX_ID"]
      anchor = "\"id\": \"" id "\""
    }
    index($0, "\"id\": \"") > 0 {
      n++
      if ($0 !~ /"status": "/ || $0 !~ /"updated_at": "/) {
        printf "ticket-ops: FAIL: 索引第 %d 行条目行缺同行的 status/updated_at 字段——一条目一行排版被破坏（票 33 终裁 T2）\n", FNR > "/dev/stderr"
        bad = 1
      }
      if (index($0, anchor) > 0) hits++
    }
    END {
      rc = 0
      if (n == 0) { printf "ticket-ops: FAIL: 索引中未找到条目行（\"id\": 锚点零命中）\n" > "/dev/stderr"; rc = 1 }
      if (hits != 1) { printf "ticket-ops: FAIL: 索引条目锚点命中 %d 行（须恰 1 行）: %s\n", hits, id > "/dev/stderr"; rc = 1 }
      if (bad) rc = 1
      exit rc
    }
  ' "${index_file}"
}

readme_row_check() {
  # open 预检：目录清单表行锚点（^\| `）须至少 1 行
  awk '
    /^\| `/ { hits++ }
    END {
      rc = 0
      if (hits == 0) { printf "ticket-ops: FAIL: issues-README 未找到目录清单表行（^| ` 锚点零命中）\n" > "/dev/stderr"; rc = 1 }
      exit rc
    }
  ' "${readme_file}"
}

readme_token_check() {
  # take/flip 预检：`任务票 <NN>；` 锚点恰 1 行，且其后存在闭合的反引号状态 token
  # 实现注记：位置运算只用 index/substr（同一单位），不用 length——macOS awk 对多字节
  # 字符串 index 回字符位、length 回字节数，混用会错位（票 41 夹具实证）。
  RD_ANCHOR="任务票 ${nn}；" awk '
    BEGIN { anchor = ENVIRON["RD_ANCHOR"] }
    index($0, anchor) > 0 {
      hits++
      p = index($0, anchor)
      s = substr($0, p)
      j = index(s, "`")
      s2 = ""
      if (j > 0) s2 = substr(s, j + 1)
      k = 0
      if (j > 0) k = index(s2, "`")
      if (j == 0 || k == 0) {
        printf "ticket-ops: FAIL: issues-README 第 %d 行锚点后反引号状态 token 缺失或未闭合\n", FNR > "/dev/stderr"
        bad = 1
      }
    }
    END {
      rc = 0
      if (hits != 1) { printf "ticket-ops: FAIL: issues-README 状态行锚点命中 %d 行（须恰 1 行）: %s\n", hits, anchor > "/dev/stderr"; rc = 1 }
      if (bad) rc = 1
      exit rc
    }
  ' "${readme_file}"
}

ticket_body_check() {
  # open 专属预检（票 79）：新票 JSON 本体过 ticket-record.schema.json 校验＋理由非空
  # 断言——required 集（顶层＋allOf if kind=task then）自 schema 文件现场提取，schema 为
  # 唯一权威；非空＝字符串 strip 后非空（blocked_by 须数组，空数组合法＝无依赖）；另断言
  # 本体 id 与 --id 一致（防校验错文件）。只读零写入；任何不过即 exit 1（报文指名缺失项）。
  command -v python3 >/dev/null 2>&1 || \
    die1 'python3 不可用——open 本体 schema 校验无法执行（fail-closed；票 79 起为本体校验依赖）'
  [ -f "${ticket_file}" ] || \
    die1 "新票本体不存在: ${issues_dir_rel}/${id}.json——票 79 起 open 须先落位过 schema 的票 JSON 本体再开票"
  [ -r "${ticket_file}" ] || die1 "新票本体不可读: ${ticket_file}"
  [ -f "${schema_file}" ] || \
    die1 "schema 文件不存在: ${schema_file}（open 本体校验 fail-closed；schema 权威＝Agent Up 包内 references/schemas/ticket-record.schema.json，development-process §5.2）"
  [ -r "${schema_file}" ] || die1 "schema 文件不可读: ${schema_file}"
  python3 - "${ticket_file}" "${schema_file}" "${id}" <<'PYEOF'
import json
import sys

ticket_path, schema_path, want_id = sys.argv[1], sys.argv[2], sys.argv[3]


def fail(msg):
    sys.stderr.write(f"ticket-ops: FAIL: {msg}\n")
    sys.exit(1)


try:
    with open(ticket_path, "r", encoding="utf-8") as fh:
        body = json.load(fh)
except json.JSONDecodeError as exc:
    fail(f"新票本体非合法 JSON: facts/requirements/tickets/{want_id}.json（第 {exc.lineno} 行 {exc.msg}）")
except OSError as exc:
    fail(f"新票本体不可读: {ticket_path}（{exc.strerror}）")

if not isinstance(body, dict):
    fail(f"新票本体顶层须为 JSON object: {ticket_path}")

try:
    with open(schema_path, "r", encoding="utf-8") as fh:
        schema = json.load(fh)
except (OSError, json.JSONDecodeError) as exc:
    fail(f"schema 文件不可读或非合法 JSON: {schema_path}（{exc}）")

if not isinstance(schema, dict) or not isinstance(schema.get("required"), list):
    fail(f"schema 形状不合预期（缺顶层 required 数组）: {schema_path}——fail-closed")

problems = []

body_id = body.get("id")
if body_id != want_id:
    problems.append(f"本体 id 与 open --id 不一致（本体 {body_id!r} != --id {want_id!r}）")

kind_props = (schema.get("properties") or {}).get("kind") or {}
kind_enum = kind_props.get("enum") if isinstance(kind_props, dict) else None
body_kind = body.get("kind")
if isinstance(kind_enum, list) and body_kind is not None and body_kind not in kind_enum:
    problems.append(f"kind 不在 schema 枚举内: {body_kind!r}")


def check_required(names, scope):
    # 非空口径：字符串 strip 后非空（空白串按空拒，票 79 只断言非空不做内容判断）
    for name in names:
        if name not in body:
            problems.append(f"{scope}缺少 {name}")
            continue
        value = body[name]
        if name == "blocked_by":
            if not isinstance(value, list):
                problems.append(f"{scope}{name} 须为数组（无依赖为空数组）")
            continue
        if not isinstance(value, str) or not value.strip():
            problems.append(f"{scope}{name} 为空或非字符串（非空断言，票 79）")


check_required(schema["required"], "顶层 ")

task_required = []
for sub in schema.get("allOf") or []:
    if not isinstance(sub, dict):
        continue
    cond_props = (sub.get("if") or {}).get("properties") or {}
    if isinstance(cond_props.get("kind"), dict) and cond_props["kind"].get("const") == "task":
        then = sub.get("then") or {}
        if isinstance(then.get("required"), list):
            task_required.extend(then["required"])
if body_kind == "task":
    check_required(task_required, "task ")

if problems:
    fail("新票本体 schema 校验未过（" + schema_path + "）：" + "；".join(problems))
sys.exit(0)
PYEOF
}

# 生成器定位（收尾投影再生必需；同目录优先，PATH 回退；缺失即预检停止）
# ledger（票 100 无票账本行入口）不触投影面，不要求生成器在位。
gen_cmd=''
if [ "${cmd}" = 'ledger' ]; then
  :
elif [ -f "${script_dir}/generate-progress.sh" ] && [ -r "${script_dir}/generate-progress.sh" ]; then
  gen_cmd="${script_dir}/generate-progress.sh"
  printf 'ticket-ops: 生成器定位: 同目录 %s\n' "${gen_cmd}"
else
  _rest=${PATH:-}
  while [ -n "${_rest}" ]; do
    _dir=${_rest%%:*}
    case ${_rest} in
      *:*) _rest=${_rest#*:} ;;
      *) _rest='' ;;
    esac
    [ -n "${_dir}" ] || continue
    if [ -f "${_dir}/generate-progress.sh" ] && [ -r "${_dir}/generate-progress.sh" ]; then
      gen_cmd="${_dir}/generate-progress.sh"
      break
    fi
  done
  if [ -n "${gen_cmd}" ]; then
    printf 'ticket-ops: 生成器定位: PATH %s\n' "${gen_cmd}"
  else
    die1 "投影生成器 generate-progress.sh 不可用（同目录与 PATH 均未找到）——预检停止：按 development-process §12.5 派生载体独占写，票务面写入必须伴随投影再生；将生成器落位到本脚本同目录（${scripts_dir_rel}/）或 PATH 后重跑"
  fi
fi

t_dir=${TMPDIR:-/tmp}
now_ua=$(date +%Y-%m-%dT%H:%M)
today=$(date +%F)

if [ "${cmd}" = 'open' ]; then
  ticket_body_check || die1 "新票本体校验未通过: ${issues_dir_rel}/${id}.json——未产生任何写入"
  index_check open || die1 "索引预检未通过: ${issues_index_rel}（${id}）——未产生任何写入"
  readme_row_check || die1 "issues-README 预检未通过: ${issues_readme_rel}——未产生任何写入"
elif [ "${cmd}" = 'ledger' ]; then
  : # 无票面：账本行形状校验已于公共段完成（票 100），无索引/README 预检
else
  index_check flip || die1 "索引预检未通过: ${issues_index_rel}（${id}）——未产生任何写入"
  readme_token_check || die1 "issues-README 预检未通过: ${issues_readme_rel}（任务票 ${nn}）——未产生任何写入"
  if [ "${cmd}" = 'flip' ]; then
    actual_time_check || die1 "actual_time 收口校验未通过: ${issues_dir_rel}/${id}.json——未产生任何写入（票 100 fail-closed）"
  fi
fi

# ---- 写入（索引 → README → 账本 → 投影再生＋--check） ----

if [ "${cmd}" = 'open' ]; then
  mv "${tmp_idx}" "${index_file}"
  tmp_idx=''
  printf "ticket-ops: 索引新增条目: ${issues_index_rel}（%s → ready，updated_at %s）\n" "${id}" "${now_ua}"

  row="| \`${id}.json\` | 任务票 ${nn}；\`ready\`（${today} 开票） | ${title} |"
  tmp_readme=$(mktemp "${t_dir%/}/tix-readme.XXXXXX")
  RD_ROW=${row} awk '
    BEGIN { row = ENVIRON["RD_ROW"] }
    { lines[++n] = $0; if ($0 ~ /^\| `/) last = n }
    END {
      if (last == 0) { printf "ticket-ops: FAIL: issues-README 目录清单表行锚点零命中——拒绝插入\n" > "/dev/stderr"; exit 1 }
      for (i = 1; i <= n; i++) { print lines[i]; if (i == last) print row }
    }
  ' "${readme_file}" > "${tmp_readme}" || {
    rm -f "${tmp_readme}"; tmp_readme=''
    die1 "issues-README 行插入未通过校验——索引已写入（${id}），README 未写入"
  }
  mv "${tmp_readme}" "${readme_file}"
  tmp_readme=''
  printf "ticket-ops: README 追加目录清单行: ${issues_readme_rel}（任务票 %s；\`ready\`）\n" "${nn}"
elif [ "${cmd}" != 'ledger' ]; then
  tmp_idx=$(mktemp "${t_dir%/}/tix-idx.XXXXXX")
  IDX_ID=${id} IDX_STATUS=${status} IDX_UA=${now_ua} awk '
    BEGIN {
      id = ENVIRON["IDX_ID"]; nstatus = ENVIRON["IDX_STATUS"]; nua = ENVIRON["IDX_UA"]
      anchor = "\"id\": \"" id "\""
    }
    index($0, "\"id\": \"") > 0 {
      n++
      if ($0 !~ /"status": "/ || $0 !~ /"updated_at": "/) {
        printf "ticket-ops: FAIL: 索引第 %d 行条目行缺同行的 status/updated_at 字段——拒绝单写\n", FNR > "/dev/stderr"
        bad = 1
        print
        next
      }
      if (index($0, anchor) > 0) {
        hits++
        line = $0
        sub(/"status": "[^"]*"/, "\"status\": \"" nstatus "\"", line)
        sub(/"updated_at": "[^"]*"/, "\"updated_at\": \"" nua "\"", line)
        print line
        next
      }
      print
      next
    }
    { print }
    END {
      rc = 0
      if (n == 0) { printf "ticket-ops: FAIL: 索引中未找到条目行\n" > "/dev/stderr"; rc = 1 }
      if (hits != 1) { printf "ticket-ops: FAIL: 索引条目锚点命中 %d 行（须恰 1 行）: %s\n", hits, id > "/dev/stderr"; rc = 1 }
      if (bad) rc = 1
      exit rc
    }
  ' "${index_file}" > "${tmp_idx}" || {
    rm -f "${tmp_idx}"; tmp_idx=''
    die1 "索引单写未通过校验: ${issues_index_rel}（${id}）——未产生任何写入"
  }
  mv "${tmp_idx}" "${index_file}"
  tmp_idx=''
  printf "ticket-ops: 索引单写: ${issues_index_rel}（%s → %s，updated_at %s）\n" "${id}" "${status}" "${now_ua}"

  tmp_readme=$(mktemp "${t_dir%/}/tix-readme.XXXXXX")
  RD_ANCHOR="任务票 ${nn}；" RD_STATUS=${status} awk '
    BEGIN { anchor = ENVIRON["RD_ANCHOR"]; nstatus = ENVIRON["RD_STATUS"] }
    index($0, anchor) > 0 {
      hits++
      p = index($0, anchor)
      s = substr($0, p)
      j = index(s, "`")
      s2 = ""
      if (j > 0) s2 = substr(s, j + 1)
      k = 0
      if (j > 0) k = index(s2, "`")
      if (j == 0 || k == 0) {
        printf "ticket-ops: FAIL: issues-README 第 %d 行锚点后反引号状态 token 缺失或未闭合——拒绝改写\n", FNR > "/dev/stderr"
        bad = 1
        print
        next
      }
      print substr($0, 1, p - 1) substr(s, 1, j) nstatus substr(s2, k)
      next
    }
    { print }
    END {
      rc = 0
      if (hits != 1) { printf "ticket-ops: FAIL: issues-README 状态行锚点命中 %d 行（须恰 1 行）: %s\n", hits, anchor > "/dev/stderr"; rc = 1 }
      if (bad) rc = 1
      exit rc
    }
  ' "${readme_file}" > "${tmp_readme}" || {
    rm -f "${tmp_readme}"; tmp_readme=''
    die1 "README 状态 token 替换未通过校验——索引已单写（${id} → ${status}），README 未写入"
  }
  mv "${tmp_readme}" "${readme_file}"
  tmp_readme=''
  printf "ticket-ops: README 状态列替换: ${issues_readme_rel}（任务票 %s → \`%s\`，行内其余文本不动）\n" "${nn}" "${status}"
fi

if [ ! -f "${ledger_file}" ]; then
  mkdir -p "${repo_root}/$(dirname "${changes_ledger_rel}")"
  : > "${ledger_file}" || die1 "账本懒创建失败: ${ledger_file}——已写部分如实报告"
fi
[ -w "${ledger_file}" ] || die1 "账本不可写: ${ledger_file}——已写部分如实报告"
ledger_lineno=$(( $(wc -l < "${ledger_file}" | tr -d ' ') + 1 ))
printf '%s\n' "${ledger_line}" >> "${ledger_file}" || die1 "账本落行失败: ${ledger_file}——已写部分如实报告"
printf "ticket-ops: 账本落行: ${changes_ledger_rel}（第 %d 行）\n" "${ledger_lineno}"

if [ "${cmd}" = 'ledger' ]; then
  : # 无票面写入收口（票 100）：不触索引/README/投影，仅账本落行
  printf 'ticket-ops: PASS（ledger 完成：账本落行 %d 行，无票面写入——票 100 无票账本行入口）\n' "${ledger_lineno}"
  exit 0
fi

sh "${gen_cmd}" "${repo_root}" || die1 "投影再生失败（生成器 exit 非 0）——已写部分如实报告：索引与 README 与账本（${ledger_lineno} 行）均已写入、投影未刷新；按生成器报因处理后核对重跑"
sh "${gen_cmd}" --check "${repo_root}" || die1 "投影一致性核对未过（--check exit 非 0）——已写部分如实报告：索引与 README 与账本（${ledger_lineno} 行）均已写入；投影与索引不一致，核对后重跑"
printf "ticket-ops: 投影已再生并核对: ${progress_current_rel}\n"

printf 'ticket-ops: PASS（%s 完成：索引、issues-README、账本、投影 --check 全部落地）\n' "${cmd}"
