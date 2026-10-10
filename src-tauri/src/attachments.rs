use crate::error::ApiResult;
use crate::types::{AttachmentItem, AttachmentKind, PendingAttachment};
use base64::Engine as _;
use std::path::{Path, PathBuf};
use tauri::Manager;

/// 附件能力 —— PRD《04》§3 的桌面化落地：落盘 attachments/<uuid>/<name>，
/// 文本类生成 excerpt 摘要注入 LLM 上下文；预览走 att:// 协议。

const TEXT_EXCERPT_HEAD: usize = 400;
const TEXT_EXCERPT_TAIL: usize = 400;
const MAX_ATTACHMENTS: usize = 20;
const MAX_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 50 * 1024 * 1024;

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "json", "js", "jsx", "ts", "tsx", "css", "scss", "html", "xml", "yml",
    "yaml", "py", "rb", "go", "rs", "java", "kt", "swift", "c", "h", "cpp", "hpp", "cs", "php",
    "sh", "sql", "toml", "ini", "csv", "log",
];

fn ext_of(name: &str) -> String {
    let lower = name.to_lowercase();
    lower.rsplit('.').next().unwrap_or("").to_string()
}

fn is_text(name: &str, mime: &str) -> bool {
    TEXT_EXTENSIONS.contains(&ext_of(name).as_str())
        || mime.starts_with("text/")
        || mime.contains("json")
        || mime.contains("javascript")
        || mime.contains("xml")
}

fn build_excerpt(data: &[u8], name: &str, mime: &str) -> Option<String> {
    if data.is_empty() || !is_text(name, mime) {
        return None;
    }
    let text = String::from_utf8_lossy(data);
    let chars: Vec<char> = text.chars().take(1201).collect();
    if chars.len() <= 1200 {
        return Some(text.to_string());
    }
    let head: String = chars[..TEXT_EXCERPT_HEAD].iter().collect();
    let tail: String = chars[chars.len() - TEXT_EXCERPT_TAIL..].iter().collect();
    Some(format!("{head}\n…(中间内容截断)…\n{tail}"))
}

/// 解析并持久化渲染层提交的附件（对应 parseAndUploadAttachments）。
pub fn persist_attachments(
    state_dir: &Path,
    pending: &[PendingAttachment],
) -> ApiResult<Vec<AttachmentItem>> {
    let root = state_dir;
    if pending.len() > MAX_ATTACHMENTS {
        return Err(crate::error::ApiError::bad_request("附件数量超过限制"));
    }
    std::fs::create_dir_all(&root)?;
    let mut items: Vec<AttachmentItem> = Vec::new();
    let mut total = 0usize;
    for p in pending {
        let safe_name: String = p.name.replace(['/', '\\'], "_");
        let key = format!("{}/{}", uuid::Uuid::new_v4(), safe_name);
        let target: PathBuf = root.join(&key);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if p.data.len() > ((MAX_ATTACHMENT_BYTES + 2) / 3) * 4 + 4 {
            return Err(crate::error::ApiError::bad_request("附件编码超过大小限制"));
        }
        let bytes = match base64::engine::general_purpose::STANDARD.decode(&p.data) {
            Ok(bytes) => bytes,
            Err(e) => {
                for item in &items {
                    let _ = std::fs::remove_file(root.join(&item.key));
                }
                return Err(crate::error::ApiError::bad_request(format!(
                    "附件数据解码失败: {e}"
                )));
            }
        };
        if bytes.len() > MAX_ATTACHMENT_BYTES || (total + bytes.len()) > MAX_TOTAL_BYTES {
            for item in &items {
                let _ = std::fs::remove_file(root.join(&item.key));
            }
            return Err(crate::error::ApiError::bad_request("附件超过大小限制"));
        }
        total += bytes.len();
        if let Err(e) = std::fs::write(&target, &bytes) {
            for item in &items {
                let _ = std::fs::remove_file(root.join(&item.key));
            }
            let _ = std::fs::remove_file(&target);
            return Err(e.into());
        }
        items.push(AttachmentItem {
            name: p.name.clone(),
            mime: if p.mime.is_empty() {
                "application/octet-stream".into()
            } else {
                p.mime.clone()
            },
            size: if p.size == 0 {
                bytes.len() as i64
            } else {
                p.size
            },
            kind: p.kind,
            key,
            source_path: Some(p.relative_path.clone().unwrap_or_else(|| p.name.clone())),
            excerpt: if p.kind == AttachmentKind::Image {
                None
            } else {
                build_excerpt(&bytes, &p.name, &p.mime)
            },
        });
    }
    Ok(items)
}

