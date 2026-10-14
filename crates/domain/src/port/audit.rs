//! 基础数据审核状态出站端口
//!
//! 覆盖 supplier / purchaser / product / warehouse 四张基础表的
//! 审核状态读写（旧 set_basic_audit_status / check_basic_not_confirmed /
//! is_audit_confirmed 共享逻辑的端口化）。

use crate::error::DomainError;

/// 基础数据表（白名单枚举，避免动态表名拼接注入）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicTable {
    Supplier,
    Purchaser,
    Product,
    Warehouse,
}

impl BasicTable {
    pub fn as_str(&self) -> &'static str {
        match self {
            BasicTable::Supplier => "supplier",
            BasicTable::Purchaser => "purchaser",
            BasicTable::Product => "product",
            BasicTable::Warehouse => "warehouse",
        }
    }
}

pub trait BasicAuditRepo: Send + Sync {
    /// 更新审核状态，返回是否命中记录（rows_affected > 0）
    async fn set_audit_status(&self, table: BasicTable, id: i64, status: &str) -> Result<bool, DomainError>;
    /// 读取审核状态（不存在返回 None）
    async fn get_audit_status(&self, table: BasicTable, id: i64) -> Result<Option<String>, DomainError>;
}
