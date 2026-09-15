<!-- Input: 目标项目盘点与访谈确认的流程事实、seed 快速规则、产物生命周期协议与三阶段协作约定。 -->
<!-- Output: 读取阶梯与权威层级、产物生命周期协议、工作分类与接手、两层代理协作、委派合同、检查点、会话恢复协议（四种命名状态模型、恢复顺序、run record、写入所有权、故障处理、不变量）、验证与提交的权威运行时协议。 -->
<!-- Pos: 交付流程权威（seed 七件之一，唯一流程事实源）；一旦我被更新，务必更新我的开头注释，以及所属文件夹的 README.md。 -->

# 开发流程

本文是本项目 agent 开发的唯一流程权威；根目录 `AGENTS.md` 只负责路由。目标：每次开发都能被验证、被接续、被提交，并在异常中断或上下文丢失后从事实恢复，而不是凭聊天记忆猜测。

## 1. 快速摘要

- 工作分四类：只读问答不领取任务；治理/文档维护由 Coordinator 按本流程记录；功能/修复/重构按复杂度进入 C0-C3 路径。C2/C3 使用 Implementation → Review → Commit 分工；C0/C1 可由 Coordinator 直接完成或按需委派。
- 读取阶梯五级：L0 `AGENTS.md` → L1 `docs/progress.md` 与目录 README → L2 任务票/规格/`docs/agent/artifacts.yaml` → L3 相关源码测试 → L4 按需；按任务类型取最小读取范围。
- 权威层级六层：用户决策 > 代码/测试/事实记录 > 规格/任务合同 > 流程规则（本文）> Agent Up 模板 > 派生文件；冲突即停报告，不自行择优。
- 产物五类：Seed 初始化即生成；Conditional 命中触发矩阵才创建；Record 只追加或按状态机更新、模板升级不得覆盖；Derived 可重建但须标注来源/时间/覆盖范围/失效条件；Adapter 仅宿主支持且任务需要时生成。
- 条件产物按触发矩阵默认不创建；不为目录完整性预建空目录或占位文件。
- 同步门槛：仅契约、拓扑、行为、派生关系四类变化触发级联同步；错别字、排版、纯追加记录不触发。
- 产物状态机：absent → proposed → approved → generated → stale → refreshed/superseded；用户拒绝记 deferred + 原因。
- 初始化与产物增删自动评估 Artifact Plan（Required/Recommended/Deferred + 理由）；治理产物逐件登记 `docs/agent/artifacts.yaml`（十四字段，含 status）。
- 三道门禁：草案可先以 `draft`/`proposed` 落盘且不覆盖当前 approved 规范；同一用户指令在明确范围内持续有效；实施或生效须基于可追溯的 `approved` 决策；无验证证据不算完成；Git Commit 须有独立 Review pass。
- Coordinator（主 agent）默认负责理解、授权范围内的规划写入、拆分、派发与验收；无 subagent 时可直接完成 C0/C1（含小范围可逆代码）并留验证与 Checkpoint。C2/C3 使用分工与独立 Review；仅在确有需要时委派，不强制每票创建三名执行体。委派可用简表，复杂任务再使用完整字段；任何执行体不得创建下级执行体。
- `docs/progress.md` 保持当前状态摘要，详细证据写任务票 Checkpoint 或无票治理的 changes 指定区；检查点必须可验证。
- 中断恢复只从事实出发：先将 SessionState 置为 `recovering`，再读取最小必要记录、确认 owner、核实文件/验证/结论并落盘恢复摘要；事实优先级为实际文件/测试/Git > Checkpoint/run record > progress > 聊天记录；禁止用破坏性操作"恢复干净"。
- 状态使用命名空间：`SessionState`、`TaskState`、`RequestState`、`ApprovalState`（draft/proposed/approved）与 `Phase`（coordination/intake/triage/implementation/review/commit）；产品领域状态由产品规格另行定义。写入所有权按角色矩阵执行。
- C2/C3、跨会话、中断、无 Git 基线或归属不明时生成 run record（字段定义以本文 §12.4 为准）；C0/C1 通常只写 Checkpoint；六条不变量按风险适用。
- 验证必须记录命令、环境、结果和日期；未运行的验证明确写 `N/A + reason`，禁止用"应该可以"代替证据。

## 2. 读取阶梯与权威层级

### 2.1 读取阶梯（L0-L4）

| Level | 内容 | 定位 |
| --- | --- | --- |
| L0 | 根目录 `AGENTS.md` | 路由与边界；每个会话第一步 |
| L1 | `docs/progress.md` + 各目录 `README.md` | 当前状态与导航地图 |
| L2 | 当前任务票、相关规格、`docs/agent/artifacts.yaml` | 任务合同与规范本体 |
| L3 | 与改动相关的源码、测试 | 实施对象事实 |
| L4 | 协议手册、`docs/changes.md` 历史、架构资料、ADR、research | 协议细节与背景，按需加载 |

### 2.2 任务型最小读取范围

| 任务类型 | 必读 | 按需 |
| --- | --- | --- |
| 解释/问答 | L0 | L1 |
| 文档/治理维护 | L0、L1、L2（本次涉及件） | L4 |
| 新功能/实施 | L0、L1、L2、L3 | L4 |
| Review | L0、L2、实际 diff/文件 | L3、L4 |
| Commit | L0、L2（含 Review pass 证据）、Git 状态 | 无 |

### 2.3 权威层级（高到低）

1. 用户决策；
2. 目标项目代码、测试与事实记录；
3. 规格、任务合同；
4. 流程规则（本文）；
5. Agent Up 模板；
6. 项目地图等派生文件。

#### R-DP-001 按阶梯读取 `MUST`

