//! 基础数据审核状态仓储实现：SQL 与旧 set_basic_audit_status /
//! check_basic_not_confirmed 逐字一致（表名来自白名单枚举）

use sqlx::sqlite::SqlitePool;
use sqlx::AssertSqlSafe;

use domain::error::DomainError;
use domain::port::audit::{BasicAuditRepo, BasicTable};

#[derive(Clone)]
pub struct SqliteBasicAuditRepo {
    pool: SqlitePool,
}

impl SqliteBasicAuditRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl BasicAuditRepo for SqliteBasicAuditRepo {
    async fn set_audit_status(&self, table: BasicTable, id: i64, status: &str) -> Result<bool, DomainError> {
        let sql = format!("UPDATE {} SET audit_status = ? WHERE id = ?", table.as_str());
        let result = sqlx::query(AssertSqlSafe(sql))
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(result.rows_affected() > 0)
    }

    async fn get_audit_status(&self, table: BasicTable, id: i64) -> Result<Option<String>, DomainError> {
        let sql = format!("SELECT audit_status FROM {} WHERE id = ?", table.as_str());
        sqlx::query_scalar(AssertSqlSafe(sql.as_str()))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }
}
