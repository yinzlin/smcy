//! 分类域入站端口（UseCase 契约）

use domain::port::category::CategoryRow;

use crate::error::UseCaseError;

pub trait CategoryUseCase: Send + Sync {
    async fn list(&self) -> Vec<CategoryRow>;
    async fn tree(&self, entity_type: &str) -> Vec<serde_json::Value>;
    async fn create(
        &self,
        name: String,
        parent_id: Option<i64>,
        entity_type: String,
        sort_order: Option<i32>,
    ) -> Result<(), UseCaseError>;
    async fn rename(&self, id: i64, name: String) -> Result<String, UseCaseError>;
    async fn delete(&self, id: i64) -> Result<String, UseCaseError>;
}
