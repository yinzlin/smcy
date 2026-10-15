//! 库存查询域入站端口（UseCase 契约）

use std::collections::HashMap;

use crate::error::UseCaseError;

pub trait QueryStockUseCase: Send + Sync {
    /// 库存余额/收发存（历史时点/实时两种口径，日期非法由 HTTP 层前置拦截）→ 200 JSON {items,total,page,page_size,mode}
    async fn stock_balance(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 库存余额/收发存导出 → xlsx 字节（文件名由 HTTP 层按口径拼接）
    async fn export_stock_balance(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 库存流水（时间线/分户视图，余额应用层累计）→ 200 JSON {total,page,page_size,items}
    async fn stock_flow(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 出入库记录导出 → xlsx 字节（文件名由 HTTP 层按口径拼接）
    async fn export_stock_flow(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 出入库统计 → 200 JSON {rows,warehouses,total_in_amount,total_out_amount,total_discounted_out_amount,total_gross_profit}
    async fn stock_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 出入库统计导出 → xlsx 字节
    async fn export_stock_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 报销口径出入库统计 → 200 JSON
    async fn stock_summary_reimburse(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 报销口径出入库统计导出 → xlsx 字节
    async fn export_stock_summary_reimburse(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 库存预警（低于最低/高于最高）→ 200 JSON {low_stock,high_stock}
    async fn stock_warning(&self) -> Result<String, UseCaseError>;
    /// 库存预警导出 → xlsx 字节
    async fn export_stock_warning(&self) -> Result<Vec<u8>, UseCaseError>;
    /// 呆滞库存 → 200 JSON 数组
    async fn slow_stock(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 呆滞库存导出 → xlsx 字节
    async fn export_slow_stock(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
}
