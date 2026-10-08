// 浏览器端视觉验收入口（dev only）：以 /mock.html 访问，不走 Tauri。
// 通过 window.__TAURI_INTERNALS__.invoke 注入夹具数据，页面代码零改动。
// URL hash 带 ?stage= 可切场景：understand / plan / implement / waiting / failed / done
import { createRoot } from 'react-dom/client';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { Toaster } from 'sonner';
import '../globals.css';
import { ThemeProvider } from '@/components/theme-provider';
import { Sidebar } from '@/components/sidebar';
import WorkspacePage from '@/pages/workspace';
import RequirementPage from '@/pages/requirement';
import type { Decision, ExecutionLog, Project, Requirement, RequirementDetail, Task } from '../../shared/types';

const project: Project = {
  id: 'proj-1',
  name: 'agent-up',
  description: '面向结果的 Agent 交付样例项目',
  status: 'active',
  path: '/Users/bic/Projects/agent-up',
  created_at: '2026-09-28T09:00:00Z',
  updated_at: '2026-09-30T08:00:00Z',
};

const questionsV2 = [
  {
    id: 'q-1',
    question: 'docs/CONTEXT.md 缺失时，领域术语应如何处理?',
    reason: '现有权威术语无法在当前目录查证，继续登记会产生未经确认的新事实。',
    options: [
      '补建 docs/CONTEXT.md，并仅登记用户确认的术语',
      '改用仓库中指定的其他现有治理账本作为权威来源',
      '本轮仅标记需用户裁定，不创建或修改任何文件',
    ],
  },
  {
    id: 'q-2',
    question: '本轮是否进入治理记录落盘阶段?',
    reason: '落盘属于语义变化，必须先经过用户确认、验证和独立 Review 三道门禁。',
    options: ['仅输出理解 JSON，不落盘', '用户确认后写入 docs/CONTEXT.md', '用户确认后写入已有治理账本'],
  },
];

const criteriaV2 = [
  { id: 'sc-1', criteria: '明确记录 pending-domain-terms 的来源、可查证状态及需用户裁定事项。' },
  { id: 'sc-2', criteria: '根据 package.json 与 README.md 明确项目验证入口为 pnpm test，其底层执行 cargo test --manifest-path src-tauri/Cargo.toml。' },
  { id: 'sc-3', criteria: '复杂度、车道、风险面和 profile 七维判断符合治理规则。' },
];

const risksV2 = [
  { id: 'rk-1', risk: '治理账本写入属于语义变化，需三道门禁', level: 'high' as const },
  { id: 'rk-2', risk: '术语来源不一致可能导致理解结果不可追溯', level: 'medium' as const },
];

const ambiguitiesV2 = [{ item: '“治理补全”的范围', clarification: '默认仅覆盖领域术语与验证命令，不扩展到全部治理规则。' }];

const scenario = new URLSearchParams(window.location.hash.split('?')[1] ?? '').get('stage') ?? 'implement';
const t = (offsetSec: number) => new Date(Date.now() - offsetSec * 1000).toISOString();

function task(id: string, step_type: Task['step_type'], title: string, status: Task['status'], startedSecAgo: number | null, completedSecAgo: number | null): Task {
  return {
    id,
    requirement_id: 'req-1',
    step_type,
    title,
    description: null,
    status,
    result: null,
    error_message: status === 'failed' ? '验证命令 pnpm test 退出码 1：2 个用例未通过' : null,
    started_at: startedSecAgo === null ? null : t(startedSecAgo),
    completed_at: completedSecAgo === null ? null : t(completedSecAgo),
    tokens: null,
    created_at: t(startedSecAgo ?? completedSecAgo ?? 600),
  };
}

function log(id: string, step: string, message: string, secAgo: number, level: ExecutionLog['level'] = 'info', task_id: string | null = null): ExecutionLog {
  // 「执行者：」行的结构化镜像：与后端 log_stage_execution 的 details 同构（executor/runtime_kind）
  const executorMatch = message.match(/^执行者：([^，]+)/);
  const kindMatch = message.match(/^执行者：(ZCode|Codex|OpenCode)/);
  const details = executorMatch
    ? { executor: executorMatch[1], runtime_kind: kindMatch ? { ZCode: 'zcode', Codex: 'codex', OpenCode: 'opencode' }[kindMatch[1]] : null }
    : null;
  return { id, requirement_id: 'req-1', task_id, step, level, message, details, created_at: t(secAgo) };
}

