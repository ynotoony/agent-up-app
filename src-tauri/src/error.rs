use serde::ser::SerializeStruct;
use serde::Serialize;

/// 业务错误：code 对应 PRD 的 HTTP 语义（400/404/409/500）。
#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: u16,
    pub message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError { code: 400, message: message.into() }
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        ApiError { code: 404, message: message.into() }
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        ApiError { code: 409, message: message.into() }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        ApiError { code: 500, message: message.into() }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for ApiError {}

impl Serialize for ApiError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("ApiError", 2)?;
        s.serialize_field("code", &self.code)?;
        s.serialize_field("message", &self.message)?;
        s.end()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub fn err_str(e: ApiError) -> String {
    e.message
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        ApiError::internal(format!("数据库错误: {e}"))
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::internal(format!("JSON 错误: {e}"))
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::internal(format!("IO 错误: {e}"))
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::internal(format!("网络错误: {}", readable_error(&e.to_string())))
    }
}

/// 错误信息过滤敏感项（对应 PRD lib/http.ts readableError）。
pub fn readable_error(text: &str) -> String {
    let mut out = text
        .replace("Bearer ", "Bearer [filtered]")
        .to_string();
    if let Some(idx) = out.find("sk-") {
        let end = (idx + 12).min(out.len());
        out.replace_range(idx..end, "[filtered]");
    }
    out
}
