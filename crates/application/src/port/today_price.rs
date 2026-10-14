//! 今日进价采集入站端口

use crate::error::UseCaseError;
use domain::port::today_price::TodayPriceSaveItem;

/// 今日进价采集用例端口（仅超级管理员；权限检查在 HTTP 层）
pub trait TodayPriceUseCase: Send + Sync {
    /// 采集页聚合数据（预序列化 JSON 文本，保证逐字节兼容）
    async fn items(&self, date: Option<String>) -> String;
    /// 今日价录入（UPSERT 分供应商价 + 联动 product.purchase_price/max/min + 价格日志 + 加成率重算；
    /// 返回 {"success":true,"message":"已保存 N 个商品的今日进价"}）
    async fn save(&self, items: Vec<TodayPriceSaveItem>) -> String;
    /// 按供应商导出清单 Excel，返回 (文件名, xlsx 字节)
    async fn export_excel(&self, date: Option<String>, print_values: bool) -> Result<(String, Vec<u8>), UseCaseError>;
    /// A4 三栏排版导出，返回 (文件名, xlsx 字节)
    async fn export_a4(&self, date: Option<String>, print_values: bool) -> Result<(String, Vec<u8>), UseCaseError>;
    /// 按商品分类导出，返回 (文件名, xlsx 字节)
    async fn export_by_category(&self, date: Option<String>, print_values: bool) -> Result<(String, Vec<u8>), UseCaseError>;
}
