//! 采购查询域出站端口（purchase_order / purchase_order_item 聚合查询、价格趋势与采购比价）

use crate::error::DomainError;

/// 采购订单查询行（api_query_purchase_order / *_export：8 列）
#[derive(Debug, Clone)]
pub struct PurchaseOrderQueryRow {
    pub id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub final_amount: f64,
    pub status: String,
    pub remark: Option<String>,
    pub supplier_name: String,
}

/// 采购价格明细行（api_query_purchase_price：11 列）
#[derive(Debug, Clone)]
pub struct PriceItemRow {
    pub product_name: String,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub production_date: Option<String>,
    pub remark: Option<String>,
    pub shelf_life: Option<String>,
    pub order_date: String,
    pub supplier_name: String,
    pub contact_phone: Option<String>,
}

/// 采购台账导出行（api_query_purchase_price_export：10 列，单价/数量/金额可空）
#[derive(Debug, Clone)]
pub struct PriceExportRow {
    pub product_name: String,
    pub unit: Option<String>,
    pub unit_price: Option<f64>,
    pub quantity: Option<f64>,
    pub amount: Option<f64>,
    pub production_date: Option<String>,
    pub remark: Option<String>,
    pub shelf_life: Option<String>,
    pub order_date: String,
    pub supplier_name: String,
}

/// 按供应商汇总行（api_query_purchase_summary / *_export：3 列）
#[derive(Debug, Clone)]
pub struct SummarySupplierRow {
    pub name: String,
    pub quantity: f64,
    pub amount: f64,
}

/// 按商品汇总行（api_query_purchase_summary / *_export：4 列）
#[derive(Debug, Clone)]
pub struct SummaryProductRow {
    pub product_name: String,
    pub spec: Option<String>,
    pub quantity: f64,
    pub amount: f64,
}

/// 价格趋势-商品信息行（SELECT id, name, spec, base_unit FROM product）
#[derive(Debug, Clone)]
pub struct ProductInfoRow {
    pub id: i64,
    pub name: String,
    pub spec: Option<String>,
    pub base_unit: String,
}

/// 价格趋势-采购明细行（7 列：日期/单价/单位/基础数量/数量/金额/供应商）
#[derive(Debug, Clone)]
pub struct TrendPurchaseRow {
    pub date: String,
    pub price: f64,
    pub unit: String,
    pub bq: Option<f64>,
    pub qty: Option<f64>,
    pub amount: Option<f64>,
    pub supplier_name: String,
}

/// 价格趋势-售价明细行（7 列：日期/单价/单位/基础数量/数量/金额/采购单位）
#[derive(Debug, Clone)]
pub struct TrendSellingRow {
    pub date: String,
    pub price: f64,
    pub unit: String,
    pub bq: Option<f64>,
    pub qty: Option<f64>,
    pub amount: Option<f64>,
    pub purchaser_name: String,
}

/// 采购比价-商品基础信息行（SELECT name, base_unit FROM product）
#[derive(Debug, Clone)]
pub struct ProductBaseRow {
    pub name: String,
    pub base_unit: String,
}

/// 采购比价-历史采购明细行（8 列）
#[derive(Debug, Clone)]
pub struct PriceCompareRow {
    pub date: String,
    pub price: f64,
    pub unit: String,
    pub bq: Option<f64>,
    pub qty: Option<f64>,
    pub amount: Option<f64>,
    pub supplier_id: i64,
    pub supplier_name: String,
}

/// 采购比价-今日采集价行（4 列）
#[derive(Debug, Clone)]
pub struct SupplierTodayPriceRow {
    pub supplier_id: i64,
    pub supplier_name: String,
    pub price: f64,
    pub updated_at: String,
}

pub trait QueryPurchaseRepo: Send + Sync {
    /// 采购订单查询总数（行为保持：旧 handler 对该查询 .unwrap()，失败即 panic）
    async fn count_orders(
        &self,
        supplier_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
    ) -> Result<i64, DomainError>;
    /// 采购订单查询分页数据（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_orders(
        &self,
        supplier_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<Vec<PurchaseOrderQueryRow>, DomainError>;
    /// 采购订单导出数据（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_orders_for_export(
        &self,
        supplier_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
    ) -> Result<Vec<PurchaseOrderQueryRow>, DomainError>;
    /// 采购价格明细总数（行为保持：旧 handler 对该查询 .unwrap()，失败即 panic）
    async fn count_price_items(&self, product_name: &str, supplier_id: &str) -> Result<i64, DomainError>;
    /// 采购价格明细分页（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_price_items(
        &self,
        product_name: &str,
        supplier_id: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<Vec<PriceItemRow>, DomainError>;
    /// 采购汇总-按供应商（行为保持：查询失败 unwrap_or_default 返回空）
    async fn summary_by_supplier(&self, start_date: &str, end_date: &str) -> Result<Vec<SummarySupplierRow>, DomainError>;
    /// 采购汇总-按商品（行为保持：查询失败 unwrap_or_default 返回空）
    async fn summary_by_product(&self, start_date: &str, end_date: &str) -> Result<Vec<SummaryProductRow>, DomainError>;
    /// 价格趋势-商品信息（行为保持：旧 handler .ok().flatten() 吞错 → None 视为不存在 → 404）
    async fn fetch_product_info(&self, product_id: i64) -> Result<Option<ProductInfoRow>, DomainError>;
    /// 价格趋势-同基础单位商品 id 列表（行为保持：查询失败 unwrap_or_default 返回空 → 空点位 200）
    async fn list_same_base_product_ids(&self, name: &str, base_unit: &str) -> Result<Vec<i64>, DomainError>;
    /// 价格趋势-采购明细（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_trend_purchase(
        &self,
        product_ids: &[i64],
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<TrendPurchaseRow>, DomainError>;
    /// 价格趋势-售价明细（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_trend_selling(
        &self,
        product_ids: &[i64],
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<TrendSellingRow>, DomainError>;
    /// 采购比价-商品基础信息（行为保持：旧 handler .ok().flatten() 吞错 → None 视为不存在 → 404）
    async fn fetch_product_base(&self, product_id: i64) -> Result<Option<ProductBaseRow>, DomainError>;
    /// 采购比价-历史采购明细（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_price_history(&self, product_id: i64) -> Result<Vec<PriceCompareRow>, DomainError>;
    /// 采购比价-今日采集价（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_today_supplier_prices(&self, product_id: i64) -> Result<Vec<SupplierTodayPriceRow>, DomainError>;
    /// 采购台账导出（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_price_items_for_export(&self, product_name: &str, supplier_id: &str) -> Result<Vec<PriceExportRow>, DomainError>;
    /// 汇总导出-按供应商（行为保持：查询失败 unwrap_or_default 返回空）
    async fn summary_export_by_supplier(&self, start_date: &str, end_date: &str) -> Result<Vec<SummarySupplierRow>, DomainError>;
    /// 汇总导出-按商品（行为保持：查询失败 unwrap_or_default 返回空）
    async fn summary_export_by_product(&self, start_date: &str, end_date: &str) -> Result<Vec<SummaryProductRow>, DomainError>;
}
