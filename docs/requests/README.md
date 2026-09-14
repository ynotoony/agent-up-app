<!-- Input: 已确认需求、Intake 会话边界与 Triage 结论、REQ-*.md 请求文件。 -->
<!-- Output: 需求请求目录的队列规则、状态机和直接成员登记。 -->
<!-- Pos: 需求请求索引（seed 七件之一）；一旦我被更新，务必更新我的开头注释，以及所属文件夹的 README.md。 -->

# 需求请求

## 架构

- 本目录登记从 Intake 进入的需求请求（`REQ-YYYYMMDD-NN.md`）；请求是需求事实记录，不是规格，也不是任务票。
- 请求状态机：`proposed -> triaged -> accepted -> specified -> ready ->（进入任务流程）`；旁支状态 `rejected`、`deferred`、`needs-user-decision`。
- 请求正文记录原文、来源、时间、影响评估和待确认项；未确认事实标 `【待定：...】`。
- 状态与证据摘要在 [`../progress.md`](../progress.md)；状态流转由 Triage 写回本目录成员文件，不以聊天记录为唯一事实。

#### R-RQ-001 Intake 写入边界 `MUST`

- **When**：Intake 会话接收新需求时。
- **Action**：只在本目录新建 `REQ` 文件并登记目录清单；IF 新需求影响进行中任务的验收 THEN 该任务标 `blocked: scope-change` 交 Triage 承接 ELSE 不触碰任何进行中任务。
- **Forbidden**：Intake 修改规格、任务票、代码或进行中任务的范围；跳过落盘直接开工。
- **Stop if**：新需求与既有规格或请求冲突 → 停止并报告冲突双方，等用户裁决。
- **Evidence**：新 `REQ` 文件落盘且目录清单登记一致。
- **Owner**：Intake 会话执行体。
- **Authority**：`docs/development-process.md`（产物生命周期与代理协作）。

#### R-RQ-002 状态流转归属 `MUST`

- **When**：推进任何请求状态时。
- **Action**：仅 Triage 更新请求状态并产出规格与任务票；`rejected` / `deferred` 必须记录原因；`needs-user-decision` 等待用户裁决，授权前零动作。
- **Forbidden**：其他阶段直改请求状态；把 `rejected` 当 `deferred` 静默丢弃；状态流转不留证据。
- **Stop if**：状态无法判定 → 报告用户，不猜。
- **Evidence**：请求文件状态行与 `../progress.md` 摘要一致，且含流转依据。
- **Owner**：Triage 执行体。
- **Authority**：`docs/development-process.md`（产物状态机）。

## 目录清单

| 名字 | 地位 | 功能 |
| --- | --- | --- |
| `README.md` | 目录索引 | 说明需求请求队列规则并登记直接成员。 |
| `REQ-YYYYMMDD-NN.md` | 需求请求 | 一条需求的事实记录：原文、来源、影响与状态。 |

## Intake/Delivery 并行规则

- Intake Session（用户说"加需求"）与 Delivery Session（用户说"继续剩余任务"）可并行存在；二者写入目标不相交，唯一目录交集是本目录——Intake 写新 `REQ` 文件，Delivery 不写本目录。
- Intake 只写新 REQ：不改当前任务 Scope、验收、`docs/progress.md`、任何规格或任务票；不在 Intake 中做 Triage 结论；REQ 落盘后即结束 Intake 写入。
- 当前任务 Scope 默认冻结；Triage 评估新请求影响：IF 新需求影响当前任务验收 THEN 当前任务标 `blocked: scope-change`，由 Triage 建新规格/票承接 ELSE 当前任务继续；影响评估不确定 → Request 置 `needs-user-decision`，当前任务保持冻结直至裁决。
- 写入所有权矩阵（与 `docs/development-process.md` 会话恢复协议一致；冲突以更严格者为准）：

| 会话/角色 | 允许写入 | 禁止 |
| --- | --- | --- |
| Intake | 仅新 REQ 文件 | 当前任务、progress、规格、票 |
| Triage | Request 状态流转、规格、任务票 | 产品实现文件、Implementation Checkpoint |
| Delivery（Implementation） | 任务 Scope 内文件 + Implementation Checkpoint | Review 记录、Commit 操作 |
| Review | 审查记录 | 修改被审内容 |
| Commit | 暂存与提交记录 | 扩大范围、push、deploy |

#### R-RQ-003 并行写入边界 `MUST`

- **When**：Intake 与 Delivery 并行运行，或任何会话写入触及 `docs/requests/` 或 `docs/progress.md` 时。
- **Action**：Intake 只在本目录新建 `REQ` 文件并登记目录清单，随后结束写入；Delivery 推进当前任务且不写本目录；两个 session 不得同时写 `docs/progress.md`。
- **Forbidden**：Intake 修改当前任务 Scope、验收或 progress；Delivery 借 Intake 扩大当前任务范围；把新需求塞进当前任务的 Scope 或验收。
- **Stop if**：发现并行写入痕迹（写入目标被其他 session 占用或同一文件被并行修改）→ 停止写、以文件现状为准、报告用户。
- **Evidence**：每处变更可归属到唯一会话角色；新 REQ 文件五项内容齐备且状态为 `proposed`。
- **Owner**：Intake 会话执行体与 Delivery 会话执行体（各自边界内）。
- **Authority**：`docs/development-process.md`（会话恢复协议：三套状态机、恢复流程与写入所有权）。
