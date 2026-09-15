<!-- Input: SPEC-001、用户确认的 M0 顺序与当前仓库状态。 -->
<!-- Output: M0 状态与事实合同任务的可执行任务票、验收和 Checkpoint。 -->
<!-- Pos: AgentUp M0 任务票；实施细节和后续任务不得写入本票。 -->

# 01 — M0-01 状态与事实合同

**Type:** docs

**Priority:** P1

**What to build:** 形成一份批准的 M0 合同，统一需求生命周期、任务/会话/审批状态，定义最小事实记录、事件和 Tauri command 边界，并给出首条可恢复验收路径。

**Blocked by:** 无——可立即开始。

**Status:** done

- [x] `RequestLifecycle` 与交付治理状态使用独立命名空间，并有映射规则。
- [x] M0 事实 envelope、revision、原子写入和事件幂等规则已定义。
- [x] `scan_project`、`preview_initialize`、`initialize_project`、`create_request`、`load_project` 的副作用和权限边界已定义。
- [x] 首条“扫描 → 确认初始化 → 创建需求 → 关闭重开恢复”验收路径已定义。
- [x] `CONTEXT.md` 与开发计划不再维护第二套状态机。

## Checkpoint

- **2026-09-14 / done**：创建 `docs/specs/001-m0-foundation.md`；统一 `RequestLifecycle`、`TaskState`、`SessionState`、`RequestState`、`ApprovalState`、`Phase`；同步 `CONTEXT.md`、`development-plan.md`、目录索引与产物登记。
- **验证**：`git diff --check`；YAML 产物登记解析与字段检查；状态机重复定义扫描。
- **下一步**：M0-02 补齐 JSON Schema、command/event API 产物、capabilities 清单和威胁模型；全部通过后再进入 M1 事实层实现。