const baseTasks: Task[] = [
  task('task-1', 'understand', '理解需求', 'completed', 3600, 3400),
  task('task-2', 'plan', '制定方案', 'completed', 3300, 3100),
  task('task-3', 'implement', '实施交付', 'running', 300, null),
];

const baseLogs: ExecutionLog[] = [
  log('log-0', 'understand', '执行者：ZCode，角色版本 2026-09-30T16:20:00', 3400, 'info', 'task-1'),
  log('log-1', 'understand', '初始理解完成，生成 v1', 3350, 'info', 'task-1'),
  log('log-2', 'plan', '执行者：Codex CLI，角色版本 2026-09-30T16:25:00', 3300, 'info', 'task-2'),
  log('log-3', 'plan', '方案制定完成，进入实施', 3100, 'info', 'task-2'),
  log('log-4', 'implement', '执行者：Codex CLI，角色版本 2026-09-30T16:40:00', 299, 'info', 'task-3'),
  log('log-5', 'implement', '开始实施：对照方案步骤逐项落地', 240, 'info', 'task-3'),
  log('log-6', 'implement', '正在实现第 2 步：接入理解队列单例泵…', 45, 'info', 'task-3'),
];

const understandTasks: Task[] = [task('task-u', 'understand', '理解需求', 'running', 40, null)];
const understandLogs: ExecutionLog[] = [log('ulog-0', 'understand', '执行者：Codex CLI', 39)];

const planTasks: Task[] = [
  task('task-p0', 'understand', '理解需求', 'completed', 900, 800),
  task('task-p1', 'plan', '制定方案', 'running', 60, null),
];
const planLogs: ExecutionLog[] = [
  log('plog-0', 'understand', '执行者：Codex CLI', 899, 'info'),
  log('plog-1', 'understand', '初始理解完成，生成 v1', 850, 'info'),
  log('plog-2', 'plan', '执行者：Codex CLI', 59, 'info'),
];

const waitingTasks: Task[] = [
  task('task-w1', 'understand', '理解需求', 'completed', 1200, 1100),
  task('task-w2', 'plan', '制定方案', 'completed', 1000, 900),
  task('task-w3', 'implement', '实施交付', 'waiting_decision', 800, null),
];
const waitingLogs: ExecutionLog[] = [
  log('wlog-0', 'implement', '执行者：Codex CLI', 799, 'info'),
  log('wlog-1', 'implement', '开始实施：对照方案步骤逐项落地', 750, 'info'),
  log('wlog-2', 'implement', '挂起决策点等待用户定夺：迁移策略选择逐步迁移还是一次性切换？', 700, 'warn'),
];

const failedTasks: Task[] = [
  task('task-f1', 'understand', '理解需求', 'completed', 2400, 2300),
  task('task-f2', 'plan', '制定方案', 'completed', 2200, 2100),
  task('task-f3', 'implement', '实施交付', 'completed', 2000, 1500),
  task('task-f4', 'verify', '验证交付', 'failed', 1400, 1300),
];
const failedLogs: ExecutionLog[] = [
  log('flog-0', 'verify', '执行者：Codex CLI', 1399, 'info'),
  log('flog-1', 'verify', '开始验证：对照成功标准逐条核验', 1380, 'info'),
  log('flog-2', 'verify', '正在核验第 2 条：验证命令 pnpm test', 1350, 'info'),
  log('flog-3', 'verify', '验证未通过：2 个用例未通过', 1300, 'error'),
];

const doneTasks: Task[] = [
  task('task-d1', 'understand', '理解需求', 'completed', 7200, 7000),
  task('task-d2', 'plan', '制定方案', 'completed', 6900, 6700),
  task('task-d3', 'implement', '实施交付', 'completed', 6600, 6000),
  task('task-d4', 'verify', '验证交付', 'completed', 5900, 5700),
];
const doneLogs: ExecutionLog[] = [
  log('dlog-0', 'verify', '执行者：Codex CLI', 5899, 'info'),
  log('dlog-1', 'verify', '验证通过，需求交付完成 ✓', 5700, 'info'),
];

