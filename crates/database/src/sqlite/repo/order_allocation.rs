//! consumable_allocation 仓储实现：SQL 与旧 api_allocation_* handler 逐字一致
//!
//! 行为保持要点：EXISTS/SUM 查询失败由用例层吞错（unwrap_or）；
//! INSERT 的 Err 由用例层映射为"创建分摊方案失败: {原始错误}"。

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::order_allocation::{
    AllocRow, AllocSummaryRow, AllocatedOrderRow, OrderAllocationRepo,
};

#[derive(Clone)]
pub struct SqliteOrderAllocationRepo {
    pool: SqlitePool,
}

impl SqliteOrderAllocationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl OrderAllocationRepo for SqliteOrderAllocationRepo {
    async fn exists_by_source(&self, source_order_id: i64) -> Result<bool, DomainError> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM consumable_allocation WHERE source_order_id = ?)",
        )
        .bind(source_order_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)
    }

    async fn sum_item_amounts(&self, order_id: i64, item_ids: &[i64]) -> Result<f64, DomainError> {
        let placeholders = item_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT COALESCE(SUM(amount), 0.0) FROM sales_order_item WHERE order_id = ? AND id IN ({})",
            placeholders
        );
        let mut q = sqlx::query_scalar::<_, f64>(AssertSqlSafe(sql.as_str())).bind(order_id);
        for iid in item_ids {
            q = q.bind(*iid);
        }
        q.fetch_one(&self.pool).await.map_err(storage)
    }

    async fn create(
        &self,
        source_order_id: i64,
        total_amount: f64,
        remark: &str,
        source_item_ids: &str,
    ) -> Result<i64, DomainError> {
        let res = sqlx::query(
            "INSERT INTO consumable_allocation(source_order_id, total_amount, allocated_amount, remaining_balance, status, remark, created_at, source_item_ids) VALUES (?, ?, 0, ?, 0, ?, datetime('now','localtime'), ?)",
        )
        .bind(source_order_id)
        .bind(total_amount)
        .bind(total_amount)
        .bind(remark)
        .bind(source_item_ids)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.last_insert_rowid())
    }

    async fn fetch_first_by_source(
        &self,
        source_order_id: i64,
    ) -> Result<Option<AllocSummaryRow>, DomainError> {
        let row = sqlx::query(
            "SELECT * FROM consumable_allocation WHERE source_order_id = ? ORDER BY id ASC LIMIT 1",
        )
        .bind(source_order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.map(|r| AllocSummaryRow {
            id: r.get("id"),
            source_order_id: r.get("source_order_id"),
            total_amount: r.get("total_amount"),
            allocated_amount: r.get("allocated_amount"),
            remaining_balance: r.get("remaining_balance"),
            status: r.get("status"),
            remark: r.get("remark"),
            created_at: r.get("created_at"),
            completed_at: r.get("completed_at"),
            source_item_ids: r.get("source_item_ids"),
        }))
    }

    async fn list_allocated_orders(&self) -> Result<Vec<AllocatedOrderRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT ca.id as alloc_id, ca.source_order_id, ca.total_amount, ca.allocated_amount, ca.remaining_balance,
                    ca.status, ca.remark, ca.created_at, ca.completed_at,
                    so.order_no, so.order_date, so.purchaser_id,
                    p.name as purchaser_name
             FROM consumable_allocation ca
             LEFT JOIN sales_order so ON ca.source_order_id = so.id
             LEFT JOIN purchaser p ON so.purchaser_id = p.id
             ORDER BY ca.created_at DESC, ca.id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| AllocatedOrderRow {
                alloc_id: row.get("alloc_id"),
                source_order_id: row.get("source_order_id"),
                order_no: row.get("order_no"),
                order_date: row.get("order_date"),
                purchaser_id: row.get("purchaser_id"),
                purchaser_name: row.get("purchaser_name"),
                total_amount: row.get("total_amount"),
                allocated_amount: row.get("allocated_amount"),
                remaining_balance: row.get("remaining_balance"),
                status: row.get("status"),
                remark: row.get("remark"),
                created_at: row.get("created_at"),
                completed_at: row.get("completed_at"),
            })
            .collect())
    }

    async fn terminate(&self, source_order_id: i64, remark: &str) -> Result<u64, DomainError> {
        let res = sqlx::query(
            "UPDATE consumable_allocation SET status = 3, remark = ?, completed_at = datetime('now','localtime') WHERE source_order_id = ? AND status != 2",
        )
        .bind(remark)
        .bind(source_order_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.rows_affected())
    }

    async fn fetch_by_source(&self, source_order_id: i64) -> Result<Option<AllocRow>, DomainError> {
        let row = sqlx::query(
            "SELECT id, allocated_amount, remaining_balance, total_amount, status FROM consumable_allocation WHERE source_order_id = ?",
        )
        .bind(source_order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.map(|r| AllocRow {
            id: r.get("id"),
            allocated_amount: r.get("allocated_amount"),
            remaining_balance: r.get("remaining_balance"),
            total_amount: r.get("total_amount"),
            status: r.get("status"),
        }))
    }

    async fn delete_by_source(&self, source_order_id: i64) -> Result<u64, DomainError> {
        let res = sqlx::query(
            "DELETE FROM consumable_allocation WHERE source_order_id = ? AND allocated_amount <= 0.0001 AND status != 2",
        )
        .bind(source_order_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.rows_affected())
    }

    async fn target_order_exists(&self, target_order_id: i64) -> Result<bool, DomainError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sales_order WHERE id = ?)")
            .bind(target_order_id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    async fn insert_tail_item(
        &self,
        target_order_id: i64,
        source_order_id: i64,
        source_remark: &str,
        amount: f64,
        today: &str,
    ) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO order_supplement_item(target_order_id, source_order_id, source_remark, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, amount, allocate_date, operation_type, target_order_item_id) VALUES (?, ?, ?, ?, ?, '', '', '', '次', 0, 1, ?, ?, 'new_item', NULL)",
        )
        .bind(target_order_id)
        .bind(source_order_id)
        .bind(source_remark)
        .bind(-1)
        .bind("分摊尾差")
        .bind(amount)
        .bind(amount)
        .bind(today)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn mark_completed(&self, alloc_id: i64) -> Result<u64, DomainError> {
        let res = sqlx::query(
            "UPDATE consumable_allocation SET status = 2, completed_at = datetime('now','localtime'), remaining_balance = 0 WHERE id = ?",
        )
        .bind(alloc_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.rows_affected())
    }
}
