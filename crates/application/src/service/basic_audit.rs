//! 基础数据审核用例：supplier / purchaser / product / warehouse 通用审核流转
//!
//! 自旧 set_basic_audit_status 端口化：super_admin 校验、rows_affected
//! 判定、操作日志记录逻辑逐字保持。

use domain::port::audit::{BasicAuditRepo, BasicTable};
use domain::port::oplog::OperationLogRepo;
use domain::UserCtx;

use crate::error::UseCaseError;
use crate::port::audit::BasicAuditUseCase;

#[derive(Clone)]
pub struct BasicAuditService<R: BasicAuditRepo, L: OperationLogRepo> {
    repo: R,
    oplog: L,
}

impl<R: BasicAuditRepo, L: OperationLogRepo> BasicAuditService<R, L> {
    pub fn new(repo: R, oplog: L) -> Self {
        Self { repo, oplog }
    }
}

impl<R: BasicAuditRepo, L: OperationLogRepo> BasicAuditUseCase for BasicAuditService<R, L> {
    async fn set_audit_status(
        &self,
        ctx: &UserCtx,
        table: BasicTable,
        id: i64,
        to_status: &str,
        action: &str,
    ) -> Result<(), UseCaseError> {
        if ctx.role != "super_admin" {
            return Err(UseCaseError::Forbidden("仅超级管理员可执行审核/反审核操作".to_string()));
        }
        let affected = self
            .repo
            .set_audit_status(table, id, to_status)
            .await
            .map_err(|e| UseCaseError::Storage(format!("操作失败: {}", e)))?;
        if affected {
            let status_text = if to_status == "confirmed" { "已审核" } else { "待审核" };
            self.oplog
                .log(
                    ctx,
                    action,
                    table.as_str(),
                    &id.to_string(),
                    &format!("{} ID={} 审核状态 → {}", table.as_str(), id, status_text),
                )
                .await;
            Ok(())
        } else {
            Err(UseCaseError::NotFound("记录不存在".to_string()))
        }
    }
}