const scenarioConfig: Record<string, { requirement: Pick<RequirementDetail['requirement'], 'status' | 'current_step' | 'confirmed_at'>; tasks: Task[]; logs: ExecutionLog[]; decisions: Decision[] }> = {
  understand: {
    requirement: { status: 'understanding', current_step: 'understand', confirmed_at: null },
    tasks: understandTasks,
    logs: understandLogs,
    decisions: [],
  },
  plan: {
    requirement: { status: 'planning', current_step: 'plan', confirmed_at: t(800) },
    tasks: planTasks,
    logs: planLogs,
    decisions: [],
  },
  implement: {
    requirement: { status: 'implementing', current_step: 'implement', confirmed_at: t(3100) },
    tasks: baseTasks,
    logs: baseLogs,
    decisions: [],
  },
  waiting: {
    requirement: { status: 'waiting_decision', current_step: 'implement', confirmed_at: t(1100) },
    tasks: waitingTasks,
    logs: waitingLogs,
    decisions: [
      {
        id: 'dec-1',
        requirement_id: 'req-1',
        task_id: 'task-w3',
        question: '迁移策略选择：逐步迁移还是一次性切换？',
        context: '存量数据约 1.2 万条，双写窗口期影响一致性。',
        options: [
          { label: '逐步迁移', value: 'gradual', description: '双写灰度，风险低但周期长', risk: '低' },
          { label: '一次性切换', value: 'bigbang', description: '停机窗口内完成，周期短', risk: '中' },
        ],
        recommended: 'gradual',
        status: 'pending',
        user_choice: null,
        resolved_at: null,
        created_at: t(700),
      },
    ],
  },
  failed: {
    requirement: { status: 'failed', current_step: 'verify', confirmed_at: t(2300) },
    tasks: failedTasks,
    logs: failedLogs,
    decisions: [],
  },
  done: {
    requirement: { status: 'completed', current_step: null, confirmed_at: t(7000) },
    tasks: doneTasks,
    logs: doneLogs,
    decisions: [],
  },
};

const chosen = scenarioConfig[scenario] ?? scenarioConfig.implement;

const detail: RequirementDetail = {
  requirement: {
    ...chosen.requirement,
    id: 'req-1',
    project_id: 'proj-1',
    content: '治理补全：逐条回答以下特定项并给出答案 JSON，形成可追溯的结构化理解结果。',
    content_preview: '治理补全：逐条回答以下特定项并给出答案 JSON…',
    goal_summary: '核查项目治理补全所需的领域术语与验证命令，形成可追溯的结构化理解结果。',
    success_criteria: criteriaV2,
    risks: risksV2,
    ambiguities: ambiguitiesV2,
    attachments: null,
    mode: 'high_risk',
    current_version: 2,
    complexity: 'C1',
    complexity_reason: '单模块治理核对，涉及少量文件查证',
    profile: null,
    risk_surfaces: ['ledger_write'],
    lane: 'full',
    estimated_minutes: 10,
    actual_minutes: 4,
    total_tokens: 1601656,
    created_at: '2026-09-30T16:20:00+08:00',
    updated_at: '2026-09-30T16:31:00+08:00',
    project_name: 'agent-up',
  },
  tasks: chosen.tasks,
  decisions: chosen.decisions,
  artifacts: [],
  logs: chosen.logs,
  versions: [
    {
      id: 'ver-1',
      requirement_id: 'req-1',
      version: 1,
      goal_summary: '补全项目治理缺口：核对领域术语来源与验证命令，输出结构化理解结果。',
      success_criteria: [{ id: 'sc-0', criteria: '完成领域术语与验证命令的查证并给出结论。' }],
      risks: null,
      ambiguities: null,
      questions: null,
      source: 'initial',
      user_input: null,
      created_at: '2026-09-30T16:24:00+08:00',
    },
    {
      id: 'ver-2',
      requirement_id: 'req-1',
      version: 2,
      goal_summary: '核查项目治理补全所需的领域术语与验证命令，形成可追溯的结构化理解结果；当前仅输出判断，不执行落盘登记。',
      success_criteria: criteriaV2,
      risks: risksV2,
      ambiguities: ambiguitiesV2,
      questions: scenario === 'understand' ? [] : questionsV2,
      source: 'question_answer',
      user_input: '1. 统一领域术语以哪份来源为准？ 答：docs/CONTEXT.md 既有词条（默认）\n2. 答案 JSON 是否需要落盘登记到治理记录？ 答：落盘到 docs/CONTEXT.md 或账本',
      created_at: '2026-09-30T16:31:06+08:00',
    },
  ],
};

