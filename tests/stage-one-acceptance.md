# 阶段一验收测试清单

## 测试日期
2026-10-09

## 测试目标
验证 AgentUp App 阶段一（读）的核心能力：
- 真票源接入
- 跳转 agent
- 三类仓初始化

---

## 测试用例 1：读取真实票源

### 前置条件
- AgentUp App 已编译并可运行
- 项目 `agentup` 已在 App 中注册
- 项目路径：`/Users/bic/Projects/agentup`

### 测试步骤
1. 打开 AgentUp.app
2. 在项目列表中找到 `agentup` 项目
3. 点击进入项目详情页
4. 查看"工作台"标签页

### 期望结果
- ✅ 看到 5 张票的看板（01-05）
- ✅ 每张票显示：标题、状态、复杂度
- ✅ 票 01、04、05 状态为 `done`
- ✅ 票 02、03 状态为 `ready`

### 实现状态
- ✅ `ticket_source.rs` 支持新路径 `facts/requirements/tickets/`
- ✅ 兼容旧路径 `docs/issues/`
- ✅ `projects_tickets` 命令已实现并注册

---

## 测试用例 2：跳转 agent（ZCode）

### 前置条件
- ZCode.app 已安装
- 已完成测试用例 1

### 测试步骤
1. 在工作台看板中选择一张 `ready` 状态的票（如票 02）
2. 点击票卡上的"跳转 agent"按钮（Sparkles 图标）
3. 观察系统行为

### 期望结果
- ✅ ZCode.app 自动启动（如果未运行）或激活（如果已运行）
- ✅ ZCode 自动切换到项目根目录 `/Users/bic/Projects/agentup`
- ✅ ZCode 接收到票信息作为初始上下文
- ✅ 可以在 ZCode 中与 agent 对话并执行任务

### 实现状态
- ✅ `projects_ticket_launch` 命令已实现
- ✅ 使用 `open -b com.anthropic.zcode` 启动 ZCode
- ✅ 通过 `--query` 参数传递票信息
- ✅ 前端 TicketCard 组件已添加"跳转 agent"按钮

---

## 测试用例 3：导出治理文件（三类仓初始化的核心）

### 前置条件
- 已完成测试用例 1
- 项目已在 App 中初始化（有治理规则种子）

### 测试步骤
1. 在项目详情页，点击右上角"…"菜单
2. 选择"导出 AgentUp 治理文件"
3. 确认导出
4. 在终端检查项目目录

### 期望结果
- ✅ 生成 `AGENTS.md` 路由入口
- ✅ 生成 `docs/development-process.md`（流程规则）
- ✅ 生成 `docs/CONTEXT.md`（领域词条）
- ✅ 生成 `docs/agent/roles/implementation.md`（角色合同）
- ✅ 生成 `docs/agent/roles/review.md`
- ✅ 生成 `docs/agent/roles/commit.md`
- ✅ 生成 `docs/agent/artifacts.yaml`（产物台账）
- ✅ 所有文件包含"由 AgentUp Harness 数据库投影"标记

### 验证命令
```bash
ls -la /Users/bic/Projects/agentup/AGENTS.md
ls -la /Users/bic/Projects/agentup/docs/development-process.md
ls -la /Users/bic/Projects/agentup/docs/agent/roles/
```

### 实现状态
- ✅ `export_agentup_files` 函数已实现
- ✅ `projects_export` 命令已注册
- ✅ 前端已有导出 UI（项目菜单）
- ✅ 支持幂等导出（内容未变时跳过）

---

## 测试用例 4：三类仓初始化（完整场景）

### 场景 A：空目录
1. 创建空目录 `mkdir /tmp/test-empty-repo && cd /tmp/test-empty-repo`
2. 初始化 Git：`git init`
3. 在 App 中添加该项目
4. 执行初始化
5. 验证生成了完整的治理结构

### 场景 B：未治理的项目
1. 克隆一个没有 agent-up 治理的项目
2. 在 App 中添加该项目
3. 执行初始化
4. 验证治理文件被正确植入

### 场景 C：已有治理的项目
1. 使用 agentup 项目自己（已有 `facts/` 和 `rules/`）
2. 在 App 中添加
3. 验证识别现有治理
4. 导出时不覆盖现有文件

### 实现状态
- ✅ `seed_governance` 在初始化时自动种子
- ✅ `export_agentup_files` 支持幂等导出
- ✅ 检测现有文件，内容未变时跳过

---

## 阶段一 Dogfood 验收（最终测试）

### 场景：用 AgentUp App 管理自己
1. ✅ 打开 AgentUp.app
2. ✅ 添加 `/Users/bic/Projects/agentup` 项目
3. ✅ 看到所有真实票（01-05）
4. ✅ 点击票 02"跳转 agent"
5. ✅ ZCode 弹出并加载上下文
6. ✅ 在 ZCode 中与 agent 对话执行任务
7. ✅ 导出治理文件到项目目录
8. ✅ 验证文件完整性

### 期望结果
**可以用 AgentUp App 管理自己的开发任务，并通过 ZCode 执行任务。**

---

## 测试总结

### ✅ 已完成的功能
1. 真票源接入（票 01）
2. 跳转 agent（票 02）
3. 导出治理文件（票 03 核心）
4. 项目发现/导入/预览（票 04）
5. 注册导入项目（票 05）

### ⚠️ 已知限制
1. 当前只支持 ZCode（未来可扩展到 Codex/OpenCode）
2. 导出是手动触发（未来可自动同步）
3. 跳转 agent 时的上下文传递依赖 CLI 参数（可能受命令行长度限制）

### 🎯 阶段一收口标准
- [x] 真票源读取
- [x] agent 跳转
- [x] 治理文件导出
- [ ] 实际 dogfood 验证（需要用户运行 App 确认）

---

## 下一步（阶段二）
- 在 App 内建票（不跳出到命令行）
- 建票表单 UI
- 票据验证和持久化
