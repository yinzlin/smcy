//! 分类聚合出站端口

use serde::Serialize;

use crate::error::DomainError;

/// category 表行结构（字段顺序 = 列表接口 JSON 字段顺序）
#[derive(Debug, Clone, Serialize)]
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub entity_type: String,
    pub sort_order: i32,
}

/// 新建分类命令
#[derive(Debug, Clone)]
pub struct NewCategory {
    pub name: String,
    pub parent_id: Option<i64>,
    pub entity_type: String,
    pub sort_order: i32,
}

pub trait CategoryRepo: Send + Sync {
    /// 全量列表（ORDER BY entity_type, sort_order, id）
    async fn list_all(&self) -> Result<Vec<CategoryRow>, DomainError>;
    /// 树构建用列表（ORDER BY sort_order, id）
    async fn list_for_tree(&self) -> Result<Vec<CategoryRow>, DomainError>;
    async fn create(&self, c: &NewCategory) -> Result<(), DomainError>;
    async fn rename(&self, id: i64, name: &str) -> Result<(), DomainError>;
    async fn delete(&self, id: i64) -> Result<(), DomainError>;
    /// 子分类数量
    async fn count_children(&self, id: i64) -> Result<i64, DomainError>;
    /// 各实体表对该分类的引用数
    async fn count_supplier_refs(&self, id: i64) -> Result<i64, DomainError>;
    async fn count_purchaser_refs(&self, id: i64) -> Result<i64, DomainError>;
    async fn count_product_refs(&self, id: i64) -> Result<i64, DomainError>;
}
