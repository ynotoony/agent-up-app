<!-- Input: EXEC-001 W5、SPEC-003、用户默认批准。 -->
<!-- Output: fake/replay provider 与 M0-C 门；无真模型。 -->
<!-- Pos: M5 Agent M0-C 合同；approved。 -->

# SPEC-008 — M5 Agent 与 M0-C

**ApprovalState:** approved

**依据**：EXEC-001 W5；用户默认批准。

## 目标

用户能对已批准范围跑 **fake/replay** Implement，再跑独立 fake Review。Review `fail` 不能 `commit_changes`。工具不能写 `included=false` 或范围外路径。每次 run 必须记下 M0-C 字段。本切片不接真实模型厂商。

## M0-C

没有本门，不得接真模型。

**上下文组装（后者不得覆盖前者；最高优先在上）**

1. system prompt 与 `prompt_version`
2. 已批准 scope（`included=true` 的相对路径）
3. task
4. 用户 decision
5. discussion / attachment 文本（**不可信**）
6. 范围内文件摘录

discussion / attachment 中的字符串（含 `ignore previous instructions`）不得升格为 system 或 policy，不得改 `prompt_version`，不得扩大或绕过 scope。

**记录**

- 每次 run 必填记录字符串 `prompt_version`（来自 `start_run` 输入，不来自讨论）。
- `provider` 本切片仅为 `fake` 或 `replay`。
- fake/replay 仍记录非负整数 `token_input` / `token_output` 估计。
- Review 在 `finish_run` 必填 `verdict`：`pass` | `fail`。
- 密钥不得出现在 facts、events 或 renderer。

**成本与时限（硬顶来自 EXEC-001）**

- 上下文默认 32k token，硬顶 128k。
- 单次 run 15 min。
- 超限：失败该 run；`run_state` 为 `closed`（token/上下文）或 `interrupted`（15 min）；**不得静默截断**用户事实（scope / task / decision）。本切片无上调 32k 的 command。

**重试**

- provider 超时最多自动重试 1 次。
- injection / policy / 越权写入失败不重试。

**并发**

- 同一项目同时最多 1 个 `kind=implement` 且 `run_state=active`，加 1 个 `kind=review` 且 `active`。
- 第三路 `start_run` → `conflict`，不写 run 事实。

## Run 事实（沿用 SPEC-003，本切片补字段）

沿用 `kind`（本切片只启动 `implement` | `review`）、`run_state`（`active|interrupted|unknown|recovering|closed`）、`started_at`、可选 `ended_at` / `error_code`。

本切片每次 run **额外必填**：`prompt_version`、`provider`（`fake|replay`）。关闭或中断时必填 `token_input`、`token_output`。Review 关闭时必填 `verdict`。

SPEC-003 未给 run 持久事件。本切片用 run 事实新 revision，不发明 run 事件。

`load_run` 只从 `.agentup/` 事实读取，不以 SQLite 为源。

## Commands

错误 envelope `{ok,command,error}`。路径规则同 SPEC-002 相对路径。renderer 无 fs / shell / process / network / secret。fake/replay provider **进程内**，无网络、无真实 HTTP。

本切片新增错误码：`conflict`（第三路并发）、`review_required`（无通过的最新 Review 却 `commit_changes`）。其余沿用公共码。

| Command | 副作用 | 要点 |
| --- | --- | --- |
| `start_run` | 写 run 事实；`run_state=active` | 输入 `project_id`、`run_id`、`request_id`、`task_id`、`kind`（`implement`\|`review`）、`provider`（`fake`\|`replay`）、`prompt_version`、`expected_revision`（新建为 `0`）。缺 `prompt_version`、非法 kind/provider → `invalid_input`。第三路 → `conflict`。 |
| `apply_fake_script` | 按脚本调用工具；可能写范围内源文件 | 仅 `provider` 为 `fake`/`replay` 且 run `active`。输入 `project_id`、`run_id`、`calls`（脚本化 tool 调用）、`expected_revision`。越权写 → `invalid_input`，**文件不变**。 |
| `finish_run` | run 新 revision：`run_state=closed`，`ended_at` | 输入 `project_id`、`run_id`、`expected_revision`、`token_input`、`token_output`；Review 必填 `verdict` `pass`\|`fail`。缺 verdict → `invalid_input`，run 仍 active。 |
| `commit_changes` | 记录该 task 的 implement 产物被接受 | 输入 `project_id`、`task_id`、`request_id`。当且仅当该 task **最新** Review run 的 `verdict=pass`。否则 `review_required` 或 `invalid_input`；**不改**工作区文件。本 command **不是**对 AgentUp 仓库做 git commit，也不在本切片对用户项目执行 git commit。 |
| `cancel_run` | `active` → `interrupted`，持久化 | 输入 `project_id`、`run_id`、`expected_revision`。记下已有 token 估计。 |
| `load_run` | 无写 | 输入 `project_id`、`run_id`。从 `.agentup/` 读回 `run_state` 与 M0-C 字段。 |

旧 `expected_revision` → `revision_conflict`，`error.details.revision` 为当前值。

## 工具 / 写入

`apply_fake_script` 允许的 tool：

| Tool | 规则 |
| --- | --- |
| `read` | 项目内相对路径可读。 |
| `write` | 只能写当前批准 scope 中 `included=true` 的 `relative_path`。无 scope 或 included 为空 → **禁止写源码**（场景 S3），`invalid_input`，零文件变化。 |
| `run_command` | 默认拒绝。本切片允许列表为空，或仅测试用 `echo` fixture；其它命令 `invalid_input`，无进程副作用。 |

范围外 / `included=false` 的 `write` → `invalid_input`，目标文件字节不变。

## 验收

1. Fake implement 能写范围内文件；范围外 write 被拒绝，该文件不变。
2. Review `verdict=fail` → `commit_changes` 失败且文件保持原样；`pass` → `commit_changes` 成功。
3. 不可信 discussion 不能改 `prompt_version` 或绕过 scope。
4. Drop Runtime 后新 `load_run` 从 `.agentup/` 恢复 `run_state`。
5. 第三路并发 `start_run` 被拒绝（`conflict`）。
6. 无真实 HTTP。

## 范围外

真实模型 API；W5.5 崩溃/子进程清理（超出 `cancel_run`）；W6 结果版本；UI chrome；打包；Linux；自动更新。
