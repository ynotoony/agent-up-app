<!-- Input: 根 README、docs/development-process.md、docs/agent/artifacts.yaml。 -->
<!-- Output: 治理文档目录导航和产物入口。 -->
<!-- Pos: 项目治理文档索引；成员变化时同步 artifacts.yaml。 -->
<!-- read_when: 需要读取流程、计划、任务、规格或记录时。 -->

# 文档目录

## 架构

- `development-process.md` 是开发流程唯一权威。
- `development-plan.md` 是 Tauri 2 产品开发计划。
- `agent/artifacts.yaml` 登记治理产物及其依赖。
- `progress.md`、`changes.md`、`requests/` 保存可恢复记录。

## 目录清单

| 名字 | 地位 | 功能 |
| --- | --- | --- |
| `development-process.md` | 流程权威 | 读取阶梯、风险分级门禁、恢复和提交协议。 |
| `development-plan.md` | 开发计划 | 产品、事实层、runtime、测试和发布计划（待确认）。 |
| `execution-plan.md` | 执行计划 | 首版波次、场景脚本和预算默认值；`ApprovalState: approved`。 |
| `CONTEXT.md` | 领域上下文 | 用户、Agent、任务、讨论、范围和结果术语。 |
| `progress.md` | 当前记录 | 里程碑状态和可验证检查点。 |
| `changes.md` | 追加记录 | 需求和治理变更审计。 |
| `requests/` | 需求队列 | REQ 请求和 Intake 规则。 |
| `issues/` | 任务票索引 | 多票开发任务和依赖。 |
| `specs/` | 规格索引 | 公共行为和接口规格。 |
| `research/` | 调研索引 | Tauri 方案和外部证据。 |
| `agent/` | Agent 产物 | 角色合同和机器产物索引。 |