- **When**：任何会话开始取用治理资源时。
- **Action**：按 §2.2 任务型最小读取范围表取用；必读集合不得跳过；L4 一律按需，不默认加载。
- **Forbidden**：跳过 L0 直接动手；用聊天记忆替代阶梯；一次全量加载全部治理文件。
- **Stop if**：必读集合内信息互相矛盾 → 走 R-DP-002 冲突处理。
- **Evidence**：会话产出能引用其读取层级来源（文件路径或检查点指针）。
- **Owner**：当前会话执行体。
- **Authority**：本文 §2（读取阶梯协议基线）。

#### R-DP-002 冲突即停 `MUST`

- **When**：任何两个权威来源对同一事实给出不一致结论时。
- **Action**：区分事实观测与目标规范：实际文件/测试证明现状，但不自动推翻更高层级的用户确认规格；仅当同层权威事实或用户意图无法裁决时停止受影响工作并报告。
- **Forbidden**：自行选择一种解释继续写；静默合并两种说法；用低层级来源覆盖高层级来源。
- **Stop if**：需要用户裁决的实质范围、不可逆权限或冲突意图无法确定 → 暂停受影响工作；用户离线不阻止已授权范围内的工作。
- **Evidence**：冲突报告与用户裁决结果落盘到 `docs/changes.md` 或任务票。
- **Owner**：当前会话执行体。
- **Authority**：本文 §2.3（权威层级协议基线）。

## 3. 仓库与目录边界

| 目录 | 用途 | 是否进 Git |
| --- | --- | --- |
| `src-tauri/` | Tauri 2 Rust runtime、命令、事实层、权限、Agent 和存储 | 是 |
| `src/` | 全新 renderer、领域适配和 UI 组件 | 是 |
| `schemas/` | JSON Schema 和 fixture contract | 是 |
| `tests/` | 单元、集成和 E2E 测试 | 是 |
| `docs/` | 治理、计划、规格、任务和验证记录 | 是 |
| `properties/` | React 原型和设计参考 | 否，已加入 `.gitignore` |

不进 Git：依赖目录、构建产物、覆盖率、日志、真实 `.env`、密钥、真实数据和临时导入文件。根 Git 必须同时保存 AgentUp 源码和治理事实；用户被管理项目的 `.agentup/` 位于项目目录之外，不属于本仓库提交范围，由 AgentUp runtime 读取和维护。

## 4. 工作状态与接手

`docs/progress.md` 是任务状态索引；任务票与 Checkpoint 保存详细依据，三者通过指针保持一致，不宣称 progress 单独承载全部事实。先按工作类型决定是否进入任务流程：

- 回答、解释、只读审查和诊断：允许只读检查，不领取任务，不修改进度，不实施修复。
- 用户明确要求的治理或文档维护：可以不领业务任务；改变项目事实时写入对应规格或 `docs/changes.md`。
- 功能、修复或重构：C0/C1 可由 Coordinator 直接执行并记录；C2/C3 需要任务合同（有票时 blocker 全部 done）并在开始、检查点和结束时更新进度。

### 4.1 复杂度与授权档位

- **C0**：纯排版、纯追加记录、索引同步或可逆文案调整；通常只需 Checkpoint。
- **C1**：单模块、局部且可回滚的实现或治理修改（含小范围代码），无数据迁移、权限变化或外部集成。Coordinator 可直接实施，也可按需委派。
- **C2**：跨模块、公共行为/接口、治理权限或跨会话交付；必须有明确合同、验证和独立 Review，按需使用 Implementation。
- **C3**：数据迁移、安全边界、难逆操作或多个高风险面；必须分工实施、独立 Review，并生成 run record。

用户一次明确指令在其范围内持续有效；`ApprovalState: approved` 可由可追溯的用户明确指令产生，无需逐文件二次批准。常规实现不确定性可由 Coordinator 选择并记录；只有实质范围变化、不可逆权限/数据风险或用户意图冲突才暂停询问。

任务票用 `Type`（`feature` / `bug` / `refactor` / `docs`）和 `Priority`（`P0` 正在中断 / `P1` 核心路径或阻塞链 / `P2` 普通交付 / `P3` 文档治理）分类。进度必须覆盖完整 `TaskState`；`docs/progress.md` 可只展示摘要状态，但必须能指向票内完整状态与证据。明细写任务票 Checkpoint 区；progress 保持摘要长度，不设硬行数。

### Session 开始（接手门禁）

1. 读 `AGENTS.md` 判定工作类型；C2/C3 功能、修复或重构需要领取任务，C0/C1 可由 Coordinator 直接执行并记录。
2. 执行 `git status --short`、`git log -5 --oneline`、`git diff --stat`，对照进度判断脏文件归属；不能假设未提交改动可以丢弃。
3. 重跑与本次工作直接相关的最后一条验证命令，确认最后检查点仍成立。
4. 发现任务范围变化，先进入拆票流程，不直接动手。

## 5. 产物生命周期

### 5.1 生命周期五类

| 类别 | 含义 | 默认策略 |
| --- | --- | --- |
| Seed | 最小骨架件 | 初始化即生成；缺失即补齐 |
| Conditional | 满足触发矩阵才存在 | 命中任一条件才创建（§5.2） |
| Record | 事实记录（进度、变更、运行记录、请求） | 唯一 owner 写入；只追加或按状态机更新；模板升级不得覆盖 |
| Derived | 可从事实源重建的产物（项目地图、渲染图等） | 可重建；重建须标注来源/时间/覆盖范围/失效条件 |
| Adapter | 平台适配文件 | 仅宿主支持且任务需要时生成 |

#### R-DP-003 记录保护 `MUST`

