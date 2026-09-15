<!-- Input: SPEC-001、M0-01 Checkpoint、开发计划中的 M0 交付标准。 -->
<!-- Output: M0-02 的机器契约任务：JSON Schema、command/event API、capabilities 与威胁模型。 -->
<!-- Pos: M0-02 任务票；不实现 Tauri runtime、renderer 或事实 writer。 -->

# 02 — M0-02 Runtime 合同

**Type:** docs

**Priority:** P1

**What to build:** 把 M0-01 的状态与事实合同转成机器可检查的 JSON Schema，并形成最小 Tauri command/event API、capabilities 清单和威胁模型，为事实层实现提供可验证边界。

**Blocked by:** 01 — M0-01 状态与事实合同（done）。

**Status:** done

- [x] `fact-record` 和 `event` JSON Schema 可解析且覆盖 M0-01 的必填字段与 M0 type-specific payload。
- [x] command/event API 对每个命令的输入、输出、副作用、错误和权限有明确记录，并有 registry/result/error schema。
- [x] capabilities 清单遵守 renderer 默认无系统 API 权限和项目根目录边界。
- [x] 威胁模型覆盖路径逃逸、未确认写入、revision 冲突、事件重放和敏感数据泄露。
- [x] schemas 目录、产物索引、进度和变更记录同步。

## Implementation Checkpoint

- **2026-09-14 / review_ready**：已新增 `schemas/fact-record.schema.json`、`schemas/event.schema.json`、`docs/specs/002-m0-runtime-contract.md` 和 `schemas/README.md`；合同覆盖五个 typed command、四类 event、最小 deny-by-default capabilities，以及五类威胁。已同步根 README、specs README、产物索引、progress 和 changes。
- **验证**：Ruby JSON.parse、JSON Schema 关键字段静态检查、YAML 解析/字段/路径检查和 `git diff --check` 已运行；Draft 2020-12 validator 未发现本地可用命令，已记录为 N/A。
- **澄清**：只读 command 产生的 `scan_completed`/`request.rehydrated` 是 runtime notification，不追加事实；confirmation token 只提供预览绑定，不能证明受攻陷 renderer 中的真实人类确认。
- **Review 修复**：fact content 改为 type-specific `oneOf`（request/project/manifest 与其他记录）；event schema 约束 event/aggregate/delivery/payload 组合；新增 command registry schema、五命令 registry fixture、result/error schemas；固定 `fp-v1` 算法、manifest revision 来源和临时目录原子提交/失败清理策略。
- **2026-09-15 / review_ready**：修复 Independent Review fail 的两个 P0，并收紧并列缺口。`command-error.schema.json` 允许且仅允许 `details.relative_path|revision|field`，`revision_conflict` 强制 `details.revision`。`SPEC-002` 以 `fp-v1` 为唯一 fingerprint 权威，删除「算法由实现规格另行固定」。result schema 按 command 约束 `data`；registry schema 按 command name 绑定输入/输出/副作用/错误/capability/emits。`initialize_project` 补 `invalid_input`/`path_not_found`；`preview_initialize` 补 `permission_denied`。明确 schema_version 1 request `lifecycle` 只能是 `draft`。未实现 runtime，不自审，不 commit。
- **验证**：2026-09-15；Ruby JSON.parse 六份 schema/fixture；静态检查 error details 三键、`revision_conflict` if/then、scan 段无「另行固定」、registry 五命令错误列表与 SPEC 表一致；YAML 26 条/14 字段/路径；`git diff --check`。Draft 2020-12 validator 仍不可用，记 `N/A + reason`。

- **2026-09-15 / review_ready（错误 envelope）**：对齐 Independent Review 新 P0。采用已有 schema 形状，不扁平化。`SPEC-002` 错误结果顶层仅为 `ok: false`、`command`、`error`；`error` 内为 `error.code`、`error.message`、可选 `error.details`。`revision_conflict` 与威胁模型改为必须写 `error.details.revision`。`command-error.schema.json` 已是该包装，未改；未改 fact/event schema，未实现 runtime，不自审，不 commit。
- **验证**：2026-09-15；`ruby JSON.parse` 六份 `schemas/*.json` 通过；全文检索 SPEC：错误结果顶层不再把 `code`/`message`/`details` 写成与 `ok`/`command` 同级；`details` 只作为 `error.details`；`git diff --check` 通过；`docs/agent/artifacts.yaml` 未改，`docs/specs/002-m0-runtime-contract.md` 与 `schemas/command-error.schema.json` 路径仍存在。Draft 2020-12 validator 仍不可用，记 `N/A + reason`。需要另一个未参与本次修复的执行体做独立 Review。

