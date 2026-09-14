<!-- Input: 目标项目盘点与访谈确认的边界事实、seed 治理骨架与快速规则。 -->
<!-- Output: 所有 agent 的统一路由入口：先读什么、快速规则、工作类型路由与停止条件。 -->
<!-- Pos: 仓库级 Agent 路由器（seed 七件之一）；一旦我被更新，务必更新我的开头注释，以及所属文件夹的 README.md。 -->

# Agent 工作入口

本文件只做路由与边界声明，不复制流程细节。唯一流程权威：[`docs/development-process.md`](./docs/development-process.md)。

## 先读什么

1. 本文件（每个会话第一步）。
2. `docs/progress.md` 与涉及目录的 `README.md`：当前状态与导航。
3. 当前任务票、相关规格与 `docs/agent/artifacts.yaml`：任务合同、规范本体与产物登记。
4. 与改动相关的源码、测试：实施对象事实。
5. `docs/changes.md` 等历史与协议资料：按需读取。

完整读取阶梯、任务型最小读取范围与权威层级见 `docs/development-process.md`；本文件不复制其细节。

## 快速规则

#### R-AG-001 记录保护 `MUST`

- **When**：模板升级、初始化补缺或任何自动化写入触及事实记录类产物（进度、变更、请求、运行记录）时。
- **Action**：保留既有事实记录，只追加或按状态机更新；补缺只补结构性缺失（如缺表头、缺登记行）。
- **Forbidden**：用新模板重写历史记录；静默截断或覆盖既有条目。
- **Stop if**：记录内容与模板结构冲突 → 报告用户并给选项，等待裁决。
- **Evidence**：既有条目在变更前后可对照。
- **Owner**：该记录产物登记的写入 owner。
- **Authority**：`docs/development-process.md`（产物生命周期：记录保护）。

#### R-AG-002 条件产物默认不创建 `MUST`

- **When**：规划任何任务的产物与治理文件时。
- **Action**：IF 命中触发矩阵至少一条 THEN 经 Artifact Plan 与用户确认后创建 ELSE 不创建。
- **Forbidden**：为目录完整性预建空目录、占位文件或"以后会用到"的产物。
- **Stop if**：触发条件命中与否无法判定 → 列入 Artifact Plan 待决项，向用户确认。
- **Evidence**：每个条件产物的创建能指向命中的触发条件行。
- **Owner**：当前任务执行体。
- **Authority**：`docs/development-process.md`（产物生命周期：触发矩阵）。

#### R-AG-003 三道门禁 `MUST`

- **When**：推进任何写入、交付或提交时。
- **Action**：语义变化未经用户确认不写文件；无验证证据不算完成；无独立 Review 通过不提交。
- **Forbidden**：以自动化评估替代用户确认；用"应该可以"代替验证证据；跳过独立 Review 提交。
- **Stop if**：任一门禁不满足 → 停止并记录阻塞。
- **Evidence**：写文件前的确认记录、完成时的验证证据、提交前的 Review 结论。
- **Owner**：各阶段执行体。
- **Authority**：`docs/development-process.md`（代理协作：三道门禁）。

## 工作类型路由

- **回答、解释、只读审查或诊断**：只读检查，不领取任务、不修改进度、不实施修复。
- **治理或文档维护**：按 [`docs/development-process.md`](./docs/development-process.md) 执行；改变项目事实时写入对应规格或 `docs/changes.md`。
- **功能、修复或重构**：先读上下文、规格、任务票与进度，领取 blocker 已完成的任务，再按流程执行。
- **代理协作**：主 agent 按阶段派发执行体；角色边界、委派合同与阶段顺序以流程文档为准，执行体不得创建下级执行体。

## 停止条件

- 权威来源对同一事实给出不一致结论 → 停止修改，报告冲突双方与各自层级，等用户裁决。
- 任务 blocker 未完成、前置未满足或任务范围变化 → 不动手，先报告或进入拆票流程。
- 无依据的领域事实、目录职责或验证命令 → 写【待定：...】并报告，不编造。
- 用户不在场 → 保持停止状态并记录阻塞，不推进。

## 最小仓库边界

- 代码放仓库根目录：Tauri 2 runtime 在 `src-tauri/`，全新 renderer 在 `src/`，测试在 `tests/`，schema 在 `schemas/`。
- 项目事实写入各项目根目录的 `.agentup/`；App 私有 SQLite 只做索引和缓存，不能成为事实源。
- `properties/` 是被 `.gitignore` 忽略的 React 原型和设计参考，不是生产代码输入。
- 不覆盖、回滚、重置、清理或删除不属于当前工作的改动；密钥、真实客户数据和生产环境变量不得进入 Git。
- 受 Git 管理的目录必须有 `README.md` 登记直接成员；人工维护且语法允许注释的文本文件必须有 `Input / Output / Pos` 契约头，并维护与所属目录 README 的联动声明。
- 产物生成、删除或移动同步登记 `docs/agent/artifacts.yaml`；未经用户明确要求，不自动推送远端、部署或发布。

详细规则唯一入口：[`docs/development-process.md`](./docs/development-process.md)。
