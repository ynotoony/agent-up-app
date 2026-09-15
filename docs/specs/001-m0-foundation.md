<!-- Input: 用户要求先统一状态模型并开始 AgentUp M0；现有 CONTEXT、开发计划与开发流程。 -->
<!-- Output: AgentUp M0 的批准规格：状态模型、事实/事件合同、command 边界与首条恢复验收路径。 -->
<!-- Pos: M0 事实层实施的当前规格；新决策以新规格或变更记录替代，不覆盖历史。 -->

# SPEC-001 — M0 事实层与状态合同

**ApprovalState:** approved

**批准依据**：用户于 2026-09-14 明确要求按“统一状态模型 → M0 规格 → 事实层”顺序开始；本规格只覆盖 M0，不批准后续 Agent 执行、附件或产品化范围。

## 问题陈述

当前仓库同时使用产品阶段、任务交付状态和会话状态，名称相近但没有映射。事实目录、事件、Tauri command 和首条恢复路径也只有计划描述，无法直接实施或验证。

## 方案

M0 先建立一条可恢复的事实链路：只读扫描项目目录，预览初始化计划，用户确认后创建最小 `.agentup/`，创建需求事实和事件，关闭后重新读取并恢复状态。所有持久副作用由 runtime command 完成，renderer 不直接访问文件系统。

## 状态模型

状态模型必须使用命名空间，不把不同模型的值写在同一个 `status` 字段中：

| 模型 | 允许值 | 用途 |
| --- | --- | --- |
| `RequestLifecycle` | `draft` / `understanding` / `discussion` / `waiting_user_decision` / `waiting_info` / `planning` / `executing` / `reviewing` / `fixing` / `result_ready` / `accepted` / `failed` / `paused` / `cancelled` / `rejected` | 用户可见的需求生命周期；一个需求只能有一个当前值 |
| `TaskState` | `ready` / `in_progress` / `blocked` / `review_ready` / `review_pass` / `review_fail` / `done` | 任务票的交付状态；中断不新增 TaskState |
| `SessionState` | `active` / `interrupted` / `unknown` / `recovering` / `closed` | 一次 Agent 会话的生命周期 |
| `RequestState` | `proposed` / `triaged` / `accepted` / `specified` / `ready` / `rejected` / `deferred` / `needs-user-decision` | 需求队列状态，不替代 RequestLifecycle |
| `ApprovalState` | `draft` / `proposed` / `approved` | 规格、Artifact Plan 或其他需用户确认内容的批准状态 |
| `Phase` | `coordination` / `intake` / `triage` / `implementation` / `review` / `commit` | 当前工作阶段，不是产品状态 |

`RequestLifecycle` 是产品语义；其余模型是交付治理语义。`RequestLifecycle: executing` 可以对应一个或多个 `TaskState: in_progress`；`RequestLifecycle: reviewing` 对应 `review_ready`、`review_pass` 或 `review_fail`；`RequestLifecycle: result_ready` 要求相关任务已完成并有结果版本。具体转换必须由事件驱动，用户不能直接写状态字段。

## 事实记录合同

每种事实记录共享以下 envelope；`content` 由记录类型约束，不能用任意 JSON 代替类型校验：

```json
{
  "id": "stable-id",
  "type": "request|discussion|plan|task|scope|run|decision|result|attachment",
  "schema_version": 1,
  "project_id": "stable-project-id",
  "request_id": "stable-request-id-or-null",
  "revision": 1,
  "source": "user|agent|system|tool",
  "created_at": "ISO-8601",
  "updated_at": "ISO-8601",
  "content": {},
  "metadata": {}
}
```

- 记录通过新 revision 更新，旧 revision 不覆盖。
- 写入必须校验 `expected_revision`；不匹配返回冲突，不静默覆盖。
- 写入使用临时文件和原子替换；失败保留旧版本。
- `.agentup/manifest.json` 记录事实格式版本、项目 ID 和初始化时间。
- M0 不把 SQLite 当事实源；SQLite projection 和重建属于后续 M1。

## 事件合同

事件是不可变追加记录，至少包含：`event_id`、`event_type`、`aggregate_type`、`aggregate_id`、`aggregate_revision`、`source`、`occurred_at`、`payload`。M0 必须实现并测试：

- `project.scan_completed`
- `project.initialized`
- `request.created`
- `request.rehydrated`

事实 revision 与事件 `aggregate_revision` 必须一致；重复事件 ID 不得产生第二次状态变化。

## Runtime command 边界

M0 只暴露以下 typed commands：

| Command | 副作用 | 约束 |
| --- | --- | --- |
| `scan_project` | 无 | 只读；返回项目名、类型、已有文档和 `.agentup` 状态 |
| `preview_initialize` | 无 | 返回将创建的路径和文件，不写磁盘 |
| `initialize_project` | 写入 | 必须携带用户确认 token；只在项目根目录创建最小 `.agentup/` |
| `create_request` | 写入 | 校验项目 ID、RequestLifecycle 和 revision；追加 `request.created` |
| `load_project` | 无 | 从 `.agentup/` 读取事实；不存在 SQLite 时仍可工作 |

所有 command 必须返回结构化成功或错误结果；renderer 只能通过 Tauri invoke 调用。M0 不执行 shell、不接入真实模型、不写项目代码、不读取项目根目录外路径。

## 首条验收路径

1. 选择一个项目目录，`scan_project` 完成且目录内容 hash 不变。
2. `preview_initialize` 返回创建预览，预览前后项目目录无新增文件。
3. 用户确认后，`initialize_project` 创建 `manifest.json`、事件目录和初始化事件。
4. `create_request` 创建 `RequestLifecycle: draft` 的需求事实及 `request.created` 事件。
5. 关闭并重新打开应用，`load_project` 从 `.agentup/` 恢复项目和需求，不依赖 SQLite 或 renderer 内存。
6. 重复提交相同 command 或使用旧 revision 时，系统拒绝重复写入并返回可读冲突。

## 范围外

真实 Agent provider、命令执行、项目文件修改、附件、SQLite projection、任务自动拆分、Review runner、跨平台打包、自优化和 retro skill 均不属于本规格。

## 测试决策

M0 先测试事实 writer/reader、revision 冲突、事件幂等、初始化前零写入和关闭重开恢复；测试通过 runtime 接口验证外部行为，不测试具体文件 helper 的内部实现。

## 后续

本规格是 M0-01。M0-02 继续补齐每种记录类型的 JSON Schema、command/event API 产物、capabilities 清单和威胁模型；M0 全部通过后再创建 M1 事实层实现任务。SQLite projection 仍属于 M1。任何新增状态或 command 必须更新本规格或新建规格，不能直接写入实现。
