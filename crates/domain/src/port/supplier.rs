//! 供应商聚合出站端口

use serde::Serialize;

use crate::error::DomainError;

/// supplier 表行结构（字段顺序 = 列表接口 JSON 字段顺序）
#[derive(Debug, Clone, Serialize)]
pub struct SupplierRow {
    pub id: i64,
    pub name: String,
    pub contact: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub business_scope: Option<String>,
    pub remark: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub audit_status: Option<String>,
    pub is_designated: i64,
    pub designated_no: Option<String>,
    pub legal_person: Option<String>,
    pub credit_code: Option<String>,
    pub license_no: Option<String>,
    pub license_expire: Option<String>,
    pub contract_no: Option<String>,
    pub contract_start: Option<String>,
    pub contract_end: Option<String>,
}

/// 列表筛选条件
#[derive(Debug, Clone, Default)]
pub struct SupplierFilter {
    pub category_id: Option<i64>,
    pub keyword_pattern: String,
}

/// 新建/更新供应商命令（更新按 id 全量覆盖，行为与旧 UPDATE 语句一致）
#[derive(Debug, Clone, Default)]
pub struct UpsertSupplier {
    pub name: String,
    pub contact: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub business_scope: Option<String>,
    pub remark: Option<String>,
    pub category_id: Option<i64>,
    pub is_designated: i64,
    pub designated_no: Option<String>,
    pub legal_person: Option<String>,
    pub credit_code: Option<String>,
    pub license_no: Option<String>,
    pub license_expire: Option<String>,
    pub contract_no: Option<String>,
    pub contract_start: Option<String>,
    pub contract_end: Option<String>,
}

/// 导入行（INSERT OR IGNORE 短字段集）
#[derive(Debug, Clone, Default)]
pub struct ImportSupplier {
    pub name: String,
    pub contact: String,
    pub phone: String,
    pub address: String,
    pub business_scope: String,
    pub remark: String,
    pub category_id: Option<i64>,
}

pub trait SupplierRepo: Send + Sync {
    /// 列表（分类递归树过滤 + 关键字 LIKE，ORDER BY id DESC）
    async fn list(&self, filter: &SupplierFilter) -> Result<Vec<SupplierRow>, DomainError>;
    /// 导出用全量列表（无过滤，ORDER BY id ASC）
    async fn list_for_export(&self) -> Result<Vec<SupplierRow>, DomainError>;
    async fn create(&self, s: &UpsertSupplier) -> Result<(), DomainError>;
    async fn update(&self, id: i64, s: &UpsertSupplier) -> Result<(), DomainError>;
    async fn delete(&self, id: i64) -> Result<(), DomainError>;
    async fn get_audit_status(&self, id: i64) -> Result<Option<String>, DomainError>;
    /// 按名称与实体类型查分类 id（不存在返回 None）
    async fn find_category_id(&self, name: &str, entity_type: &str) -> Result<Option<i64>, DomainError>;
    /// 导入单行（INSERT OR IGNORE），返回是否实际插入
    async fn import_row(&self, s: &ImportSupplier) -> Result<bool, DomainError>;
}
