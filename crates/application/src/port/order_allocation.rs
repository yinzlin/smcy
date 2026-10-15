//! 耗材分摊域入站端口（UseCase 契约）

use crate::error::UseCaseError;

pub trait OrderAllocationUseCase: Send + Sync {
    /// 创建分摊方案 → 200 JSON {"id":..,"total_amount":..}
    async fn create(&self, req: serde_json::Value) -> Result<String, UseCaseError>;
    /// 方案摘要 → 200 JSON（不存在时 {"exists":false}）
    async fn summary(&self, source_order_id: i64) -> Result<String, UseCaseError>;
    /// 全部已分摊源订单 → 200 JSON 数组
    async fn allocated_orders(&self) -> Result<String, UseCaseError>;
    /// 终止方案 → "终止成功"
    async fn terminate(&self, req: serde_json::Value) -> Result<String, UseCaseError>;
    /// 取消方案 → "取消成功"
    async fn cancel(&self, req: serde_json::Value) -> Result<String, UseCaseError>;
    /// 完成分摊 → 200 JSON {"auto_tail": bool}
    async fn complete(&self, req: serde_json::Value) -> Result<String, UseCaseError>;
}
