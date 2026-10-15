//! 订单增补（分摊增项）域入站端口（UseCase 契约）

use crate::error::UseCaseError;
use domain::OrderSupplementItemReq;

pub trait OrderSupplementUseCase: Send + Sync {
    /// 创建增项 → 200 JSON {"id":..}
    async fn create(&self, req: OrderSupplementItemReq) -> Result<String, UseCaseError>;
    /// 按目标订单列出增项 → 200 JSON 数组
    async fn list_by_target(&self, order_id: i64) -> Result<String, UseCaseError>;
    /// 按源订单列出增项 → 200 JSON 数组
    async fn list_by_source(&self, order_id: i64) -> Result<String, UseCaseError>;
    /// 删除增项并回滚分摊余额 → "回滚成功"
    async fn delete(&self, id: i64) -> Result<String, UseCaseError>;
    /// 目标订单增补比价 → 200 JSON（real_total/supplement_total/allocation_total/items）
    async fn compare(&self, order_id: i64) -> Result<String, UseCaseError>;
    /// 调整订单分页列表 → 200 JSON（分页+合计）
    async fn adjusted_orders(
        &self,
        params: std::collections::HashMap<String, String>,
    ) -> Result<String, UseCaseError>;
}
