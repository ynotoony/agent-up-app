<!-- Input: 已派发工作、阶段报告、验证证据和当前仓库状态。 -->
<!-- Output: 当前进度、可恢复检查点、阻塞与下一步的紧凑索引。 -->
<!-- Pos: 项目进度事实源；每票不超过 5 行。 -->

# 进度记录

| 工作 | 状态 | Checkpoint / 证据 | 下一步 |
| --- | --- | --- | --- |
| AgentUp Tauri 2 初始化 | done | 2026-09-14；治理骨架、根 Git、`.gitignore`、登记和静态校验通过；见本次提交 | M0：形成 schema、状态机、Tauri command/event 草案 |

状态取值：`ready / in_progress / blocked / done`。明细写任务票 Checkpoint 区。
