<!-- Input: SPEC-001/002、EXEC-001 W1、开发计划 §3 事实类型。 -->
<!-- Output: M1 其余用户事实类型的 content 合同、目录布局、SQLite 投影与重建规则。 -->
<!-- Pos: M1 事实层合同；proposed，批准前不得把未定义 content 写入实现。 -->

# SPEC-003 — M1 事实层合同

**ApprovalState:** proposed

**依据**：M0-03 `done`；EXEC-001 W1 要求其余用户事实类型可脱离 UI 存活，且 SQLite 只做投影。本文件补齐 SPEC-002 显式冻结的 generic content。

## 目标与边界

M1 让 `.agentup/` 能保存并读回 `discussion`、`plan`、`task`、`scope`、`run`、`decision`、`result`、`attachment`。写入仍走原子替换和 `expected_revision` CAS。删除 SQLite 文件后，从事实目录重建投影，结果与删除前一致。

M1 不接入真实 Agent、不写项目源文件、不扩大 renderer 系统 API、不把附件二进制读进 renderer。新 Tauri command 不是本规格的前提；测试与后续里程碑通过 runtime 内部 writer/reader 完成。若后续票要暴露 command，必须另改批准规格。

`schema_version: 1` 的 `request` 仍只能 `lifecycle: draft`（SPEC-002）。其他 RequestLifecycle 值仍要以后的规格升版本。

## 目录

按需创建，不预建空目录。M1 用户事实文件在 `.agentup/facts/<type>s/<id>.r<revision>.json`（例如 `facts/discussions/d1.r1.json`）。持久事件在 `.agentup/events/`。旧 revision 文件不覆盖。

附件**二进制**若存在，只出现在项目根内由 `content.relative_path` 指出的路径；事实 record 只存元数据。M1 writer 不负责把字节写入磁盘。

「读回」指 runtime 内部 reader（与测试 fixture）。M1 不要求 `load_project.data.facts` 返回这八类；该 command 仍按 SPEC-002 返回 manifest 与 request。

## Content 合同

共享 envelope 仍是 SPEC-001。下列 `content` 取代 generic object。`metadata` 仍为对象，不得放正文。

### discussion
必填：`request_id`、`author_source`（`user|agent|system`）、`body`、`posted_at`。可选：`attachment_ids`（id 数组）。`body` 最大 100000 **UTF-8 字节**。不得把图片字节放进 content。

### plan
必填：`request_id`、`summary`、`success_criteria`（字符串数组，至少 1）、`constraints`（数组，可空）、`status`（`draft|active|superseded`）。

### task
必填：`request_id`、`title`、`task_state`（TaskState 枚举）、`parent_id`（id 或 null）。可选：`depends_on`（id 数组）。用户不可直接写 `task_state`；变更必须伴随事件。

### scope
必填：`request_id`、`task_id`、`entries`。每条 entry：`relative_path`、`reason`、`source`（`user|agent|system|tool`）、`included`（boolean）。路径规则与 SPEC-002 相对路径相同。

### run
必填：`request_id`、`task_id`（可 null）、`kind`（`implement|review|commit|command|verify`）、`run_state`（`active|interrupted|unknown|recovering|closed`）、`started_at`。可选：`ended_at`、`error_code`。

### decision
必填：`request_id`、`question`、`options`（至少 2 条字符串）、`status`（`open|chosen|cancelled`）。可选：`chosen`、`rationale`。`chosen` 仅在 `chosen` 状态必填。

### result
必填：`request_id`、`version`（正整数）、`summary`、`acceptance`（`pending|accepted|rejected`）。可选：`evidence_paths`（相对路径数组）。结果不可变：同一 `id` 不得改 content，只能新 id 或新 version 记录。

### attachment
必填：`request_id`、`relative_path`、`media_type`、`byte_length`（非负整数）、`sha256`（64 位 hex）。禁止绝对路径。单文件上限 10 MiB。单需求附件总大小按该 request **当前最新 revision** 的 `byte_length` 合计，上限 100 MiB；被取代的旧 revision 不计入。

## 事件

持久事件仍用 event schema envelope。M1 最少增加并测试：

| Event | aggregate | 何时 | 持久 |
| --- | --- | --- | --- |
| `discussion.posted` | discussion | 讨论写入成功 | 是 |
| `task.state_changed` | task | task_state 变更 | 是 |
| `scope.updated` | scope | 范围新 revision | 是 |
| `decision.recorded` | decision | 用户选择或取消 | 是 |
| `result.published` | result | 新结果版本 | 是 |

重复 `event_id` → `event_replay`，不二次应用。上表未列出的类型在 M1 不持久化事件；调用方若仍传 `event_id`，该 id 不入库、不参与唯一性检查。

## SQLite 投影

- 路径：项目 `.agentup/` 内的派生文件，不是事实源。
- 缺失或损坏：`load_project` 与重建必须仍只靠事实文件成功。
- 重建：扫描事实与持久事件，按 revision 排序物化索引；不得改写事实文件。
- 禁止：把 SQLite 当 CAS 权威；用投影“修复”畸形事实（返回 `malformed_fact`）。

## 验收

1. Fixture 写入上述八类事实各至少一条；有上表事件的类型必须带对应持久事件。进程重启后经内部 reader 读回字段一致。
2. 旧 revision 写入被拒绝；重复 event id 被拒绝。
3. 删除 SQLite 后重建，索引与删除前一致。
4. 附件 content 不含字节；超限拒绝。
5. renderer capability 不因 M1 增加 fs/shell/network。

## 范围外

Agent provider、项目源码写入、讨论 UI、范围自动生成、打包。
