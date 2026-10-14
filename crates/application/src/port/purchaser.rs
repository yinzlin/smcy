//! 采购单位域入站端口（UseCase 契约）

use domain::port::purchaser::PurchaserRow;

use crate::error::UseCaseError;

pub trait PurchaserUseCase: Send + Sync {
    async fn list(&self, category_id: Option<i64>, keyword_pattern: String) -> Vec<PurchaserRow>;
    /// 导出 Excel（xlsx 字节流，列结构与旧 handler 逐字节一致）
    async fn export_excel(&self) -> Result<Vec<u8>, UseCaseError>;
    /// 导入（rows 为 Excel/CSV 解析后的二维表），返回结果消息
    async fn import(&self, rows: Vec<Vec<String>>) -> Result<String, UseCaseError>;
    async fn create(&self, cmd: domain::port::purchaser::UpsertPurchaser) -> Result<String, UseCaseError>;
    async fn update(&self, id: i64, cmd: domain::port::purchaser::UpsertPurchaser) -> Result<String, UseCaseError>;
    async fn delete(&self, id: i64) -> Result<String, UseCaseError>;
}
