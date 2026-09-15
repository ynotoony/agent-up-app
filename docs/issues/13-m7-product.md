<!-- Input: approved SPEC-011。 -->
<!-- Output: S1–S3 E2E、诊断脱敏、打包门。 -->
<!-- Pos: M7 垂直切片。 -->

# 13 — M7-01 三场景与发布门

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 三条场景 command 路径可重复跑绿；诊断包脱敏；打包开启且无自动更新。

**Blocked by:** W6 done；SPEC-011 approved。

**Status:** review_pass

- [x] S1–S3 E2E
- [x] export_diagnostics 脱敏
- [x] bundle.active true 且无 updater
- [x] 隐私与手动更新说明

## Implementation Checkpoint

**2026-09-15 / review_ready**：S1–S3 E2E、export_diagnostics、bundle.active、无 updater。未 push、不自审。

## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。对照 approved SPEC-011 验收 1–5 与本票：S1/S2/S3 映射到 `m7_scenarios.rs` 三条测试且全绿；`export_diagnostics` 写 `.agentup/diagnostics/<id>.json`，去掉 `project_path`、绝对路径字符串改为 `[path]`、密钥类字段改为 `[redacted]`，附件事实不含字节；`bundle.active=true` 且 Cargo.toml / tauri.conf / package.json / Cargo.lock 无 updater；`properties/` gitignore，tsconfig/vite 只吃 `src/`；renderer `button`/`input` 无负 tabindex、无正 tabindex 陷阱。无足以单独否决本票的 P0。Coordinator 可派发 Commit。不 git commit / 不 push。
- **审查范围**：`docs/issues/13-m7-product.md`（本 Review 只改 Status 并追加本段）、`docs/specs/011-m7-product.md`（approved）、commit `4c1c7c7`（`src-tauri/src/{diagnostics,lib,runtime}.rs`、`src-tauri/tests/m7_scenarios.rs`、`src-tauri/tauri.conf.json`、permissions/capabilities、schemas、`src/App.tsx`、`tests/{m0_renderer_capabilities,m7_pack_gate}.py`、`docs/{privacy,manual-update}.md`）。对照未改的 `agent_run.rs` / `discussion.rs` / `results.rs` / `typed_facts.rs` 只读核验写范围、附件无字节、反馈开新任务。未改 src-tauri 实现、未代修、未 commit、未改 `docs/progress.md`、未写 `gen/`。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M7 Implementation（`4c1c7c7`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 CST。工作树 `/private/tmp/codex-w7-product` 分支 `codex/w7-product`。依据独立 Review 门禁，此路径可独立。不以 Implementation 自述为证据。用户禁止改 impl / 禁止 progress / 禁止 gen。
- **Diff / 证据快照**：`HEAD=4c1c7c7752c1034fc36ecaf050f56b14bdf18305`（`feat(m7): s1-s3 e2e diagnostics and pack gate`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_pass`。

### 验证（本 Review 复跑，2026-09-15，cwd `/private/tmp/codex-w7-product`）

| 检查 | 结果 | 环境 |
| --- | --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` | pass：49 passed / 0 failed。lib.rs 0；main.rs 0；m0_runtime 7；m1_facts 4；m1_projection 3；m2_5_external 5；m2_project 4；m3_discussion 2；m4_scope_task 4；m5_5_reliability 5；m5_fake_agent 6；m6_results 5；m7_scenarios 4 | rustc/cargo；`--offline`；`CARGO_TARGET_DIR=/tmp/codex-w7-product-target` |
| `python3 tests/m0_renderer_capabilities.py` | pass：`renderer capability static check ok`；scanned 3 renderer files | python3 |
| `python3 tests/m7_pack_gate.py` | pass：`pack gate ok` | python3 |
| `git diff --check` | pass：clean | git；相对工作区 |

### P0

无未关闭、足以单独否决本票的 P0。验收 1–5 均有对口测试或静态门；S1 反馈开新一轮由既有 `submit_feedback` 实现，测试未再断言 task。

### 逐项验收（对照 SPEC-011 验收 1–5）

