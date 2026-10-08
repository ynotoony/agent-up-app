use serde::{Deserialize, Serialize};

pub type Id = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementStatus {
    Pending,
    /// 文档引入的需求：理解归类完成前隐藏于需求列表（与正常需求理解区分）。
    Initializing,
    Understanding,
    Questioning,
    AwaitingConfirmation,
    Planning,
    Implementing,
    Verifying,
    Completed,
    Failed,
    WaitingDecision,
}

impl RequirementStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RequirementStatus::Pending => "pending",
            RequirementStatus::Initializing => "initializing",
            RequirementStatus::Understanding => "understanding",
            RequirementStatus::Questioning => "questioning",
            RequirementStatus::AwaitingConfirmation => "awaiting_confirmation",
            RequirementStatus::Planning => "planning",
            RequirementStatus::Implementing => "implementing",
            RequirementStatus::Verifying => "verifying",
            RequirementStatus::Completed => "completed",
            RequirementStatus::Failed => "failed",
            RequirementStatus::WaitingDecision => "waiting_decision",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "initializing" => RequirementStatus::Initializing,
            "understanding" => RequirementStatus::Understanding,
            "questioning" => RequirementStatus::Questioning,
            "awaiting_confirmation" => RequirementStatus::AwaitingConfirmation,
            "planning" => RequirementStatus::Planning,
            "implementing" => RequirementStatus::Implementing,
            "verifying" => RequirementStatus::Verifying,
            "completed" => RequirementStatus::Completed,
            "failed" => RequirementStatus::Failed,
            "waiting_decision" => RequirementStatus::WaitingDecision,
            _ => RequirementStatus::Pending,
        }
    }
    pub fn is_terminal(&self) -> bool {
        matches!(self, RequirementStatus::Completed | RequirementStatus::Failed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementMode {
    Fast,
    Standard,
    HighRisk,
    Emergency,
}

impl RequirementMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RequirementMode::Fast => "fast",
            RequirementMode::Standard => "standard",
            RequirementMode::HighRisk => "high_risk",
            RequirementMode::Emergency => "emergency",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "fast" => RequirementMode::Fast,
            "high_risk" => RequirementMode::HighRisk,
            "emergency" => RequirementMode::Emergency,
            _ => RequirementMode::Standard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Understand,
    Question,
    Plan,
    Implement,
    Verify,
    Review,
    Iterate,
}

impl StepType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StepType::Understand => "understand",
            StepType::Question => "question",
            StepType::Plan => "plan",
            StepType::Implement => "implement",
            StepType::Verify => "verify",
            StepType::Review => "review",
            StepType::Iterate => "iterate",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "question" => StepType::Question,
            "plan" => StepType::Plan,
            "implement" => StepType::Implement,
            "verify" => StepType::Verify,
            "review" => StepType::Review,
            "iterate" => StepType::Iterate,
            _ => StepType::Understand,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    WaitingDecision,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::WaitingDecision => "waiting_decision",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Pending,
    Resolved,
    Skipped,
}

impl DecisionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DecisionStatus::Pending => "pending",
            DecisionStatus::Resolved => "resolved",
            DecisionStatus::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Code,
    Document,
    Preview,
    Config,
    Markdown,
}

impl ArtifactType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArtifactType::Code => "code",
            ArtifactType::Document => "document",
            ArtifactType::Preview => "preview",
            ArtifactType::Config => "config",
            ArtifactType::Markdown => "markdown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnderstandingSource {
    Initial,
    UserFeedback,
    QuestionAnswer,
    Iteration,
    Manual,
}

impl UnderstandingSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnderstandingSource::Initial => "initial",
            UnderstandingSource::UserFeedback => "user_feedback",
            UnderstandingSource::QuestionAnswer => "question_answer",
            UnderstandingSource::Iteration => "iteration",
            UnderstandingSource::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    File,
    Folder,
    Image,
}

impl AttachmentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AttachmentKind::File => "file",
            AttachmentKind::Folder => "folder",
            AttachmentKind::Image => "image",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "folder" => AttachmentKind::Folder,
            "image" => AttachmentKind::Image,
            _ => AttachmentKind::File,
        }
    }
}

// ---------------------------------------------------------------- entities

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub description: Option<String>,
    /// 项目工作目录（初始化绑定）；未绑定的旧项目为 None。
    pub path: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_count: Option<i64>,
}

/// 存量文档索引（content 不随列表下发，用 doc_read 按需取全文）。
#[derive(Debug, Clone, Serialize)]
pub struct ProjectDoc {
    pub id: Id,
    pub project_id: Id,
    pub rel_path: String,
    pub kind: String,
    pub title: String,
    pub excerpt: Option<String>,
    pub has_content: bool,
    pub content_chars: Option<i64>,
    pub file_mtime: Option<String>,
    pub file_missing: bool,
    /// 已转换成的需求 id（初始化自动转换或手动「转为需求」后写入，防重复转换）。
    pub requirement_id: Option<Id>,
    pub created_at: String,
    pub updated_at: String,
}

