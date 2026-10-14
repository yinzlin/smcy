//! 仓库聚合出站端口

use serde::Serialize;

use crate::error::DomainError;

/// warehouse 表行结构（字段顺序 = 列表接口 JSON 字段顺序）
#[derive(Debug, Clone, Serialize)]
pub struct WarehouseRow {
    pub id: i64,
    pub name: String,
    pub code: Option<String>,
    pub address: Option<String>,
    pub contact: Option<String>,
    pub phone: Option<String>,
    pub status: i32,
    pub sort_order: i32,
    pub audit_status: Option<String>,
    pub create_at: Option<String>,
    pub update_at: Option<String>,
}

/// 新建仓库命令
#[derive(Debug, Clone)]
pub struct NewWarehouse {
    pub name: String,
    pub code: Option<String>,
    pub address: Option<String>,
    pub contact: Option<String>,
    pub phone: Option<String>,
    pub sort_order: i32,
}

/// 更新仓库命令
#[derive(Debug, Clone)]
pub struct UpdateWarehouse {
    pub id: i64,
    pub name: String,
    pub code: Option<String>,
    pub address: Option<String>,
    pub contact: Option<String>,
    pub phone: Option<String>,
    pub status: i32,
    pub sort_order: i32,
}

pub trait WarehouseRepo: Send + Sync {
    async fn list_all(&self) -> Result<Vec<WarehouseRow>, DomainError>;
    async fn create(&self, w: &NewWarehouse) -> Result<(), DomainError>;
    async fn update(&self, w: &UpdateWarehouse) -> Result<(), DomainError>;
    async fn delete(&self, id: i64) -> Result<(), DomainError>;
    /// 该仓库下的库存记录数
    async fn count_inventory(&self, id: i64) -> Result<i64, DomainError>;
    /// 读取审核状态（不存在返回 None）
    async fn get_audit_status(&self, id: i64) -> Result<Option<String>, DomainError>;
}
