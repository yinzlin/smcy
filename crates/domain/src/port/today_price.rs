//! 今日进价采集出站端口：聚合查询 / 分供应商价 UPSERT / 进价区间联动
//!
//! SQL 与旧 api_product_today_price_* handler 逐字一致，行为保持注释见各方法。

use crate::error::DomainError;

/// 采集页聚合行（按 供应商×商品×销售单位 分组）
#[derive(Debug, Clone)]
pub struct TodayPriceItemRow {
    pub product_id: i64,
    pub product_name: Option<String>,
    pub spec: Option<String>,
    pub supplier_id: i64,
    pub supplier_name: Option<String>,
    pub base_unit: Option<String>,
    pub purchase_price: Option<f64>,
    pub max_purchase_price: Option<f64>,
    pub min_purchase_price: Option<f64>,
    pub supplier_price: Option<f64>,
    /// 销售单位（SQL COALESCE 保证非 NULL，空串=基础单位）
    pub sale_unit: String,
    pub unit_qty: Option<f64>,
    pub base_qty: Option<f64>,
}

/// 按供应商导出行（按 供应商×商品 分组，带合计数量）
#[derive(Debug, Clone)]
pub struct TodayPriceExportRow {
    pub product_id: i64,
    pub product_name: Option<String>,
    pub spec: Option<String>,
    pub supplier_id: i64,
    pub supplier_name: Option<String>,
    pub base_unit: Option<String>,
    pub purchase_price: Option<f64>,
    pub max_purchase_price: Option<f64>,
    pub min_purchase_price: Option<f64>,
    pub supplier_price: Option<f64>,
    pub total_qty: Option<f64>,
}

/// 按分类导出行（按 商品 分组，带分类与父分类）
#[derive(Debug, Clone)]
pub struct TodayPriceCategoryRow {
    pub product_id: i64,
    pub product_name: Option<String>,
    pub spec: Option<String>,
    pub base_unit: Option<String>,
    pub purchase_price: Option<f64>,
    pub max_purchase_price: Option<f64>,
    pub min_purchase_price: Option<f64>,
    pub total_qty: Option<f64>,
    pub category_name: Option<String>,
    pub parent_name: Option<String>,
}

/// 今日价录入入参（单条）
#[derive(Debug, Clone, Copy)]
pub struct TodayPriceSaveItem {
    pub product_id: i64,
    pub supplier_id: i64,
    pub price: f64,
}

/// 今日进价采集仓储端口
pub trait TodayPriceRepo: Send + Sync {
    /// 采集页聚合查询（date=None 取 pending/sorting 在途单；失败按旧行为吞错返回空）
    async fn fetch_items(&self, date: Option<&str>) -> Result<Vec<TodayPriceItemRow>, DomainError>;
    /// 按供应商导出聚合查询（失败按旧行为吞错返回空）
    async fn fetch_export_rows(&self, date: Option<&str>) -> Result<Vec<TodayPriceExportRow>, DomainError>;
    /// 按分类导出聚合查询（失败按旧行为吞错返回空）
    async fn fetch_category_rows(&self, date: Option<&str>) -> Result<Vec<TodayPriceCategoryRow>, DomainError>;
    /// 分供应商价 UPSERT（失败返回 Err，用例层 continue 跳过该条）
    async fn upsert_supplier_price(&self, product_id: i64, supplier_id: i64, price: f64) -> Result<(), DomainError>;
    /// 进价与区间快照（purchase_price, max_purchase_price, min_purchase_price；None=商品不存在或查询失败）
    async fn get_price_range(&self, product_id: i64) -> Result<Option<(f64, f64, f64)>, DomainError>;
    /// 同步更新 product.purchase_price 与 max/min 区间（成功才计数 + 记日志 + 触发重算）
    async fn update_price_range(&self, product_id: i64, price: f64, max_price: f64, min_price: f64) -> Result<(), DomainError>;
}
