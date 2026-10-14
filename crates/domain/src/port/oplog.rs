//! 操作审计日志出站端口

use crate::UserCtx;

pub trait OperationLogRepo: Send + Sync {
    /// 记录操作日志（内部吞错：日志失败不影响业务结果，与旧行为一致）
    async fn log(&self, user: &UserCtx, action: &str, target_type: &str, target_id: &str, detail: &str);
}
