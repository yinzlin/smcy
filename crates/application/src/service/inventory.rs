//! 库存用例：列表查询

use domain::port::inventory::{InventoryRepo, InventoryRow};

use crate::port::inventory::InventoryUseCase;

#[derive(Clone)]
pub struct InventoryService<R: InventoryRepo> {
    repo: R,
}

impl<R: InventoryRepo> InventoryService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: InventoryRepo> InventoryUseCase for InventoryService<R> {
    async fn list(&self) -> Vec<InventoryRow> {
        // 行为保持：查询失败旧 unwrap_or_default → 空数组
        self.repo.list_all().await.unwrap_or_default()
    }
}
