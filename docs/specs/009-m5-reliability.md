<!-- Input: EXEC-001 W5.5、SPEC-008、用户默认批准。 -->
<!-- Output: 取消、超时、重试、崩溃、休眠、子进程清理的运行可靠性合同。 -->
<!-- Pos: M5.5 运行可靠性合同；approved。 -->

# SPEC-009 — M5 运行可靠性

**ApprovalState:** approved

**依据**：EXEC-001 W5.5；用户默认批准。

## 目标

W5 fake agent 之后、进 W6 之前，run 必须能被取消、超时、有限重试，并在崩溃/关 App 后停下。真实模型仍不接。关闭 App 不会让 agent 继续跑。

## Commands

错误 envelope `{ok,command,error}`。沿用已有错误码。`timeout` 写在 run 事实的 `error_code`；把 run 关掉的 tick **可以** `ok`。已关闭后再 apply/tick → `invalid_input`。无真实模型、无 UI。

`cancel_run` 已存在：`active` → `interrupted` 并持久化。本切片把它当一等退出：取消后 `apply_fake_script` → `invalid_input`，之后零写入。

| Command | 副作用 | 要点 |
| --- | --- | --- |
| `cancel_run` | `active` → `interrupted`，持久化 | 输入 `project_id`、`run_id`、`expected_revision`。此后该 run 禁止再写文件。关 App 的进程内替身也是本命令。 |
| `advance_run_clock` | 注入已过时间；达阈值则中断 | 输入 `project_id`、`run_id`、`elapsed_ms`、`expected_revision`。仅 fake/replay 且 run `active`。`elapsed_ms >= 900000`（15 min）→ `run_state=interrupted`，`error_code=timeout`，持久化。不得静默继续。未达阈值则仍 `active`。已关闭 → `invalid_input`。 |
| `apply_fake_script` | 按脚本调工具；可能写范围内文件 | 沿用 SPEC-008。可含 `{tool:"timeout"}` 模拟 provider 超时。 |

## 超时 / 重试 / 崩溃 / 子进程

- **墙钟 15 min**：用 `advance_run_clock`，禁止真睡 15 min。到点必须 `interrupted` + `error_code=timeout` + 持久化。
- **provider 超时**：同一 active run，第一次 `{tool:"timeout"}` 自动重试一次，run 仍 `active`；连续第二次 timeout → `interrupted` 并持久化。injection / policy / 越权写入 **永不**重试。
- **崩溃**：drop Runtime 时 run 仍 active；新 `load_run` 为 `active` 或 `recovering`，由调用方 `cancel_run`。本切片不得拉起 OS 子进程（`run_command` 仍拒绝）。
- **休眠 / 关 App**：产品规则——关 App 即停 run，不把 agent 留在后台。本切片用 `cancel_run` 作为进程内替身，不测真实关 App。
- **子进程清理**：`run_command` 允许列表为空。`start_run` / `apply_fake_script` / `cancel_run` / `advance_run_clock` 不得留下子进程。测试：静态/源码断言无进程 spawn，加上 fake timeout 后 `cancel_run`。

## 验收

1. `cancel_run` 后 `apply_fake_script` 失败；取消后文件不变。
2. `advance_run_clock` 超过 15 min → `run_state=interrupted`，`error_code=timeout`；restart 后 `load_run` 仍在。
3. 一次模拟 provider timeout 会重试；连续第二次 timeout 中断。
4. 越权写入仍不重试。
5. 无子进程被拉起。

## 范围外

真实模型；W6 结果版本；UI；打包。
