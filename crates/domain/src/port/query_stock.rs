//! 库存查询域出站端口（库存余额/收发存、库存流水、出入库统计、库存预警、呆滞库存）

use crate::error::DomainError;

/// 库存余额-历史时点行（期初/期间入出/期末；base_price 供期末金额=期末数量×基准价计算）
#[derive(Debug, Clone)]
pub struct StockBalanceHistoryRow {
    pub product_id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub opening_qty: f64,
    pub period_in: f64,
    pub period_out: f64,
    pub closing_qty: f64,
    pub base_price: f64,
}

/// 库存余额-实时行（amount = 库存数量 × 基准价）
#[derive(Debug, Clone)]
pub struct StockBalanceRealtimeRow {
    pub product_id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub quantity: f64,
    pub amount: f64,
    pub min_stock: f64,
    pub max_stock: f64,
}

/// 库存流水聚合行（balance 列由用例层按时间线累计，SQL 中固定 0.0 占位）
#[derive(Debug, Clone)]
pub struct StockFlowRow {
    pub create_time: String,
    pub movement_type: String,
    pub summary: String,
    pub direction: String,
    pub product_id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub in_quantity: f64,
    pub out_quantity: f64,
    pub ref_no: Option<String>,
    pub party_name: Option<String>,
    pub production_date: Option<String>,
    pub batch_no: Option<String>,
    pub orig_quantity: f64,
    pub orig_unit: Option<String>,
}

/// 流水期初净额行（基准日期前各商品按订单聚合的净额）
#[derive(Debug, Clone)]
pub struct StockFlowOpeningRow {
    pub product_id: i64,
    pub net: f64,
}

/// 库存预警行（limit_stock：低于最低库存时取 min_stock，高于最高库存时取 max_stock）
#[derive(Debug, Clone)]
pub struct StockWarningRow {
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub current_stock: f64,
    pub limit_stock: f64,
}

/// 呆滞库存行（JSON 版：last_sale_date 缺省文案"无销售记录"）
#[derive(Debug, Clone)]
pub struct SlowStockRow {
    pub id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub current_stock: f64,
    pub last_sale_date: String,
}

/// 呆滞库存行（导出版：含库存金额，last_sale_date 缺省文案"无"，排序追加 p.id ASC）
#[derive(Debug, Clone)]
pub struct SlowStockExportRow {
    pub id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub current_stock: f64,
    pub amount: f64,
    pub last_sale_date: String,
}

/// 出入库统计：采购入库按日+仓库聚合行
#[derive(Debug, Clone)]
pub struct StockSummaryPurchaseRow {
    pub day: String,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub amount: f64,
    pub item_count: i64,
    pub order_count: i64,
}

/// 出入库统计：销售出库明细行（含订单下浮率，金额下浮在用例层按行计算）
#[derive(Debug, Clone)]
pub struct StockSummarySalesRow {
    pub day: String,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub amount: f64,
    pub order_id: i64,
    pub discount_rate: f64,
}

