//! 库存域入站端口（UseCase 契约）

use domain::port::inventory::InventoryRow;

pub trait InventoryUseCase: Send + Sync {
    /// 库存列表（查询失败吞为空数组）
    async fn list(&self) -> Vec<InventoryRow>;
}
