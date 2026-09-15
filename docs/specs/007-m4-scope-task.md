<!-- Input: EXEC-001 W4、SPEC-003、用户默认批准。 -->
<!-- Output: 范围版本/差异与任务状态只能经事件变更。 -->
<!-- Pos: M4 范围与任务合同；approved。 -->

# SPEC-007 — M4 范围与任务

**ApprovalState:** approved

**依据**：EXEC-001 W4；用户默认批准。

## 目标

用户能看到当前任务涉及哪些相对路径、为什么纳入、相对上一版改了什么。任务可拆成带子任务。`task_state` 不能由用户直接写事实字段；必须走 command 并产生 `task.state_changed`。

## Commands

错误 envelope `{ok,command,error}`。路径规则同 SPEC-002 相对路径。无 fs/shell。

| Command | 副作用 | 要点 |
| --- | --- | --- |
| `put_scope` | 写 scope 新 revision + `scope.updated` | 输入 `project_id`、`scope_id`、`request_id`、`task_id`、`entries`、`expected_revision`。entry：`relative_path`、`reason`、`source`、`included`。 |
| `diff_scope` | 无 | 输入 `scope_id`、`from_revision`、`to_revision`。输出 added/removed/changed（按 relative_path）。 |
| `put_task` | 写 task 事实；新建时 `task_state=ready` | 输入 `project_id`、`task_id`、`request_id`、`title`、`parent_id`（可 null）、`depends_on`、`expected_revision`。禁止在 content 里带 `task_state`。 |
| `set_task_state` | 新 revision + `task.state_changed` | 输入 `task_id`、`task_state`（TaskState 枚举）、`expected_revision`。非法值 `invalid_input`。 |
| `load_board` | 无 | 输入 `project_id`、`request_id`。输出 tasks（含 parent/depends/state）与每个 task 的最新 scope 摘要。 |

## 验收

1. 两次 `put_scope` 后 `diff_scope` 能说明增删改路径。
2. `put_task` 带 `task_state` 被拒绝；`set_task_state` 产生事件且 load_board 显示新状态。
3. 子任务 `parent_id` 指向主任务，关闭重开后仍在。
4. 旧 revision → `revision_conflict` 且 details.revision 为当前值。

## 范围外

Agent 自动拆分、写项目源码、Review runner。
