<!-- Input: SPEC-003、M1-01。 -->
<!-- Output: SQLite 仅投影、可从事实重建的外部行为。 -->
<!-- Pos: M1-02 任务票；SQLite 不得成为事实源。 -->

# 05 — M1-02 SQLite 投影重建

**Type:** feature

**Priority:** P1

**Complexity:** C3（数据重建、不可把投影当权威）

**What to build:** 删除 SQLite 之后仍能从 `.agentup/` 重建项目、需求、讨论、任务、范围、运行、决定和结果索引，且与删除前一致。

**Blocked by:** 04 — M1-01 事实类型合同落地。

**Status:** done

- [x] 投影缺失不阻止 load。
- [x] 损坏投影不改写事实，返回可读错误或触发只读重建。
- [x] 重建后索引与事实 revision 对齐。
- [x] 测试删除 SQLite 再重建。

## Implementation Checkpoint

**2026-09-15 / in_progress**：M1-01 done。领取 C3 Implementation。SQLite 不得成为事实源。

**2026-09-15 / review_ready**：Implementation 交付 SQLite 投影重建。`rebuild_projection(project_id)` 扫描 `.agentup` 事实与持久事件，按 revision 物化 `.agentup/cache.sqlite`；不改写事实文件，不把 SQLite 当 CAS/事实源。缺失或损坏的 sqlite 不阻止 `load_project` / `read_fact`。畸形事实返回 `malformed_fact`，不写入“修好”的投影，也不改原文件。未新增 Tauri command / renderer capability，未接 Agent。`rusqlite 0.32`（`bundled`）从本地 cargo cache 解析，`--offline` 一次通过，未联网下载。
验证（2026-09-15，macOS aarch64，cwd `/Users/bic/Projects/agent-up`）：
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → lib 0；`tests/m0_runtime.rs` 7 passed；`tests/m1_facts.rs` 4 passed；`tests/m1_projection.rs` 3 passed（八类写入后重建、删 sqlite 再重建索引一致、畸形事实不被投影修好、损坏/缺失 sqlite 不阻断 load）。
- `git diff --check` → 无错误。
下一步：独立 Review（本 Implementation 不自审）。不 push。

## Independent Review Checkpoint

- **2026-09-15 / review_pass**
- **结论**：pass。对照 approved SPEC-003 的 SQLite 投影合同与本票五条核对，实现把 `.agentup/cache.sqlite` 当作可丢弃投影；`load_project` / `read_fact` 不读 sqlite；畸形事实返回 `malformed_fact` 且不改写事实、不“修好”投影；删 sqlite 后再 rebuild 的索引与删除前一致；本 diff 未扩大 renderer fs/shell capability。无足以单独否决本票的 P0 机器合同互斥。
- **审查范围**：`docs/issues/05-m1-sqlite-projection.md`（本 Review 只改 Status 并追加本段）、`docs/specs/003-m1-fact-layer.md`（approved）、commit `efd57ae`（`src-tauri/src/projection.rs`、`src-tauri/src/runtime.rs` 两行 include/import、`src-tauri/tests/m1_projection.rs`、`src-tauri/Cargo.toml`/`Cargo.lock`）、对照未改的 `src-tauri/src/lib.rs`、`src-tauri/src/typed_facts.rs`、`src-tauri/permissions/m0.toml`、`src-tauri/capabilities/m0-default.json`、`src/App.tsx`。未改 src-tauri 实现、未代修、未 commit、未改 progress。
- **独立性**：新 Review 执行体；本会话首条指令即为独立审查。未参与 M1-02 Implementation（`efd57ae`），未共享该实现上下文。身份：Review 执行体；模型：grok-4.6；时间：2026-09-15 14:13 CST。依据 `R-RR-002`，此路径可独立。
- **Diff / 证据快照**：`HEAD=efd57ae7dcbf8da5a155e8cbd82c670b5eb5dcb4`（分支 `codex/m1-02-sqlite`）。审查前工作区仅未跟踪 `src-tauri/gen/`；本 Review 只追加本 Checkpoint 并将 Status 改为 `review_pass`。

### P0

无未关闭、足以单独否决本票的 P0 机器合同互斥。