- **2026-09-15 / done**：用户在 Independent Review pass 之后回复「继续」，Coordinator 将其记录为批准 `SPEC-002` 并关闭本票。本票不包含 Tauri runtime 实现。


## Independent Review Checkpoint

- **2026-09-14 / review_fail**
- **结论**：fail。不得进入 Commit，不得批准 `SPEC-002`，不得拆 M0 runtime 实现票。
- **审查范围**：M0-02 任务合同、Implementation Checkpoint、`docs/specs/002-m0-runtime-contract.md`、`docs/specs/001-m0-foundation.md`、`schemas/*`、相关 README/产物登记、未提交 diff 快照。不审查 Tauri runtime 实现（本票范围内不存在）。
- **独立性**：本会话为新 Codex thread，首条用户指令为「继续」；未参与受审内容实现、未共享实现上下文。审查身份：Review 执行体；模型：grok-4.6；时间：2026-09-14。依据 `R-RR-002`，此路径可独立。
- **Diff / 证据快照**：`HEAD=8c6eee996750595a793f32b262d69d82180a8639`；受审内容为工作区未提交变更与未跟踪文件（含 `schemas/`、`docs/specs/002-m0-runtime-contract.md`、本票）。审查前后未修改受审 schema/规格。

### 验证

| 检查 | 结果 | 环境 |
| --- | --- | --- |
| `ruby -rjson -e 'ARGV.each { \|path\| JSON.parse(File.read(path)); puts path }' schemas/*.json` | pass：6 个 JSON 可解析 | Ruby JSON.parse；cwd `/Users/bic/Projects/agent-up` |
| `git diff --check` | pass：无空白错误 | git；相对 `HEAD` |
| `docs/agent/artifacts.yaml` 14 字段、路径存在、无重复 path | pass：26 条 | Ruby YAML |
| Draft 2020-12 validator | N/A + reason：本机无 `ajv` / `check-jsonschema` / Python `jsonschema`；JSON.parse 不能证明 schema 语义 | node v22.23.1；python3 无 jsonschema |

### 逐项验收

| 任务验收 | 判断 | 证据 |
| --- | --- | --- |
| fact/event schema 可解析且覆盖 M0 envelope 与 M0 type-specific payload | 部分成立，不单独构成 fail | `schemas/fact-record.schema.json:11-14,27-43`；`schemas/event.schema.json:8-15`；request/project/manifest/event 组合有约束。八种非 M0 用户类型仍是 `generic-content`（`minProperties: 1`），与 `docs/specs/001-m0-foundation.md:104`「补齐每种记录类型」有缺口，但本票验收写的是 M0 payload。 |
| command/event API 有明确记录，并有 registry/result/error schema | **不成立** | 文件存在且五命令 fixture 与 prose 表字段名对齐（`schemas/command-registry.json:4-8` vs `docs/specs/002-m0-runtime-contract.md:44-105`）。但 error schema 无法表达规格要求的 details，见阻塞问题 1。 |
| capabilities 默认拒绝、renderer 无系统 API | 成立 | `docs/specs/002-m0-runtime-contract.md:119-132` |
| 威胁模型覆盖五类威胁 | **不成立（机器合同层）** | prose 有五类（`docs/specs/002-m0-runtime-contract.md:134-147`）；`revision_conflict` 要求「报告当前 revision」，error schema 禁止该字段，见阻塞问题 1。 |
| 目录/索引/进度/变更同步 | 成立（登记层） | `schemas/README.md:12-19`；artifacts 含 schema/spec/issue 登记；路径均存在。 |

### 阻塞问题

1. **错误 details 合同自相矛盾（P0）**  
   `docs/specs/002-m0-runtime-contract.md:40` 要求错误 `details` 只包含可安全展示的相对路径、revision 或字段名；同文件 `142` 要求 `revision_conflict`「报告当前 revision」。`schemas/command-error.schema.json:7` 将 `details` 定义为 `{ "type": "object", "additionalProperties": false }` 且无 `properties`，因此唯一合法 details 是 `{}`。实现要么违反 schema，要么无法按威胁模型报告 revision/字段/相对路径。这直接否定「实现不得只依赖 prose」。