| 必须核对 | 判断 | 测试证据 | 实现证据 |
| --- | --- | --- | --- |
| 1. S1–S3 三条测试全绿 | 成立 | `src-tauri/tests/m7_scenarios.rs:38-70` S1：init→draft request→讨论+附件→scope→fake write 范围内→review fail 则 `commit_changes`=`review_required`→review pass→commit→publish/accept→`submit_feedback`→drop Runtime 后 `load_project`/`load_result_history`。`:74-95` S2：最小 scope `src/save.ts`，越权写 `src/button.tsx` 失败，范围内写成功并 publish。`:97-113` S3：无 included scope，写源码 `invalid_input`，只 publish/accept 评估 result，源文件不变 | create_request 缺 lifecycle 时插入 `draft`：`runtime.rs` create_request。越权写：`agent_run.rs:164-170`。feedback 带 title 则 `put_task`：`results.rs:63-78` |
| 2. `export_diagnostics` 无绝对路径、无密钥、无附件字节 | 成立 | `m7_scenarios.rs:115-126`：metadata `api_key=sk-live-secret` 导出后文本不含项目绝对路径、不含该密钥、不含 PNG/`bytes` | 写 `.agentup/diagnostics/<id>.json`：`diagnostics.rs:2-36`。只扫 `facts/`：`:38-59`。删 `project_path`；`/` 或盘符字符串→`[path]`；键含 `api_key`/`secret`/`password` 或 `token`/`_token`→`[redacted]`：`:61-106`。附件 content 只有 path/sha/length，禁止 bytes：`discussion.rs` add_attachment content；`typed_facts.rs` parse_attachment |
| 3. 仓库无 updater 依赖；`bundle.active` 为 true | 成立 | `tests/m7_pack_gate.py:6-16`：Cargo.toml / tauri.conf / package.json 禁 updater；`bundle.active is True` | `src-tauri/tauri.conf.json:32-43` `bundle.active=true`，targets `app`/`dmg`，无 updater 键。`src-tauri/Cargo.toml` tauri features `[]`，无 plugin-updater。`package.json` 无 plugin-updater。`Cargo.lock` 无 `tauri-plugin`。`docs/manual-update.md:5-14` 手动更新；卸载不删 `.agentup/` |
| 4. `properties/` 不在构建输入 | 成立 | `tests/m7_pack_gate.py:20-21`：App.tsx 不含 `properties/`。`.gitignore:1-2` `/properties/` | `tsconfig.json:20` include 仅 `src`。`vite.config.ts` `outDir=dist`。tauri `frontendDist=../dist`。工作区无 `properties/` 目录 |
| 5. renderer 主路径控件可键盘聚焦（button/input 无正 tabindex 陷阱；无负 tabindex） | 成立 | `tests/m7_pack_gate.py:17-24`：禁 `tabIndex={-` / `tabIndex="-`；要求存在 button 与 input。`tests/m0_renderer_capabilities.py` 仍禁 fs/shell/network/secret，且允许 `export_diagnostics` | `src/App.tsx:75-149`：原生 `<input>`/`<button type="button">`/`<textarea>`，无 `tabIndex`、无 `disabled`。`docs/privacy.md:9-12` 诊断不上送 |

测试名与断言对齐：S1/S2/S3 与 diagnostics 四测锁的就是对应合同。验收 1 的「反馈开新一轮」测试只调用 `submit_feedback` 未断言新 task，不构成 P0 否决。

### 非 P0 残留（不挡本门）

- S1 未断言 `submit_feedback` 新建 ready task；实现带 title 会 `put_task`。
- S2 未调用 `load_result_history`；只 publish。实现与 M6 测试仍可读。
- `diagnostics_redact` 未先 `add_attachment`；「无附件字节」靠附件事实 schema + 只扫 `facts/`，测试里 `contains("bytes")` 是弱启发式。
- 密钥脱敏按字段名，不是值扫描；`token` 只精确匹配或 `_token` 后缀，比规格 `(?i)(api_key|secret|token|password)` 窄。正文里的绝对路径不会被整段替换。
- 场景「公共失败」未在 `m7_scenarios.rs` 复锁；未确认不写盘 / 错误不回显根外路径 / Runtime 恢复仍由既有 m0 测试覆盖。
- `privacy.md` / `manual-update.md` 未登记 `docs/agent/artifacts.yaml`（本 Review 不代补）。
- renderer 新增按钮走裸 `invoke`，不更新 Result `<pre>`。permissions 描述仍写「M0, M2, M3 and M4」。

### 后续

移交 Commit。本 pass 只关闭 M7 Independent Review 门禁；不 git commit / 不 push。并列缺口不阻塞本票 Commit，除非 Commit 执行体发现实现在审查之后被改动。Coordinator 可以派发 merge/commit 执行体。
