use crate::error::{ApiError, ApiResult};
use crate::types::{
    AttachmentItem, RequirementMode, RequirementVersion, UnderstandingAmbiguity,
    UnderstandingCriteria, UnderstandingQuestion, UnderstandingResult, UnderstandingSource, UnderstandingRisk,
};
use crate::task_engine;

/// 需求理解服务 —— 移植自 PRD《04-执行引擎与AI》§2。
/// persona 在 userData/roles/understand.agent.md（种子保存在 roles.rs）；此处保留为种子出处注释。
#[allow(dead_code)]
pub const SYSTEM_PROMPT: &str = r#"你是"需求理解引擎"，负责把用户的一句话需求提炼为结构化理解结果。

要求：
1. 只输出一个 JSON 对象，不要输出任何其它文字、解释或 markdown 代码块。
2. goal_summary：用 1~2 句话复述用户目标（不是照抄原文，是工程视角的准确复述）。
3. success_criteria：3~5 条具体、可验证的成功标准，每条 { "id": "sc-1", "criteria": "..." }。
4. risks：针对该需求的具体风险（不是泛泛而谈），每条 { "id": "risk-1", "risk": "...", "level": "low|medium|high" }，1~4 条。
5. ambiguities：客观存在的模糊点，每条 { "item": "...", "clarification": "你采用的默认解释" }，没有则空数组。
6. questions：仅当存在真实歧义或高风险不确定点时产出（非必须，可为空数组），2~4 个；每个 { "id": "q-1", "question": "...", "reason": "为什么要问", "options": ["选项A","选项B","选项C"] }。options 必须是 2~4 个可直接点选的互斥答案，禁止空泛占位（如"其他/自定义"）。
7. mode：综合复杂度/风险/紧急度给出执行模式建议，取值 fast|standard|high_risk|emergency。

输出 JSON 结构：
{
  "goal_summary": string,
  "success_criteria": [{ "id": string, "criteria": string }],
  "risks": [{ "id": string, "risk": string, "level": "low"|"medium"|"high" }],
  "ambiguities": [{ "item": string, "clarification": string }],
  "questions": [{ "id": string, "question": string, "reason": string, "options": string[] }],
  "mode": "fast"|"standard"|"high_risk"|"emergency"
}"#;

#[derive(Debug, Default, Clone)]
pub struct UnderstandingContext {
    pub feedback: Option<String>,
    pub answering_question: bool,
    pub manual: bool,
    pub quote: Option<String>,
    pub attachments: Vec<AttachmentItem>,
}

pub struct UnderstandOptions {
    pub context: UnderstandingContext,
    pub prior_versions: Vec<RequirementVersion>,
    pub fallback_mode: RequirementMode,
}

impl Default for UnderstandOptions {
    fn default() -> Self {
        UnderstandOptions {
            context: UnderstandingContext::default(),
            prior_versions: Vec::new(),
            fallback_mode: RequirementMode::Standard,
        }
    }
}

// ---------------------------------------------------------------- 上下文组装

fn serialize_prior_versions(prior: &[RequirementVersion]) -> String {
    if prior.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = prior
        .iter()
        .map(|v| {
            let criteria: Vec<String> = v
                .success_criteria
                .clone()
                .unwrap_or_default()
                .iter()
                .map(|c| format!("-{}", c.criteria))
                .collect();
            format!(
                "【v{}】(来源:{}) 目标:{}\n成功标准:\n{}",
                v.version,
                source_label(v.source),
                v.goal_summary.clone().unwrap_or_else(|| "无".into()),
                criteria.join("\n")
            )
        })
        .collect();
    format!(
        "\n历史理解版本(供参考，请在新版本中吸收已澄清的信息并保持一致，除非用户反馈要求修正):\n{}",
        lines.join("\n")
    )
}

fn source_label(source: UnderstandingSource) -> &'static str {
    match source {
        UnderstandingSource::Initial => "initial",
        UnderstandingSource::UserFeedback => "user_feedback",
        UnderstandingSource::QuestionAnswer => "question_answer",
        UnderstandingSource::Iteration => "iteration",
        UnderstandingSource::Manual => "manual",
    }
}

