use crate::types::{RequirementMode, StepType};

/// 动态任务模式引擎 —— 移植自 PRD《04-执行引擎与AI》§1。

pub const EMERGENCY_SIGNALS: &[&str] = &[
    "urgent", "asap", "critical", "hotfix", "production down", "outage",
    "紧急", "立即", "马上", "线上故障", "生产事故", "崩溃", "宕机", "火线",
];

pub const HIGH_RISK_SIGNALS: &[&str] = &[
    "database", "migration", "production", "security", "payment", "delete all", "auth",
    "数据库", "迁移", "生产环境", "安全", "支付", "密钥", "权限", "删库",
];

pub const FAST_SIGNALS: &[&str] = &[
    "simple", "quick", "rename", "color", "text", "copy", "style tweak",
    "简单", "快速", "改名", "颜色", "文案", "样式微调", "换一下", "改一下",
];

pub fn count_signals(content: &str, signals: &[&str]) -> usize {
    let lower = content.to_lowercase();
    signals.iter().filter(|s| lower.contains(*s)).count()
}

#[derive(Debug, Clone)]
pub struct ModeAnalysis {
    pub mode: RequirementMode,
    pub confidence: f64,
    pub reason: String,
}

pub fn analyze_requirement_mode(content: &str) -> ModeAnalysis {
    if count_signals(content, EMERGENCY_SIGNALS) > 0 {
        return ModeAnalysis { mode: RequirementMode::Emergency, confidence: 0.9, reason: "检测到紧急/线上故障信号".into() };
    }
    let high_risk = count_signals(content, HIGH_RISK_SIGNALS);
    if high_risk >= 2 {
        return ModeAnalysis { mode: RequirementMode::HighRisk, confidence: 0.85, reason: "命中多个高风险关键词，涉及数据/安全/生产".into() };
    }
    let fast = count_signals(content, FAST_SIGNALS);
    if fast > 0 && high_risk == 0 {
        return ModeAnalysis { mode: RequirementMode::Fast, confidence: 0.8, reason: "任务描述指向简单改动".into() };
    }
    if high_risk == 1 {
        return ModeAnalysis { mode: RequirementMode::Standard, confidence: 0.7, reason: "存在一个高风险信号，保持标准流程".into() };
    }
    ModeAnalysis {
        mode: RequirementMode::Standard,
        confidence: 0.6,
        reason: if content.chars().count() > 200 { "需求描述较长，保持标准流程".into() } else { "默认标准流程".into() },
    }
}

pub struct ModeSteps {
    pub mode: RequirementMode,
    pub steps: Vec<StepType>,
    pub requires_user_confirmation: bool,
    pub auto_verify: bool,
}

pub fn get_mode_steps(mode: RequirementMode) -> ModeSteps {
    match mode {
        RequirementMode::Fast => ModeSteps {
            mode,
            steps: vec![StepType::Understand, StepType::Implement, StepType::Verify],
            requires_user_confirmation: false,
            auto_verify: true,
        },
        RequirementMode::HighRisk => ModeSteps {
            mode,
            steps: vec![StepType::Understand, StepType::Question, StepType::Plan, StepType::Implement, StepType::Verify, StepType::Review],
            requires_user_confirmation: true,
            auto_verify: true,
        },
        RequirementMode::Emergency => ModeSteps {
            mode,
            steps: vec![StepType::Understand, StepType::Implement, StepType::Verify],
            requires_user_confirmation: false,
            auto_verify: false,
        },
        RequirementMode::Standard => ModeSteps {
            mode,
            steps: vec![StepType::Understand, StepType::Question, StepType::Plan, StepType::Implement, StepType::Verify],
            requires_user_confirmation: true,
            auto_verify: true,
        },
    }
}

pub fn step_label(step: StepType) -> &'static str {
    match step {
        StepType::Understand => "理解需求",
        StepType::Question => "质疑与澄清",
        StepType::Plan => "制定方案",
        StepType::Implement => "实施",
        StepType::Verify => "验证",
        StepType::Review => "审查",
        StepType::Iterate => "迭代",
    }
}

pub fn mode_label(mode: RequirementMode) -> &'static str {
    match mode {
        RequirementMode::Fast => "快速模式",
        RequirementMode::Standard => "标准模式",
        RequirementMode::HighRisk => "高风险模式",
        RequirementMode::Emergency => "紧急模式",
    }
}

pub fn status_label(status: &str) -> &'static str {
    match status {
        "pending" => "排队中",
        "initializing" => "初始化中",
        "understanding" => "理解中",
        "questioning" => "质疑中",
        "awaiting_confirmation" => "待处理",
        "planning" => "方案中",
        "implementing" => "实施中",
        "verifying" => "验证中",
        "completed" => "已完成",
        "failed" => "失败",
        "waiting_decision" => "待决策",
        _ => "排队中",
    }
}