2. **fingerprint 算法双重权威（P0）**  
   Implementation Checkpoint 声称已固定 `fp-v1`。`docs/specs/002-m0-runtime-contract.md:30-32` 确实给出算法。同文件 `56` 又写「其算法由实现规格另行固定」。同一批准前规格里两句互斥，runtime 实现票无法选择权威来源。

### 非阻塞缺口（不单独 fail，修复时一并处理）

- `schemas/command-result.schema.json:7` 的 `data` 为无约束 object；registry 只列字段名、不列类型，成功输出仍不可机器检查。
- `schemas/command-registry.schema.json:8` 的 `errors.items` 是自由字符串，未绑定公共错误码枚举，也不按 command name 约束输入/输出。
- `schemas/fact-record.schema.json:29` 把 request `lifecycle` 冻成 `draft`；若这是 schema_version 1 的 M0 冻结，规格未显式声明。`39-43` 的 generic 类型仍可塞任意对象。
- `initialize_project` 错误表无 `invalid_input` / `path_not_found`（prose 与 registry 一致）；路径消失或输入畸形时出口不明。
- Draft 2020-12 语义校验仍为 N/A。

### 后续

交回 Implementation。必须先消除两个 P0（error details 白名单与 fingerprint 单一权威），并让 prose、registry、result/error schema 对得上。修复后保持 `review_ready`，由**另一个**未参与该修复的执行体重审。本 Review 不代修。


- **2026-09-15 / review_fail**
- **结论**：fail。不得进入 Commit，不得批准 `SPEC-002`，不得拆 M0 runtime 实现票。
- **审查范围**：M0-02 任务合同、Implementation Checkpoint、当前 `docs/specs/002-m0-runtime-contract.md`、`docs/specs/001-m0-foundation.md`、`schemas/*`、相关 README/产物登记、工作区未提交快照。不审查 Tauri runtime 实现（本票范围内不存在）。未采信 Implementation 自述，按当前文件重审。
- **独立性**：本会话为新执行体，首条用户指令即为独立 Review；未参与 schema/规格实现，未共享实现上下文。审查身份：Review 执行体；模型：grok-4.6；时间：2026-09-15。依据 `R-RR-002`，此路径可独立。
- **Diff / 证据快照**：`HEAD=8c6eee996750595a793f32b262d69d82180a8639`；受审内容仍为工作区未提交变更与未跟踪文件。本 Review 只改本票 `Status` 并追加本 Checkpoint；未修改 schema/规格/登记。

### 验证（2026-09-15，本执行体实跑）

| 检查 | 结果 | 环境 |
| --- | --- | --- |
| `ruby -rjson -e 'ARGV.each { \|path\| JSON.parse(File.read(path)); puts path }' schemas/*.json` | pass：6 个 JSON 可解析（`command-error.schema.json`、`command-registry.json`、`command-registry.schema.json`、`command-result.schema.json`、`event.schema.json`、`fact-record.schema.json`） | Ruby 2.6.10p210；cwd `/Users/bic/Projects/agent-up` |
| `git diff --check` | pass：无空白错误 | git；相对 `HEAD` |
| `docs/agent/artifacts.yaml` 14 字段、路径存在、无重复 id/path | pass：26 条，缺键/空值/缺文件均为空 | Ruby YAML（Psych） |
| Draft 2020-12 validator | N/A + reason：本机无 `ajv` CLI、无 `check-jsonschema`、无 Python `jsonschema`、无 Node `ajv` 模块；JSON.parse 不能证明 schema 语义 | node v22.23.1；Python 3.14.6 |

### 上次 P0 复核

