// AgentUp Harness 共享类型 —— 与 PRD《02-数据模型》《03-后端API契约》一一对应。
export type RequirementStatus =
  | 'pending'
  | 'initializing'
  | 'understanding'
  | 'questioning'
  | 'awaiting_confirmation'
  | 'planning'
  | 'implementing'
  | 'verifying'
  | 'completed'
  | 'failed'
  | 'waiting_decision';

export type RequirementMode = 'fast' | 'standard' | 'high_risk' | 'emergency';

export type StepType = 'understand' | 'question' | 'plan' | 'implement' | 'verify' | 'review' | 'iterate';

export type TaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'waiting_decision';

export type DecisionStatus = 'pending' | 'resolved' | 'skipped';

export type ArtifactType = 'code' | 'document' | 'preview' | 'config' | 'markdown';

export type UnderstandingSource = 'initial' | 'user_feedback' | 'question_answer' | 'iteration' | 'manual';

/** App 原生接入：发现结果仅供预览，确认时由后端重新验证来源。 */
export interface GitDiscovery {
  branch: string | null;
  head: string | null;
  dirty: boolean;
}

export interface ManagementSource {
  path: string;
  kind: string;
  category: 'current' | 'history' | 'context' | 'mixed' | 'unknown';
  item_count: number;
  current_count: number;
  history_count: number;
  unknown_count: number;
  fingerprint: string;
}

export interface ProjectImport {
  schema_version: number;
  id: string;
  name: string;
  created_at: string;
  sources: ManagementSource[];
}

export interface ProjectDiscovery {
  path: string;
  name: string;
  git: GitDiscovery;
  directories: string[];
  sources: ManagementSource[];
  warnings: string[];
  scanned_at: string;
  fingerprint: string;
  native_project: ProjectImport | null;
}

export interface ImportProjectInput {
  path: string;
  fingerprint: string;
  selected_sources: string[];
}

export interface ImportProjectResult {
  profile: ProjectImport;
  /** Existing SQLite projects retain their id and history when reconnected. */
  project: Project;
}

/** App 原生目标与任务；文件是事实来源，不映射旧需求流水线。 */
export interface NativeTask {
  id: string;
  title: string;
  description: string;
  acceptance: string[];
  depends_on: string[];
  capabilities: string[];
}

export interface GoalRun {
  id: string;
  runtime_id: string;
  started_at: string;
  finished_at: string | null;
  tokens: number | null;
}

export interface NativeGoal {
  schema_version: number;
  id: string;
  project_id: string;
  content: string;
  status: 'draft' | 'planning' | 'awaiting_confirmation' | 'ready' | 'failed';
  revision: number;
  created_at: string;
  updated_at: string;
  summary: string;
  questions: string[];
  tasks: NativeTask[];
  run: GoalRun | null;
  error: string | null;
}

export interface VerificationCommand { program: string; args: string[] }
export interface DeliveryCheck extends VerificationCommand { exit_code: number | null; passed: boolean; output: string }
export interface DeliveryWorktree { root: string; path: string; branch: string; base_head: string; base_branch: string }
export interface DeliveryDiff { files: string[]; stat: string; patch: string; fingerprint: string }
export interface NativeDelivery {
  schema_version: number;
  id: string;
  goal_id: string;
  goal_revision: number;
  task_id: string;
  task_title: string;
  status: 'running' | 'verifying' | 'reviewing' | 'awaiting_acceptance' | 'accepted' | 'failed';
  started_at: string;
  updated_at: string;
  runtime_id: string;
  worktree: DeliveryWorktree | null;
  diff: DeliveryDiff | null;
  checks: DeliveryCheck[];
  review: { passed: boolean; summary: string; findings: string[] } | null;
  error: string | null;
  commit: string | null;
}

export interface Project {
  id: string;
  name: string;
  description: string | null;
  /** 项目工作目录（初始化绑定）；未绑定的旧项目为 null */
  path: string | null;
  status: string;
  created_at: string;
  updated_at: string;
  req_count?: number;
}

export type ProjectDocKind = 'requirement' | 'readme' | 'doc';

/** 存量文档索引（content 不随列表下发，用 docRead 取全文） */
export interface ProjectDoc {
  id: string;
  project_id: string;
  rel_path: string;
  kind: ProjectDocKind;
  title: string;
  excerpt: string | null;
  has_content: boolean;
  content_chars: number | null;
  file_mtime: string | null;
  file_missing: boolean;
  /** 已转换成的需求 id（初始化自动转换或手动「转为需求」后写入，防重复转换） */
  requirement_id: string | null;
  created_at: string;
  updated_at: string;
}

