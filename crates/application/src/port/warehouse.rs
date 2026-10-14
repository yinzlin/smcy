//! 仓库域入站端口（UseCase 契约）

use domain::port::warehouse::WarehouseRow;

use crate::error::UseCaseError;

pub trait WarehouseUseCase: Send + Sync {
    async fn list(&self) -> Vec<WarehouseRow>;
    async fn create(
        &self,
        name: String,
        code: Option<String>,
        address: Option<String>,
        contact: Option<String>,
        phone: Option<String>,
        sort_order: Option<i32>,
    ) -> Result<String, UseCaseError>;
    async fn update(
        &self,
        id: i64,
        name: String,
        code: Option<String>,
        address: Option<String>,
        contact: Option<String>,
        phone: Option<String>,
        status: Option<i32>,
        sort_order: Option<i32>,
    ) -> Result<String, UseCaseError>;
    async fn delete(&self, id: i64) -> Result<String, UseCaseError>;
}
