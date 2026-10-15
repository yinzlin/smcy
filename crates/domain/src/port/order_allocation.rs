//! 耗材分摊聚合出站端口（consumable_allocation 表）

use serde::Serialize;

use crate::error::DomainError;

/// 分摊方案首行（summary 接口：SELECT * ... ORDER BY id ASC LIMIT 1）
#[derive(Debug, Clone, Serialize)]
pub struct AllocSummaryRow {
    pub id: i64,
    pub source_order_id: i64,
    pub total_amount: f64,
    pub allocated_amount: f64,
    pub remaining_balance: f64,
    pub status: i64,
    pub remark: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    /// 逗号分隔字符串（与列存储格式一致，由用例层解析）
    pub source_item_ids: Option<String>,
}

/// 分摊方案核心行（cancel/complete 的按 source_order_id 查询）
#[derive(Debug, Clone)]
pub struct AllocRow {
    pub id: i64,
    pub allocated_amount: f64,
    pub remaining_balance: f64,
    pub total_amount: f64,
    pub status: i64,
}

/// 已进入分摊的源订单行（allocated_orders 接口，13 字段）
#[derive(Debug, Clone, Serialize)]
pub struct AllocatedOrderRow {
    pub alloc_id: i64,
    pub source_order_id: i64,
    pub order_no: Option<String>,
    pub order_date: Option<String>,
    pub purchaser_id: Option<i64>,
    pub purchaser_name: Option<String>,
    pub total_amount: f64,
    pub allocated_amount: f64,
    pub remaining_balance: f64,
    pub status: i64,
    pub remark: Option<String>,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
}

pub trait OrderAllocationRepo: Send + Sync {
    /// 是否已存在分摊方案（行为保持：旧 handler 对查询失败 unwrap_or(false)）
    async fn exists_by_source(&self, source_order_id: i64) -> Result<bool, DomainError>;
    /// 按勾选明细 id 列表求金额合计（动态 IN 列表）
    async fn sum_item_amounts(&self, order_id: i64, item_ids: &[i64]) -> Result<f64, DomainError>;
    /// 创建分摊方案，返回 last_insert_rowid
    async fn create(
        &self,
        source_order_id: i64,
        total_amount: f64,
        remark: &str,
        source_item_ids: &str,
    ) -> Result<i64, DomainError>;
    /// 首条分摊方案（summary）
    async fn fetch_first_by_source(
        &self,
        source_order_id: i64,
    ) -> Result<Option<AllocSummaryRow>, DomainError>;
    /// 全部已分摊源订单（LEFT JOIN 销售单/采购方）
    async fn list_allocated_orders(&self) -> Result<Vec<AllocatedOrderRow>, DomainError>;
    /// 终止：UPDATE status=3，返回 rows_affected
    async fn terminate(&self, source_order_id: i64, remark: &str) -> Result<u64, DomainError>;
    /// 按 source_order_id 取核心行（cancel/complete；行为保持：错误不吞）
    async fn fetch_by_source(
        &self,
        source_order_id: i64,
    ) -> Result<Option<AllocRow>, DomainError>;
    /// 取消：按条件删除（allocated_amount <= 0.0001 AND status != 2），返回 rows_affected
    async fn delete_by_source(&self, source_order_id: i64) -> Result<u64, DomainError>;
    /// 目标订单是否存在（行为保持：查询失败 unwrap_or(false)）
    async fn target_order_exists(&self, target_order_id: i64) -> Result<bool, DomainError>;
    /// 写入尾差增项（行为保持：INSERT 失败吞错，返回 Err 由用例层 let _ =）
    async fn insert_tail_item(
        &self,
        target_order_id: i64,
        source_order_id: i64,
        source_remark: &str,
        amount: f64,
        today: &str,
    ) -> Result<(), DomainError>;
    /// 完成分摊：UPDATE status=2, completed_at, remaining_balance=0，返回 rows_affected
    async fn mark_completed(&self, alloc_id: i64) -> Result<u64, DomainError>;
}
