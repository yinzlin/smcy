//! 验收单（food_accept）聚合出站端口

use serde::Serialize;

use crate::error::DomainError;
use crate::AcceptReq;

/// 验收单列表行（JOIN 供应商/采购方名称）
#[derive(Debug, Clone, Serialize)]
pub struct FoodAcceptRow {
    pub id: i64,
    pub supplier_id: i64,
    pub purchaser_id: i64,
    pub car_no: Option<String>,
    pub supply_time: String,
    pub total_price: f64,
    pub discount_rate: f64,
    pub final_price: f64,
    pub status: String,
    pub supplier_name: String,
    pub purchaser_name: String,
}

pub trait FoodAcceptRepo: Send + Sync {
    /// 创建验收单主记录并逐条写入明细（明细失败吞错），返回主记录 last_insert_rowid
    async fn create(&self, req: &AcceptReq) -> Result<i64, DomainError>;
    /// 验收单列表（行为保持：查询失败由用例层吞错返回空数组）
    async fn list(&self) -> Result<Vec<FoodAcceptRow>, DomainError>;
}
