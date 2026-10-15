//! 系统管理仓储实现：SQL 与旧 api_system_config* / api_backup* / api_restore* /
//! api_operation_log_* / api_inspect_corrupted_items / api_clean_* handler 逐字一致
//!
//! 行为保持：吞错（unwrap_or_default/.ok()/let _ =）由用例层执行；
//! COUNT 查询的 unwrap panic 语义由用例层保留；
//! VACUUM INTO 的文件路径由用例层传入（单引号拼接 + AssertSqlSafe 与旧代码一致）。

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::system_mgmt::{
    BackupRecordRow, CorruptedItemRow, OperationLogFilter, OperationLogRow, SystemMgmtRepo,
};

#[derive(Clone)]
pub struct SqliteSystemMgmtRepo {
    pool: SqlitePool,
}

impl SqliteSystemMgmtRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

/// 操作日志动态 WHERE 与绑定值（SQL 片段与旧 api_operation_log_list/export 逐字一致）
fn build_operation_log_where(filter: &OperationLogFilter) -> (String, Vec<String>) {
    let mut where_sql = String::from(" WHERE 1=1");
    let mut binds: Vec<String> = Vec::new();
    if !filter.username.is_empty() {
        where_sql.push_str(" AND username LIKE ?");
        binds.push(format!("%{}%", filter.username));
    }
    if !filter.action.is_empty() {
        where_sql.push_str(" AND action LIKE ?");
        binds.push(format!("%{}%", filter.action));
    }
    if !filter.start_date.is_empty() {
        where_sql.push_str(" AND date(created_at) >= ?");
        binds.push(filter.start_date.clone());
    }
    if !filter.end_date.is_empty() {
        where_sql.push_str(" AND date(created_at) <= ?");
        binds.push(filter.end_date.clone());
    }
    (where_sql, binds)
}

fn operation_log_row(r: &sqlx::sqlite::SqliteRow) -> OperationLogRow {
    OperationLogRow {
        id: r.get("id"),
        user_id: r.get("user_id"),
        username: r.get("username"),
        action: r.get("action"),
        target_type: r.get("target_type"),
        target_id: r.get("target_id"),
        detail: r.get("detail"),
        created_at: r.get("created_at"),
    }
}