- **When**：模板升级、初始化补缺或任何自动化写入触及 Record 类产物时。
- **Action**：保留既有事实记录；补缺只补结构性缺失（如缺表头、缺登记行）。
- **Forbidden**：用新模板重写历史记录；静默截断或覆盖既有条目。
- **Stop if**：Record 内容与模板结构冲突 → 报告用户并给选项，等待裁决。
- **Evidence**：Record 文件的既有条目在变更前后可对照（Git diff 或变更记录）。
- **Owner**：该 Record 产物登记的唯一写入者。
- **Authority**：本文 §5.1（生命周期基线）。

#### R-DP-004 Derived 重建标注 `MUST`

- **When**：创建或重建任何 Derived 类产物时。
- **Action**：标注生成来源（事实源路径与工具）、生成时间、覆盖范围与失效条件。
- **Forbidden**：无标注重建；把 Derived 当作独立权威来源。
- **Stop if**：事实源不存在或不可读 → 不重建，报告缺失。
- **Evidence**：产物头部或 `docs/agent/artifacts.yaml` 登记含四项标注。
- **Owner**：该 Derived 产物登记的 owner。
- **Authority**：本文 §5.1（生命周期基线）。

#### R-DP-005 Adapter 生成条件 `MUST`

- **When**：评估是否生成平台适配文件时。
- **Action**：IF 目标宿主支持该平台映射 AND 当前任务需要 THEN 生成 ELSE 不生成（记录 deferred + 原因）。
- **Forbidden**：为"功能齐全"预生成所有宿主适配；无任务需要时创建。
- **Stop if**：宿主能力不明 → 按宿主能力评估后再决定。
- **Evidence**：生成决定指向命中的宿主与任务需要。
- **Owner**：Coordinator 或按授权的 Implementation 执行体（经用户确认的 Artifact Plan）。
- **Authority**：本文 §5.1（生命周期基线）。

### 5.2 条件产物触发矩阵

条件产物默认不创建；命中下列至少一条才创建。复杂度采用本文 §4.1 的本地 C0-C3 定义；任务只需记录等级与简短理由，不依赖外部复杂度文件。

| 产物 | 创建触发条件（满足其一） |
| --- | --- |
| `docs/CONTEXT.md` | 确认了项目特有术语、角色或状态 |
| `docs/specs/` | 修改公共行为或接口；存在多条验收路径；跨会话交付；C2/C3 任务合同不足；用户要求 |
| `docs/issues/` | 需要 ≥2 张票；存在 Blocked by 依赖；跨会话交接 |
| `docs/research/` | 需要外部调研、方案对比或 spike |
| `docs/adr/` | 决策难回退且缺少背景会困惑 |
| `docs/architecture/` | 用户要求；C2/C3 跨模块；涉及多 Adapter |
| `docs/agent/runs/` | C2/C3；跨会话；发生中断；无 Git 基线；改动归属不明 |
| 项目地图 | 跨模块导航需要；仓库规模大；用户要求；旧地图过期 |
| `scripts/` | 同一验证被重复使用、跨票复用或用户要求沉淀时 |

#### R-DP-006 默认不创建 `MUST`

- **When**：规划初始化或任何任务的产品与治理产物时。
- **Action**：条件产物仅在命中触发矩阵至少一条时创建；创建决定写入 Artifact Plan 并经用户确认。
- **Forbidden**：为目录完整性预建空目录、占位文件或"以后会用到"的产物。
- **Stop if**：触发条件命中与否无法判定 → 列入 Artifact Plan 待决项，向用户确认。
- **Evidence**：每个条件产物的创建能指向命中的触发条件行。
- **Owner**：Coordinator 或按授权的 Implementation 执行体（经用户确认的 Artifact Plan）。
- **Authority**：本文 §5.2（触发矩阵基线）。

### 5.3 产物登记（docs/agent/artifacts.yaml）

治理产物逐件登记，字段十四个（含 `status`）；缺值写 `N/A + reason`，不留空：

| 字段 | 含义 |
| --- | --- |
| id | 产物稳定标识 |
| status | 当前产物状态（absent / proposed / approved / generated / stale / refreshed / superseded / deferred） |
| path | 仓库内路径 |
| kind | 产物类型（governance/spec/request/task/record/map/adapter/script 等） |
| authority | 在权威层级（§2.3）中的层级 |
| owner | 唯一写入者 |
| lifecycle | Seed / Conditional / Record / Derived / Adapter |
| trigger | 创建触发条件（对齐 §5.2 触发矩阵；Seed 写 `always`） |
| read_when | 读取时机（对齐 §2 读取阶梯与任务类型） |
| sync_on | 触发级联同步的变化类型（对齐 §5.4 同步门槛） |
| depends_on | 依赖的其他产物 id |
| generated_from | 生成来源（模板或事实源；Derived 必填） |
| platform | 平台绑定信息；平台无关产物写 `neutral` |
| update_policy | 更新策略（追加 / 状态机更新 / 可重建 / 禁止覆盖） |

#### R-DP-007 产物登记 `MUST`

- **When**：生成、删除或移动任何治理产物时。
- **Action**：`docs/agent/artifacts.yaml` 同步登记或注销对应条目，十四字段（含 status）按实际填写；不适用的字段记 `N/A + reason`。
- **Forbidden**：存在无登记的治理产物；登记与实际文件漂移而不处理。
- **Stop if**：发现登记与实际漂移 → 将相关产物标 `stale` 并报告，不静默修正任一侧。
- **Evidence**：登记与仓库文件清单可对照。
- **Owner**：Coordinator 或 Implementation（按产物归属与当前授权）。
- **Authority**：本文 §5.3（登记协议基线）。

### 5.4 同步门槛

