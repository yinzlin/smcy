//! 统计概览域入站端口（UseCase 契约）

use std::collections::HashMap;

use crate::error::UseCaseError;

pub trait QueryStatsUseCase: Send + Sync {
    /// 月度进销存概览 → 200 JSON {purchase_total,sales_total,stock_total,profit_total,purchase_by_supplier,sales_by_purchaser}
    async fn overview(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 月度进销存概览导出 → xlsx 字节（文件名由 HTTP 层拼接）
    async fn export_overview(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 品类进销存统计 → 200 JSON 数组
    async fn category_stats(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 品类进销存统计导出 → xlsx 字节
    async fn export_category_stats(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 单据汇总 → 200 JSON 数组（同月 UNION ALL 两段按 max 合并后按月份升序）
    async fn document_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 单据汇总导出 → xlsx 字节
    async fn export_document_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
}
