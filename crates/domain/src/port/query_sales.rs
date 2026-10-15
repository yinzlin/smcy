//! 销售查询域出站端口（sales_order / sales_order_item 聚合查询、报销口径与商品排行）

use crate::error::DomainError;

/// 销售订单查询行（api_query_sales_order / *_export：9 列）
#[derive(Debug, Clone)]
pub struct SalesOrderQueryRow {
    pub id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub final_amount: f64,
    pub status: String,
    pub remark: Option<String>,
    pub purchaser_name: String,
}

/// 销售价格明细分页行（api_query_sales_price：7 列）
#[derive(Debug, Clone)]
pub struct SalesPriceItemRow {
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub purchaser_name: String,
    pub unit_price: f64,
    pub order_date: String,
    pub quantity: f64,
}

/// 销售价格导出行（api_query_sales_price_export：7 列，列序与分页行不同）
#[derive(Debug, Clone)]
pub struct SalesPriceExportRow {
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub order_date: String,
    pub purchaser_name: String,
}

/// 销售汇总-按采购单位行（api_query_sales_summary / *_export：3 列）
#[derive(Debug, Clone)]
pub struct SalesSummaryPurchaserRow {
    pub name: String,
    pub quantity: f64,
    pub sales_amount: f64,
}

/// 销售汇总-按商品行（api_query_sales_summary / *_export：4 列）
#[derive(Debug, Clone)]
pub struct SalesSummaryProductRow {
    pub product_name: String,
    pub spec: Option<String>,
    pub quantity: f64,
    pub sales_amount: f64,
}

/// 报销口径-按采购单位聚合行（真实明细/分摊增项共用：purchaser_id + name + 金额）
#[derive(Debug, Clone)]
pub struct ReimbursePurchaserRow {
    pub purchaser_id: i64,
    pub name: String,
    pub amount: f64,
}

/// 报销口径-来源单真实明细行（GROUP BY so.id，用于扣除分摊来源单）
#[derive(Debug, Clone)]
pub struct ReimburseOrderRow {
    pub purchaser_id: i64,
    pub name: String,
    pub order_id: i64,
    pub amount: f64,
}

/// 报销口径-商品真实明细行（spec 经 COALESCE 为空串）
#[derive(Debug, Clone)]
pub struct ReimburseProductRealRow {
    pub product_name: String,
    pub spec: String,
    pub order_id: i64,
    pub quantity: f64,
    pub amount: f64,
}

/// 报销口径-商品分摊增项行（spec 经 COALESCE 为空串）
#[derive(Debug, Clone)]
pub struct ReimburseProductSuppRow {
    pub product_name: String,
    pub spec: String,
    pub quantity: f64,
    pub amount: f64,
}

/// 商品排行-畅销 TOP 行（api_query_product_rank / *_export 共用：4 列）
#[derive(Debug, Clone)]
pub struct RankTopRow {
    pub product_name: String,
    pub spec: Option<String>,
    pub quantity: f64,
    pub amount: f64,
}

/// 商品排行-呆滞商品行（stock_quantity/last_sale_date 取值失败吞错为默认值）
#[derive(Debug, Clone)]
pub struct RankSlowRow {
    pub product_name: String,
    pub spec: Option<String>,
    pub stock_quantity: f64,
    pub last_sale_date: Option<String>,
}

pub trait QuerySalesRepo: Send + Sync {
    /// 销售订单查询总数（行为保持：旧 handler 对该查询 .unwrap()，失败即 panic）
    async fn count_orders(
        &self,
        purchaser_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
    ) -> Result<i64, DomainError>;
    /// 销售订单查询分页数据（order_clause 由用例层按 sort_field 白名单拼装）
    /// 行为保持：查询失败 unwrap_or_default 返回空
    async fn list_orders(
        &self,
        purchaser_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
        order_clause: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<Vec<SalesOrderQueryRow>, DomainError>;
    /// 销售订单导出数据（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_orders_for_export(
        &self,
        purchaser_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
    ) -> Result<Vec<SalesOrderQueryRow>, DomainError>;
    /// 销售价格明细总数（行为保持：旧 handler 对该查询 .unwrap()，失败即 panic）
    async fn count_price_items(
        &self,
        product_name: &str,
        product_id: &str,
        purchaser_id: &str,
    ) -> Result<i64, DomainError>;
    /// 销售价格明细分页（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_price_items(
        &self,
        product_name: &str,
        product_id: &str,
        purchaser_id: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<Vec<SalesPriceItemRow>, DomainError>;
    /// 销售汇总-按采购单位（行为保持：查询失败 unwrap_or_default 返回空）
    async fn summary_by_purchaser(&self, start_date: &str, end_date: &str) -> Result<Vec<SalesSummaryPurchaserRow>, DomainError>;
    /// 销售汇总-按商品（行为保持：查询失败 unwrap_or_default 返回空）
    async fn summary_by_product(&self, start_date: &str, end_date: &str) -> Result<Vec<SalesSummaryProductRow>, DomainError>;
    /// 分摊来源订单 id 列表（行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_allocation_source_ids(&self) -> Result<Vec<i64>, DomainError>;
    /// 报销口径-真实明细按采购单位聚合（行为保持：查询失败 unwrap_or_default 返回空）
    async fn reimburse_real_by_purchaser(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimbursePurchaserRow>, DomainError>;
    /// 报销口径-分摊增项按目标单采购单位聚合（行为保持：查询失败 unwrap_or_default 返回空）
    async fn reimburse_supp_by_purchaser(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimbursePurchaserRow>, DomainError>;
    /// 报销口径-真实明细按订单聚合（用于扣除分摊来源单，行为保持：查询失败 unwrap_or_default 返回空）
    async fn reimburse_real_by_order(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimburseOrderRow>, DomainError>;
    /// 报销口径-商品真实明细（排除来源单由用例层过滤，行为保持：查询失败 unwrap_or_default 返回空）
    async fn reimburse_product_real(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimburseProductRealRow>, DomainError>;
    /// 报销口径-商品分摊增项（行为保持：查询失败 unwrap_or_default 返回空）
    async fn reimburse_product_supp(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimburseProductSuppRow>, DomainError>;
    /// 商品排行-畅销 TOP10（行为保持：查询失败 unwrap_or_default 返回空）
    async fn rank_top(&self, start_date: &str, end_date: &str) -> Result<Vec<RankTopRow>, DomainError>;
    /// 商品排行-呆滞商品 TOP50（行为保持：查询失败 unwrap_or_default 返回空）
    async fn rank_slow(&self, start_date: &str, end_date: &str) -> Result<Vec<RankSlowRow>, DomainError>;
    /// 销售价格导出（注意：导出无 product_id 过滤，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_price_for_export(&self, product_name: &str, purchaser_id: &str) -> Result<Vec<SalesPriceExportRow>, DomainError>;
    /// 销售汇总导出-按采购单位（ORDER BY 带 p.id 平级稳定排序，行为保持：查询失败 unwrap_or_default 返回空）
    async fn summary_export_by_purchaser(&self, start_date: &str, end_date: &str) -> Result<Vec<SalesSummaryPurchaserRow>, DomainError>;
    /// 销售汇总导出-按商品（ORDER BY 带品名/规格平级稳定排序，行为保持：查询失败 unwrap_or_default 返回空）
    async fn summary_export_by_product(&self, start_date: &str, end_date: &str) -> Result<Vec<SalesSummaryProductRow>, DomainError>;
}