| 变化类别 | 示例 | 触发动作 |
| --- | --- | --- |
| 契约变化 | 治理文件或公共模块的 Input/Output/Pos、权威归属、所有权、生命周期、对外接口变化 | 更新相应契约头/元数据/README 登记，级联 `depends_on`；普通源码/记录不机械加头 |
| 拓扑变化 | 文件新增/删除/移动/重命名、目录职责变化 | 更新目录 README 成员登记、`docs/agent/artifacts.yaml`、项目地图 |
| 行为变化 | 流程、门禁、权限、验证要求、公共接口变化 | 更新协议引用与进度状态 |
| 派生关系变化 | 生成来源、依赖关系、平台绑定变化 | 更新 `generated_from`/`depends_on`/`platform` 并校验 Derived 标注 |

不触发：错别字与排版修正；纯追加记录（`docs/changes.md` 新条目、`docs/progress.md` 状态行、请求追加）。

#### R-DP-008 语义触发同步 `MUST`

- **When**：任何治理文件或产物发生变更后自查联动时。
- **Action**：IF 变更命中四类语义变化之一 THEN 执行对应级联同步；契约头与 README 仅限治理文件、目录索引和公共模块，其他文件不机械更新。命中判断与类别写入变更记录。
- **Forbidden**："一改就更新 README"式机械联动；语义变化却不更新登记。
- **Stop if**：变化类别无法归类 → 按待决项报告用户。
- **Evidence**：变更记录注明命中/未命中同步门槛及类别。
- **Owner**：本次变更的执行体。
- **Authority**：本文 §5.4（同步门槛基线）。

### 5.5 产物状态机

```text
absent ──触发命中──> proposed ──用户确认──> approved ──生成──> generated
generated ──漂移或触发同步而未同步──> stale ──重新生成/同步──> refreshed
generated/refreshed ──被新产物取代──> superseded
proposed ──用户拒绝──> deferred（记录原因）
absent ──用户拒绝──> deferred（记录原因）
```

#### R-DP-009 状态机驱动 `MUST`

- **When**：创建、维护或淘汰任何条件产物时。
- **Action**：按状态机流转并在 `docs/agent/artifacts.yaml` 或任务票记录当前状态；`stale` 产物在恢复前不得当作可信导航。
- **Forbidden**：跳过 `proposed` 直接生成需用户确认的条件产物；把 `deferred` 当 `rejected` 静默丢弃；`superseded` 后继续引用旧产物为权威。
- **Stop if**：状态无法判定 → 以实际文件为准并报告。
- **Evidence**：状态流转有落盘记录（登记行、Checkpoint 或变更条目）。
- **Owner**：Coordinator 或 Implementation（按产物归属与当前授权）。
- **Authority**：本文 §5.5（状态机基线）。

### 5.6 Artifact Plan

初始化与阶段推进自动评估产物需求，产出 Artifact Plan：逐项标 `Required` / `Recommended` / `Deferred`，附理由与命中的触发条件行；Plan 可先以 `draft` 或 `proposed` 状态落盘供用户审阅，确认后标 `approved`，不留在聊天中。

Artifact Plan 使用 `ApprovalState` 记录草案、提议和用户确认；`docs/agent/artifacts.yaml` 的 `status` 只记录产物生命周期。两者不能互相替代。

#### R-DP-010 Artifact Plan 评估 `MUST`

- **When**：执行初始化，或任务规划涉及产物增删时。
- **Action**：自动评估产物需求并产出 Artifact Plan：逐项标 `Required` / `Recommended` / `Deferred`，附理由与命中的触发条件；Plan 可作为 `draft`/`proposed` 文件先落盘并呈现给用户，用户确认后转为 `approved`，再驱动实施。
- **Forbidden**：跳过评估直接创建；Plan 只留在聊天中不落盘。
- **Stop if**：评估依据不足 → 相应项标 Deferred + 原因，不猜。
- **Evidence**：Plan 落盘于差异清单、任务票或等价协调记录。
- **Owner**：Coordinator 或 Implementation（按当前授权）。
- **Authority**：本文 §5.6（Artifact Plan 基线）。

## 6. 代理协作（按复杂度分工）

Coordinator 负责理解、授权范围内的规划写入、拆分、派发和验收，并可读取代码完成验收。C0/C1 可由 Coordinator 直接实施；C2/C3 默认由 Implementation 实施并由未参与实现的 Review 者审查。仅在有必要且宿主可用时委派，不强制每票创建三个执行体。

- **Implementation**：按合同实施并验证，不承担正式 Review。
- **Review**：只读核验行为、范围、证据及必要的跨切面登记；不修改受审内容。
- **Commit**：Coordinator 或受委派执行体按白名单提交；任何 Git Commit 仍需独立 Review pass。

独立 Review 要求审查者未参与受审内容实现；同模型不同执行体可以，不同模型共享实现上下文不自动独立。无 subagent 时可用新 session、不同执行身份或用户审查，并如实记录。

### 委派合同固定模板

委派至少写明目标、范围、权限、验收、证据、依赖和 owner；C2/C3 或风险较高时使用以下完整字段：

```text
Repository / Absolute Path:
Task:
Goal:
Scope:
Out of Scope:
Context / Evidence:
Constraints:
Forbidden Changes:
Acceptance Criteria / Definition of Done:
Verification:
Output Contract:
Dependencies / Blockers:
Risks:
Budget / Checkpoints:
```

`Goal` 用可观察结果描述；`Scope` 列允许修改的绝对路径；`Context / Evidence` 列出子代理必须先读的文件使其不依赖聊天上下文；`Output Contract` 规定返回的报告格式；`Budget / Checkpoints` 用于规划资源，默认值为建议上限而非硬截止，接近预算时先落盘 Checkpoint，必要时可在记录理由后继续。

派发前检查目标、范围、权限、验收和 owner 可理解；复杂合同再检查完整字段。子代理发现事实冲突或前置未满足时停止并按 Output Contract 返回。

