//! 领域错误：出站端口契约的统一失败类型
//!
//! 不携带 HTTP 语义——状态码与响应体形态由 backend 适配器映射。

#[derive(Debug)]
pub enum DomainError {
    /// 目标资源不存在
    NotFound(&'static str),
    /// 输入不满足业务规则（携带面向用户的中文文案）
    InvalidInput(String),
    /// 唯一性/状态冲突（携带面向用户的中文文案）
    Conflict(String),
    /// 存储访问失败（携带技术细节，不直接透出给前端）
    Storage(String),
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainError::NotFound(m) => write!(f, "not found: {}", m),
            DomainError::InvalidInput(m) => write!(f, "invalid input: {}", m),
            DomainError::Conflict(m) => write!(f, "conflict: {}", m),
            DomainError::Storage(m) => write!(f, "storage: {}", m),
        }
    }
}