fn serialize_context(context: &UnderstandingContext) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(feedback) = &context.feedback {
        parts.push(if context.answering_question {
            format!("用户对质疑的回答如下（请据此消除歧义并更新理解）：\n{feedback}")
        } else {
            format!("用户的补充反馈如下（请据此校准理解）：\n{feedback}")
        });
    }
    if context.manual {
        parts.push("用户手动触发了重新理解，请在原有基础上复核并优化理解结果。".into());
    }
    if let Some(quote) = &context.quote {
        parts.push(format!(
            "用户针对的理解原文片段：\n\"\"\"{quote}\"\"\"\n请据此重点校准该部分理解。"
        ));
    }
    if !context.attachments.is_empty() {
        let lines: Vec<String> = context
            .attachments
            .iter()
            .map(|a| {
                let excerpt = a
                    .excerpt
                    .as_ref()
                    .map(|e| format!(" 内容摘要: {e}"))
                    .unwrap_or_default();
                format!("[{}] {} ({}, {} bytes){}", a.kind.as_str(), a.name, a.mime, a.size, excerpt)
            })
            .collect();
        parts.push(format!("参考附件（用于补充/校准理解）：\n{}", lines.join("\n")));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("\n{}", parts.join("\n\n"))
    }
}

// ---------------------------------------------------------------- 清洗与规范化

fn extract_json_block(raw: &str) -> ApiResult<String> {
    let text = raw.replace("```json", "").replace("```", "");
    let start = text.find('{').ok_or_else(|| ApiError::internal("未能从模型输出中定位 JSON"))?;
    let end = text.rfind('}').ok_or_else(|| ApiError::internal("未能从模型输出中定位 JSON"))?;
    if end <= start {
        return Err(ApiError::internal("未能从模型输出中定位 JSON"));
    }
    Ok(text[start..=end].to_string())
}

pub fn parse_and_normalize(raw: &str, fallback_mode: RequirementMode) -> ApiResult<UnderstandingResult> {
    let block = extract_json_block(raw)?;
    let parsed: serde_json::Value =
        serde_json::from_str(&block).map_err(|e| ApiError::internal(format!("需求理解失败: {e}")))?;

    let goal_summary = parsed
        .get("goal_summary")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if goal_summary.is_empty() {
        return Err(ApiError::internal("需求理解失败: 模型未返回目标复述"));
    }

    let mode = parsed
        .get("mode")
        .and_then(|v| v.as_str())
        .map(RequirementMode::parse)
        .unwrap_or(fallback_mode);

    let arr = |key: &str| -> Vec<serde_json::Value> {
        parsed.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default()
    };

    let success_criteria: Vec<UnderstandingCriteria> = arr("success_criteria")
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let (id, text) = if item.is_string() {
                (format!("sc-{}", i + 1), item.as_str().unwrap().to_string())
            } else {
                (
                    item.get("id").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| format!("sc-{}", i + 1)),
                    item.get("criteria").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                )
            };
            if text.trim().is_empty() { None } else { Some(UnderstandingCriteria { id, criteria: text }) }
        })
        .take(5)
        .collect();

    let risks: Vec<UnderstandingRisk> = arr("risks")
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let (id, risk, level) = if item.is_string() {
                (format!("risk-{}", i + 1), item.as_str().unwrap().to_string(), "medium".to_string())
            } else {
                (
                    item.get("id").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| format!("risk-{}", i + 1)),
                    item.get("risk").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    match item.get("level").and_then(|v| v.as_str()) {
                        Some("low") => "low".to_string(),
                        Some("high") => "high".to_string(),
                        _ => "medium".to_string(),
                    },
                )
            };
            if risk.trim().is_empty() { None } else { Some(UnderstandingRisk { id, risk, level }) }
        })
        .take(4)
        .collect();

    let ambiguities: Vec<UnderstandingAmbiguity> = arr("ambiguities")
        .iter()
        .filter_map(|item| {
            let ambiguity_item = item.get("item").and_then(|v| v.as_str())?;
            let clarification = item.get("clarification").and_then(|v| v.as_str()).unwrap_or_default();
            if ambiguity_item.trim().is_empty() { None } else {
                Some(UnderstandingAmbiguity { item: ambiguity_item.to_string(), clarification: clarification.to_string() })
            }
        })
        .take(5)
        .collect();

    let questions: Vec<UnderstandingQuestion> = arr("questions")
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let question = item.get("question").and_then(|v| v.as_str())?.to_string();
            let reason = item.get("reason").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let options: Vec<String> = item
                .get("options")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|o| o.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let options: Vec<String> = options.into_iter().take(4).collect();
            if question.trim().is_empty() || options.len() < 2 { None } else {
                Some(UnderstandingQuestion {
                    id: item.get("id").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| format!("q-{}", i + 1)),
                    question,
                    reason,
                    options,
                })
            }
        })
        .take(4)
        .collect();

    // 治理判级（可选字段，非法值静默忽略回落 None）
    let complexity = parsed
        .get("complexity")
        .and_then(|v| v.as_str())
        .map(String::from)
        .filter(|c| ["C0", "C1", "C2", "C3"].contains(&c.as_str()));
    let lane = parsed
        .get("lane")
        .and_then(|v| v.as_str())
        .map(String::from)
        .filter(|l| ["full", "user_review", "micro"].contains(&l.as_str()));
    let risk_surfaces = parsed
        .get("risk_surfaces")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str())
                .filter(|s| crate::project_init::RISK_SURFACES.contains(s))
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty());
    let profile = parsed.get("profile").filter(|v| v.is_array()).cloned();

    // 自动归类（可选字段）：理解阶段判定该需求是否"已经做完/纯资料"
    let already_done = parsed.get("already_done").and_then(|v| v.as_bool());
    let already_done_reason = parsed
        .get("already_done_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(UnderstandingResult {
        goal_summary,
        success_criteria,
        risks,
        ambiguities,
        questions,
        mode,
        complexity,
        lane,
        risk_surfaces,
        profile,
        already_done,
        already_done_reason,
    })
}