### 上下文与 token 卫生（所有角色强制）

上下文是稀缺资源；本节规则与两层协作目的一致——让主对话和每个阶段执行体只装必要信息：

1. 大文件优先按行号定位（如 `rg -n`），再按行号区间读；重复读取只在文件发生变化或验证需要时进行。
2. 权威文档只读指定节：先定位节标题，再按区间读，不整读全文。
3. 冗长命令输出先落盘 `/tmp`，对话里只回报命令与结果摘要行。
4. 报告保持可读且聚焦证据；复杂任务可超过建议长度并说明原因。代码证据优先给 `文件:行号区间`，不贴大段 diff 或完整输出。
5. 工具调用、报告长度和上下文预算均为建议值；接近资源边界先落盘 Checkpoint，必要时记录理由后继续。
6. 已读且未变化的文件不重读；不在对话或报告里复述文件全文。
7. `docs/progress.md` 保持摘要，明细留在任务票 Checkpoint 区；无票治理可在 `docs/changes.md` 指定区追加。
8. 需要切换上下文时以实际信息量和风险判断；切换前落盘 Checkpoint，恢复时从事实接续。
9. **阶段边界决定上下文**：访谈 → 规格 → 拆票在同一上下文完成（同一次思考）；每张票的实现从新上下文开始，只凭票文件与 Git 状态接手，不依赖聊天记忆。

### 三道门禁

#### R-DP-011 三道门禁 `MUST`

- **When**：按 Artifact Plan、任务票或阶段顺序推进任何写入、交付或提交时。
- **Action**：草案可先以 `draft`/`proposed` 落盘且不覆盖 approved；实施/生效须有可追溯用户明确指令形成 `ApprovalState: approved`；按 C0-C3 运行相关验证；Git Commit 须有独立 Review pass。
- **Forbidden**：以"自动化评估已判断"替代用户确认；借产物维护绕过阶段门禁。
- **Stop if**：实质范围、不可逆权限/数据风险或用户意图冲突无法裁决 → 停止受影响工作；其他低风险常规不确定性可选择并记录。
- **Evidence**：写文件前的确认记录、完成时的验证证据、Commit 前的 Review pass 证据。
- **Owner**：各阶段执行体或 Coordinator（按阶段与降级路径写入所有权）。
- **Authority**：本文 §6（门禁基线）。

### 检查点纪律

检查点必须是可验证状态（例如"静态检查通过"），不能写"xx 开发中"。达到检查点记录相关验证结果；C2/C3 交付完整后才派 Review，Commit 仍需独立 Review pass。

## 7. 开发节点

1. `理解完成`：规格、领域词汇、阻塞关系和验收条件无冲突。
2. `Interface 确认`：外部行为、命令、错误和权限规则明确。
3. `测试失败`：关键外部行为已有失败测试或等价可验证证据。
4. `核心行为通过`：核心逻辑经接口级测试通过。
5. `端到端接通`：存储、页面和必要 Adapter 穿过同一条业务路径。
6. `验证完成`：测试、类型、构建和相关验证通过。
7. `文档完成`：合同要求的进度、变更和任务状态已更新，实现完整交付。
8. `审查通过`：独立 Review 只读审查通过。
9. `已提交`：独立 Commit 按白名单完成提交。

节点 1～7 是实施参考路径；C0/C1 可在相关验证和 Checkpoint 完成后交付，C2/C3 需 Review，只有合同要求提交时才以 Commit 收尾。任务可 `TaskState: done` 且 `commit_status: pending`。"看起来能用"但没有相关验证和持久记录，不算完成。

## 8. 何时拆票

出现以下任一条件时考虑拆票：包含两个可独立演示或独立回滚的用户结果；同时引入新领域对象和真实外部 Adapter；公共接口或高风险面难以在一次交付中验证；新需求不在当前 approved 规格内。拆票依据写入任务或协调记录。

拆票方法：先更新规格或 `docs/changes.md` 说明为什么；票是**垂直切片**——一刀穿透所有层（数据、业务接口、页面、测试），完成的票可独立演示或验证，体量以一个新鲜上下文窗口能完成为限；每票写明 `Blocked by`（只列真正门禁它的票）、验收标准和不做什么；按依赖顺序编号入 `docs/issues/`，只有 blocker 全部 `done` 的票可以领取（frontier）。宽重构例外：机械的大范围变更按 expand–contract 走——先扩展（新旧并存）、分批迁移（每批一票）、最后收缩（删除旧形态）；批次无法独立保绿时共享集成分支，绿只在最终集成票承诺。票模板见 `docs/issues/README.md`。

## 9. Git 提交

- 根目录只有一个 Git 仓库，默认分支 `main`；功能用 `ticket/<NN>-<slug>` 分支，小型单人工作经用户同意可直接在 `main`。
- 提交格式 `<type>(<scope>): <结果>`，分支名由用户合同或既有仓库约定确定；未明确时使用当前分支，不自行创建或切换分支。type 允许 `feat` `fix` `docs` `test` `refactor` `build` `chore`。
- 一个提交是一个可解释的恢复点；不把多个任务塞进一个提交，不为"干净历史"重写或丢弃未确认归属的改动。
- 未经用户明确要求不 push、不部署、不发布。

## 10. 实现纪律（最懒可行）

像最懒的高级工程师一样写代码：懒是高效，不是马虎。理解问题永远不偷懒——先读任务和相关代码、走通真实流程，再按阶梯停在最先站得住的一级：

1. 这功能需要存在吗？推测性需求直接不做，一句话说明（YAGNI）。
2. 仓库里已经有了？先找再写，复用既有的 helper、类型、模式。
3. 标准库能做？用它。
4. 平台原生特性能覆盖？用它（原生控件优于选型库、数据库约束优于应用代码）。
5. 已安装的依赖能解决？用它；几行代码能做的事不加新依赖。
6. 能一行？就一行。
7. 最后才写：能工作的最小代码。

