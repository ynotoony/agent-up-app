<!-- Input: tests/ 目录成员与 standby 地位裁决事实（facts/project/archive/docs/lineage.md、rules/project.md 项目自有条款 1，2026-10-08）。 -->
<!-- Output: 静态门禁脚本备用库的导航：成员登记与冻结声明。 -->
<!-- Pos: tests 目录索引；一旦我被更新，务必更新我的开头注释，以及所属文件夹的 README.md。 -->

# tests（m0_* 静态门禁脚本 · standby 备用）

## 架构

- 本目录九件 `m0_*` 脚本（agent-up 前仓静态门禁）按 2026-10-08 合并裁决为 **standby 备用**：非现役门禁，不在任何验证链中执行。
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
