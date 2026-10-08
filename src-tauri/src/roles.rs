use crate::error::{ApiError, ApiResult};
use std::path::{Path, PathBuf};

/// 阶段角色 agentmd —— 每个阶段的 persona 与判断原则落为 userData/roles/*.agent.md。
/// 文件是本体：种子只在缺失时播种，升级永不覆盖；用户可清空（视同默认）。
/// 输出 JSON 契约不在角色文件里，由 orchestrator 在 prompt 末尾强制追加。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleStage {
    Understand,
    Plan,
    Implement,
    Verify,
    Review,
}

impl RoleStage {
    pub const ALL: &'static [RoleStage] = &[
        RoleStage::Understand,
        RoleStage::Plan,
        RoleStage::Implement,
        RoleStage::Verify,
        RoleStage::Review,
    ];

    pub fn key(self) -> &'static str {
        match self {
            RoleStage::Understand => "understand",
            RoleStage::Plan => "plan",
            RoleStage::Implement => "implement",
            RoleStage::Verify => "verify",
            RoleStage::Review => "review",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RoleStage::Understand => "需求理解",
            RoleStage::Plan => "方案规划",
            RoleStage::Implement => "代码实施",
            RoleStage::Verify => "验证审查",
            RoleStage::Review => "交付审查",
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        RoleStage::ALL.iter().copied().find(|s| s.key() == key)
    }
}

/// 角色种子 = 现有硬编码提示词的 persona 部分（JSON 契约拆出，由引擎追加）。
pub(crate) fn default_role_md(stage: RoleStage) -> &'static str {
    match stage {
        RoleStage::Understand => r#"你是"需求理解"角色，负责把用户的一句话需求提炼为结构化理解结果。

原则：
- goal_summary：用 1~2 句话复述用户目标（不是照抄原文，是工程视角的准确复述）。
- success_criteria：3~5 条具体、可验证的成功标准。
- risks：针对该需求的具体风险（不是泛泛而谈），1~4 条。
- ambiguities：客观存在的模糊点，每条附上你采用的默认解释，没有则空数组。
- questions：仅当存在真实歧义或高风险不确定点时产出（可为空数组），2~4 个；每个问题附 2~4 个可直接点选的互斥答案，禁止空泛占位（如"其他/自定义"）。
- mode：综合复杂度/风险/紧急度给出执行模式建议（fast|standard|high_risk|emergency）。"#,
        RoleStage::Plan => r#"你是"方案规划"角色。基于需求与理解结果产出实施方案。

原则：
- 方案要可执行：步骤按推进顺序排列，每步落到具体改动。
- 风险与考量：覆盖理解阶段给出的风险与歧义，说明应对方式。
- decision：仅在确实需要用户定夺时给出（2~3 个互斥选项并给推荐），否则为 null。
- 支持分阶段推进：范围过大时先给核心路径，再列后续阶段。"#,
        RoleStage::Implement => r#"你是"代码实施"角色。按方案执行代码变更与配置修改，输出可检查的交付物。

原则：
- 严格贴着方案步骤实施，不擅自扩大范围。
- 变更清单逐条可核对，涉及文件给出路径与用途说明。
- 给出可执行的测试计划，覆盖全部成功标准。"#,
        RoleStage::Verify => r#"你是"验证审查"角色。对照成功标准逐条核验实施结果，决定是否允许交付。

原则：
- 不信任实施方陈述，只认证据：每条结论都要指向实施结果中的具体内容。
- 逐条核对成功标准，不通过的要说明缺陷与影响。
- 保持怀疑：宁可判不通过并给出复现路径，也不放过疑似问题。"#,
        RoleStage::Review => r#"你是"交付审查"角色，针对高风险变更做最终审查。

原则：
- 审查回滚预案、敏感操作与变更影响面。
- 发现项按严重程度排列，给出缓解建议。
- 结论明确：是否放行交付。"#,
    }
}

fn roles_dir(state: &crate::db::AppState) -> PathBuf {
    state.roles_dir.clone()
}

fn role_path(dir: &Path, stage: RoleStage) -> PathBuf {
    dir.join(format!("{}.agent.md", stage.key()))
}

/// 首次启动播种：只补缺失的文件，永不覆盖已有文件。
pub fn seed_roles(state: &crate::db::AppState) -> ApiResult<()> {
    let dir = roles_dir(state);
    std::fs::create_dir_all(&dir)?;
    for stage in RoleStage::ALL {
        let path = role_path(&dir, *stage);
        if !path.exists() {
            std::fs::write(&path, default_role_md(*stage))?;
        }
    }
    Ok(())
}

pub fn list_roles(state: &crate::db::AppState) -> ApiResult<Vec<RoleInfo>> {
    let dir = roles_dir(state);
    std::fs::create_dir_all(&dir)?;
    let mut roles = Vec::new();
    for stage in RoleStage::ALL {
        let path = role_path(&dir, *stage);
        let (content, modified_at) = match std::fs::read_to_string(&path) {
            Ok(content) => {
                let mtime = std::fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| chrono::DateTime::<chrono::Utc>::from(std::time::UNIX_EPOCH + d).to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
                (content, mtime)
            }
            Err(_) => (default_role_md(*stage).to_string(), None),
        };
        roles.push(RoleInfo {
            stage: stage.key().to_string(),
            label: stage.label().to_string(),
            path: path.display().to_string(),
            content,
            modified_at,
        });
    }
    Ok(roles)
}

pub fn get_role(state: &crate::db::AppState, stage: RoleStage) -> ApiResult<RoleInfo> {
    list_roles(state)?
        .into_iter()
        .find(|r| r.stage == stage.key())
        .ok_or_else(|| ApiError::not_found("角色不存在"))
}

pub fn save_role(state: &crate::db::AppState, stage: RoleStage, content: &str) -> ApiResult<RoleInfo> {
    let dir = roles_dir(state);
    std::fs::create_dir_all(&dir)?;
    let path = role_path(&dir, stage);
    std::fs::write(&path, content)?;
    get_role(state, stage)
}

/// 恢复默认：删除文件后重新播种该角色。
pub fn reset_role(state: &crate::db::AppState, stage: RoleStage) -> ApiResult<RoleInfo> {
    let path = role_path(&roles_dir(state), stage);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    seed_roles(state)?;
    get_role(state, stage)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RoleInfo {
    pub stage: String,
    pub label: String,
    pub path: String,
    pub content: String,
    pub modified_at: Option<String>,
}
