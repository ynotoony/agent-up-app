<!-- Input: 已确认的需求、治理决定、范围限制和实施验证记录。 -->
<!-- Output: 按日期追加的项目事实与治理变更审计记录。 -->
<!-- Pos: 变更历史权威，只增不改。 -->

# 变更记录

## 2026-09-14 - 切换为仓库根目录全新 Tauri App

- **范围**：开发计划移至仓库根目录；`properties/` 仅作参考并加入 `.gitignore`；初始化 Agent Up 治理骨架；未创建业务代码。
- **验证**：2026-09-14；`git diff --check` 通过；Ruby YAML 解析 `docs/agent/artifacts.yaml` 通过；14 条登记项字段和路径核对通过；`properties/` 被 `.gitignore` 忽略；未运行原型 `npm` 命令，理由：原型目录按用户要求不进入本仓库。
- **同步门槛**：命中拓扑和契约变化，已同步根 README、docs README、development-process 和 artifacts 索引。
- **阶段与边界**：治理初始化已完成；未创建 `src-tauri/`、未安装 Tauri、未接入模型、未执行部署；开发计划仍待用户最终批准。

## 2026-09-14 - 审计并修订 Agent 治理规则

- **范围**：解除主 Agent 零写入死锁；允许草案以 `draft/proposed` 落盘、`approved` 后生效；统一状态命名空间；补齐 Coordinator 与按 mode 的 run record 写入权；明确恢复顺序、Review 降级、进度摘要和一次性验证规则；修复不存在的外部引用。
- **验证**：`git diff --check`；Ruby YAML 解析与 14 字段检查；过期引用检索通过。
- **同步门槛**：命中治理契约、状态、所有权和派生关系变化，已同步 AGENTS、流程、角色合同、任务票/规格/请求索引与 artifacts 登记。

## 2026-09-14 - 规则复审修正

- **范围**：明确 C0-C3 不变量的适用范围；统一 Coordinator、Implementation、Intake、Triage、Review、Commit 的写入归属；修正 C0/C1 免票路由、REQ 登记、Commit 的混合改动隔离和 Artifact Plan 与产物生命周期状态的区分；缺少触发证据的条件索引标记为 `stale`。
- **验证**：`git diff --check`；Ruby YAML 解析、14 条登记项和 14 字段检查；旧状态/外部引用扫描通过。

## 2026-09-14 - 条件索引待复核计划

- **Artifact Plan（ApprovalState: proposed）**：
  - `docs/issues/README.md`：Required；触发矩阵 §5.2“需要 ≥2 张票或存在 Blocked by 依赖”；当前触发事实为本次治理审计存在多项依赖；批准状态待用户确认。
  - `docs/specs/README.md`：Required；触发矩阵 §5.2“修改公共行为/接口或存在多条验收路径”；当前触发事实为本次治理规则修改涉及公共流程行为；批准状态待用户确认。
  - `docs/research/README.md`：Deferred；本次没有外部调研、方案比较或 spike；不因目录存在而恢复。
- **状态**：以上条件索引在批准证据补齐前保持 `status: stale`，不得作为可信导航；用户确认后再按状态机转为 `approved/generated` 或 `deferred`。
