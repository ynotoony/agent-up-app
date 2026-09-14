<!-- Input: 已确认的需求、治理决定、范围限制和实施验证记录。 -->
<!-- Output: 按日期追加的项目事实与治理变更审计记录。 -->
<!-- Pos: 变更历史权威，只增不改。 -->

# 变更记录

## 2026-09-14 - 切换为仓库根目录全新 Tauri App

- **范围**：开发计划移至仓库根目录；`properties/` 仅作参考并加入 `.gitignore`；初始化 Agent Up 治理骨架；未创建业务代码。
- **验证**：2026-09-14；`git diff --check` 通过；Ruby YAML 解析 `docs/agent/artifacts.yaml` 通过；14 条登记项字段和路径核对通过；`properties/` 被 `.gitignore` 忽略；未运行原型 `npm` 命令，理由：原型目录按用户要求不进入本仓库。
- **同步门槛**：命中拓扑和契约变化，已同步根 README、docs README、development-process 和 artifacts 索引。
- **阶段与边界**：治理初始化已完成；未创建 `src-tauri/`、未安装 Tauri、未接入模型、未执行部署；开发计划仍待用户最终批准。