type InvokeArgs = Record<string, unknown>;

// ?stage=workspace 时走工作台看板：覆盖各泳道的夹具需求。
// 标题取值三分支：有 goal_summary（已理解）→ 用总结；存量文档未理解 → 路径当标题；普通 → 用户原话。
const workspaceReqs: Requirement[] = [
  ['req-queue', 'pending', '给设置页加语言切换，中英双语，默认跟随系统', null],
  ['req-init', 'initializing', '来自存量文档「docs/specs/011-understanding.md」：<!-- Output: 理解事实... -->内脏原文', null],
  ['req-under', 'understanding', '来自存量文档「docs/specs/009-m0-core.md」：<!-- Output: 内脏原文', '固定模型评估基线，保证后续迭代可横向对比'],
  ['req-q', 'questioning', '来自存量文档「docs/issues/31-w7-kanban-columns.md」：内脏原文', '看板列收敛为四列后，项目页统计口径同步'],
  ['req-input', 'planning', '来自存量文档「docs/specs/015-ui-prd-visual.md」：Input: 内脏原文', '落实 2026-09-22 四项设计裁决到 UI PRD'],
  ['req-conf', 'awaiting_confirmation', '治理补全：逐条回答以下待定项并给出答案 JSON，铁律（agent-up 协议 v2）', '补全治理待定项并产出可追溯的答案 JSON'],
  ['req-doc', 'awaiting_confirmation', '来自存量文档「properties/AgentUp-Harness-reference/docs/CLI.md」：<!-- Input: 内脏 --> drain 命令补 --once 参数', null],
  ['req-dec', 'waiting_decision', '迁移策略选择：逐步迁移还是一次性切换，影响双写窗口', null],
  ['req-plan', 'planning', '把工作台看板改为四列等宽布局', null],
  ['req-imp', 'implementing', '测试测试测试', null],
  ['req-verify', 'verifying', '给删除项目加二次确认弹窗', null],
  ['req-done', 'completed', '修复侧边栏项目列表刷新丢焦点', '修复侧边栏项目列表刷新丢焦点'],
  ['req-fail', 'failed', '验证命令 pnpm test 退出码 1：2 个用例未通过，修复 flaky 的轮询时序', null],
].map(([id, status, content, summary], i) => ({
  id: id as string,
  project_id: 'proj-1',
  content: content as string,
  content_preview: content as string,
  // 与后端一致：存量文档转换的需求同时登记结构化来源路径（content 里的信封只是历史原文）
  source_doc_path: (content as string).match(/^来自存量文档「(.+?)(?:」[：:]?|$)/)?.[1] ?? null,
  goal_summary: (summary as string | null) ?? null,
  success_criteria: null,
  risks: null,
  ambiguities: null,
  attachments: null,
  status: status as Requirement['status'],
  mode: 'standard',
  current_step: null,
  current_version: (i % 3) + 1,
  confirmed_at: null,
  complexity: ['C0', 'C1', 'C2'][i % 3],
  complexity_reason: null,
  profile: null,
  risk_surfaces: null,
  lane: 'full',
  estimated_minutes: null,
  actual_minutes: null,
  total_tokens: null,
  created_at: new Date(Date.now() - (i + 1) * 3600_000).toISOString(),
  updated_at: new Date(Date.now() - (i + 1) * 1800_000).toISOString(),
  project_name: 'agent-up',
}));