不许偷懒简化：信任边界的输入校验、防数据丢失的错误处理、安全措施、无障碍基本项、用户明确要求的东西。Bug 修根因不修症状——改前先查所有调用方，在公共路径上一处修好。非平凡逻辑（分支、循环、解析、金额/安全路径）必须留下一个最小可运行检查；平凡一行代码不需要测试，YAGNI 对测试同样成立。有已知天花板的刻意简化，在代码里注释标明天花板和升级路径。

## 11. 验证与记录

验证按风险选择与改动直接相关的证据：C0 通常做格式/链接/静态检查；C1 做局部行为、类型或构建检查；C2 增加接口/跨模块和独立 Review；C3 增加迁移演练、安全和恢复验证。无必要时不强制 TDD、全层 E2E 或共享 harness。每项验证记录命令、环境、结果和日期；未运行写 `N/A + reason`。

### 共享验证 harness

只有验证会重复使用、跨票复用或用户要求时才沉淀 `scripts/` harness；一次性命令记录在 Checkpoint 即可。

## 12. 会话恢复协议

Session（agent_session）是单次会话的易失上下文，随时可能中断或消失；project_network 是仓库内持久事实——文件、Git 状态、`docs/agent/artifacts.yaml`、progress、Checkpoint、run record——跨会话存续。任何需要跨会话存续的状态、结论或进度一律落盘到 project_network；恢复只从事实出发，不从聊天记忆出发。§4 Session 开始的接手门禁是本节恢复流程的快速形态；条文冲突以本节为准。

### 12.1 状态模型（命名空间固定）

| 状态模型 | 取值 | 说明 |
| --- | --- | --- |
| `SessionState` | `active` / `interrupted` / `unknown` / `recovering` / `closed` | 会话生命周期；`interrupted` 只表示 Session，不改变任务状态 |
| `TaskState` | `ready` / `in_progress` / `blocked` / `review_ready` / `review_pass` / `review_fail` / `done` | 交付状态；done 可与未提交并存，另记录提交状态 |
| `RequestState` | `proposed` / `triaged` / `accepted` / `specified` / `ready` / `rejected` / `deferred` / `needs-user-decision` | 需求队列状态 |
| `ApprovalState` | `draft` / `proposed` / `approved` | 审批状态，与 TaskState 分开；approved 由可追溯用户明确指令产生 |
| `Phase` | `coordination` / `intake` / `triage` / `implementation` / `review` / `commit` | 工作阶段，不是产品领域状态 |

产品领域生命周期使用 `RequestLifecycle`，当前值域与转换由批准规格 [`SPEC-001`](./specs/001-m0-foundation.md) 定义；它不与上述交付状态混用。`ApprovalState: draft` 表示未定稿草案，`proposed` 表示已提交用户审阅，`approved` 表示用户在明确范围内作出的可追溯确认；草案不得覆盖当前 approved 规范，后续会话沿用仍在范围内的 approved 指令。

#### R-DP-012 状态模型取值固定 `MUST`

- **When**：记录或推断任一状态模型时。
- **Action**：使用命名空间值（如 `TaskState: in_progress`、`Phase: review`）；落盘到对应票、REQ、run 或 progress。
- **Forbidden**：自造状态名、把产品状态写入交付状态字段、只在聊天中保留状态。
- **Stop if**：实际情况不属于允许值且无法归入模型 → 报告并暂停受影响工作。
- **Evidence**：状态字段和转换可静态检索。
- **Owner**：按 §12.5 写入所有权矩阵。
- **Authority**：本文 §12.1。

### 12.2 事实优先级（四层）

冲突时按以下顺序取信（高到低）：

1. 实际文件、测试结果、Git 状态；
2. Checkpoint / run record；
3. `docs/progress.md`；
4. 聊天记录。

低层级信息与高层级矛盾时以高层级为准并修正低层级记录，裁决结果落盘到被修正的记录。

#### R-DP-013 事实优先级 `MUST`

- **When**：任何来源对同一事实给出不一致结论时。
- **Action**：按本节四层顺序取信；IF 高层级来源内部互相矛盾且无法用实际文件裁决 THEN 停止并报告用户 ELSE 修正低层级记录并落盘。
- **Forbidden**：用聊天记忆覆盖文件事实；用 progress 覆盖 Checkpoint 或实际文件。
- **Stop if**：高层级来源互相矛盾且无法裁决 → 报告用户。
- **Evidence**：裁决结果落盘到被修正的记录。
- **Owner**：当前会话执行体。
- **Authority**：本文 §12.2。

### 12.3 恢复流程

Delivery Session 启动或接管时：

1. 读取最小必要上下文（AGENTS.md、progress、相关票/Checkpoint、必要时 run record）；
2. 确认当前 owner 与授权范围；
3. 先把 SessionState 置为 `recovering` 并落盘；
4. 对照实际文件、Git 和已有验证核实改动；
5. 仅在证据失效或发生新变化时重跑验证；
6. 记录恢复摘要与结论 `resume` / `blocked` / `conflict`，再继续或报告；
7. 若中断发生在 Review 前，交付后重新独立 Review。

Intake 只处理新 REQ 与索引并转交 Triage，不修改进行中的任务票。用户离线不阻止已授权工作。

#### R-DP-014 恢复顺序 `MUST`

- **When**：接管中断工作时。
- **Action**：按上述顺序确认 owner、恢复状态、文件与证据，并使 progress 摘要与票/Checkpoint/run 详细记录一致。
- **Forbidden**：跳过 owner/授权确认；无变化却机械重跑全部验证；凭聊天记忆恢复。
- **Stop if**：实际文件与规范或授权范围存在无法裁决的冲突。
- **Evidence**：Checkpoint/run record 含 SessionState、owner、结论与验证依据。
- **Owner**：当前 Coordinator 或运行 owner。
- **Authority**：本文 §12.3。

