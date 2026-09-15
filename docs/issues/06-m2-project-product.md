<!-- Input: approved SPEC-004。 -->
<!-- Output: 多项目登记、失效、重绑定、删除备份的外部行为。 -->
<!-- Pos: M2 垂直切片票。 -->

# 06 — M2-01 项目产品切片

**Type:** feature

**Priority:** P1

**Complexity:** C3

**What to build:** 用户能登记两个项目、看到目录失效、重绑定后恢复同一项目，并能在确认后备份并删除 `.agentup/` 而不动项目源码。

**Blocked by:** M1-02 done；SPEC-004 approved。

**Status:** done

- [x] `list_projects` / `register_project` / `rebind_project` / `remove_agentup`
- [x] 两项目隔离；missing path 不写项目盘
- [x] remove 先备份再删 `.agentup/`，不动源码
- [x] capability 只扩这四条 command；仍无 fs/shell

## Implementation Checkpoint

**2026-09-15 / in_progress**：W1 done。按 SPEC-004 实施。

**2026-09-15 / review_ready**：Implementation 交付四条 command。App 索引写在应用数据目录 `app-index.sqlite`，不是项目事实源；`list_projects` 只更新索引里的 `path_state`，missing 时 scan/load 返回 `path_not_found` 且不重建项目盘。`register_project` 要求已 scan；`rebind_project` 仅在旧 path missing 时改索引并校验 manifest `project_id`。`remove_agentup` 空 token → `confirmation_required`；有效一次性 token 先 sibling 备份 `.agentup.backup.<utc>` 再删 `.agentup/`，源码与 `.git` 不动。capability 只把这四条加入 allowlist，仍无 fs/shell。测试用 `mint_remove_confirmation` 签发 remove token（非 Tauri command）。
验证（2026-09-15，macOS aarch64，cwd `/Users/bic/Projects/agent-up`）：
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → lib/main 0 tests；`m0_runtime` 7 passed；`m1_facts` 4 passed；`m1_projection` 3 passed；`m2_project` 4 passed（两项目隔离且 register 不写项目盘、missing 不写盘、rebind 同一 project_id、无 token 拒绝且 backup 含 manifest）。`--offline` 一次通过，未联网。
- `python3 tests/m0_renderer_capabilities.py` → renderer capability static check ok；scanned 3 renderer files。
- `git diff --check` → 无错误。
下一步：独立 Review（本 Implementation 不自审）。不 push。

## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。对照 approved SPEC-004（含 `preview_remove_agentup`）与本票核对清单，实现提供 `list_projects` / `register_project` / `rebind_project` / `preview_remove_agentup` / `remove_agentup`。App 索引在应用数据目录 `app-index.sqlite`，不是项目事实源；两项目登记后隔离；missing 时 `path_state=missing`，scan/load 返回 `path_not_found` 且不写项目盘；rebind 在旧 path missing 时恢复同一 `project_id`；无 token 拒绝删除；有效一次性 token 经 `preview_remove_agentup` 签发，先 sibling 备份再删 `.agentup/`，源码与 `.git` 不动。renderer 仍无 fs/shell；新 command 进 allowlist。无足以单独否决本票的 P0 机器合同互斥。
- **审查范围**：`docs/issues/06-m2-project-product.md`（本 Review 只改 Status 并追加本段）、`docs/specs/004-m2-project-product.md`（approved，含 `preview_remove_agentup`）、commits `b43e848` 与 `74441b7`（`src-tauri/src/project_index.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/runtime.rs` 接线、`src-tauri/tests/m2_project.rs`、permissions/capabilities、schemas、`tests/m0_renderer_capabilities.py`）。对照 `src/App.tsx`。未改 src-tauri 实现、未代修、未 commit、未改 progress。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M2 Implementation（`b43e848`、`74441b7`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 14:36 CST。依据 `R-RR-002`，此路径可独立。
- **Diff / 证据快照**：`HEAD=74441b7874fa5681a4ec597050e6495eefc03759`（分支 `codex/m2-project-product`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_pass`。

### P0

无未关闭、足以单独否决本票的 P0 机器合同互斥。

### 逐项验收（对照票核对清单与 approved SPEC-004）

| 必须核对 | 判断 | 证据 |
| --- | --- | --- |
| 1. `list_projects` / `register_project` / `rebind_project` / `preview_remove_agentup` / `remove_agentup` | 成立 | 规格：`docs/specs/004-m2-project-product.md:23-27`。Tauri 注册：`src-tauri/src/lib.rs:99-154,172-176`。实现：`src-tauri/src/project_index.rs:6-37,39-91,94-167,169-182,230-319`。registry：`schemas/command-registry.json:148-256`。`mint_remove_confirmation` 不是 Tauri command，只被 preview 调用：`project_index.rs:171,185-227` vs `lib.rs` 无该 handler。 |
| 2. 两项目隔离；missing 不写项目盘 | 成立 | 索引在 app data dir：`lib.rs:162-163`、`runtime.rs:72-78`、`project_index.rs:2-3,326-336`。`list_projects` 只更新索引 `path_state`：`project_index.rs:8-27,425-438,457-476`。两项目登记不写项目盘、请求不串、索引不落项目根：`src-tauri/tests/m2_project.rs:93-148`。拔掉目录 → `missing`，scan/load `path_not_found`，原 path 与 parent/app listing 不变：`m2_project.rs:151-192`。规格：`004-m2-project-product.md:17,23,39-40`。 |
| 3. rebind 同一 `project_id` | 成立 | 仅旧 path `missing` 才改索引；新目录 canonical；manifest `project_id` 必须一致：`project_index.rs:104-167`。测试：list 为 missing → rebind 后 load/list 同一 id 且 path 为新位置：`m2_project.rs:194-232`。规格：`004-m2-project-product.md:25,41`。 |
| 4. remove 先备份再删 `.agentup`，不动源码；无 token 拒绝 | 成立 | 空 token → `confirmation_required`：`project_index.rs:244-251`。一次性 Remove token，先 `.agentup.backup.<utc>` 复制，已存在 → `already_exists`，删失败保留备份 → `io_error`，只 `remove_dir_all(.agentup)`：`project_index.rs:256-319,478-493`。测试：空 token 拒绝且目录不变；preview 签发 token 后源码/`.git` 仍在、`.agentup` 不在、backup 含 manifest：`m2_project.rs:235-265`。规格：`004-m2-project-product.md:27,32-34,42-43`。 |
| 5. capability 无 fs/shell；token 经 `preview_remove_agentup` 签发 | 成立 | allowlist 含五条 M2 command、无 fs/shell：`src-tauri/permissions/m0.toml:4-15`、`src-tauri/capabilities/m0-default.json:1-10`。renderer 未引入 fs/shell：`src/App.tsx:1-10,59`；`python3 tests/m0_renderer_capabilities.py` pass。token 由 preview 写入 `ConfirmationKind::Remove`：`project_index.rs:169-227`；测试从 preview data 取 token：`m2_project.rs:249-252`。规格：`004-m2-project-product.md:26-27,29`。 |

### 并列缺口（residual，不构成本轮 fail）

- 票核对清单仍写「四条 command」，Implementation Checkpoint 仍写测试用 `mint_remove_confirmation`。`74441b7` 已把 token 签发提升为 `preview_remove_agentup`；本 Review 以 approved SPEC-004 为权威，不把过时清单当 P0。
- `tests/m0_renderer_capabilities.py:38-48` 未把 `preview_remove_agentup` 列入必有 allow 命令。当前 `m0.toml:13` 有该命令，静态检查仍 pass；缺这条断言不等于缺 capability。
- `src-tauri/capabilities/m0-default.json:2` 仍写「nine typed commands」；实际 allowlist 为 10 条（5 M0 + 5 M2）。描述漂移，不是权限扩大到 fs/shell。
- `remove_agentup` 在 IO 前消费 token（`project_index.rs:276`）。`already_exists` / 后续失败需重新 preview。与「一次性」同向，但没有二次使用/删失败保备份的独立测试。
- demo renderer `src/App.tsx` 仍只有 M0 command；SPEC 要求的是 capability allowlist，不是本票必须改 UI。

### 验证命令与环境

- 仓库：`/Users/bic/Projects/agent-up`；日期 2026-09-15；macOS arm64；cwd 同上。
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → pass：lib/main 0 tests；`m0_runtime.rs` 7 passed；`m1_facts.rs` 4 passed；`m1_projection.rs` 3 passed；`m2_project.rs` 4 passed。`--offline` 一次通过，未联网。
- `python3 tests/m0_renderer_capabilities.py` → pass：renderer capability static check ok；scanned 3 renderer files。
- `git diff --check` → 无错误。

### 下一步

移交 Commit。本 pass 只关闭 M2 Independent Review 门禁；不 git commit / 不 push。并列缺口不阻塞本票 Commit，除非 Commit 执行体发现实现在审查之后被改动。