const isWorkspace = scenario === 'workspace';

// URL ?stream=1 时合成流式事件：模拟后端 emit_stream 推流（token 级 delta + 消息级 message）。
// 真 Tauri 里 transformCallback 把回调存进内部表并以 id 标识、Rust 端经 id 回调；
// mock 里我们自己存回调，直接按事件名分发（忽略 target/id 机制）。
const enableStream = new URLSearchParams(window.location.hash.split('?')[1] ?? '').get('stream') === '1';
let callbackSeq = 0;
const streamCallbacks = new Map<number, (event: { event: string; payload: unknown }) => void>();

function emitMockStream() {
  if (!enableStream) return;
  const stage = scenario === 'understand' ? 'understand' : 'implement';
  const emit = (event: { event: string; payload: unknown }) => streamCallbacks.forEach((fn) => fn(event));
  const chunks = [
    '正在分析需求内容，先读取项目上下文与治理规则……',
    '目标已初步明确：',
    '「交付系统流式输出改造」',
    '。接下来我会：\n1. 逐条核对成功标准\n2. 标记风险面（数据库 schema、并发）\n3. 产出理解版本 v1',
  ];
  chunks.forEach((chunk, i) => {
    window.setTimeout(() => {
      if (i === 0) {
        emit({ event: 'requirement://req-1/stream', payload: { task_id: 'task-1', stage, event: { kind: 'notice', text: chunk } } });
      } else {
        emit({ event: 'requirement://req-1/stream', payload: { task_id: 'task-1', stage, event: { kind: 'delta', text: chunk } } });
      }
    }, 400 + i * 700);
  });
  // 8s 后一条消息级完整文本，验证「message 覆盖 delta」分支
  window.setTimeout(() => {
    emit({
      event: 'requirement://req-1/stream',
      payload: {
        task_id: 'task-1',
        stage,
        event: { kind: 'message', text: '（消息级完整输出示例）理解完成：判级 C1、标准车道 user_review，产出 2 个澄清问题。' },
      },
    });
  }, 8000);
}

(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
  invoke(cmd: string, _args?: InvokeArgs) {
    switch (cmd) {
      case 'requirements_get':
        window.setTimeout(emitMockStream, 300);
        return Promise.resolve(detail);
      case 'workspace_get':
        return Promise.resolve({ stats: { inFlight: 5, completed: 1, failed: 1, pendingDecisions: 1 }, projects: [project], requirements: isWorkspace ? workspaceReqs : [detail.requirement], pendingDecisions: [] });
      case 'projects_list':
        return Promise.resolve([project]);
      case 'runtimes_open_app':
        return Promise.resolve(null);
      case 'requirements_cancel':
        return Promise.resolve({ cancelled: true });
      case 'plugin:event|listen':
        // listen 传来的 handler 是 transformCallback 返回的 id；此处直接登记（事件名分发在 emit 时按 payload.event 匹配）
        return Promise.resolve(callbackSeq);
      default:
        return Promise.reject({ code: 404, message: `mock 未实现命令: ${cmd}` });
    }
  },
  transformCallback(callback: (event: { event: string; payload: unknown }) => void) {
    const id = callbackSeq++;
    streamCallbacks.set(id, callback);
    return id;
  },
  unregisterCallback(id: number) {
    streamCallbacks.delete(id);
  },
};

createRoot(document.getElementById('root')!).render(
  <ThemeProvider>
    <MemoryRouter initialEntries={[isWorkspace ? '/' : '/requirement/req-1']}>
      <div className="flex h-screen">
        <Sidebar />
        <div className="min-w-0 flex-1">
          <Routes>
            <Route path="/" element={<WorkspacePage />} />
            <Route path="/requirement/:id" element={<RequirementPage />} />
          </Routes>
        </div>
      </div>
      <Toaster
        position="top-center"
        toastOptions={{
          style: {
            background: 'var(--popover)',
            color: 'var(--foreground)',
            border: '1px solid var(--border)',
            fontSize: '13px',
          },
        }}
      />
    </MemoryRouter>
  </ThemeProvider>
);