pub trait QueryStockRepo: Send + Sync {
    /// 库存余额总数（历史时点口径；category_id 非空时追加分类条件与绑定，行为保持：用例层 unwrap_or(0)）
    async fn count_stock_balance_history(&self, start_bound: &str, end_bound: &str, pattern: &str, category_id: &str) -> Result<i64, DomainError>;
    /// 库存余额总数（实时口径）
    async fn count_stock_balance_realtime(&self, pattern: &str, category_id: &str) -> Result<i64, DomainError>;
    /// 库存余额分页数据（历史时点口径）
    async fn list_stock_balance_history(&self, start_bound: &str, end_bound: &str, pattern: &str, category_id: &str, page_size: i64, offset: i64) -> Result<Vec<StockBalanceHistoryRow>, DomainError>;
    /// 库存余额分页数据（实时口径）
    async fn list_stock_balance_realtime(&self, pattern: &str, category_id: &str, page_size: i64, offset: i64) -> Result<Vec<StockBalanceRealtimeRow>, DomainError>;
    /// 库存余额导出（历史时点口径：无分页，ORDER BY product_name）
    async fn list_stock_balance_history_export(&self, start_bound: &str, end_bound: &str, pattern: &str, category_id: &str) -> Result<Vec<StockBalanceHistoryRow>, DomainError>;
    /// 库存余额导出（实时口径：category_id 内联进 SQL 且仍绑定 cid，与旧 handler 逐字一致）
    async fn list_stock_balance_realtime_export(&self, pattern: &str, category_id: &str) -> Result<Vec<StockBalanceRealtimeRow>, DomainError>;
    /// 库存流水总数（where_clause 由用例层拼装；bind_pattern 决定是否绑定名称模糊匹配）
    async fn count_stock_flow(&self, where_clause: &str, bind_pattern: bool, pattern: &str) -> Result<i64, DomainError>;
    /// 库存流水分页数据（JSON 版：外层多行 SELECT + LIMIT，余额应用层累计）
    async fn list_stock_flow(&self, where_clause: &str, order_by: &str, fetch_limit: i64, bind_pattern: bool, pattern: &str) -> Result<Vec<StockFlowRow>, DomainError>;
    /// 库存流水导出（时间线补打：外层紧凑 SELECT，ORDER BY create_time,src,sid）
    async fn list_stock_flow_export_timeline(&self, where_clause: &str, bind_pattern: bool, pattern: &str) -> Result<Vec<StockFlowRow>, DomainError>;
    /// 库存流水导出（一品一户数量保管账：ORDER BY product_id,create_time,src,sid）
    async fn list_stock_flow_export_ledger(&self, where_clause: &str, bind_pattern: bool, pattern: &str) -> Result<Vec<StockFlowRow>, DomainError>;
    /// 流水期初净额（与主查询同一聚合口径；pid_list/base_date 由用例层拼装）
    async fn list_stock_flow_opening(&self, pid_list: &str, base_date: &str) -> Result<Vec<StockFlowOpeningRow>, DomainError>;
    /// 主仓名称（流水导出户头用；行为保持：用例层 unwrap_or_else 兜底"主仓"）
    async fn main_warehouse_name(&self) -> Result<String, DomainError>;
    /// 低于最低库存（JSON 版）
    async fn list_stock_warning_low(&self) -> Result<Vec<StockWarningRow>, DomainError>;
    /// 高于最高库存（JSON 版）
    async fn list_stock_warning_high(&self) -> Result<Vec<StockWarningRow>, DomainError>;
    /// 低于最低库存（导出版：排序追加 p.id ASC）
    async fn list_stock_warning_low_export(&self) -> Result<Vec<StockWarningRow>, DomainError>;
    /// 高于最高库存（导出版：排序追加 p.id ASC）
    async fn list_stock_warning_high_export(&self) -> Result<Vec<StockWarningRow>, DomainError>;
    /// 呆滞库存（JSON 版）
    async fn list_slow_stock(&self, days: i64) -> Result<Vec<SlowStockRow>, DomainError>;
    /// 呆滞库存（导出版：含库存金额）
    async fn list_slow_stock_export(&self, days: i64) -> Result<Vec<SlowStockExportRow>, DomainError>;
    /// 出入库统计：采购入库按日+仓库聚合（失败吞为空）
    async fn stock_summary_purchase_rows(&self, start_date: &str, end_date: &str) -> Vec<StockSummaryPurchaseRow>;
    /// 出入库统计：销售出库明细（失败吞为空）
    async fn stock_summary_sales_rows(&self, start_date: &str, end_date: &str) -> Vec<StockSummarySalesRow>;
    /// 出入库统计（报销口径）：分摊增项（目标单增项金额；失败吞为空）
    async fn stock_summary_adj_target_rows(&self, start_date: &str, end_date: &str) -> Vec<StockSummarySalesRow>;
    /// 出入库统计（报销口径）：来源耗材单真实金额（失败吞为空）
    async fn stock_summary_adj_source_rows(&self, start_date: &str, end_date: &str) -> Vec<StockSummarySalesRow>;
}
