//! 采购查询域入站端口（UseCase 契约）

use std::collections::HashMap;

use domain::UserCtx;

use crate::error::UseCaseError;

pub trait QueryPurchaseUseCase: Send + Sync {
    /// 采购订单查询 → 200 JSON {data,page,page_size,total,total_pages}
    async fn query_orders(&self, ctx: &UserCtx, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 采购订单导出 → xlsx 字节（响应头由 HTTP 层拼接）
    async fn export_orders(&self, ctx: &UserCtx, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 采购价格明细查询 → 200 JSON {data,page,page_size,total,total_pages}
    async fn price_items(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 采购汇总（按供应商/按商品）→ 200 JSON {by_supplier,by_product}
    async fn purchase_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 商品价格趋势 → 200 JSON（缺 product_id → 400，商品不存在 → 404，行为逐字节保持）
    async fn price_trend(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 采购比价 → 200 JSON（缺 product_id → 400，商品不存在 → 404，行为逐字节保持）
    async fn price_compare(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 采购台账导出 → xlsx 字节（is_super_admin 控制单价/金额列可见性）
    async fn export_price_items(&self, ctx: &UserCtx, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 采购汇总导出 → xlsx 字节
    async fn export_purchase_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
}
