---
id: role-commit
kind: process
authority: 权威层级第 4 级（流程规则：Commit 阶段角色合同）；冲突以目标项目 development-process 为准
lifecycle: Live
read_when: Review pass 后需要提交时
trigger: 提交白名单、授权或阶段出口语义变化
owner: Coordinator 或 Commit 执行体
update_policy: 语义变化经 Review 门禁并同步 development-process
depends_on: development-process
---
<!-- Input: 提交授权、文件白名单、Review pass 证据、版本库状态。 -->
<!-- Output: 提交范围核对、提交 ID 与文件清单，或停止原因。 -->
<!-- Pos: Commit 阶段角色合同；治理与公共模块语义变化时同步所属目录登记。 -->

# Commit 角色合同

Commit 是可选收尾阶段。Coordinator 可按白名单执行，也可委派 Commit；不强制第三个执行体。任何 Git Commit 必须有独立 Review pass。低风险工作可以交付而保持未提交，并在记录中说明提交状态。

## 1. 输入

提交合同（白名单、提交信息、授权）、独立 Review pass、版本库状态和已落盘 Checkpoint。

## 2. 所需能力

`inspect`、`vcs-read`、`vcs-write`；能力由宿主映射。提交动作只在用户授权范围内执行。

## 3. Scope 边界

只暂存白名单文件。白名单之外已识别的他人改动可以留存；但若同一文件同时包含他人改动且无法用行级或内容级证据隔离自己的变更，必须停止。未知归属或受审范围冲突也必须停止。空文件、删除文件是合法变更；禁止的是没有任何变更的空提交。

## 4. 禁止行为

#### R-RC-001 白名单提交 `MUST`

- **When**：创建 Git Commit 前。
- **Action**：核对白名单、授权、Review pass与实际 diff，仅在文件级或行/内容级归属可证明时暂存自己的白名单范围，回报提交 ID 和文件清单。
- **Forbidden**：暂存白名单外文件、push/deploy、回滚他人改动、创建无变更空提交。
- **Stop if**：白名单为空、授权缺失、Review pass 缺失或自己的范围无法区分。
- **Evidence**：逐项范围核对和 vcs-read 结果。
- **Owner**：Coordinator 或 Commit。
- **Authority**：目标项目 development-process §6、§9。

#### R-RC-002 记录翻转 `MUST`

- **When**：任务状态或提交状态需要更新时。
- **Action**：仅更新合同明确授权的记录；产品提交与记录提交可分开，done 与未提交可并存并记录状态。
- **Forbidden**：改写历史记录或翻转未授权字段。
- **Stop if**：记录 owner 不明或存在并行写入。
- **Evidence**：记录变更和提交文件清单。
- **Owner**：对应记录 owner。
- **Authority**：目标项目 development-process §5、§12.5。

## 5. 验收条件

- 白名单逐项核对完成，实际提交文件与白名单一致。
- 独立 Review pass 已落盘并可复核。
- 提交 ID、信息、文件清单和提交状态可复核。

## 6. 输出格式

白名单核对｜Review pass 证据｜范围检查命令与结果｜提交 ID/信息/文件清单，或停止原因。

## 7. 阶段出口

提交成功后记录提交状态；未提交的已完成工作保持 `TaskState: done` 并明确 `commit_status: pending`。异常时停止，不自行扩大范围。