| P0 | 判断 | 证据 |
| --- | --- | --- |
| 1. command-error `details` 能否表达相对路径、revision、字段名；`revision_conflict` 能否报告当前 revision；`{}` 不得是唯一合法 details | **已关闭** | `docs/specs/002-m0-runtime-contract.md:42` 将 `details` 限制为 `relative_path`/`revision`/`field` 且至少一项，并要求 `revision_conflict` 写 `details.revision`。`schemas/command-error.schema.json:95-118` 的 `$defs/details` 有三键白名单、`additionalProperties: false`、`minProperties: 1`；`:56-58` 引用该定义；`:60-91` 在 `code=revision_conflict` 时强制 `details` 且 `details.revision` 为整数 `minimum: 0`。空对象 `{}` 不再合法。 |
| 2. fingerprint 是否只有一个权威 fp-v1；SPEC 不得再出现「算法由实现规格另行固定」或等价双重权威 | **已关闭** | `docs/specs/002-m0-runtime-contract.md:32-34` 定义唯一算法 `fp-v1`；同文件 `:58` 写明必须使用本合同算法、不得另写实现规格或第二套 hash。全文件检索无「另行固定」。机器形状为 `^fp-v1:[0-9a-f]{16}$`（`schemas/command-result.schema.json:390-392`、`schemas/fact-record.schema.json:15`、`schemas/event.schema.json:10`）。 |

### 并列缺口复核

| 缺口 | 判断 | 证据 |
| --- | --- | --- |
| command-result `data` 按 command 约束 | **已收** | `schemas/command-result.schema.json:29-377` 对五个 command 分别 `if/then` 约束 `data` 的 required/properties/`additionalProperties: false`。 |
| command-registry.schema 绑定公共错误码，并按 command name 约束输入/输出/错误/capability/emits | **已收（registry 层）** | `schemas/command-registry.schema.json:93-109` `$defs/error-code`；`:154-161` `errors.items` 引用该枚举；`:181-449` 按 name 把 input/output/side_effect/errors/capability/emits 冻成 `const`。fixture `schemas/command-registry.json` 与之对齐。 |
| initialize_project / preview_initialize 错误码与 SPEC 表一致 | **已收** | SPEC `docs/specs/002-m0-runtime-contract.md:67,79` 含 `invalid_input`/`path_not_found`（initialize 另有 confirmation_* 等）；registry `:45-51,:71-80` 与 schema `:264-327` 一致。 |
| schema_version 1 request lifecycle 冻结为 draft 是否在规格中显式声明 | **已收** | `docs/specs/002-m0-runtime-contract.md:23`；`schemas/fact-record.schema.json:27-30`。 |

### 逐项验收（本票勾选项）

| 任务验收 | 判断 | 证据 |
| --- | --- | --- |
| fact/event schema 可解析且覆盖 M0 envelope 与 M0 type-specific payload | 成立（M0 范围） | JSON.parse 通过。`schemas/fact-record.schema.json:6-10,27-43`；`schemas/event.schema.json:6,12-15`。八种非 M0 用户类型仍是 `generic-content`（`:40,44`），但 SPEC-002:23 已声明本版本只约束 envelope，且 M0 command 不得创建这些类型。 |
| command/event API 有明确记录，并有 registry/result/error schema | **不成立** | 五命令 prose/registry/result 字段名已对齐，但错误结果 JSON 形状与 prose 互斥，见阻塞问题 1。 |
| capabilities 默认拒绝、renderer 无系统 API | 成立 | `docs/specs/002-m0-runtime-contract.md:119-132` |
| 威胁模型覆盖五类威胁 | 成立（控制表） | `docs/specs/002-m0-runtime-contract.md:134-147`；`revision_conflict` 的 details 机器合同已能表达当前 revision。 |
| 目录/索引/进度/变更同步 | 成立（登记层；本 Review 未改这些文件） | `schemas/README.md:12-19`；artifacts 含 spec/schema/issue 共 26 条，路径均存在。 |

### 阻塞问题

1. **错误结果 envelope 与 prose 互斥（新 P0）**  
   `docs/specs/002-m0-runtime-contract.md:42` 规定错误结果包含顶层字段 `ok: false`、`command`、稳定 `code`、`message`、可选 `details`（与成功结果顶层 `ok`/`command`/`data` 平行）。`schemas/command-error.schema.json:6-11,12-24` 将根对象 `additionalProperties` 设为 false，required 为 `ok`/`command`/`error`，合法键只有这三个；`code`/`message`/`details` 被放进 `:25-59` 的 `error` 包装对象。按 prose 发出的 `{ok:false, command, code, message, details}` 通不过 schema；按 schema 发出的 `{ok:false, command, error:{code,message,details}}` 违反规格字段布局。实现必须猜测权威来源。这直接命中「机器合同与 prose 再出现实现必须猜测的互斥条款」。上次 details 白名单 P0 已关，不能掩盖本条。

