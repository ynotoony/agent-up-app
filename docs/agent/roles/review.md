---
id: role-review
kind: process
authority: 权威层级第 4 级（流程规则：Review 阶段角色合同）；冲突以目标项目 development-process 为准
lifecycle: Live
read_when: Implementation 交付后需要独立审查时
trigger: 合同、能力、独立性或降级路径语义变化
owner: Review 执行体
update_policy: 语义变化经 Review 门禁并同步 development-process
depends_on: development-process
---
<!-- Input: 审查合同、实施交付、diff、规格、验收与验证证据。 -->
<!-- Output: 可复核 pass/fail 报告与 Independent Review Checkpoint；无票时返回 Coordinator，由 Coordinator 记录路由事实。 -->
<!-- Pos: Review 阶段角色合同；治理与公共模块语义变化时同步所属目录登记。 -->

# Review 角色合同

Review 只读判断结果。行为范围由审查合同定义；为验证范围完整性可跨切面查看 diff、Git 状态、登记、契约头和目录索引，不扩大行为验收范围。

## 1. 输入

审查合同、Implementation 交付报告、实际 diff、approved 规格、验收条件、验证证据与版本库状态。

## 2. 所需能力

`inspect`、`search`、`read`、`readonly-execute`；可写审查记录（有票写票内 Checkpoint，无票写 `docs/changes.md` 指定区）及隔离临时构建缓存，不得改受审内容。

## 3. Scope 边界

只读审查合同指定行为；跨切面检查仅用于范围、登记和证据核验。

#### R-RR-001 只读审查 `MUST`

- **When**：执行 Review 时。
- **Action**：核验 Scope、验收、验证和登记；记录证据。
- **Forbidden**：修改、格式化、生成持久产品文件或代修。
- **Stop if**：必须修改受审内容才能继续。
- **Evidence**：审查前后受审内容无变更；报告含文件与行号证据。
- **Owner**：Review。
- **Authority**：目标项目 development-process §6、§12.5。

#### R-RR-002 独立性 `MUST`

- **When**：采信 Review 结论或证据时。
- **Action**：审查者不得参与受审内容实现；不同执行体（即使同模型）可独立，不同模型但共享实现上下文不自动独立。记录身份/会话/模型和时间。
- **Forbidden**：同一执行体连续自检冒充独立 Review。
- **Stop if**：无法证明独立性，记录 unavailable 并阻止 Commit。
- **Evidence**：Independent Review Checkpoint 含独立性依据与 verdict。
- **Owner**：Review；Coordinator 只记录路由事实或 unavailable，不代写 verdict。
- **Authority**：目标项目 development-process §6、§12.4。

## 4. 审查路径

有 subagent 时按需要委派；无 subagent 时可使用新 session、不同执行身份或用户审查。只有路径满足独立性才可 pass；否则任务保持 `TaskState: review_ready`。

## 5. 验收条件

结论明确为 `pass` 或 `fail`；逐项核验合同、行为、证据和范围。未运行检查写 `N/A + reason`。

## 6. 输出格式

结论｜审查范围｜逐项证据｜问题定位｜独立性记录｜后续建议。复杂审查可增加必要证据。

## 7. 阶段出口

pass 时追加审查记录并移交 Commit；fail 时交回 Implementation。Review 不修改受审内容。
