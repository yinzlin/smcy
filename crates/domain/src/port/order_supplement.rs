//! 订单增补（分摊增项）聚合出站端口（order_supplement_item 表 + 调整订单查询）

use serde::Serialize;

use crate::error::DomainError;

/// 增项行（按目标订单列表，JOIN 源订单号）
#[derive(Debug, Clone, Serialize)]
pub struct SupplementByTargetRow {
    pub id: i64,
    pub target_order_id: i64,
    pub source_order_id: i64,
    pub source_order_no: Option<String>,
    pub source_remark: Option<String>,
    pub product_id: i64,
    pub product_name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: String,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub allocate_date: String,
    pub operation_type: String,
    pub target_order_item_id: Option<i64>,
}

/// 增项行（按源订单列表，JOIN 目标订单号；旧 SELECT 不含 target_order_item_id）
#[derive(Debug, Clone, Serialize)]
pub struct SupplementBySourceRow {
    pub id: i64,
    pub target_order_id: i64,
    pub target_order_no: Option<String>,
    pub source_order_id: i64,
    pub source_remark: Option<String>,
    pub product_id: i64,
    pub product_name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: String,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub allocate_date: String,
    pub operation_type: String,
}

/// 删除前读取的增项行
#[derive(Debug, Clone)]
pub struct SupplementDeleteRow {
    pub source_order_id: i64,
    pub amount: f64,
    pub operation_type: String,
}

/// 比价：销售单真实明细行（LEFT JOIN 商品/分类）
#[derive(Debug, Clone)]
pub struct CompareItemRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub category_name: Option<String>,
    pub parent_name: Option<String>,
}

/// 比价：增项行
#[derive(Debug, Clone)]
pub struct CompareSupplementRow {
    pub id: i64,
    pub target_order_id: i64,
    pub source_order_id: i64,
    pub source_remark: Option<String>,
    pub product_id: i64,
    pub product_name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: String,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub allocate_date: String,
    pub operation_type: String,
    pub target_order_item_id: Option<i64>,
}

/// 调整订单筛选条件（由用例层解析 Query 参数后构造）
#[derive(Debug, Clone, Default)]
pub struct AdjustedOrdersFilter {
    /// keyword 非空时的 %kw% 模糊串，None 表示不筛选
    pub keyword_like: Option<String>,
    pub purchaser_id: Option<i64>,
    /// asc / desc / 其他（默认排序）
    pub sort_order: String,
    pub page: i64,
    pub page_size: i64,
}

/// 调整订单分页行
#[derive(Debug, Clone, Serialize)]
pub struct AdjustedOrderRow {
    pub id: i64,
    pub order_no: String,
    pub order_date: String,
    pub purchaser_name: Option<String>,
    pub purchaser_id: i64,
    pub total_amount: f64,
    pub adjust_amount: f64,
    pub adjust_count: i64,
    pub last_adjust_date: Option<String>,
}

/// 增项写入输入（domain 自有类型，不含 serde）
#[derive(Debug, Clone)]
pub struct OrderSupplementInput {
    pub target_order_id: i64,
    pub source_order_id: i64,
    pub source_remark: Option<String>,
    pub product_id: i64,
    pub product_name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: String,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub allocate_date: String,
    pub operation_type: String,
    pub target_order_item_id: Option<i64>,
}

pub trait OrderSupplementRepo: Send + Sync {
    /// 写入增项，返回 last_insert_rowid
    async fn create(&self, req: &OrderSupplementInput) -> Result<i64, DomainError>;
    /// 分摊余额消耗：allocated += delta, remaining -= delta, status CASE
    /// （行为保持：旧 handler 吞错，返回 Err 由用例层 let _ =）
    async fn apply_pool_delta(&self, source_order_id: i64, pool_delta: f64) -> Result<(), DomainError>;
    /// 冲减后补 completed_at（行为保持：吞错）
    async fn touch_completed_at(&self, source_order_id: i64) -> Result<(), DomainError>;
    /// 按目标订单列出增项
    async fn list_by_target(&self, order_id: i64) -> Result<Vec<SupplementByTargetRow>, DomainError>;
    /// 按源订单列出增项
    async fn list_by_source(&self, order_id: i64) -> Result<Vec<SupplementBySourceRow>, DomainError>;
    /// 删除前读取（行为保持：旧 handler 对查询失败 unwrap_or(None)）
    async fn fetch_for_delete(&self, id: i64) -> Result<Option<SupplementDeleteRow>, DomainError>;
    /// 删除增项行，返回 rows_affected
    async fn delete(&self, id: i64) -> Result<u64, DomainError>;
    /// 回滚分摊余额：allocated -= delta, remaining += delta, status CASE
    /// （行为保持：吞错）
    async fn rollback_pool(&self, source_order_id: i64, pool_delta: f64) -> Result<(), DomainError>;
    /// 回滚时清空完结时间（行为保持：吞错）
    async fn clear_completed_at(&self, source_order_id: i64) -> Result<(), DomainError>;
    /// 已分摊金额归零时删除方案回到初始态（行为保持：吞错）
    async fn delete_if_zero(&self, source_order_id: i64) -> Result<(), DomainError>;
    /// 比价：销售单真实明细（LEFT JOIN product/category/parent）
    async fn list_compare_items(&self, order_id: i64) -> Result<Vec<CompareItemRow>, DomainError>;
    /// 比价：目标订单全部增项
    async fn list_compare_supplements(
        &self,
        order_id: i64,
    ) -> Result<Vec<CompareSupplementRow>, DomainError>;
    /// 调整订单总数（动态筛选 + INNER JOIN GROUP BY，行为保持：吞错）
    async fn count_adjusted(&self, filter: &AdjustedOrdersFilter) -> Result<i64, DomainError>;
    /// 调整订单金额合计 (real, adjust)（行为保持：吞错 → (0.0, 0.0)）
    async fn sum_adjusted(&self, filter: &AdjustedOrdersFilter) -> Result<(f64, f64), DomainError>;
    /// 调整订单分页列表（行为保持：吞错 → 空数组）
    async fn list_adjusted(&self, filter: &AdjustedOrdersFilter) -> Result<Vec<AdjustedOrderRow>, DomainError>;
}
