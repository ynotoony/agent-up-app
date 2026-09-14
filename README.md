<!-- Input: 仓库文件清单、docs/README.md、docs/development-process.md、docs/development-plan.md。 -->
<!-- Output: AgentUp 仓库入口与顶层目录导航。 -->
<!-- Pos: 仓库根目录导航；直接成员变化时同步更新。 -->
<!-- read_when: 每次进入仓库后定位代码、文档和事实边界时。 -->

# AgentUp

AgentUp 是一个基于 Tauri 2 的本地 Agent 工作台。生产 App 从仓库根目录全新开始，`properties/` 只保留 React 原型和设计参考，并已被 Git 忽略。

## 顶层边界

- `src-tauri/`：Rust runtime、项目事实、权限、Agent 和本地存储。
- `src/`：全新 renderer 和 UI 组件。
- `schemas/`、`tests/`：契约和验证。
- `docs/`：流程、规格、计划、任务和审计记录。
- 每个用户项目的事实保存在该项目根目录 `.agentup/`。

## 直接成员

| 名字 | 地位 | 功能 |
| --- | --- | --- |
| `AGENTS.md` | 路由入口 | 指向唯一开发流程和项目边界。 |
| `.gitignore` | 仓库配置 | 忽略原型、依赖、构建产物和本地敏感数据。 |
| `docs/` | 治理文档 | 保存开发流程、计划、规格和记录。 |
| `properties/` | 未跟踪参考 | React 原型、截图和设计压缩包；不进 Git。 |
