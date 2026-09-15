<!-- Input: 已派发工作、阶段报告、验证证据和当前仓库状态。 -->
<!-- Output: 当前进度、可恢复检查点、阻塞与下一步的紧凑索引。 -->
<!-- Pos: 项目进度事实源；每票不超过 5 行。 -->

# 进度记录

| 工作 | 状态 | Checkpoint / 证据 | 下一步 |
| --- | --- | --- | --- |
| AgentUp Tauri 2 初始化 | done | 2026-09-14；治理骨架、根 Git、`.gitignore`、登记和静态校验通过；见本次提交 | M0：形成 schema、状态机、Tauri command/event 草案 |
| M0-01 状态与事实合同 | done | 2026-09-14；见 `docs/issues/01-m0-foundation.md` 与 `docs/specs/001-m0-foundation.md`；diff/YAML/重复状态扫描通过 | M0-02：JSON Schema、API 产物、capabilities 与威胁模型 |
| M0-02 Runtime 合同 | done | 2026-09-15；Independent Review pass 后用户回复「继续」批准 `SPEC-002`；见 `docs/issues/02-m0-runtime-contract.md` | M0-03 runtime 恢复切片 |
| EXEC-001 首版执行计划 | approved | 2026-09-15；见 `docs/execution-plan.md` | W3 退出；下一步 W4 |
| M0-03 Runtime 恢复切片 | done | 2026-09-15；独立 Review pass；实现在 `codex/m0-03-runtime` | W1：事实层 type-specific schema 与 writer/reader |
| SPEC-003 M1 事实层合同 | approved | 2026-09-15；`docs/specs/003-m1-fact-layer.md` | 已批准；M1-01 重审 |
| M1-01 事实类型落地 | done | 2026-09-15；独立 Review pass；`codex/m1-01-writer` | W2 |
| M1-02 SQLite 投影 | done | 2026-09-15；独立 Review pass；`codex/m1-02-sqlite` | W2 项目列表/失效/删除备份 |

状态取值：完整 `TaskState` 为 `ready / in_progress / blocked / review_ready / review_pass / review_fail / done`；本表可展示其中摘要，但明细与 Review 状态必须写任务票 Checkpoint。
| M2-01 项目产品切片 | done | 2026-09-15；独立 Review pass；`codex/m2-project-product` | W2.5 外部变化门 |
| M2.5 外部变化门 | done | 2026-09-15；独立 Review pass；`codex/w2.5-external` | W3 讨论与附件 |
| M3-01 讨论与附件切片 | done | 2026-09-15；独立 Review pass；`codex/w3-discussion` | W4 范围与任务 |
