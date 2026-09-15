<!-- Input: EXEC-001 W2.5、SPEC-002/004、用户默认批准。 -->
<!-- Output: 外部变化不得静默丢改动的合同。 -->
<!-- Pos: W2.5 外部变化门；approved。 -->

# SPEC-005 — 外部变化门

**ApprovalState:** approved

**依据**：EXEC-001 W2.5；用户默认批准。

## 规则

1. **外部改文件**：preview 之后项目文件（非 `.agentup/`、非 `.git/`）被外部改动，则 `fp-v1` 变化；`initialize_project` 必须 `confirmation_expired`，不得写入。
2. **Git 未提交变更**：工作区普通文件的未提交改动计入 `fp-v1`（`.git/` 目录本身仍排除）。不得把「未 git add」当成未变化。
3. **符号链接**：不得跟随穿越项目根；失败 `path_outside_project`，不写盘。
4. **并发写入**：同一 request 的旧 `expected_revision` 不得覆盖；`revision_conflict` 且 `error.details.revision` 为当前值；不产生第二事件。
5. **目录失效**：canonical 路径不存在或不是目录 → `path_not_found` / `path_state=missing`，不在失效路径上创建 `.agentup/`。

静默丢用户改动（覆盖、跳过冲突、把过期 token 当有效）一律禁止。

## 验收

上述五类各有外部行为测试。本门不过，不准开始 W3 讨论/附件实现。
