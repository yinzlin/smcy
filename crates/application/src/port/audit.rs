//! 基础数据审核入站端口（UseCase 契约）

use domain::port::audit::BasicTable;
use domain::UserCtx;

use crate::error::UseCaseError;

pub trait BasicAuditUseCase: Send + Sync {
    /// 审核/反审核基础数据（仅 super_admin；成功后记录操作日志）
    async fn set_audit_status(
        &self,
        ctx: &UserCtx,
        table: BasicTable,
        id: i64,
        to_status: &str,
        action: &str,
    ) -> Result<(), UseCaseError>;
}