/// 治理结构本体（app 替代 agent-up 技能）：body 结构按 kind 定死。
#[derive(Debug, Clone, Serialize)]
pub struct GovernanceItem {
    pub id: Id,
    pub project_id: Id,
    pub kind: String,
    pub key: String,
    pub title: String,
    pub body: serde_json::Value,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 初始化报告的动作项。action: created | kept | skipped | warned。
#[derive(Debug, Clone, Serialize)]
pub struct InitStep {
    pub item: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct InitReport {
    pub path: String,
    pub name: String,
    pub steps: Vec<InitStep>,
    pub docs_added: i64,
    pub docs_updated: i64,
    pub docs_missing: i64,
    pub governance_seeded: i64,
    pub pending_open: i64,
    /// 本次初始化由需求类文档自动创建的需求条数。
    pub requirements_created: i64,
    /// 其中按本地字段规则（完成信号词）直接归档为已完成的条数（不占理解队列）。
    pub locally_archived: i64,
    /// 新建需求的 id 列表（命令层据此串行启动理解，不参与展示）。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub converted_requirement_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InitProjectOutcome {
    pub project: Project,
    pub report: InitReport,
    pub already_registered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentItem {
    pub name: String,
    #[serde(rename = "type")]
    pub mime: String,
    pub size: i64,
    pub kind: AttachmentKind,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnderstandingCriteria {
    pub id: String,
    pub criteria: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnderstandingRisk {
    pub id: String,
    pub risk: String,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnderstandingAmbiguity {
    pub item: String,
    pub clarification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnderstandingQuestion {
    pub id: String,
    pub question: String,
    pub reason: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnderstandingResult {
    pub goal_summary: String,
    #[serde(rename = "success_criteria")]
    pub success_criteria: Vec<UnderstandingCriteria>,
    pub risks: Vec<UnderstandingRisk>,
    pub ambiguities: Vec<UnderstandingAmbiguity>,
    #[serde(default)]
    pub questions: Vec<UnderstandingQuestion>,
    pub mode: RequirementMode,
    /// 复杂度判级 C0-C3（agent-up 协议 §2.1；任一命中即该级、取最高）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<String>,
    /// 车道建议 full|user_review|micro（准入判据见 complexity-profile §2.3）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    /// 命中的高风险面（五项受控枚举的子集）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_surfaces: Option<Vec<String>>,
    /// Profile 七维（D/B/I/U/S/M/O × required|conditional|advisory|na）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<serde_json::Value>,
    /// 理解阶段自动归类：该需求描述的工作是否已在当前仓库中完成 / 该文档仅为既存资料。
    /// true → 需求直接归档为已完成，不进实施流水线。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub already_done: Option<bool>,
    /// 归类依据（落日志与理解版本，可审计）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub already_done_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Requirement {
    pub id: Id,
    pub project_id: Id,
    /// 全量需求原文：仅详情接口（get_requirement）填充；列表查询不取该列（5s 轮询带宽考量），序列化时省略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// 列表查询返回的截断预览（前 200 字符），卡片单/双行展示足够
    pub content_preview: String,
    /// 存量文档来源路径：项目初始化/手动转换写入，替代对 content 信封前缀的字符串解析
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_doc_path: Option<String>,
    pub goal_summary: Option<String>,
    pub success_criteria: Option<Vec<UnderstandingCriteria>>,
    pub risks: Option<Vec<UnderstandingRisk>>,
    pub ambiguities: Option<Vec<UnderstandingAmbiguity>>,
    pub attachments: Option<Vec<AttachmentItem>>,
    pub status: RequirementStatus,
    pub mode: RequirementMode,
    pub current_step: Option<String>,
    pub current_version: i64,
    pub confirmed_at: Option<String>,
    pub complexity: Option<String>,
    pub complexity_reason: Option<String>,
    pub profile: Option<serde_json::Value>,
    pub risk_surfaces: Option<Vec<String>>,
    pub lane: Option<String>,
    pub estimated_minutes: Option<i64>,
    pub actual_minutes: Option<i64>,
    pub total_tokens: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementVersion {
    pub id: Id,
    pub requirement_id: Id,
    pub version: i64,
    pub goal_summary: Option<String>,
    pub success_criteria: Option<Vec<UnderstandingCriteria>>,
    pub risks: Option<Vec<UnderstandingRisk>>,
    pub ambiguities: Option<Vec<UnderstandingAmbiguity>>,
    pub questions: Option<Vec<UnderstandingQuestion>>,
    pub source: UnderstandingSource,
    pub user_input: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: Id,
    pub requirement_id: Id,
    pub step_type: StepType,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub result: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    /// 该阶段 token 用量（runtime 事件流聚合；模拟引擎为估算值）。
    pub tokens: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionOption {
    pub label: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Decision {
    pub id: Id,
    pub requirement_id: Id,
    pub task_id: Option<Id>,
    pub question: String,
    pub context: Option<String>,
    pub options: Vec<DecisionOption>,
    pub recommended: Option<String>,
    pub status: DecisionStatus,
    pub user_choice: Option<DecisionOption>,
    pub resolved_at: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Artifact {
    pub id: Id,
    pub requirement_id: Id,
    pub task_id: Option<Id>,
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub title: String,
    pub content: Option<String>,
    pub url: Option<String>,
    pub metadata: Option<serde_json::Value>,
    /// 阶段键（plan/implement/verify/review）：版本作用域与前端回看匹配键；title 仅人读展示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub version: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionLog {
    pub id: Id,
    pub requirement_id: Id,
    pub task_id: Option<Id>,
    pub step: String,
    pub level: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub created_at: String,
}

/// 执行编排：每阶段可指定运行时（注册表 id），未指定的阶段落 default_runtime。
/// review（交付审查）跟随 verify，可显式覆盖。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationSettings {
    #[serde(rename = "default_runtime")]
    pub default_runtime: Option<String>,
    #[serde(rename = "stage_runtimes")]
    pub stage_runtimes: std::collections::HashMap<String, String>,
}

impl Default for OrchestrationSettings {
    fn default() -> Self {
        OrchestrationSettings { default_runtime: None, stage_runtimes: std::collections::HashMap::new() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceStats {
    #[serde(rename = "projectCount")]
    pub project_count: i64,
    #[serde(rename = "totalRequirements")]
    pub total_requirements: i64,
    #[serde(rename = "inProgress")]
    pub in_progress: i64,
    pub completed: i64,
    pub failed: i64,
    #[serde(rename = "pendingDecisions")]
    pub pending_decisions: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceData {
    pub stats: WorkspaceStats,
    pub projects: Vec<Project>,
    #[serde(rename = "requirements")]
    pub requirements: Vec<Requirement>,
    #[serde(rename = "pendingDecisions")]
    pub pending_decisions: Vec<Decision>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDashboard {
    pub project: Project,
    pub stats: WorkspaceStats,
    pub requirements: Vec<Requirement>,
    #[serde(rename = "pendingDecisions")]
    pub pending_decisions: Vec<Decision>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementDetail {
    pub requirement: Requirement,
    pub tasks: Vec<Task>,
    pub decisions: Vec<Decision>,
    pub artifacts: Vec<Artifact>,
    pub logs: Vec<ExecutionLog>,
    pub versions: Vec<RequirementVersion>,
}

// ---------------------------------------------------------------- request payloads

#[derive(Debug, Clone, Deserialize)]
pub struct PendingAttachment {
    pub name: String,
    #[serde(rename = "type")]
    pub mime: String,
    pub size: i64,
    pub kind: AttachmentKind,
    #[serde(default)]
    pub relative_path: Option<String>,
    /// 附件字节：base64 编码（IPC JSON 传输）。
    pub data: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRequirementInput {
    #[serde(rename = "project_id")]
    pub project_id: Id,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<PendingAttachment>,
    /// 预计耗时（分钟），可选。
    #[serde(default)]
    pub estimated_minutes: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReunderstandInput {
    #[serde(default)]
    pub feedback: Option<String>,
    #[serde(default)]
    pub answering: Option<bool>,
    #[serde(default)]
    pub manual: Option<bool>,
    #[serde(default)]
    pub quote: Option<String>,
    #[serde(default)]
    pub attachments: Vec<PendingAttachment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IterateInput {
    pub feedback: String,
    #[serde(default)]
    pub attachments: Vec<PendingAttachment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDecisionInput {
    #[serde(rename = "requirement_id")]
    pub requirement_id: Id,
    pub question: String,
    #[serde(default)]
    pub context: Option<String>,
    pub options: Vec<DecisionOption>,
    #[serde(default)]
    pub recommended: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveDecisionInput {
    pub id: Id,
    pub choice: DecisionOption,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateProjectInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SaveOrchestrationInput {
    #[serde(default, deserialize_with = "double_option::deserialize")]
    pub default_runtime: Option<Option<String>>,
    #[serde(default)]
    pub stage_runtimes: Option<std::collections::HashMap<String, Option<String>>>,
}


/// 区分「字段缺失」与「显式 null」：null → Some(None)，有值 → Some(Some(v))。
pub mod double_option {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Ok(Some(Option::<T>::deserialize(deserializer)?))
    }
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id() -> Id {
    uuid::Uuid::new_v4().to_string()
}
