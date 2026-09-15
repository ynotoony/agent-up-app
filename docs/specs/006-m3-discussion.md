<!-- Input: EXEC-001 W3、SPEC-003、用户默认批准。 -->
<!-- Output: 文字讨论与附件落盘、关闭重开可恢复。 -->
<!-- Pos: M3 讨论/附件合同；approved。 -->

# SPEC-006 — M3 讨论与附件

**ApprovalState:** approved

**依据**：EXEC-001 W3；用户默认批准。

## 目标

用户可以对一条 draft 需求发文字评论，并附上图片文件。关掉 App 后从 `.agentup/` 恢复评论与附件文件。附件字节不留在 renderer 内存合同里。尚无 Agent 回复。

## Commands

错误 envelope 仍 `{ok,command,error}`。renderer 无 fs/shell。

| Command | 副作用 | 要点 |
| --- | --- | --- |
| `post_discussion` | 写事实+`discussion.posted` | 输入 `project_id`、`request_id`、`body`、可选 `attachment_ids`。body UTF-8 ≤100000 字节。 |
| `add_attachment` | 写 `.agentup/attachments/<id>` 字节 + attachment 事实 | 输入 `project_id`、`request_id`、`media_type`、`bytes`（binary）。单文件 ≤10MiB，单需求最新 revision 合计 ≤100MiB。`relative_path` 必须为 `attachments/<id>`。禁止绝对路径。超限 `invalid_input`，不落盘。 |
| `load_request_thread` | 无 | 输入 `project_id`、`request_id`。输出按时间排序的 discussions 与 attachments **元数据**（含 relative_path、byte_length、sha256），不含文件字节。 |

`add_attachment` 不得把 bytes 写入 fact content。关闭重开后 `load_request_thread` 与磁盘文件 sha256 一致。

## 验收

1. 文字讨论 + 两张图，新 Runtime load_request_thread 仍在，磁盘上有两个附件文件。
2. 超大附件拒绝，不创建文件、不写事实。
3. 无 Agent、不写项目源码。
