//! 财务查询域出站端口（供应商/采购单位往来、分摊来源、财务结算、收支流水、毛利明细）

use crate::error::DomainError;

/// 供应商往来对账行（api_query_supplier_balance / *_export：purchase_total 与 unpaid 同源）
#[derive(Debug, Clone)]
pub struct SupplierBalanceRow {
    pub id: i64,
    pub name: String,
    pub purchase_total: f64,
    pub unpaid: f64,
}

/// 采购单位往来对账行（api_query_purchaser_balance / *_export：sales_total 与 unreceived 同源）
#[derive(Debug, Clone)]
pub struct PurchaserBalanceRow {
    pub id: i64,
    pub name: String,
    pub sales_total: f64,
    pub unreceived: f64,
}

/// 分摊来源单行（api_query_allocation_source / *_export 主查询）
#[derive(Debug, Clone)]
pub struct AllocationSourceRow {
    pub source_order_id: i64,
    pub total_amount: f64,
    pub allocated_amount: f64,
    pub remaining_balance: f64,
    pub status: i64,
    pub order_no: String,
    pub order_date: String,
}

/// 分摊去向行（来源单分摊到的各目标单及金额，HAVING SUM <> 0）
#[derive(Debug, Clone)]
pub struct AllocationTargetRow {
    pub order_no: String,
    pub amount: f64,
}

/// 财务结算合并订单行（采购/销售 UNION ALL，party 为供应商或采购单位）
#[derive(Debug, Clone)]
pub struct FinanceSettlementRow {
    pub order_type: String,
    pub source_id: i64,
    pub order_no: String,
    pub order_date: String,
    pub party_name: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub amount_reduction: f64,
    pub final_amount: f64,
    pub is_settled: i64,
}

/// 收支流水行（total_amount/final_amount 取值失败吞错为 0.0）
#[derive(Debug, Clone)]
pub struct IncomeExpenseRow {
    pub order_date: String,
    pub type_name: String,
    pub total_amount: f64,
    pub final_amount: f64,
    pub direction: String,
}

/// 毛利明细行（数值列取值失败吞错为 0.0）
#[derive(Debug, Clone)]
pub struct ProfitDetailRow {
    pub order_no: String,
    pub order_date: String,
    pub product_name: String,
    pub quantity: f64,
    pub sale_price: f64,
    pub purchase_price: f64,
    pub profit: f64,
    pub sale_amount: f64,
    pub cost_amount: f64,
}

pub trait QueryFinanceRepo: Send + Sync {
    /// 供应商往来对账（restricted 时仅查 supplier_id，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_supplier_balance(&self, restricted: bool, supplier_id: i64) -> Result<Vec<SupplierBalanceRow>, DomainError>;
    /// 采购单位往来对账（restricted 时仅查 purchaser_id，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_purchaser_balance(&self, restricted: bool, purchaser_id: i64) -> Result<Vec<PurchaserBalanceRow>, DomainError>;
    /// 分摊来源主查询（JSON 版：ORDER BY so.order_date DESC，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_allocation_sources(&self, start_date: &str, end_date: &str) -> Result<Vec<AllocationSourceRow>, DomainError>;
    /// 分摊去向（JSON 版嵌套查询：ORDER BY amount DESC，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_allocation_targets(&self, source_order_id: i64) -> Result<Vec<AllocationTargetRow>, DomainError>;
    /// 分摊来源主查询（导出版：ORDER BY so.order_date DESC, so.id DESC，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_allocation_sources_export(&self, start_date: &str, end_date: &str) -> Result<Vec<AllocationSourceRow>, DomainError>;
    /// 分摊去向（导出版嵌套查询：ORDER BY 追加 osi.target_order_id ASC，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_allocation_targets_export(&self, source_order_id: i64) -> Result<Vec<AllocationTargetRow>, DomainError>;
    /// 财务结算总数（where_sql/type_filter 由用例层拼装，行为保持：查询失败 unwrap_or(0)）
    async fn count_finance_settlement(&self, where_sql: &str, type_filter: &str) -> Result<i64, DomainError>;
    /// 财务结算分页数据（LIMIT/OFFSET 由仓储层内联拼装，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_finance_settlement(&self, where_sql: &str, type_filter: &str, page_size: i64, offset: i64) -> Result<Vec<FinanceSettlementRow>, DomainError>;
    /// 收支流水（JSON 版：ORDER BY order_date，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_income_expense(&self, start_date: &str, end_date: &str) -> Result<Vec<IncomeExpenseRow>, DomainError>;
    /// 收支流水（导出版：含 id as oid，ORDER BY order_date, direction, oid，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_income_expense_export(&self, start_date: &str, end_date: &str) -> Result<Vec<IncomeExpenseRow>, DomainError>;
    /// 毛利明细（JSON 版：ORDER BY so.order_date, so.order_no，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_profit_detail(&self, start_date: &str, end_date: &str) -> Result<Vec<ProfitDetailRow>, DomainError>;
    /// 毛利明细（导出版：ORDER BY so.order_date, so.id, soi.id，行为保持：查询失败 unwrap_or_default 返回空）
    async fn list_profit_detail_export(&self, start_date: &str, end_date: &str) -> Result<Vec<ProfitDetailRow>, DomainError>;
}
