# AgentUp — 本地优先的 Agent 工作台（macOS，Tauri 2）

2026-10-08 由两仓合并而来：`agent-up-app`（AgentUp Harness：一句话需求 → 理解 → 质疑 → 方案 → 实施 → 验证 → 迭代，真流式 agent runtime）＋ `agent-up`（四阶段产品路线、真票上板、治理票务）。治理流程、票单、血统与产品路线等事实记录仅存维护者本地，不入本远端仓。

**应用主体** = Harness 代码线（Rust 后端 + React 19 渲染层，SQLite 本地存储，真流式 runtime：codex / opencode / zcode 三 CLI 注册表＋流式事件＋运行中取消）。**产品方向** = ROUTE-PRODUCT 四阶段阶梯：真票上板 → 跳转 agent → App 内建票 → agent 出方案人把关 → 派活收口。

## 快速开始

```bash
pnpm install                # 仅 pnpm
pnpm dev                    # 开发模式（Vite HMR + Tauri 窗口）
pnpm dist                   # 打包 .app → src-tauri/target/release/bundle/macos/
pnpm typecheck              # 渲染层 TS 类型检查
pnpm test                   # Rust 端到端冒烟（全状态机，需 cargo）
```

直接使用打包产物：打开 `src-tauri/target/release/bundle/macos/AgentUp Harness.app`。

## 发布（标准化流程）

```bash
scripts/release.sh <版本号> [--notes-file <文件>] [--dry-run] [--yes]
```

一条龙执行：预检（main 干净且与远端同步）→ 三处版本号一致性校验（`package.json` / `tauri.conf.json` / `Cargo.toml`）→ 门禁（typecheck ＋ cargo test 全绿）→ tag/Release 防重 → `pnpm dist` 构建（.app ＋ .dmg）→ DMG 挂载验证 → 确认后打 tag、推送并发布 GitHub Release（附 DMG）。

发版步骤：①改三处版本号并提交推送；②`pnpm test` 自查；③跑 `scripts/release.sh <版本号>`（建议先 `--dry-run`）；④脚本确认提示时回车 `y`。任一步失败即整体中止，不做部分写入。

- 默认**亮色**主题（底栏可切换，选择持久化）。
- 「添加项目」（侧栏 + / 工作台按钮）直接弹出**系统文件夹选择器**，选中文件夹即创建同名项目（描述保存路径）。
数据存储于 `~/Library/Application Support/com.agentup.harness/`（SQLite + 附件）。

### 接入真实 AI（可选）

`设置 → AI 服务` 填入任意 **OpenAI 兼容**服务的 Base URL / API Key / 模型（默认指向豆包 Ark endpoint，模型 `doubao-seed-2-0-pro-260215`）。

- 填好 Key 后点 **「获取可用模型」**：应用会试连 `{base_url}/models` 拉取你账号下的模型列表做下拉选择，同时验证连通性。
- 模型名必须与服务商一致（如 Ark 上是 `glm-5-3-flash-260828` 而非网页名 `glm-5.3-flash`），且需在服务商控制台**开通**该模型后才能调用（Ark 未开通会返回 `ModelNotOpen`）。
- 调用失败时自动**降级到本地模拟引擎**继续走完交付链路，并在执行日志中记录降级原因，不会卡死流程。
**不填 Key 时使用本地模拟引擎**：理解/方案/实施/验证由确定性规则生成，可离线体验完整状态链路（理解 → 质疑 → 确认 → 方案 → 决策 → 实施 → 验证 → 迭代）。

## 架构

```
shared/                   主进程与渲染层共享的类型与常量（状态标签、阶段定义）
src-tauri/                Rust 后端
  src/types.rs            数据模型（镜像 PRD《02-数据模型》9 张表 + 请求结构）
  src/db.rs               rusqlite（bundled SQLite）：全部 CRUD/聚合/JOIN project_name
  src/task_engine.rs      动态模式分析（fast/standard/high_risk/emergency）+ getModeSteps
  src/understanding.rs    LLM 需求理解（系统提示词/parse_and_normalize 清洗/本地模拟引擎）
  src/llm.rs              OpenAI 兼容客户端（reqwest + rustls）
  src/orchestrator.rs     执行编排：理解/方案/实施/验证/审查异步流水线，决策挂起与恢复，迭代重建
  src/attachments.rs      附件落盘 + att:// 自定义协议预览（替代 S3）
  src/commands.rs         Tauri commands —— 与 PRD《03-后端API契约》路由 1:1 映射
  src/error.rs            ApiError{code,message}（400/404/409 语义与 PRD 一致）
  tests/smoke.rs          端到端冒烟：创建→理解→质疑→确认→方案→决策→实施→验证→迭代
src/                      渲染层（React 19 + Vite + Tailwind 4）
  pages/                  工作台 / 项目 / 新建项目 / 需求详情（两栏 + 5s 轮询）/ 设置
  components/             理解面板、质疑作答、方案、决策、任务时间线、产物/日志、反馈迭代、附件采集
  globals.css             DESIGN.md 2.1 全套语义 token（双主题，亮 -600 / 暗 -400）
```

## 与 PRD 的映射差异（桌面化适配）

| PRD（Web/Coze 运行时） | Tauri 桌面版 |
|---|---|
| Next.js App Router + REST | Tauri commands（同构信封语义，错误码 400/404/409 一致） |
| Supabase PostgreSQL + Drizzle | 本地 SQLite（rusqlite bundled），schema 镜像 02-数据模型 |
| coze-coding-dev-sdk LLMClient | OpenAI 兼容 `/chat/completions` + 设置页配置；未配置 Key → 本地模拟引擎 |
| S3 对象存储 + 签名 URL | 本地文件 + `att://` 自定义协议（图片缩略图/灯箱） |
| DEPLOY_RUN_PORT 等环境变量 | 无需；数据在 `~/Library/Application Support/com.agentup.harness/` |

其余（状态机、模式判定、幂等 409、质疑互斥作答、选区引用、版本时间线、反馈迭代、5s 轮询、双主题语义色、圆角/Tab/横幅等设计规则）均按 PRD 原样实现。

## 验证

- `cargo test --test smoke`（AGENTUP_TEST_FAST=1 加速模拟延时）：全链路 19 项断言——项目/需求/附件 → 理解 v1 → 回答质疑 v2 → 确认 → 方案 → 决策挂起/解决 → 实施 → 验证 → completed → 迭代 v3 → 再次 completed，含聚合统计与附件持久化。
- 打包产物已实测：启动、窗口、数据库初始化（`agentup.db` WAL）正常。
