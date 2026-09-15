<!-- Input: 已派发工作、阶段报告、验证证据和当前仓库状态。 -->
<!-- Output: 当前进度、可恢复检查点、阻塞与下一步的紧凑索引。 -->
<!-- Pos: 项目进度事实源；每票不超过 5 行。 -->

# 进度记录

| 工作 | 状态 | Checkpoint / 证据 | 下一步 |
| --- | --- | --- | --- |
| AgentUp Tauri 2 初始化 | done | 2026-09-14；治理骨架、根 Git、`.gitignore`、登记和静态校验通过；见本次提交 | M0：形成 schema、状态机、Tauri command/event 草案 |
| M0-01 状态与事实合同 | done | 2026-09-14；见 `docs/issues/01-m0-foundation.md` 与 `docs/specs/001-m0-foundation.md`；diff/YAML/重复状态扫描通过 | M0-02：JSON Schema、API 产物、capabilities 与威胁模型 |
| M0-02 Runtime 合同 | done | 2026-09-15；Independent Review pass 后用户回复「继续」批准 `SPEC-002`；见 `docs/issues/02-m0-runtime-contract.md` | M0-03 runtime 恢复切片 |
| EXEC-001 首版执行计划 | approved | 2026-09-15；见 `docs/execution-plan.md` | W1：批准 SPEC-003 后领取 M1-01 |
| M0-03 Runtime 恢复切片 | done | 2026-09-15；独立 Review pass；实现在 `codex/m0-03-runtime` | W1：事实层 type-specific schema 与 writer/reader |
| SPEC-003 M1 事实层合同 | approved | 2026-09-15；`docs/specs/003-m1-fact-layer.md` | 已批准；M1-01 重审 |
| M1-01 事实类型落地 | review_ready | 2026-09-15；writer/reader 测试绿；见 `docs/issues/04-m1-fact-types.md` | 独立 Review；SPEC-003 仍为 proposed |
| M1-02 SQLite 投影 | in_progress | 票 `docs/issues/05-m1-sqlite-projection.md` | C3 Implementation |

状态取值：完整 `TaskState` 为 `ready / in_progress / blocked / review_ready / review_pass / review_fail / done`；本表可展示其中摘要，但明细与 Review 状态必须写任务票 Checkpoint。
