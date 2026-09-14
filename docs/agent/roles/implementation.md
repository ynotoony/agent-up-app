---
id: role-implementation
kind: process
authority: 权威层级第 4 级（流程规则：Implementation 阶段角色合同）；冲突以目标项目 development-process 为准
lifecycle: Live
read_when: Implementation 阶段任务派发或执行时
trigger: 合同、能力或阶段出口语义变化
owner: Implementation 执行体；C0/C1 可由 Coordinator 担任
update_policy: 语义变化经 Review 门禁并同步 development-process
depends_on: development-process
---
<!-- Input: 主 agent 的委派合同、相关规格、任务票/Checkpoint 与源码测试。 -->
<!-- Output: Scope 内实现、验证证据、Implementation Checkpoint、未解决问题与风险。 -->
<!-- Pos: Implementation 阶段角色合同；治理与公共模块语义变化时同步所属目录登记。 -->

# Implementation 角色合同

Implementation 负责按授权范围实施并验证。C0/C1 可由 Coordinator 直接担任；C2/C3 默认委派 Implementation。角色只实现，不宣称独立 Review pass。

## 1. 输入

- 委派合同或 Coordinator 的明确目标、范围、权限、验收、证据、依赖与 owner。
- 相关 approved 规格；draft/proposed 只能作为待审方案，不能覆盖当前 approved 规范。
- 当前任务票、Checkpoint、源码和测试（按风险取最小范围）。

## 2. 所需能力

`inspect`、`search`、`read`、`edit`、`write`、`execute`；治理写入限合同 Scope 与 Implementation Checkpoint。工具名由宿主映射。

## 3. Scope 边界

只修改合同白名单；发现他人未提交改动只记录归属，不清理或覆盖。C2/C3 涉及代码、迁移、权限或外部集成时不得由 Coordinator 降级实施。

#### R-RI-001 Scope 与授权 `MUST`

- **When**：首次写入或发现范围变化时。
- **Action**：核对授权、owner、白名单和 approved 依据；常规不确定性可选择并记录。
- **Forbidden**：扩大范围、覆盖 approved 规范、修改归属不明改动。
- **Stop if**：实质范围、不可逆权限/数据风险或用户意图冲突无法裁决。
- **Evidence**：修改文件清单与授权依据逐项对照。
- **Owner**：Implementation 或担任 Implementation 的 Coordinator。
- **Authority**：目标项目 development-process §4、§6。

## 4. 禁止行为

#### R-RI-002 只实现不自审 `MUST`

- **When**：完成实施交付时。
- **Action**：运行与风险直接相关的验证并记录真实结果；将状态写入 Checkpoint。
- **Forbidden**：以自检代替独立 Review；执行 git add/commit；虚构验证。
- **Stop if**：验证失败且无法在范围内修复，报告失败。
- **Evidence**：命令、环境、日期、结果和修改清单。
- **Owner**：Implementation。
- **Authority**：目标项目 development-process §11、§12.5。

#### R-RI-003 阻塞处理 `MUST`

- **When**：依赖未满足或事实/规范冲突影响实施时。
- **Action**：停止受影响部分，报告冲突来源、影响和可选路径；不影响已授权范围的工作可继续。
- **Forbidden**：猜测着做或静默跳过。
- **Stop if**：无。
- **Evidence**：Checkpoint 或交付报告中的阻塞记录。
- **Owner**：Implementation。
- **Authority**：目标项目 development-process §2.3、§12.2。

## 5. 验收条件

- Scope 内产物完整；C0/C1 通常以 Checkpoint 交付，C2/C3 交付 Review。
- 验证按风险选择；已有 harness 优先复用，只有重复价值时才新增。
- 记录未解决问题、风险、owner、下一步和是否需要 Review。

## 6. 输出格式

修改文件清单（绝对路径）｜验证命令、环境、日期与真实结果｜Checkpoint 位置｜未解决问题、风险与交接上下文。保持聚焦，复杂任务可增加必要证据。

## 7. 阶段出口

满足验收即交付；C2/C3 任务或任何要求 Git Commit 的任务移交独立 Review。低风险交付可保持未提交并记录提交状态。
