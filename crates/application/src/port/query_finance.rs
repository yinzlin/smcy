//! 财务查询域入站端口（UseCase 契约）

use std::collections::HashMap;

use domain::UserCtx;

use crate::error::UseCaseError;

pub trait QueryFinanceUseCase: Send + Sync {
    /// 供应商往来对账 → 200 JSON 数组
    async fn supplier_balance(&self, ctx: &UserCtx) -> Result<String, UseCaseError>;
    /// 供应商往来对账导出 → xlsx 字节（响应头由 HTTP 层拼接）
    async fn export_supplier_balance(&self, ctx: &UserCtx) -> Result<Vec<u8>, UseCaseError>;
    /// 采购单位往来对账 → 200 JSON 数组
    async fn purchaser_balance(&self, ctx: &UserCtx) -> Result<String, UseCaseError>;
    /// 采购单位往来对账导出 → xlsx 字节（响应头由 HTTP 层拼接）
    async fn export_purchaser_balance(&self, ctx: &UserCtx) -> Result<Vec<u8>, UseCaseError>;
    /// 分摊来源查询 → 200 JSON 数组（含每单分摊去向）
    async fn allocation_source(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 分摊来源导出 → xlsx 字节
    async fn export_allocation_source(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 财务结算查询 → 200 JSON {data,page,page_size,total,total_pages,summary}
    async fn finance_settlement(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 收支流水 → 200 JSON 数组
    async fn income_expense(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 收支流水导出 → xlsx 字节
    async fn export_income_expense(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 毛利明细 → 200 JSON 数组
    async fn profit_detail(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 毛利明细导出 → xlsx 字节
    async fn export_profit_detail(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
}
