<!-- Input: tests/ 目录成员、standby 地位裁决事实与经用户确认的聚焦回归 harness。 -->
<!-- Output: 静态门禁脚本备用库与手动聚焦回归 harness 的导航及成员登记。 -->
<!-- Pos: tests 目录索引；一旦我被更新，务必更新我的开头注释，以及所属文件夹的 README.md。 -->

# tests（m0_* 静态门禁脚本 · standby 备用）

## 架构

- 本目录九件 `m0_*` 脚本（agent-up 前仓静态门禁）按 2026-10-08 合并裁决为 **standby 备用**：非现役门禁，不在任何验证链中执行。`ticket-ops-empty-index.sh` 是经用户确认的单问题手动回归，不加入现役门禁链。
- `release-preflight.sh` 是发布输入/工作树预检回归，只使用临时仓库与 stub 工具，不执行真实构建、打 tag、推送或发布。
- 现役门禁链只认 `cargo test --manifest-path src-tauri/Cargo.toml` ＋ `pnpm typecheck`（`rules/project.md` 项目自有条款 2）；复活本目录脚本或新增门禁机械须用户裁决（防「九处 parity 门」过度流程化死因复发）。
- 潜在再利用：`m0_budget_parity.py` 的预算核对形态可经用户裁决改造为其他机械检查；改造前零写入。

## 目录清单

| 名字 | 地位 | 功能 |
| --- | --- | --- |
| `README.md` | 目录索引 | standby 声明与成员登记（本件）。 |
| `m0_budget_parity.py` | standby 脚本 | 前仓治理预算核对（备用）。 |
| `m0_cli_parity.py` | standby 脚本 | 前仓 CLI 契约核对（备用）。 |
| `m0_command_registry_parity.py` | standby 脚本 | 前仓命令注册表核对（备用）。 |
| `m0_registry_check.py` | standby 脚本 | 前仓注册表静态检查（备用）。 |
| `m0_renderer_capabilities.py` | standby 脚本 | 前仓渲染层能力核对（备用）。 |
| `m0_repo_isolation.py` | standby 脚本 | 前仓仓隔离核对（备用）。 |
| `m0_schema_parse.py` | standby 脚本 | 前仓 schema 解析检查（备用）。 |
| `m0_schema_validate.mjs` | standby 脚本 | 前仓 schema 校验（备用）。 |
| `m0_ui_style.py` | standby 脚本 | 前仓 UI 样式核对（备用）。 |
| `ticket-ops-empty-index.sh` | 手动聚焦回归 | 验证空 `issues` 索引首次及后续开票、拒绝路径零写入和派生投影一致性；用法：`sh tests/ticket-ops-empty-index.sh`。 |
| `release-preflight.sh` | 手动聚焦回归 | 验证缺失 notes 在任何发布工具调用前失败、非忽略未跟踪文件阻断预检且忽略 facts 获豁免；用法：`sh tests/release-preflight.sh`。 |
