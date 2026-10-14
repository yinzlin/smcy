//! 库存聚合出站端口

use serde::Serialize;

use crate::error::DomainError;

/// inventory JOIN product/warehouse 列表行（字段顺序 = 列表接口 JSON 字段顺序）
#[derive(Debug, Clone, Serialize)]
pub struct InventoryRow {
    pub id: i64,
    pub product_id: i64,
    pub warehouse_id: i64,
    pub warehouse_name: Option<String>,
    pub name: String,
    pub spec: Option<String>,
    pub quantity: f64,
    pub min_stock: f64,
    pub max_stock: f64,
}

pub trait InventoryRepo: Send + Sync {
    /// 库存列表（inventory JOIN product，LEFT JOIN warehouse；查询失败按旧行为吞为空）
    async fn list_all(&self) -> Result<Vec<InventoryRow>, DomainError>;
}