### 非阻塞剩余风险（不单独构成此次 fail 的充分条件）

- `schemas/command-error.schema.json` 仍允许任意五命令与任意公共错误码组合；按 command 的错误码白名单只在 registry，不在 error result schema。
- `schemas/command-result.schema.json:334-345` 的 `load_project.data.facts/events` 仍是无约束 object 数组，未 `$ref` fact/event schema。
- `load_project` 可选输入 `project_id` 只在 prose（`docs/specs/002-m0-runtime-contract.md:96`），registry `input_required` 仅 `project_path`。
- Draft 2020-12 语义校验仍为 N/A；`if/then`/`$ref` 组合未经 validator 执行。
- 八种 generic 用户类型 content 仍可塞任意非空对象；与 approved SPEC-001 的「每种类型校验 content」仍有层级张力，但被 proposed SPEC-002:23 显式冻结，且本票验收是 M0 payload。

### 后续

交回 Implementation。必须先消除错误结果顶层字段与 `command-error.schema.json` 的互斥（二选一对齐：要么 schema 改为顶层 `code`/`message`/`details`，要么 SPEC 改为 `error` 包装并同步所有表格）。修完保持 `review_ready`，由**另一个**未参与该修复的执行体重审。本 Review 不代修。


- **2026-09-15 / review_pass（错误 envelope 修复后重审）**
- **结论**：pass。上次阻塞的错误结果 envelope 互斥已消除；三个必须复核的 P0 均关闭。不得把并列缺口升级为本轮 fail。本 Review 不实现、不代修、不 commit。
- **审查范围**：M0-02 任务合同、Implementation Checkpoint（含 2026-09-15 envelope 修复条目）、当前 `docs/specs/002-m0-runtime-contract.md`、`docs/specs/001-m0-foundation.md`、`schemas/*`、`docs/agent/artifacts.yaml` 路径存在性、只读 `git status`。不审查 Tauri runtime 实现（本票范围内不存在）。不改写既有 Checkpoint 正文。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与本次 envelope 修复的 Implementation，未共享该修复的实现上下文。模型：Codex。时间：2026-09-15 11:43 CST。

### P0 复核（envelope 修复后）

| P0 | 判断 | 证据 |
| --- | --- | --- |
| 1. 错误结果 JSON 形状：SPEC prose 与 `command-error.schema.json` 必须同一套。成功 `{ok,command,data}`，错误 `{ok,command,error}`。不得再出现顶层 `code`/`message`/`details` 与 `error` 包装并存的双权威 | **已关闭** | 当前 SPEC `docs/specs/002-m0-runtime-contract.md:42` 写明成功顶层仅为 `ok: true`、`command`、`data`；错误顶层仅为 `ok: false`、`command`、`error`；`error.code`/`error.message` 必填，`error.details` 可选。全文件检索：协议形状只在 `:42` 定义，无「稳定 `code`」与 `ok`/`command` 同级并列，无顶层 `code`/`message`/`details` 第二套。机器合同 `schemas/command-error.schema.json:6-11` required=`ok,command,error`；`:5` `additionalProperties: false`；`:12-24` 根 properties 仅这三键；`:25-47` 把 `code`/`message`/`details` 放进 `error`。成功侧 `schemas/command-result.schema.json:6-11` required=`ok,command,data`，`:5` `additionalProperties: false`。按 `{ok,command,error:{code,message,details}}` 发出同时满足 prose 与 schema；按旧扁平 `{ok,command,code,message,details}` 会被 schema 拒绝，也被当前 prose 拒绝。上次 fail 所引 SPEC:42 旧文（顶层稳定 `code`）已被 Implementation 改掉；本轮核对的是修复后文本，不是「修过了」本身。 |
| 2. `error.details` 仅 `relative_path`/`revision`/`field`；`revision_conflict` 必须能且必须表达 `error.details.revision` | **已关闭** | SPEC `:42` 白名单三键且至少一项；`:42` 与威胁模型 `:144` 均要求 `revision_conflict` 写 `error.details.revision`。schema `$defs/details`（`schemas/command-error.schema.json:83-119`）`additionalProperties: false`、`minProperties: 1`，properties 仅三键。`revision_conflict` if/then 在 `error` 对象内（`:48-80`）：`then.required` 含 `details`，且 `details.revision` 为 integer `minimum: 0`。因此该码既能表达、也被强制表达 `error.details.revision`。 |
| 3. fingerprint 唯一权威 `fp-v1`，不得「另行固定」 | **已关闭** | SPEC `:32-34` 定义 `fp-v1` 算法；`:58` 禁止另写实现规格或第二套 hash。当前 SPEC 全文无「另行固定」。机器形状 `^fp-v1:[0-9a-f]{16}$`：`schemas/command-result.schema.json:390-392`、`schemas/fact-record.schema.json:15`、`schemas/event.schema.json:10`。 |