impl SystemMgmtRepo for SqliteSystemMgmtRepo {
    async fn upsert_system_config(&self, key: &str, value: &str) -> Result<(), DomainError> {
        sqlx::query("INSERT OR REPLACE INTO system_config (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn list_system_config(&self) -> Result<Vec<(String, String)>, DomainError> {
        let rows = sqlx::query("SELECT key, value FROM system_config")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| {
                let key: String = row.get("key");
                let value: String = row.get("value");
                (key, value)
            })
            .collect())
    }

    async fn list_backup_records(&self) -> Result<Vec<BackupRecordRow>, DomainError> {
        let rows = sqlx::query("SELECT id, backup_time, file_name, size FROM backup_record ORDER BY backup_time DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| BackupRecordRow {
                id: r.get::<i64, _>("id"),
                backup_time: r.get::<String, _>("backup_time"),
                file_name: r.get::<String, _>("file_name"),
                size: r.get::<i64, _>("size"),
            })
            .collect())
    }

    async fn insert_backup_record(&self, backup_time: &str, file_name: &str, size: i64) -> Result<(), DomainError> {
        sqlx::query("INSERT INTO backup_record (backup_time, file_name, size) VALUES (?, ?, ?)")
            .bind(backup_time)
            .bind(file_name)
            .bind(size)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn get_backup_file_name(&self, id: i64) -> Result<Option<String>, DomainError> {
        let row = sqlx::query("SELECT file_name FROM backup_record WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| r.get::<String, _>("file_name")))
    }

    async fn delete_backup_record(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM backup_record WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn vacuum_into(&self, path: &str) -> Result<(), DomainError> {
        let vacuum_sql = format!("VACUUM INTO '{}'", path);
        sqlx::query(AssertSqlSafe(vacuum_sql.as_str()))
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn vacuum(&self) -> Result<(), DomainError> {
        sqlx::query("VACUUM").execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }

    async fn count_operation_logs(&self, filter: &OperationLogFilter) -> Result<i64, DomainError> {
        let (where_sql, binds) = build_operation_log_where(filter);
        let count_sql = format!("SELECT COUNT(*) as count FROM operation_log{}", where_sql);
        let mut count_q = sqlx::query(AssertSqlSafe(count_sql.as_str()));
        for b in &binds {
            count_q = count_q.bind(b);
        }
        let total_rows = count_q.fetch_one(&self.pool).await.map_err(storage)?;
        let total: i64 = total_rows.get("count");
        Ok(total)
    }

    async fn list_operation_logs(&self, filter: &OperationLogFilter, page_size: i64, offset: i64) -> Result<Vec<OperationLogRow>, DomainError> {
        let (where_sql, binds) = build_operation_log_where(filter);
        let data_sql = format!(
            "SELECT id, user_id, username, action, target_type, target_id, detail, created_at FROM operation_log{} ORDER BY id DESC LIMIT ? OFFSET ?",
            where_sql
        );
        let mut q = sqlx::query(AssertSqlSafe(data_sql.as_str()));
        for b in &binds {
            q = q.bind(b);
        }
        q = q.bind(page_size).bind(offset);
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(operation_log_row).collect())
    }

    async fn list_operation_logs_export(&self, filter: &OperationLogFilter) -> Result<Vec<OperationLogRow>, DomainError> {
        let (where_sql, binds) = build_operation_log_where(filter);
        let sql = format!(
            "SELECT id, user_id, username, action, target_type, target_id, detail, created_at FROM operation_log{} ORDER BY id DESC",
            where_sql
        );
        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        for b in &binds {
            q = q.bind(b);
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(operation_log_row).collect())
    }

    async fn list_corrupted_sales_items(&self) -> Result<Vec<CorruptedItemRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, order_id, product_id, product_name, unit, unit_price, quantity, amount FROM sales_order_item WHERE (unit_price = 0 OR quantity = 0 OR amount = 0) AND product_name != '' LIMIT 100"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| CorruptedItemRow {
                id: r.get::<i64, _>("id"),
                order_id: r.get::<i64, _>("order_id"),
                product_id: r.get::<i64, _>("product_id"),
                product_name: r.get::<Option<String>, _>("product_name"),
                unit: r.get::<Option<String>, _>("unit"),
                unit_price: r.get::<Option<f64>, _>("unit_price"),
                quantity: r.get::<Option<f64>, _>("quantity"),
                amount: r.get::<f64, _>("amount"),
            })
            .collect())
    }

    async fn list_corrupted_item_ids(&self) -> Result<Vec<i64>, DomainError> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM sales_order_item WHERE id >= 5527 AND id <= 5619"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(ids)
    }

    async fn list_sales_orders_without_items(&self) -> Result<Vec<i64>, DomainError> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT so.id FROM sales_order so LEFT JOIN sales_order_item soi ON so.id = soi.order_id WHERE soi.id IS NULL"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(ids)
    }

    async fn list_purchase_orders_without_items(&self) -> Result<Vec<i64>, DomainError> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT po.id FROM purchase_order po LEFT JOIN purchase_order_item poi ON po.id = poi.order_id WHERE poi.id IS NULL"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(ids)
    }

    async fn list_sales_orders_with_invalid_purchaser(&self) -> Result<Vec<i64>, DomainError> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT so.id FROM sales_order so LEFT JOIN purchaser p ON so.purchaser_id = p.id WHERE p.id IS NULL"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(ids)
    }

    async fn list_purchase_orders_with_invalid_supplier(&self) -> Result<Vec<i64>, DomainError> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT po.id FROM purchase_order po LEFT JOIN supplier s ON po.supplier_id = s.id WHERE s.id IS NULL"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(ids)
    }

    async fn list_sales_items_with_invalid_product(&self) -> Result<Vec<i64>, DomainError> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT soi.id FROM sales_order_item soi LEFT JOIN product p ON soi.product_id = p.id WHERE p.id IS NULL"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(ids)
    }

    async fn list_purchase_items_with_invalid_product(&self) -> Result<Vec<i64>, DomainError> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT poi.id FROM purchase_order_item poi LEFT JOIN product p ON poi.product_id = p.id WHERE p.id IS NULL"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(ids)
    }

    async fn delete_sales_order_item(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM sales_order_item WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_sales_order(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM sales_order WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_purchase_order(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM purchase_order WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_sales_order_items_by_order(&self, order_id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM sales_order_item WHERE order_id = ?")
            .bind(order_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_purchase_order_items_by_order(&self, order_id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM purchase_order_item WHERE order_id = ?")
            .bind(order_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_purchase_order_item(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM purchase_order_item WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
}