/// 安全解析 att:// key → 磁盘路径（防目录穿越）。
pub fn resolve_attachment_path(state_dir: &Path, key: &str) -> Option<PathBuf> {
    let root = state_dir;
    let candidate = root.join(key);
    let canonical_root = std::fs::canonicalize(root).ok()?;
    let canonical = std::fs::canonicalize(&candidate).ok()?;
    if canonical.starts_with(&canonical_root) && canonical.is_file() {
        Some(canonical)
    } else {
        None
    }
}

/// 删除需求附件的落盘文件：attachments JSON 里的每个 key 删 attachments/<key>（目录随文件一并清理）。
/// 文件不存在静默跳过（幂等）；仅允许删 attachments 目录内的文件（key 来自库内 JSON，防穿越）。
pub fn delete_attachment_files(state_dir: &Path, keys: &[String]) {
    let root = state_dir;
    for key in keys {
        let Some(target) = resolve_attachment_path(root, key) else {
            continue;
        };
        if target.is_file() {
            let _ = std::fs::remove_file(&target);
            if let Some(parent) = target.parent() {
                // 目录内无其他文件时移除空目录（attachments/<uuid>/<name> 的 uuid 段）
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
}

/// 收集一组需求的附件 key（删除需求/项目前先查一次）。
pub fn collect_attachment_keys(
    conn: &rusqlite::Connection,
    requirement_ids: &[String],
) -> Vec<String> {
    let mut keys = Vec::new();
    for id in requirement_ids {
        let raw: Option<String> = conn
            .query_row(
                "SELECT attachments FROM requirements WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap_or(None);
        let items: Option<Vec<AttachmentItem>> = raw.and_then(|s| serde_json::from_str(&s).ok());
        if let Some(items) = items {
            keys.extend(items.into_iter().filter_map(|a| {
                if a.key.is_empty() {
                    None
                } else {
                    Some(a.key)
                }
            }));
        }
    }
    keys
}

/// 注册 att:// 自定义协议（图片缩略图/灯箱预览用）。
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.register_uri_scheme_protocol("att", move |ctx, request| {
        let uri = request.uri();
        // key 可能带 host 段（att://<host>/<key>），统一取 path 部分
        let raw_key = if uri.path().len() > 1 {
            uri.path().to_string()
        } else {
            String::new()
        };
        let key = percent_decode(raw_key.trim_start_matches('/'));
        let base: Option<std::path::PathBuf> = ctx.app_handle().path().app_data_dir().ok();
        let body: Option<(String, Vec<u8>)> = base
            .and_then(|base| resolve_attachment_path(&base.join("attachments"), &key))
            .and_then(|path| {
                let mime = match ext_of(&key).as_str() {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    "svg" => "image/svg+xml",
                    "pdf" => "application/pdf",
                    _ => "application/octet-stream",
                };
                std::fs::read(&path)
                    .ok()
                    .map(|data| (mime.to_string(), data))
            });
        match body {
            Some((mime, data)) => http_like_response(200, &mime, data),
            None => http_like_response(404, "text/plain", b"attachment not found".to_vec()),
        }
    })
}

fn http_like_response(status: u16, mime: &str, data: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Access-Control-Allow-Origin", "*")
        .body(data)
        .expect("response")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn pending(name: &str, data: &[u8]) -> PendingAttachment {
        PendingAttachment {
            name: name.into(),
            mime: "text/plain".into(),
            size: data.len() as i64,
            kind: AttachmentKind::File,
            relative_path: None,
            data: base64::engine::general_purpose::STANDARD.encode(data),
        }
    }

    #[test]
    fn attachment_keys_are_rooted_and_traversal_is_rejected() {
        let temp = TempDir::new().unwrap();
        let items = persist_attachments(temp.path(), &[pending("a.txt", b"hello")]).unwrap();
        assert!(resolve_attachment_path(temp.path(), &items[0].key).is_some());
        assert!(resolve_attachment_path(temp.path(), "../outside").is_none());
        assert!(resolve_attachment_path(temp.path(), "%2e%2e/outside").is_none());
    }

    #[test]
    fn attachment_batch_limits_are_enforced() {
        let temp = TempDir::new().unwrap();
        let too_many: Vec<_> = (0..=MAX_ATTACHMENTS)
            .map(|_| pending("a.txt", b"x"))
            .collect();
        assert!(persist_attachments(temp.path(), &too_many).is_err());
        let too_large = pending("big.txt", &vec![b'x'; MAX_ATTACHMENT_BYTES + 1]);
        assert!(persist_attachments(temp.path(), &[too_large]).is_err());
    }
}
