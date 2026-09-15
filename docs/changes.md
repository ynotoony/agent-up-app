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

## 2026-09-14 - 归档 retro skill 调研

- **Artifact Plan（ApprovalState: approved）**：`docs/research/2026-09-retro-skill.md` 为 Required；命中触发矩阵“外部调研、方案比较或 spike”，用户明确要求保留为 research。
- **结论**：调研报告已归档，结论为暂不实施；`research-readme` 已刷新为 `status: refreshed`，报告登记为 `status: generated`。
- **验证**：报告包含来源、证据、结论和被引用字段；目录清单与 `docs/agent/artifacts.yaml` 已同步。

## 2026-09-14 - 完成 M0-01 状态与事实合同

- **范围**：创建批准规格 `SPEC-001` 和 M0 任务票；统一 `RequestLifecycle` 与 `TaskState`、`SessionState`、`RequestState`、`ApprovalState`、`Phase`；定义事实 envelope、事件、revision、原子写入和最小 Tauri command；移除 `CONTEXT.md` 与开发计划中的重复状态机。
- **Artifact Plan（ApprovalState: approved）**：`docs/specs/001-m0-foundation.md`、`docs/issues/01-m0-foundation.md` 为 Required；分别命中公共行为/接口与 M0 任务依赖触发条件。
- **验证**：`git diff --check`；YAML 产物登记解析与字段检查；状态机重复定义扫描。
- **下一步**：M0-02 补齐 JSON Schema、command/event API 产物、capabilities 清单和威胁模型；M0 全部通过后进入 M1 事实层实现。

## 2026-09-14 - 交付 M0-02 Runtime 合同

- **范围**：新增 Draft 2020-12 fact/event JSON Schema；形成五个 typed command、四类 event、renderer 默认拒绝系统权限、项目根边界和最小 Tauri capability 草案；记录路径逃逸、未确认写入、revision 冲突、事件重放和敏感数据泄露威胁及控制。
- **状态**：M0-02 `TaskState: review_ready`，`Phase: implementation` 已完成；`SPEC-002` 保持 `ApprovalState: proposed`，未生成 runtime 或 Tauri capability 实现文件。
- **验证**：2026-09-14；Ruby JSON.parse、schema required/type/source/revision 静态检查、YAML 解析与 14 字段/路径检查、`git diff --check` 通过；本地无 Draft 2020-12 JSON Schema validator，记 `N/A + reason`。
- **同步门槛**：命中拓扑、契约和派生变化；已同步根 README、`schemas/README.md`、`docs/specs/README.md`、`docs/agent/artifacts.yaml`、`docs/progress.md` 与本条变更记录。

## 2026-09-14 - 澄清 M0-02 notification 与确认边界

- **范围**：明确 `project.scan_completed` 与 `request.rehydrated` 仅为不可变 runtime notification，不伪造持久事实；持久事件仍须与事实 revision 对齐。记录 confirmation token 的边界：它绑定预览与 fingerprint，但不能证明受攻陷 renderer 中的真实人类确认。
- **后续**：涉及更高权限的 runtime 需要原生确认或等价的受信用户手势；本票不生成实现文件。

## 2026-09-14 - 修复 M0-02 Runtime 合同 Review 问题

- **范围**：fact record 改为 type-specific `oneOf`，覆盖 request/project/manifest；event schema 约束 event_type、aggregate_type、delivery 与 payload 组合；新增 command registry/result/error schemas。固定 `fp-v1` fingerprint 算法、manifest revision 来源、sibling 临时目录原子 rename 和失败清理策略。
- **状态**：M0-02 `TaskState: review_ready`；未生成 runtime、Tauri capability 或事实 writer 实现。
- **验证**：2026-09-14；五份 JSON Schema JSON.parse、静态字段/组合/命令/威胁检查、YAML 解析/14 字段/路径检查、`git diff --check` 通过；本地无 Draft 2020-12 validator，记 `N/A + reason`。

## 2026-09-14 - 补齐 M0-02 机器合同缺口

- **范围**：按独立 Review 反馈收紧 fact content 的 type-specific `oneOf`，新增 project/manifest/request 内容定义；event schema 增加 event/aggregate/delivery/payload 组合约束；新增 command registry schema 与五命令实例、typed result/error schemas；固定 manifest revision 来源、`fp-v1` root fingerprint 算法、临时目录原子提交和失败清理。
- **状态**：M0-02 `TaskState: review_ready`；仍不实现 runtime。
- **验证**：2026-09-14；六份 JSON 文件 JSON.parse、registry 五命令/唯一性检查、关键字段/组合/五类威胁静态检查、YAML 解析/14 字段/路径检查、`git diff --check` 通过；Draft 2020-12 validator 不可用，记 `N/A + reason`。

## 2026-09-15 - 修复 M0-02 Independent Review fail

- **范围**：关闭 error `details` 与威胁模型的矛盾：仅允许 `relative_path` / `revision` / `field`，`revision_conflict` 必须带 `details.revision`。删除 fingerprint「由实现规格另行固定」，`fp-v1` 为唯一算法。收紧 command result `data` 与 registry 按命令绑定；补齐 `initialize_project`/`preview_initialize` 错误码；声明 schema_version 1 request lifecycle 冻结为 `draft`。
- **状态**：M0-02 `TaskState: review_ready`；`SPEC-002` 仍为 `proposed`；不实现 runtime，不 commit。
- **验证**：2026-09-15；Ruby JSON.parse；error details 三键与 `revision_conflict` if/then 静态检查；SPEC 无「另行固定」；registry 与 SPEC 错误表对照；YAML 26 条/14 字段/路径；`git diff --check`。Draft 2020-12 validator 不可用，记 `N/A + reason`。