export type GovernanceKind = 'rule' | 'role' | 'term' | 'pending';

/** 治理结构本体（app 替代 agent-up 技能）；body 结构按 kind 定死 */
export interface GovernanceItem {
  id: string;
  project_id: string;
  kind: GovernanceKind;
  key: string;
  title: string;
  body: Record<string, unknown>;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface InitStep {
  item: string;
  /** created | kept | skipped | warned */
  action: string;
  detail: string;
}

export interface InitReport {
  path: string;
  name: string;
  steps: InitStep[];
  docs_added: number;
  docs_updated: number;
  docs_missing: number;
  governance_seeded: number;
  pending_open: number;
  /** 本次由存量文档自动创建的需求条数 */
  requirements_created: number;
  /** 其中按本地字段规则（完成信号词）直接归档为已完成的条数 */
  locally_archived: number;
  converted_requirement_ids?: string[];
}

export interface InitProjectResult {
  project: Project;
  report: InitReport;
  already_registered: boolean;
}

export interface ExportOutcome {
  written: string[];
  skipped: string[];
  target_dir: string;
}

/** 治理票（只读源：目标项目 docs/issues/index.json，票 #1） */
export interface GovernanceTicket {
  id: string;
  title: string;
  /** ready | in_progress | blocked | done | superseded | unknown */
  lane: string;
  governance_status: string;
  blocked_by: string[];
  updated_at: string | null;
  complexity: string | null;
  source: 'governance';
  body?: {
    goal: unknown;
    scope: unknown;
    acceptance: unknown;
    estimated_time: unknown;
  } | null;
}

export interface GovernanceTicketsResult {
  project_id: string;
  tickets: GovernanceTicket[];
  total: number;
  /** id/status 缺失或非法而拒收的索引行数（对账用，不静默消失） */
  skipped: number;
  unknown_statuses: Record<string, string>;
}

export interface AttachmentItem {
  name: string;
  type: string;
  size: number;
  kind: 'file' | 'folder' | 'image';
  key: string;
  sourcePath?: string;
  excerpt?: string;
}

export interface UnderstandingCriteria {
  id: string;
  criteria: string;
}

export interface UnderstandingRisk {
  id: string;
  risk: string;
  level: 'low' | 'medium' | 'high';
}

export interface UnderstandingAmbiguity {
  item: string;
  clarification: string;
}

export interface UnderstandingQuestion {
  id: string;
  question: string;
  reason: string;
  options: string[];
}

export interface UnderstandingResult {
  goal_summary: string;
  success_criteria: UnderstandingCriteria[];
  risks: UnderstandingRisk[];
  ambiguities: UnderstandingAmbiguity[];
  questions: UnderstandingQuestion[];
  mode: RequirementMode;
  /** 复杂度判级 C0-C3（agent-up 协议） */
  complexity?: string;
  /** 车道建议 full | user_review | micro */
  lane?: string;
  /** 命中的高风险面（五项受控枚举子集） */
  risk_surfaces?: string[];
  /** Profile 七维 */
  profile?: { dimension: string; status: string; reason: string }[] | null;
}

export interface RequirementVersion {
  id: string;
  requirement_id: string;
  version: number;
  goal_summary: string | null;
  success_criteria: UnderstandingCriteria[] | null;
  risks: UnderstandingRisk[] | null;
  ambiguities: UnderstandingAmbiguity[] | null;
  questions: UnderstandingQuestion[] | null;
  source: UnderstandingSource;
  user_input: string | null;
  created_at: string;
}

export interface Requirement {
  id: string;
  project_id: string;
  /** 全量原文：仅详情接口返回；列表查询不传该列（轮询带宽考量） */
  content?: string;
  /** 列表查询返回的截断预览（前 200 字符），卡片展示足够 */
  content_preview: string;
  /** 存量文档来源路径：项目初始化/手动转换时登记，界面展示用，不再解析 content 信封 */
  source_doc_path?: string | null;
  goal_summary: string | null;
  success_criteria: UnderstandingCriteria[] | null;
  risks: UnderstandingRisk[] | null;
  ambiguities: UnderstandingAmbiguity[] | null;
  attachments: AttachmentItem[] | null;
  status: RequirementStatus;
  mode: RequirementMode;
  current_step: string | null;
  current_version: number;
  confirmed_at: string | null;
  /** 复杂度判级 C0-C3 */
  complexity: string | null;
  complexity_reason: string | null;
  profile: { dimension: string; status: string; reason: string }[] | null;
  risk_surfaces: string[] | null;
  /** 车道 full | user_review | micro */
  lane: string | null;
  estimated_minutes: number | null;
  actual_minutes: number | null;
  total_tokens: number | null;
  created_at: string;
  updated_at: string;
  project_name?: string;
}

export interface TaskResultPlan {
  approach: string;
  steps: string[];
  considerations: string[];
}

export interface TaskResultDeliverable {
  summary: string;
  changes: string[];
  files: { path: string; description: string }[];
  test_plan: string;
}

export interface TaskResultVerify {
  passed: boolean;
  results: { criteria: string; passed: boolean; note: string }[];
  conclusion: string;
}

export interface Task {
  id: string;
  requirement_id: string;
  step_type: StepType;
  title: string;
  description: string | null;
  status: TaskStatus;
  result: TaskResultPlan | TaskResultDeliverable | TaskResultVerify | Record<string, unknown> | null;
  error_message: string | null;
  started_at: string | null;
  completed_at: string | null;
  /** 该阶段 token 用量（模拟引擎为估算值） */
  tokens: number | null;
  created_at: string;
}

export interface DecisionOption {
  label: string;
  value: string;
  description?: string;
  risk?: string;
}

export interface Decision {
  id: string;
  requirement_id: string;
  task_id: string | null;
  question: string;
  context: string | null;
  options: DecisionOption[];
  recommended: string | null;
  status: DecisionStatus;
  user_choice: DecisionOption | null;
  resolved_at: string | null;
  created_at: string;
}

export interface Artifact {
  id: string;
  requirement_id: string;
  task_id: string | null;
  type: ArtifactType;
  title: string;
  content: string | null;
  url: string | null;
  metadata: Record<string, unknown> | null;
  /** 阶段键（plan/implement/verify/review）：版本作用域与回看匹配键；title 仅人读展示 */
  stage?: string | null;
  version: number;
  created_at: string;
}

export interface ExecutionLog {
  id: string;
  requirement_id: string;
  task_id: string | null;
  step: string;
  level: 'info' | 'warn' | 'error';
  message: string;
  details: Record<string, unknown> | null;
  created_at: string;
}

export interface OrchestrationSettings {
  default_runtime: string | null;
  /** key 为阶段键（understand/plan/implement/verify/review），value 为注册表 runtime id */
  stage_runtimes: Partial<Record<string, string>>;
}

export interface RoleInfo {
  stage: string;
  label: string;
  path: string;
  content: string;
  modified_at: string | null;
}

export interface AiConfig {
  base_url: string;
  api_key: string;
  model: string;
}

export interface WorkspaceStats {
  projectCount: number;
  totalRequirements: number;
  inProgress: number;
  completed: number;
  failed: number;
  pendingDecisions: number;
}

export interface WorkspaceData {
  stats: WorkspaceStats;
  projects: Project[];
  requirements: Requirement[];
  pendingDecisions: Decision[];
}

export interface ProjectDashboard {
  project: Project;
  stats: WorkspaceStats;
  requirements: Requirement[];
  pendingDecisions: Decision[];
}

export interface RequirementDetail {
  requirement: Requirement;
  tasks: Task[];
  decisions: Decision[];
  artifacts: Artifact[];
  logs: ExecutionLog[];
  versions: RequirementVersion[];
}

export interface CreateRequirementResult {
  requirement: Requirement;
  understanding: UnderstandingResult;
  version: { version: number };
  has_questions: boolean;
  requiring_confirmation: boolean;
  attachment_count: number;
}

export interface ConfirmResult {
  requirement: Requirement;
  planTask: Task;
  pendingTasks: Task[];
}

export interface ReunderstandResult {
  requirement: Requirement;
  understanding: UnderstandingResult;
  version: { version: number };
  has_questions: boolean;
}

export interface IterateResult {
  requirement: Requirement;
  understanding: UnderstandingResult;
  version: { version: number };
}

// 渲染层提交附件的传输结构（字节 base64 编码后经 IPC JSON 传输）。
export interface PendingAttachment {
  name: string;
  type: string;
  size: number;
  kind: 'file' | 'folder' | 'image';
  relativePath?: string;
  data: string;
}
