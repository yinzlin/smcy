//! 统计概览域出站端口（月度进销存概览、品类统计、单据汇总）

use crate::error::DomainError;

/// 名称-金额-数量聚合行（概览按供应商采购汇总 / 按采购单位销售汇总共用）
#[derive(Debug, Clone)]
pub struct NameAmountQuantityRow {
    pub name: String,
    pub amount: f64,
    pub quantity: f64,
}

/// 品类进销存统计行（采购/销售/库存的数量与金额）
#[derive(Debug, Clone)]
pub struct CategoryStatsRow {
    pub category_name: String,
    pub purchase_quantity: f64,
    pub purchase_amount: f64,
    pub sales_quantity: f64,
    pub sales_amount: f64,
    pub stock_quantity: f64,
    pub stock_amount: f64,
}

/// 单据汇总行（按月聚合的采购/销售订单数与金额；UNION ALL 采购段/销售段各出同行）
#[derive(Debug, Clone)]
pub struct DocumentSummaryRow {
    pub month: String,
    pub purchase_count: i64,
    pub purchase_amount: f64,
    pub sales_count: i64,
    pub sales_amount: f64,
}

pub trait QueryStatsRepo: Send + Sync {
    /// 采购总额（概览 JSON 版：SQL 空格风格与旧 api_query_overview 逐字一致；行为保持：用例层 unwrap_or(0.0)）
    async fn sum_purchase_total(&self, month: &str) -> Result<f64, DomainError>;
    /// 销售总额（概览 JSON 版）
    async fn sum_sales_total(&self, month: &str) -> Result<f64, DomainError>;
    /// 库存总额（概览 JSON 版：按售价计，无月份条件）
    async fn sum_stock_total(&self) -> Result<f64, DomainError>;
    /// 按供应商采购汇总（JSON 版：多行 SQL，ORDER BY amount DESC）
    async fn list_purchase_by_supplier(&self, month: &str) -> Result<Vec<NameAmountQuantityRow>, DomainError>;
    /// 按采购单位销售汇总（JSON 版：多行 SQL，ORDER BY amount DESC）
    async fn list_sales_by_purchaser(&self, month: &str) -> Result<Vec<NameAmountQuantityRow>, DomainError>;
    /// 采购总额（概览导出版：紧凑 SQL 与旧 api_query_overview_export 逐字一致）
    async fn sum_purchase_total_export(&self, month: &str) -> Result<f64, DomainError>;
    /// 销售总额（概览导出版）
    async fn sum_sales_total_export(&self, month: &str) -> Result<f64, DomainError>;
    /// 库存总额（概览导出版）
    async fn sum_stock_total_export(&self) -> Result<f64, DomainError>;
    /// 按供应商采购汇总（导出版：单行 SQL，ORDER BY amount DESC, s.id ASC）
    async fn list_purchase_by_supplier_export(&self, month: &str) -> Result<Vec<NameAmountQuantityRow>, DomainError>;
    /// 按采购单位销售汇总（导出版：单行 SQL，ORDER BY amount DESC, p.id ASC）
    async fn list_sales_by_purchaser_export(&self, month: &str) -> Result<Vec<NameAmountQuantityRow>, DomainError>;
    /// 品类进销存统计（JSON 版：多行 SQL 含行尾空格，与旧 api_query_category_stats 逐字一致）
    async fn list_category_stats(&self, start_date: &str, end_date: &str) -> Result<Vec<CategoryStatsRow>, DomainError>;
    /// 品类进销存统计（导出版：单行紧凑 SQL，与旧 api_query_category_stats_export 逐字一致）
    async fn list_category_stats_export(&self, start_date: &str, end_date: &str) -> Result<Vec<CategoryStatsRow>, DomainError>;
    /// 单据汇总（JSON 版：多行 UNION ALL SQL，与旧 api_query_document_summary 逐字一致）
    async fn list_document_summary(&self, month: &str) -> Result<Vec<DocumentSummaryRow>, DomainError>;
    /// 单据汇总（导出版：单行紧凑 SQL，与旧 api_query_document_summary_export 逐字一致）
    async fn list_document_summary_export(&self, month: &str) -> Result<Vec<DocumentSummaryRow>, DomainError>;
}
