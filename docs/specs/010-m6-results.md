<!-- Input: EXEC-001 W6、SPEC-003、用户默认批准。 -->
<!-- Output: 不可变结果版本、反馈开新一轮、历史比较不污染当前目标。 -->
<!-- Pos: M6 结果与历史合同；approved。 -->

# SPEC-010 — M6 结果与历史

**ApprovalState:** approved

**依据**：EXEC-001 W6；用户默认批准。

## 目标

用户能发布不可变结果版本、接受或拒绝并留下记录、在接受后反馈开新一轮。历史比较只读已落盘结果，不改当前目标或任务板。

同一 `result` `id` 不得改 content（SPEC-003）。每个版本 **新 `result_id`**；`version` 按 `request_id` 递增。

## Commands

错误 envelope `{ok,command,error}`。路径规则同 SPEC-002 相对路径。无 fs/shell。无新事件类型：新结果事实发已有 `result.published`。

| Command | 副作用 | 要点 |
| --- | --- | --- |
| `publish_result` | 写新 result 事实 + `result.published` | 输入 `project_id`、`request_id`、`result_id`、`summary`、可选 `evidence_paths`。content：`request_id`、`version`（正整数；该 request 尚无结果则为 1，否则必须为当前最大 version+1）、`summary`、`acceptance=pending`、可选 `evidence_paths`（相对路径数组）。调用方不得自填 `version`/`acceptance`。已存在的 `result_id` → `already_exists`，不改旧 content。 |
| `accept_result` | 新 result 版本 + decision；不改旧 result | 输入 `project_id`、`request_id`、`source_result_id`、新 `result_id`。新事实复制 source 的 `summary`/`evidence_paths`，`version=max+1`，`acceptance=accepted`。写一条 `decision`：`status=chosen`，`chosen=accepted`。旧事实仍为原 `acceptance`。 |
| `reject_result` | 新 result 版本 + decision；不改旧 result、不删文件 | 同 `accept_result`，但 `acceptance=rejected`、`chosen=rejected`。`evidence_paths` 指向的文件不得删除。 |
| `submit_feedback` | 讨论（复用 `post_discussion` 路径）+ 可选新 task `ready` | 输入 `project_id`、`request_id`、`body`、可选 `title`（有则 `put_task` 等价新建，`task_state=ready`）。不修改任何已有 result content，包括已 accepted 版本。 |
| `load_result_history` | 无 | 输入 `project_id`、`request_id`。从 `.agentup/` 结果事实列出该 request 全部版本，最旧到最新。不改当前 request/goal，不改 task board。 |

`evidence_paths` 必须是项目相对路径；绝对路径、`\`、`..` → `invalid_input`，不写事实。非法 `version`（非正整数或不是 max+1）由 runtime 拒绝：调用方不能传入 version；内部若无法形成 max+1 → `invalid_input`。

`accept_result` / `reject_result` 的 `source_result_id` 必须属于同一 `request_id` 且已存在，否则 `invalid_input` 或 `path_not_found`。不得对同一 source 原地改 `acceptance`。

## 验收

1. 两次 `publish_result` 后 `load_result_history` 含两个版本；第一版 content 不变。
2. 接受某一版本后再拒绝后续版本；旧 accepted 版本仍为 `accepted`。
3. 接受后 `submit_feedback` 产生讨论，且不改任何 result content。
4. drop Runtime 后历史仍在；来自 `.agentup/` 事实，不是 SQLite。
5. `evidence_paths` 相对路径可通过；绝对路径拒绝。

## 范围外

UI、打包、真实模型、自动更新。