### 逐项验收（对照票核对清单与 approved SPEC-003）

| 必须核对 | 判断 | 证据 |
| --- | --- | --- |
| 1. SQLite 在项目 `.agentup/` 内且只是投影（`cache.sqlite`） | 成立 | 规格：`docs/specs/003-m1-fact-layer.md:69-74`。路径常量与写入目标：`src-tauri/src/projection.rs:2,15-84,69-71,313-336`。`rebuild_projection` 先扫事实/事件再物化 sqlite，不读 sqlite 做 CAS：`projection.rs:61-67` vs writer 仍走文件：`typed_facts.rs:98-110,327-360`。测试断言相对路径：`src-tauri/tests/m1_projection.rs:263-264`。 |
| 2. 缺失/损坏不阻止从事实文件 `load_project` / `read_fact` | 成立 | `load_project` 只读 `manifest.json`、`events/`、`facts/requests/`：`runtime.rs:537-646,591-597,1256-1294`，函数体无 sqlite。`read_fact` 读 typed 事实文件：`typed_facts.rs:197-270`。损坏 sqlite 后 `drop` 再 load/read 成功且垃圾字节不变；删除后 load 成功且不重建 sqlite：`m1_projection.rs:323-357`。 |
| 3. rebuild 不改写事实；畸形事实返回 `malformed_fact`，投影不“修好” | 成立 | 扫描校验失败在 `replace_projection_db` 之前返回：`projection.rs:61-71,166-166,255-280,313-320`。非法 JSON / 空 body 均 `malformed_fact`；目录 listing、畸形文件、已有 sqlite 字节均不变，且失败时不新建 sqlite：`m1_projection.rs:361-403`。 |
| 4. 删除 sqlite 再重建，索引与删除前一致 | 成立 | 八类事实 + 五类持久事件入索引，discussion 保留 r1/r2：`m1_projection.rs:255-307`。删文件后 load/read 仍从事实成功，第二次 rebuild 的 `facts`/`events` 与第一次相等，事实 listing 不变：`m1_projection.rs:308-319`。 |
| 5. 无新 renderer fs/shell capability | 成立 | `efd57ae` 不改 `src/`、permissions、capabilities。Tauri handler 仍五命令：`src-tauri/src/lib.rs:101-107`。白名单：`src-tauri/permissions/m0.toml:4-10`、`src-tauri/capabilities/m0-default.json:1-12`、`src/App.tsx:5-10,59`。静态检查 `python3 tests/m0_renderer_capabilities.py` pass。规格允许内部 API、不要求新 command：`docs/specs/003-m1-fact-layer.md:15,82`。 |

### 并列缺口（residual，不构成本轮 fail）

- `write_fact` 不增量更新 sqlite（`typed_facts.rs` 无 Connection）。投影在显式 `rebuild_projection` 前可以过期。本票合同是删除后重建，不是增量投影。
- 损坏 sqlite 后再 `rebuild_projection` 没有单独测试；实现是扫事实后 tmp+rename 覆盖 dest（`projection.rs:61-71,313-327`），与 SPEC「重建仍只靠事实文件成功」同向，但不等于测过。
- `runtime.rs:7` 因 `include!("projection.rs")` 引入 `rusqlite`。这是 crate 内实现依赖，不是 renderer capability。
- `docs/progress.md` 仍把 M1-01 标成 `review_ready`；那是进度摘要滞后，不是本票实现互斥。本 Review 按指令不改 progress。

### 验证命令与环境

- 仓库：`/Users/bic/Projects/agent-up`；日期 2026-09-15；macOS arm64；cwd 同上。
- `cargo test --manifest-path src-tauri/Cargo.toml --offline --lib --tests` → pass：lib/main 0 tests；`m0_runtime.rs` 7 passed；`m1_facts.rs` 4 passed；`m1_projection.rs` 3 passed。
- `python3 tests/m0_renderer_capabilities.py` → pass：scanned 3 renderer files。
- `git diff --check 9e745af efd57ae` → 无错误。

### 下一步

移交 Commit。本 pass 只关闭 M1-02 Independent Review 门禁；不 git commit / 不 push。并列缺口不阻塞本票 Commit，除非 Commit 执行体发现实现在审查之后被改动。