## 2026-09-15 - 对齐 M0-02 错误结果 envelope

- **范围**：`SPEC-002` 错误结果改为与 `schemas/command-error.schema.json` 相同的 `error` 包装：顶层仅 `ok: false`、`command`、`error`；`error` 内为 `code`、`message`、可选 `details`。`revision_conflict` 必须写 `error.details.revision`；威胁模型表同步。不把 schema 改成扁平顶层字段，不改 fact/event schema，不实现 runtime。
- **状态**：M0-02 `TaskState: review_ready`；`SPEC-002` 仍为 `proposed`；不 commit。
- **验证**：2026-09-15；Ruby JSON.parse 六份 schema/fixture 通过；SPEC 全文检索错误结果顶层不再并列 `code`/`message`/`details`，`details` 只作为 `error.details`；`git diff --check` 通过；artifacts.yaml 未改、相关路径仍存在。Draft 2020-12 validator 不可用，记 `N/A + reason`。

## 2026-09-15 - M0-02 Independent Review pass

- **范围**：envelope 修复后的独立 Review 结论为 pass；错误结果与 `command-error.schema.json` 同为 `{ok, command, error}`。先前 details 白名单与 fp-v1 权威保持关闭。
- **状态**：M0-02 `TaskState: review_pass`；`SPEC-002` 仍为 `ApprovalState: proposed`。本 pass 不批准规格，不拆 runtime 实现票，不 commit。
- **残余**：error schema 不按 command 收错误码；`load_project` facts/events 未 `$ref`；Draft 2020-12 validator 仍 N/A。不构成本票 fail。

## 2026-09-15 - 批准 SPEC-002 并拆 M0-03

- **批准依据**：M0-02 `review_pass` 后，用户被明确告知下一步只剩批准 `SPEC-002`，于 2026-09-15 回复「继续」。按流程「用户一次明确指令在其范围内持续有效」，将该回复记录为 `SPEC-002` 的 `ApprovalState: approved`。
- **Artifact Plan（ApprovalState: approved）**：`docs/issues/03-m0-runtime.md` 为 Required；命中触发矩阵「需要 ≥2 张票或存在 Blocked by 依赖」以及 C3 安全边界实现需要独立任务合同。不预建 `src-tauri/`、`src/`、`tests/` 或 Tauri capability 文件。
- **范围**：`SPEC-002` 转为 approved；M0-02 关闭为 done；新增 M0-03 垂直切片（扫描→确认初始化→创建草稿需求→关闭重开恢复，含五类威胁的外部行为验收）。未实现 runtime，未 commit。
- **验证**：YAML 产物登记字段/路径检查；`git diff --check`；SPEC-002 文内不再声称 proposed。

## 2026-09-15 - 提出首版执行计划 EXEC-001

- **Artifact Plan（ApprovalState: approved）**：`docs/execution-plan.md` 为 Required；用户明确要求补齐能打开 App 之后的执行顺序、场景验收脚本和资源预算。命中触发矩阵「用户要求」以及跨里程碑交付需要单一执行顺序。
- **范围**：新增 proposed 执行计划（W0–W7、S1–S3、预算表、发布默认建议）。不创建 M1–M7 任务票，不创建 `src-tauri/`，不批准 `development-plan.md`。顺手修正 specs README 中 SPEC-002 仍写「待批准」的索引漂移。
- **状态**：`EXEC-001` 为 `proposed`；M0-03 仍 `ready`，其实施权威仍是已批准的 SPEC-001/002。
- **验证**：YAML 产物登记字段/路径；`git diff --check`。

## 2026-09-15 - 批准 EXEC-001 并领取 M0-03

- **批准依据**：EXEC-001 提出后，用户被要求确认 §7 默认建议，回复「继续」。将该回复记录为执行计划批准，含场景脚本 S1–S3、§4 预算和 §5 发布假设。
- **范围**：`docs/execution-plan.md` → `approved`。M0-03 转为 `in_progress` 并派 C3 Implementation。不拆 M1 票，不 commit。
- **验证**：计划文内 ApprovalState 为 approved；progress 与本条同步。

## 2026-09-15 - M0-03 Implementation 交付 runtime 恢复切片

- **范围**：在仓库根新增 Tauri 2 runtime（`src-tauri/`）与 React/Vite renderer（`src/`）；实现 SPEC-002 五条 command、最小 capabilities、fp-v1 与 typed 错误 envelope。外部行为测试覆盖零写入、原子临时目录、重复 request_id/旧 revision、关闭重开、path_outside_project、symlink 与事件重放。首次创建 `docs/agent/runs/`。
- **未做**：真实 Agent、SQLite 事实源、附件、项目源码写入、git commit/push。
- **验证**：schema JSON parse；renderer capability 静态检查；`cargo test` 7 passed；`git diff --check`。
- **同步门槛**：拓扑（新增 `src/`、`src-tauri/`、`tests/`、`docs/agent/runs/`）；行为（M0 command 可用）。根 README、`docs/agent/README.md`、`artifacts.yaml` 已登记。
