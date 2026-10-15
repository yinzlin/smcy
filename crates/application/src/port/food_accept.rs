//! 验收单域入站端口（UseCase 契约）

use domain::AcceptReq;

use crate::error::UseCaseError;

pub trait FoodAcceptUseCase: Send + Sync {
    /// 创建验收单 → 旧行为：Ok 仅 200 状态码，Err 仅 500 状态码（无响应体文案）
    async fn create(&self, req: AcceptReq) -> Result<(), UseCaseError>;
    /// 验收单列表 → 200 JSON 数组
    async fn list(&self) -> Result<String, UseCaseError>;
}