### 12.4 Run record

生成条件（满足其一）：C2/C3 任务；跨会话交接；发生中断；无 Git 基线；改动归属不明。C0/C1 通常只写 Checkpoint；无票治理把证据追加到 `docs/changes.md` 指定区，不强制 run record。文件：`docs/agent/runs/<run-id>.json`；`run_id` 含日期与随机后缀。字段定义以本节表格为准，另有可选 `independent_review` 指针；本仓库不依赖外部 schema。

| 字段 | 含义 |
| --- | --- |
| run_id | 日期 + 随机后缀的唯一标识 |
| mode | 运行模式（coordination / delivery / intake / triage） |
| phase | `Phase` 值（coordination / intake / triage / implementation / review / commit） |
| task | 关联任务票标识 |
| status | 类型化对象：`{session: SessionState|null, task: TaskState|null, request: RequestState|null, approval: ApprovalState|null}` |
| scope | 本次运行的任务 Scope |
| baseline | `{vcs_ref, workspace_fingerprint}`；无 Git 时 `vcs_ref` 记 `N/A + reason` |
| modified_files | 本次运行修改的文件清单 |
| last_verified | `{command, result, at}` 最后一次验证 |
| next_step | 下一步动作 |
| blocker | 阻塞与原因；无则空 |
| network | 网络可用性状态（available / unavailable / unknown） |
| updated_at | 最后更新时间 |

- `workspace_fingerprint` 算法（定稿，版本前缀 `fp-v1`）：①收集工作区全部文件，排除 `.git/`、依赖与构建产物目录（`node_modules/`、`dist/`、`build/`、`coverage/`、`__pycache__/`、`.venv/`）与 `docs/agent/runs/`（运行记录自身不参与指纹，避免自引用漂移）；②按相对路径字典序排序，每文件一行 `<相对路径>\t<字节数>\t<mtime 纪元秒>`；③拼接后取 SHA-256 十六进制前 16 位，记 `fp-v1:<hex16>`。同算法两次计算不一致即工作区自基线后有变化，按 R-DP-013 归属。
- `independent_review` 字段（可选）定稿：独立 Review 记录本体落盘任务票 `## Independent Review Checkpoint`（Review 唯一写入目标；格式：日期、结论 pass/fail、逐项证据 `文件:行号区间`、验证命令与环境）；run record 的该字段由当前运行执行体在 Review 结论落盘后回填指针 `{verdict, ticket_ref, checkpoint, at}`，不承载审查本体。

#### R-DP-015 Run record 生成与维护 `MUST`

- **When**：命中任一生成条件，或已有 run record 的运行状态变化时。
- **Action**：创建或更新 `docs/agent/runs/<run-id>.json`，字段按本节表格；每完成一个可验证步骤更新 `last_verified` 与 `next_step`。
- **Forbidden**：C0/C1 强制创建 run record；run record 与 Checkpoint 内容矛盾不处理；编造 baseline、验证结果或网络状态。
- **Stop if**：无法确定 baseline → 如实记录 `N/A + reason`，不编造。
- **Evidence**：run record 存在、字段可对照本节表格、`updated_at` 与实际活动一致。
- **Owner**：按 `mode` 与 `phase` 由当前会话角色写入；delivery/implementation 由 Implementation 或担任 Implementation 的 Coordinator，review 由 Review，commit 由 Commit 或 Coordinator，intake 由 Intake，triage/coordination 由 Triage 或 Coordinator。
- **Authority**：本文 §12.4。

### 12.5 写入所有权矩阵

| 会话/角色 | 允许写入 | 禁止 |
| --- | --- | --- |
| Intake | 新 REQ 文件及所属索引登记 | 进行中任务票、progress、规格 |
| Triage | Request 状态流转、规格、任务票 | 产品实现文件、Implementation Checkpoint |
| Coordinator（主 agent） | 授权范围内的规格/任务/索引/进度草案与状态；C0/C1 可逆治理、记录和小范围代码；Artifact Plan；协调 Checkpoint；读取代码验收 | C2/C3 生产实现、数据迁移、权限、外部集成；Review 结论代写 |
| Delivery（Implementation） | 任务 Scope 内文件 + Implementation Checkpoint | Review 记录、Commit 操作 |
| Review | 有票时写票内 Independent Review Checkpoint；无票时向 Coordinator 返回报告，由 Coordinator 记录；Review 阶段 run record 更新由 Review 写入 | 修改被审内容、代写自己的 Review 结论 |
| Commit | 暂存与提交记录；Commit 阶段 run record 更新 | 扩大范围、push、deploy |

run record 由当前 `mode` 对应的会话角色写入（R-DP-015）；`independent_review` 指针由记录 owner 在 Review 结论落盘后回填。共享记录单 writer；禁止两个 session 同时写同一记录；`docs/requests/` 是 Intake 与 Delivery 的唯一目录交集（Intake 写新 REQ，Delivery 不写）。Intake 与 Delivery 并行规则的队列侧表述见 `docs/requests/README.md`，与本矩阵一致；冲突以更严格者为准。

#### R-DP-016 写入所有权 `MUST`

- **When**：任何会话或阶段执行体写入治理文件或仓库时。
- **Action**：只写本角色允许的目标；跨角色内容交还对应阶段。
- **Forbidden**：两个 session 同时写 `docs/progress.md`；Implementation 自审通过；Commit 重写实现内容。
- **Stop if**：写入目标被其他 session 占用或发现并行写入痕迹 → 停止写、以文件现状为准（R-DP-013）、报告。
- **Evidence**：每处变更可归属到唯一角色（对齐 `git status` 归属解释要求）。
- **Owner**：各阶段执行体或 Coordinator（按写入所有权矩阵）。
- **Authority**：本文 §12.5。