// ---------------------------------------------------------------- 本地模拟引擎

const LOCAL_HIGH_RISK: &[&str] = &["数据库", "生产", "密钥", "支付", "权限", "迁移"];

fn first_sentence(content: &str) -> String {
    let trimmed: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = trimmed.chars().collect();
    let mut end = chars.len().min(40);
    for (i, c) in chars.iter().enumerate().skip(3) {
        if *c == '。' || *c == '.' || *c == '，' || *c == ',' || *c == '；' {
            if i >= 4 {
                end = i;
                break;
            }
        }
    }
    chars[..end].iter().collect()
}

pub fn mock_understanding(content: &str, context: &UnderstandingContext) -> UnderstandingResult {
    let analysis = task_engine::analyze_requirement_mode(content);
    let subject = first_sentence(content);
    let lower = content.to_lowercase();

    let success_criteria = vec![
        UnderstandingCriteria {
            id: "sc-1".into(),
            criteria: format!("交付结果满足\"{subject}\"的原始意图，可被用户直接验收"),
        },
        UnderstandingCriteria {
            id: "sc-2".into(),
            criteria: "关键功能路径完整可用，无阻断性缺陷".into(),
        },
        UnderstandingCriteria {
            id: "sc-3".into(),
            criteria: if content.chars().count() > 30 {
                "覆盖需求中列出的全部子项，逐项可核对".into()
            } else {
                "边界场景（空输入/异常输入）处理妥当".into()
            },
        },
    ];

    let mut risks: Vec<UnderstandingRisk> = Vec::new();
    if LOCAL_HIGH_RISK.iter().any(|k| lower.contains(k)) {
        risks.push(UnderstandingRisk {
            id: "risk-1".into(),
            risk: "涉及数据/生产环境，变更需要回滚预案".into(),
            level: "high".into(),
        });
    }
    if content.chars().count() > 120 {
        risks.push(UnderstandingRisk {
            id: "risk-2".into(),
            risk: "需求描述较长，子需求之间存在隐含耦合".into(),
            level: "medium".into(),
        });
    }
    risks.push(UnderstandingRisk {
        id: "risk-3".into(),
        risk: "验收口径未明确，交付范围可能扩大".into(),
        level: "low".into(),
    });

    let mut ambiguities: Vec<UnderstandingAmbiguity> = Vec::new();
    let full_text = format!("{lower}{content}");
    if full_text.contains("整合") || full_text.contains("集成") || full_text.contains("对接") {
        ambiguities.push(UnderstandingAmbiguity {
            item: "\"集成\"的具体范围".into(),
            clarification: "默认指打通核心数据流，不包含周边系统改造".into(),
        });
    }

    let mut questions: Vec<UnderstandingQuestion> = Vec::new();
    if analysis.mode == RequirementMode::Standard || analysis.mode == RequirementMode::HighRisk {
        questions.push(UnderstandingQuestion {
            id: "q-1".into(),
            question: "这次交付的优先验收形式是什么？".into(),
            reason: "不同验收形式影响实施与验证阶段的侧重点".into(),
            options: vec!["可运行的功能演示".into(), "代码/配置变更清单".into(), "部署上线的正式版本".into()],
        });
        if analysis.mode == RequirementMode::HighRisk || content.chars().count() > 60 {
            questions.push(UnderstandingQuestion {
                id: "q-2".into(),
                question: "如实施中出现取舍，优先保障哪个维度？".into(),
                reason: "高风险或长需求需要明确的取舍原则".into(),
                options: vec!["交付速度优先".into(), "稳定性优先".into(), "功能完整度优先".into()],
            });
        }
    }

    let goal_summary = if let Some(feedback) = &context.feedback {
        let suffix = if context.answering_question { "（已吸收你对质疑的回答）" } else { "（已按反馈校准）" };
        format!("结合你的补充，目标更新为：{subject}{suffix}（反馈：{feedback}）")
    } else {
        format!("目标复述：把\"{subject}\"落地为可验收的交付物")
    };

    // 模拟引擎的判级（确定性启发，非真实 LLM 判级）
    let complexity = match analysis.mode {
        RequirementMode::Fast => Some("C0".to_string()),
        RequirementMode::HighRisk => Some("C2".to_string()),
        _ if content.chars().count() > 200 => Some("C2".to_string()),
        _ => Some("C1".to_string()),
    };
    let lane = match analysis.mode {
        RequirementMode::Fast | RequirementMode::Emergency => Some("micro".to_string()),
        RequirementMode::HighRisk => Some("full".to_string()),
        RequirementMode::Standard => Some("user_review".to_string()),
    };
    let contains_any = |keywords: &[&str]| keywords.iter().any(|k| lower.contains(k));
    let mut surfaces: Vec<String> = Vec::new();
    if contains_any(&["数据库", "迁移", "migration", "database"]) { surfaces.push("数据迁移兼容".into()); }
    if contains_any(&["权限", "auth", "登录"]) { surfaces.push("认证授权".into()); }
    if contains_any(&["密钥", "安全", "隐私", "secret"]) { surfaces.push("安全隐私".into()); }
    if contains_any(&["支付", "对接", "集成", "payment"]) { surfaces.push("外部服务".into()); }
    if contains_any(&["生产", "上线", "发布", "production"]) { surfaces.push("发布运行可靠性".into()); }
    let risk_surfaces = Some(surfaces);
    let profile = Some(serde_json::json!([
        { "dimension": "B", "status": "required", "reason": "模拟判级：行为变更为本需求主面" },
        { "dimension": "D", "status": "advisory", "reason": "模拟判级：文档按需更新" },
        { "dimension": "I", "status": "na", "reason": "模拟判级：未涉及公共接口" },
        { "dimension": "U", "status": "na", "reason": "模拟判级：未涉及交互" },
        { "dimension": "S", "status": "conditional", "reason": "模拟判级：涉密钥/权限时触发" },
        { "dimension": "M", "status": "conditional", "reason": "模拟判级：涉数据迁移时触发" },
        { "dimension": "O", "status": "advisory", "reason": "模拟判级：关注运行可靠性" },
    ]));

    // 模拟引擎的自动归类（确定性启发）：内容表明工作已完成/为既存资料 → 判已完成
    let done_keywords = ["已完成", "已实现", "已上线", "已交付", "already done", "implemented"];
    let is_done = done_keywords.iter().any(|k| lower.contains(k));
    let already_done = Some(is_done);
    let already_done_reason = if is_done {
        Some("本地模拟引擎判定：内容表明该工作已完成或为既存资料记录".to_string())
    } else {
        Some("本地模拟引擎判定：内容包含待实施的工作项".to_string())
    };

    UnderstandingResult {
        goal_summary,
        success_criteria,
        risks: risks.into_iter().take(3).collect(),
        ambiguities,
        questions,
        mode: analysis.mode,
        complexity,
        lane,
        risk_surfaces,
        profile,
        already_done,
        already_done_reason,
    }
}

// ---------------------------------------------------------------- 入口

/// 组装理解阶段的引擎载荷（数据部分）；persona 在 userData/roles/understand.agent.md，
/// JSON 契约由 orchestrator 追加。
pub fn build_understanding_payload(content: &str, options: &UnderstandOptions) -> String {
    let mut user_parts = vec![format!("一句话需求：\n{content}")];
    let prior = serialize_prior_versions(&options.prior_versions);
    if !prior.is_empty() {
        user_parts.push(prior);
    }
    let context_block = serialize_context(&options.context);
    if !context_block.is_empty() {
        user_parts.push(context_block);
    }
    user_parts.push("请输出结构化理解 JSON。".to_string());
    user_parts.join("\n\n")
}
