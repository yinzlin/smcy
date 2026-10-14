//! 应用层错误：携带 HTTP 语义与用户可见的中文文案（逐字节保持旧接口文案）

use domain::error::DomainError;

#[derive(Debug)]
pub enum UseCaseError {
    /// 业务规则不满足 → HTTP 200 + {"success":false,"message":...}（JSON 契约接口）
    Business(String),
    /// 请求不合法/业务规则不满足 → HTTP 400（文本契约接口）
    BadRequest(String),
    /// 未登录/无权 → HTTP 401
    Unauthorized(String),
    /// 已登录但角色不足 → HTTP 403
    Forbidden(String),
    /// 目标资源不存在 → HTTP 404
    NotFound(String),
    /// 存储失败 → HTTP 500（文本契约接口携带上下文文案，JSON 契约接口固定"服务器内部错误"）
    Storage(String),
}

impl From<DomainError> for UseCaseError {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::NotFound(m) => UseCaseError::NotFound(m.to_string()),
            DomainError::InvalidInput(m) => UseCaseError::BadRequest(m),
            DomainError::Conflict(m) => UseCaseError::BadRequest(m),
            DomainError::Storage(m) => UseCaseError::Storage(m),
        }
    }
}