### 12.6 阶段故障处理

#### R-DP-017 Implementation 中断 `MUST`

- **When**：Implementation 执行体中断或消失后恢复时。
- **Action**：保留已有改动，将 Session 标为 `SessionState: interrupted`，并将任务保持为 `TaskState: in_progress` 或按实际阻塞记为 `TaskState: blocked`；依据 Checkpoint/run record 记录已完成部分与 `next_step`，续跑未完成部分。
- **Forbidden**：中断即回滚；丢弃未解释的改动。
- **Stop if**：改动无法归属（不在 Scope、无记录）→ 按六条不变量第 6 条报告，不动文件。
- **Evidence**：`SessionState: interrupted`、对应 `TaskState` 与 `next_step` 落盘到 Checkpoint 或 run record。
- **Owner**：恢复后的 Delivery Session 执行体。
- **Authority**：本文 §12.6。

#### R-DP-018 Review 中断 `MUST`

- **When**：Review 执行体中断或消失后恢复时。
- **Action**：先前未完成的 Review 不得当作 pass；新 Review 重读任务合同、完整 diff 与证据，重新只读审查；IF diff 或证据在中断后发生变化 THEN 以最新实际状态为准重新审查 ELSE 按新审查流程继续。
- **Forbidden**：把半途 Review 记为 `review_pass`；凭中断前印象补结论。
- **Stop if**：任务合同或证据不可得 → 停止并报告，不出结论。
- **Evidence**：Review 报告注明审查所依据的 diff/证据快照与时间。
- **Owner**：Review 执行体（独立于实现者）。
- **Authority**：本文 §12.6。

#### R-DP-019 Commit 中断 `MUST`

- **When**：Commit 执行体中断或消失后恢复时。
- **Action**：先核查 HEAD、暂存区、白名单与独立 Review pass 证据，再决定重试提交或如实记录中断状态。
- **Forbidden**：盲目重试提交（可能产生重复提交）；跳过核查直接暂存。
- **Stop if**：Review pass 证据缺失或白名单不满足 → 不提交，记录阻塞并报告。
- **Evidence**：核查结论与提交结果（或阻塞记录）落盘。
- **Owner**：Commit 执行体。
- **Authority**：本文 §12.6。

### 12.7 破坏性恢复禁令

#### R-DP-020 破坏性操作禁令 `MUST NOT`

- **When**：任何恢复、清理或纠偏动作评估时。
- **Action**：改用非破坏路径：报告归属、请用户裁决、用新增修复替代抹除。
- **Forbidden**：`git reset --hard`；`rm -rf`；删除未跟踪文件；覆盖归属不明的文件或改动。
- **Stop if**：只有破坏性手段能达到目的 → 停止并把选项交用户。
- **Evidence**：恢复过程无上述操作；例外（用户明确授权）有落盘记录。
- **Owner**：所有执行体。
- **Authority**：本文 §12.7。

### 12.8 六条不变量（按风险适用）

1. **无持久状态不算完成**——所有 C0-C3 完成声明都必须有落盘状态。
2. **无相关验证不得交接**——所有 C0-C3 都要有相关验证；不适用时记录 `N/A + reason`。
3. **无独立 Review pass 不得 Git Commit**——所有 Git Commit 都适用；低风险工作可交付并保持未提交。
4. **文件/Git 状态优先于聊天记录**——所有恢复和归属判断都适用。
5. **网络不可用只能 fallback 或明确阻塞**——仅在任务使用网络时适用。
6. **归属不明改动不得删/盖/重置**——所有修改和恢复都适用。

#### R-DP-021 不变量硬门禁 `MUST`

- **When**：任何阶段出口判断与交接时。
- **Action**：按任务复杂度逐条核对适用不变量；适用项任一不满足即停止并阻塞，未适用项记录原因。
- **Forbidden**：以复杂度或进度压力绕过适用不变量；以口头结论替代持久证据。
- **Stop if**：不变量被违反且无法就地纠正 → 记录违反事实并报告用户。
- **Evidence**：阶段出口记录含不变量核对结果。
- **Owner**：各阶段执行体或 Coordinator。
- **Authority**：本文 §12.8。

## 13. 解释与例外

- 首次为既有项目补 seed 时，Record 类既有文件按 R-DP-003 保护，只补缺失件；既有文档、代码布局、命名和规则一律视为项目事实，只登记、只补缺。
- 本文与根 `AGENTS.md`、目录 README、任务票等文件冲突时，以本文为准修正引用；条文与三阶段执行体定义冲突时以更严格者为准。
- 本项目的初始化访谈与盘点结果已填入目录边界、角色合同和验证命令；后续未确认事实继续写【待定：...】，不编造领域事实。用户明确授权在范围内持续有效，常规选择记录即可。
- 需求请求的队列规则（Intake 边界、Triage 边界、请求状态机）见 `docs/requests/README.md`；本文只承载任务执行流程，不重复队列细节。
- 目标项目当前不是 Git 仓库时：恢复流程第 5 步退化为实际文件对照，`baseline.vcs_ref` 记 `N/A + reason`，`workspace_fingerprint` 照常计算；该状态由 Git 化任务结束。
- `needs-user-decision` 不同于 `blocked`：前者是 Request 等用户裁决，后者是 Task 停止推进。
- 三阶段执行体文件固定为 `docs/agent/roles/implementation.md`、`docs/agent/roles/review.md`、`docs/agent/roles/commit.md`；本文仍只声明能力与阶段职责，不绑定宿主工具名。
