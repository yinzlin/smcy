//! 操作审计日志仓储实现：SQL 与旧 auth::log_operation 逐字一致
//! （内部吞错：日志失败不影响业务结果）

use sqlx::sqlite::SqlitePool;

use domain::port::oplog::OperationLogRepo;
use domain::UserCtx;

#[derive(Clone)]
pub struct SqliteOperationLogRepo {
    pool: SqlitePool,
}

impl SqliteOperationLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl OperationLogRepo for SqliteOperationLogRepo {
    async fn log(&self, user: &UserCtx, action: &str, target_type: &str, target_id: &str, detail: &str) {
        let username: String = sqlx::query_scalar("SELECT COALESCE(username,'') FROM user_account WHERE id = ?")
            .bind(user.user_id)
            .fetch_one(&self.pool)
            .await
            .unwrap_or_default();
        let _ = sqlx::query(
            "INSERT INTO operation_log(user_id, username, action, target_type, target_id, detail) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(user.user_id)
        .bind(&username)
        .bind(action)
        .bind(target_type)
        .bind(target_id)
        .bind(detail)
        .execute(&self.pool)
        .await;
    }
}
