<!-- Input: mattpocock/skills 的 retro SKILL.md 原文与 AgentUp 当前治理规则。 -->
<!-- Output: 对 retro skill 的事实摘要、适用性判断与后续决策状态。 -->
<!-- Pos: AgentUp 调研归档；观点变化时新建报告，不覆盖本文件。 -->

# 2026-09 — retro skill 调研

**问题**：评估 `mattpocock/skills` 的 `retro` skill 是否适合作为 AgentUp 自优化或自进化机制。

**证据**：

- 原文：[skills/in-progress/retro/SKILL.md](https://github.com/mattpocock/skills/blob/main/skills/in-progress/retro/SKILL.md)，于 2026-09-14 读取。
- 该 skill 的名称是 `retro`，描述是对一次 coding session 做 retrospective，并且设置 `disable-model-invocation: true`，说明它需要用户主动触发。
- 它要求读取用户指定的 session logs；未指定时使用当前 session。
- 它从七类环境问题中寻找改进候选：Navigation、Automated checks、Coding standards、Global AGENTS.md、Tool economy、No-ops、Information access。
- 它只向用户按严重性呈现改进候选，不自动修改规则、代码或工具。
- 它特别区分 Implementation 与 Review：Implementation 承受较大的上下文压力，负责探索、编码和调试；Review 上下文压力较小，更适合承担编码标准检查。
- 它建议把 `AGENTS.md` 保持为轻量导航，把更长的编码标准放入独立的 `CODING_STANDARDS.md`，并要求先复用现有文档而不是重复创建规则。

**与 AgentUp 的关系**：

- 可直接复用的是会话日志复盘、候选分类、严重性排序和“只建议、不自动改环境”的边界。
- 它补充了 AgentUp 当前规则较少覆盖的工具经济、无效指令和信息访问问题。
- 它不能替代 AgentUp 的事实层、指标层、实验、审批、独立 Review、发布和回滚机制。
- 它没有定义候选改进的结构化 schema、重复性证明、实验基线、采纳状态或效果验证，因此单独使用不能构成自优化闭环。

**结论**：暂不实施 `retro` skill。保留其作为未来“会话复盘与改进候选生成”模块的参考；如果后续实现，应先把候选改进接入 AgentUp 的事实记录、风险分级、用户批准、任务票、独立 Review、实验验证和回滚流程，再决定是否自动化。

**被引用**：当前没有采用该调研结论的 ADR 或产品实现。后续若启动自优化能力，应引用本报告并新建实施规格，不修改本报告。
