// 治理票只读源（票 #1；语义移植自 agent-up 血统 ticket_source.rs，票 74/ROUTE-PRODUCT 阶段一 A1）。
//
// 看板此前只展示本应用自己的需求对象；本模块增加一条只读通道：把目标项目的真实治理票
// —— `docs/issues/index.json`（唯一真相源，issues 数组一行一票）＋可选票体 `docs/issues/<id>.json`
// ——映射为可上板展示的卡。只读契约：本模块不向目标项目写任何文件；所有路径锚定项目根，
// 恶意索引无法把我们指到项目目录之外。
//
// 泳道决策（沿 agent-up 票 74 Checkpoint）：索引行是真相源，票体是懒加载；对结构做防御性校验
// （id/status 存在且合法）而不是按 schema 版本漂移硬失败。缺失 id/status 的条目计入 skipped
// 计数而不是静默消失——对账时总数必须能和索引行数对上。

use crate::db;
use crate::error::{ApiError, ApiResult};
use rusqlite::Connection;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// 治理票状态 → 看板泳道。未知状态 fail visible 归入 unknown 泳道，不静默丢弃。
fn lane_for_status(status: &str) -> &'static str {
    match status {
        "ready" | "review_fail" => "ready",
        "in_progress" | "review_ready" | "review_pass" => "in_progress",
        "blocked" => "blocked",
        "done" => "done",
        "superseded" => "superseded",
        _ => "unknown",
    }
}

fn read_json_object(path: &Path) -> Result<Value, ApiError> {
    let bytes = std::fs::read(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => ApiError::not_found(format!("文件不存在: {}", path.display())),
        std::io::ErrorKind::PermissionDenied => ApiError::internal(format!("无读取权限: {}", path.display())),
        _ => ApiError::internal(format!("读取失败（{}）: {err}", path.display())),
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::bad_request(format!("不是有效 JSON: {}", path.display())))?;
    if !value.is_object() {
        return Err(ApiError::bad_request(format!("应为 JSON 对象: {}", path.display())));
    }
    Ok(value)
}

/// 读一个已绑定目录项目的治理票（只读）。
/// `load_bodies` = true 时逐票读 `<id>.json` 摘出 goal/scope/acceptance；false 只取索引行（首屏路径）。
pub fn load_governance_tickets(conn: &Connection, project_id: &str, load_bodies: bool) -> ApiResult<Value> {
    let root = db::get_project_path(conn, project_id)?
        .ok_or_else(|| ApiError::bad_request("项目未绑定目录，无法读取治理票"))?;
    let root = PathBuf::from(root);
    if !root.is_dir() {
        return Err(ApiError::not_found(format!("项目目录不存在: {}", root.display())));
    }
    // 优先新路径 facts/requirements/tickets/index.json（2026-10-08 治理骨架迁移）
    // 兜底旧路径 docs/issues/index.json（agent-up 前仓语境）
    let new_issues_dir = root.join("facts").join("requirements").join("tickets");
    let new_index_path = new_issues_dir.join("index.json");
    let old_issues_dir = root.join("docs").join("issues");
    let old_index_path = old_issues_dir.join("index.json");

    let (issues_dir, index_path) = if new_index_path.is_file() {
        (new_issues_dir, new_index_path)
    } else if old_index_path.is_file() {
        (old_issues_dir, old_index_path)
    } else {
        // fail-visible（沿 agent-up 票 74 AC4）：无治理票索引是用户必须看到的错误，不是空看板。
        return Err(ApiError::not_found(
            "未找到治理票索引（facts/requirements/tickets/index.json 或 docs/issues/index.json）——该项目可能尚未治理",
        ));
    };
    let index = read_json_object(&index_path)?;
    let entries = index
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request("治理票索引缺少 issues 数组"))?;

    let mut cards: Vec<Value> = Vec::new();
    let mut unknown_statuses: Map<String, Value> = Map::new();
    let mut skipped = 0usize;
    for entry in entries {
        let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        let status = entry.get("status").and_then(Value::as_str).unwrap_or("");
        // 空 id 会寻址 docs/issues/.json；超长/带路径符号的 id 可能逃出 docs/issues/——一律拒收计数。
        let id_safe = !id.is_empty()
            && id.len() <= 64
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            && !id.starts_with('-');
        if status.is_empty() || !id_safe {
            skipped += 1;
            continue;
        }
        let lane = lane_for_status(status);
        if lane == "unknown" {
            unknown_statuses.insert(id.to_string(), json!(status));
        }
        let mut card = json!({
            "id": id,
            "title": entry.get("title").and_then(Value::as_str).unwrap_or(id),
            "lane": lane,
            "governance_status": status,
            "blocked_by": entry.get("blocked_by").cloned().unwrap_or_else(|| json!([])),
            "updated_at": entry.get("updated_at").cloned().unwrap_or(Value::Null),
            "complexity": entry.get("complexity").cloned().unwrap_or(Value::Null),
            "source": "governance",
        });
        if load_bodies && id_safe {
            if let Ok(body) = read_json_object(&issues_dir.join(format!("{id}.json"))) {
                card["body"] = json!({
                    "goal": body.get("goal").cloned().unwrap_or(Value::Null),
                    "scope": body.get("scope").cloned().unwrap_or(Value::Null),
                    "acceptance": body.get("acceptance").cloned().unwrap_or(Value::Null),
                    "estimated_time": body.get("estimated_time").cloned().unwrap_or(Value::Null),
                });
            }
        }
        cards.push(card);
    }
    cards.sort_by(|a, b| a["id"].as_str().unwrap_or("").cmp(b["id"].as_str().unwrap_or("")));
    Ok(json!({
        "project_id": project_id,
        "tickets": cards,
        "total": cards.len(),
        "skipped": skipped,
        "unknown_statuses": unknown_statuses,
    }))
}