### 并列缺口（residual，不构成本轮 fail）

- `schemas/command-error.schema.json` 仍允许任意五命令与任意公共错误码组合；按 command 的错误码白名单只在 registry / SPEC 表，不在 error result schema。这是超集宽松，不是两套互斥形状。
- `schemas/command-result.schema.json:334-345` 的 `load_project.data.facts/events` 仍是无约束 object 数组，未 `$ref` fact/event schema。
- `load_project` 可选输入 `project_id` 只在 prose（`docs/specs/002-m0-runtime-contract.md:100`），registry `input_required` 仅 `project_path`（`schemas/command-registry.json:119-122`）。可选字段缺登记不是互斥条款。
- Draft 2020-12 语义校验仍为 N/A；`if/then`/`$ref` 组合未经 validator 执行。
- 八种 generic 用户类型 content 仍可塞任意非空对象；被 proposed SPEC-002:23 显式冻结，本票验收是 M0 payload。

### 逐项验收（本票勾选项）

| 任务验收 | 判断 | 证据 |
| --- | --- | --- |
| fact/event schema 可解析且覆盖 M0 envelope 与 M0 type-specific payload | 成立（M0 范围） | `ruby JSON.parse` 通过。`schemas/fact-record.schema.json:6-10,27-43`；`schemas/event.schema.json:6,12-15`。 |
| command/event API 有明确记录，并有 registry/result/error schema | **成立** | 五命令 prose/registry/result 字段名对齐；错误结果 prose 与 `command-error.schema.json` 现为同一套 `{ok,command,error}` 包装。 |
| capabilities 默认拒绝、renderer 无系统 API | 成立 | `docs/specs/002-m0-runtime-contract.md:121-134` |
| 威胁模型覆盖五类威胁 | 成立 | `docs/specs/002-m0-runtime-contract.md:136-147`；`:144` 已用 `error.details.revision`。 |
| 目录/索引/进度/变更同步 | 成立（登记层；本 Review 未改这些文件） | `schemas/README.md:12-19`；`docs/agent/artifacts.yaml` 有效 `path:` 26 条，路径均存在。注释模板 `REQ-YYYYMMDD-NN` / `project-map.json` 不计入本票产物。 |

### 验证命令与环境

- 仓库：`/Users/bic/Projects/agent-up`；日期 2026-09-15 11:43 CST；`ruby` `/usr/bin/ruby`；`python3` `/opt/homebrew/bin/python3`。
- `ruby -rjson -e 'ARGV.each { |path| JSON.parse(File.read(path)); puts "valid JSON: #{path}" }' schemas/*.json` → 六份通过：`command-error.schema.json`、`command-registry.json`、`command-registry.schema.json`、`command-result.schema.json`、`event.schema.json`、`fact-record.schema.json`。
- `git diff --check`（已跟踪文件）通过，无输出。未跟踪 schema/SPEC 额外扫描：无 TAB；`docs/issues/02-m0-runtime-contract.md` 既有 Checkpoint 第 64/67/126 行尾空白为历史内容，本 Review 不改写。
- SPEC 检索：错误协议顶层字段只在 `:42` 定义为 `ok`/`command`/`error`；`code`/`message`/`details` 只作为 `error.*`；无「另行固定」。
- artifacts：26 条有效 path 均存在（只读，未改 yaml）。
- Draft 2020-12 validator：**N/A + reason**：环境无 `jsonschema`/`jsonschema_rs`/`fastjsonschema`/`ajv`/`check-jsonschema`；`npx` 存在但本地无 ajv-cli。JSON.parse 不能证明 schema 语义。不因此 fail。

### 后续

移交 Commit。SPEC-002 仍为 `proposed`，本 pass 只关闭 M0-02 审查门禁，不把规格升级为 approved。并列缺口可在后续 runtime 实现票收紧，不阻塞本票 Commit。
