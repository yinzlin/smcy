//! 销售查询域入站端口（UseCase 契约）

use std::collections::HashMap;

use domain::UserCtx;

use crate::error::UseCaseError;

pub trait QuerySalesUseCase: Send + Sync {
    /// 销售订单查询 → 200 JSON {data,page,page_size,total,total_pages}
    async fn query_orders(&self, ctx: &UserCtx, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 销售订单导出 → xlsx 字节（响应头由 HTTP 层拼接）
    async fn export_orders(&self, ctx: &UserCtx, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 销售价格明细查询 → 200 JSON {data,page,page_size,total,total_pages}
    async fn price_items(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 销售汇总（按采购单位/按商品）→ 200 JSON {by_purchaser,by_product}
    async fn sales_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 报销口径汇总（真实明细 + 分摊增项净额）→ 200 JSON {by_purchaser,by_product}
    async fn reimburse_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 商品排行（畅销 TOP10 + 呆滞 TOP50）→ 200 JSON {top_selling,slow_moving}
    async fn product_rank(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 销售价格导出 → xlsx 字节
    async fn export_price_items(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 销售汇总导出 → xlsx 字节
    async fn export_sales_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 商品排行导出 → xlsx 字节
    async fn export_product_rank(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 报销口径汇总导出 → xlsx 字节
    async fn export_reimburse_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
}
