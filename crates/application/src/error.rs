//! 应用层错误：携带 HTTP 语义与用户可见的中文文案（逐字节保持旧接口文案）

use domain::error::DomainError;

#[derive(Debug)]
pub enum UseCaseError {
    /// 业务规则不满足 → HTTP 200 + {"success":false,"message":...}
    Business(String),
    /// 未登录/无权 → HTTP 401 + {"success":false,"message":...}
    Unauthorized(String),
    /// 存储失败 → HTTP 500（技术细节不透出）
    Storage(String),
}

impl From<DomainError> for UseCaseError {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::NotFound(m) => UseCaseError::Business(m.to_string()),
            DomainError::InvalidInput(m) => UseCaseError::Business(m),
            DomainError::Conflict(m) => UseCaseError::Business(m),
            DomainError::Storage(m) => UseCaseError::Storage(m),
        }
    }
}
