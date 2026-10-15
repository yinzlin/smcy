//! order_supplement_item 仓储实现：SQL 与旧 api_supplement_* / api_adjusted_orders handler 逐字一致
//!
//! 行为保持要点：分摊余额 UPDATE 序列的失败由用例层吞错（let _ =）；
//! count/sum/list 调整订单查询失败同样由用例层吞错兜底。

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::order_supplement::{
    AdjustedOrderRow, AdjustedOrdersFilter, CompareItemRow, CompareSupplementRow,
    OrderSupplementInput, OrderSupplementRepo, SupplementBySourceRow, SupplementByTargetRow,
    SupplementDeleteRow,
};

#[derive(Clone)]
pub struct SqliteOrderSupplementRepo {
    pool: SqlitePool,
}

impl SqliteOrderSupplementRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

/// 构造动态筛选条件与绑定值（与旧 handler 逐字一致）
fn build_cond_sql(filter: &AdjustedOrdersFilter) -> (String, Vec<String>, Vec<i64>) {
    let mut conds: Vec<String> = Vec::new();
    let mut bind_kw: Vec<String> = Vec::new();
    let mut bind_pid: Vec<i64> = Vec::new();
    if let Some(kw) = &filter.keyword_like {
        bind_kw.push(kw.clone());
        conds.push("so.order_no LIKE ?".to_string());
    }
    if let Some(pid) = filter.purchaser_id {
        bind_pid.push(pid);
        conds.push("so.purchaser_id = ?".to_string());
    }
    let cond_sql = if conds.is_empty() {
        String::new()
    } else {
        format!(" AND {}", conds.join(" AND "))
    };
    (cond_sql, bind_kw, bind_pid)
}

impl OrderSupplementRepo for SqliteOrderSupplementRepo {
    async fn create(&self, req: &OrderSupplementInput) -> Result<i64, DomainError> {
        let res = sqlx::query(
            "INSERT INTO order_supplement_item(target_order_id, source_order_id, source_remark, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, amount, allocate_date, operation_type, target_order_item_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(req.target_order_id)
        .bind(req.source_order_id)
        .bind(&req.source_remark)
        .bind(req.product_id)
        .bind(&req.product_name)
        .bind(&req.alias1)
        .bind(&req.alias2)
        .bind(&req.spec)
        .bind(&req.unit)
        .bind(req.unit_price)
        .bind(req.quantity)
        .bind(req.amount)
        .bind(&req.allocate_date)
        .bind(&req.operation_type)
        .bind(req.target_order_item_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.last_insert_rowid())
    }

    async fn apply_pool_delta(&self, source_order_id: i64, pool_delta: f64) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE consumable_allocation SET allocated_amount = allocated_amount + ?, remaining_balance = remaining_balance - ?, status = CASE WHEN remaining_balance - ? <= 0 THEN 2 ELSE 1 END WHERE source_order_id = ?",
        )
        .bind(pool_delta)
        .bind(pool_delta)
        .bind(pool_delta)
        .bind(source_order_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn touch_completed_at(&self, source_order_id: i64) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE consumable_allocation SET completed_at = datetime('now','localtime') WHERE source_order_id = ? AND status = 2 AND completed_at IS NULL",
        )
        .bind(source_order_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn list_by_target(&self, order_id: i64) -> Result<Vec<SupplementByTargetRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT soi.id, soi.target_order_id, soi.source_order_id, soi.source_remark, soi.product_id, soi.product_name, soi.alias1, soi.alias2, soi.spec, soi.unit, soi.unit_price, soi.quantity, soi.amount, soi.allocate_date, soi.operation_type, soi.target_order_item_id, so.order_no as source_order_no
             FROM order_supplement_item soi LEFT JOIN sales_order so ON soi.source_order_id = so.id
             WHERE soi.target_order_id = ? ORDER BY soi.allocate_date DESC",
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SupplementByTargetRow {
                id: row.get("id"),
                target_order_id: row.get("target_order_id"),
                source_order_id: row.get("source_order_id"),
                source_order_no: row.get("source_order_no"),
                source_remark: row.get("source_remark"),
                product_id: row.get("product_id"),
                product_name: row.get("product_name"),
                alias1: row.get("alias1"),
                alias2: row.get("alias2"),
                spec: row.get("spec"),
                unit: row.get("unit"),
                unit_price: row.get("unit_price"),
                quantity: row.get("quantity"),
                amount: row.get("amount"),
                allocate_date: row.get("allocate_date"),
                operation_type: row.get("operation_type"),
                target_order_item_id: row.get("target_order_item_id"),
            })
            .collect())
    }

    async fn list_by_source(&self, order_id: i64) -> Result<Vec<SupplementBySourceRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT soi.id, soi.target_order_id, soi.source_order_id, soi.source_remark, soi.product_id, soi.product_name, soi.alias1, soi.alias2, soi.spec, soi.unit, soi.unit_price, soi.quantity, soi.amount, soi.allocate_date, soi.operation_type, so.order_no as target_order_no
             FROM order_supplement_item soi LEFT JOIN sales_order so ON soi.target_order_id = so.id
             WHERE soi.source_order_id = ? ORDER BY soi.allocate_date DESC",
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SupplementBySourceRow {
                id: row.get("id"),
                target_order_id: row.get("target_order_id"),
                target_order_no: row.get("target_order_no"),
                source_order_id: row.get("source_order_id"),
                source_remark: row.get("source_remark"),
                product_id: row.get("product_id"),
                product_name: row.get("product_name"),
                alias1: row.get("alias1"),
                alias2: row.get("alias2"),
                spec: row.get("spec"),
                unit: row.get("unit"),
                unit_price: row.get("unit_price"),
                quantity: row.get("quantity"),
                amount: row.get("amount"),
                allocate_date: row.get("allocate_date"),
                operation_type: row.get("operation_type"),
            })
            .collect())
    }

    async fn fetch_for_delete(&self, id: i64) -> Result<Option<SupplementDeleteRow>, DomainError> {
        let row = sqlx::query(
            "SELECT source_order_id, amount, operation_type FROM order_supplement_item WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.map(|r| SupplementDeleteRow {
            source_order_id: r.get("source_order_id"),
            amount: r.get("amount"),
            operation_type: r.get("operation_type"),
        }))
    }

    async fn delete(&self, id: i64) -> Result<u64, DomainError> {
        let res = sqlx::query("DELETE FROM order_supplement_item WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(res.rows_affected())
    }

    async fn rollback_pool(&self, source_order_id: i64, pool_delta: f64) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE consumable_allocation SET allocated_amount = allocated_amount - ?, remaining_balance = remaining_balance + ?, status = CASE WHEN remaining_balance + ? < total_amount THEN 1 ELSE 0 END WHERE source_order_id = ? AND status != 3",
        )
        .bind(pool_delta)
        .bind(pool_delta)
        .bind(pool_delta)
        .bind(source_order_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn clear_completed_at(&self, source_order_id: i64) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE consumable_allocation SET completed_at = NULL WHERE source_order_id = ? AND status = 2 AND completed_at IS NOT NULL",
        )
        .bind(source_order_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn delete_if_zero(&self, source_order_id: i64) -> Result<(), DomainError> {
        sqlx::query(
            "DELETE FROM consumable_allocation WHERE source_order_id = ? AND status != 3 AND allocated_amount <= 0.0001",
        )
        .bind(source_order_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn list_compare_items(&self, order_id: i64) -> Result<Vec<CompareItemRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT soi.id, soi.product_id, soi.product_name, soi.alias1, soi.alias2, soi.spec, soi.unit, soi.unit_price, soi.quantity, soi.amount, soi.remark,
                    p.category_id, pc.name as category_name, pc.parent_id, pc2.name as parent_name
             FROM sales_order_item soi LEFT JOIN product p ON soi.product_id = p.id
             LEFT JOIN category pc ON p.category_id = pc.id
             LEFT JOIN category pc2 ON pc.parent_id = pc2.id
             WHERE soi.order_id = ?",
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| CompareItemRow {
                id: r.get("id"),
                product_id: r.get("product_id"),
                product_name: r.get("product_name"),
                alias1: r.get("alias1"),
                alias2: r.get("alias2"),
                spec: r.get("spec"),
                unit: r.get("unit"),
                unit_price: r.get("unit_price"),
                quantity: r.get("quantity"),
                amount: r.get("amount"),
                remark: r.get("remark"),
                category_name: r.get("category_name"),
                parent_name: r.get("parent_name"),
            })
            .collect())
    }

    async fn list_compare_supplements(
        &self,
        order_id: i64,
    ) -> Result<Vec<CompareSupplementRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, target_order_id, source_order_id, source_remark, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, amount, allocate_date, operation_type, target_order_item_id
             FROM order_supplement_item WHERE target_order_id = ?",
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| CompareSupplementRow {
                id: r.get("id"),
                target_order_id: r.get("target_order_id"),
                source_order_id: r.get("source_order_id"),
                source_remark: r.get("source_remark"),
                product_id: r.get("product_id"),
                product_name: r.get("product_name"),
                alias1: r.get("alias1"),
                alias2: r.get("alias2"),
                spec: r.get("spec"),
                unit: r.get("unit"),
                unit_price: r.get("unit_price"),
                quantity: r.get("quantity"),
                amount: r.get("amount"),
                allocate_date: r.get("allocate_date"),
                operation_type: r.get("operation_type"),
                target_order_item_id: r.get("target_order_item_id"),
            })
            .collect())
    }

    async fn count_adjusted(&self, filter: &AdjustedOrdersFilter) -> Result<i64, DomainError> {
        let (cond_sql, bind_kw, bind_pid) = build_cond_sql(filter);
        let count_sql = format!(
            "SELECT COUNT(*) FROM (
                SELECT so.id FROM sales_order so
                INNER JOIN order_supplement_item osi ON osi.target_order_id = so.id
                WHERE 1=1{} GROUP BY so.id
            ) t", cond_sql
        );
        let mut count_q = sqlx::query(AssertSqlSafe(count_sql.as_str()));
        for kw in &bind_kw {
            count_q = count_q.bind(kw);
        }
        for pid in &bind_pid {
            count_q = count_q.bind(pid);
        }
        count_q
            .fetch_one(&self.pool)
            .await
            .map(|r| r.get::<i64, _>(0))
            .map_err(storage)
    }

    async fn sum_adjusted(&self, filter: &AdjustedOrdersFilter) -> Result<(f64, f64), DomainError> {
        let (cond_sql, bind_kw, bind_pid) = build_cond_sql(filter);
        let sum_sql = format!(
            "SELECT COALESCE(SUM(sub.total_amount), 0.0), COALESCE(SUM(sub.adjust_amount), 0.0)
             FROM (
                SELECT so.id, so.total_amount, COALESCE(SUM(osi.amount), 0.0) as adjust_amount
                FROM sales_order so
                INNER JOIN order_supplement_item osi ON osi.target_order_id = so.id
                WHERE 1=1{} GROUP BY so.id, so.total_amount
             ) sub", cond_sql
        );
        let mut sum_q = sqlx::query(AssertSqlSafe(sum_sql.as_str()));
        for kw in &bind_kw {
            sum_q = sum_q.bind(kw);
        }
        for pid in &bind_pid {
            sum_q = sum_q.bind(pid);
        }
        let r = sum_q.fetch_one(&self.pool).await.map_err(storage)?;
        Ok((r.get::<f64, _>(0), r.get::<f64, _>(1)))
    }

    async fn list_adjusted(&self, filter: &AdjustedOrdersFilter) -> Result<Vec<AdjustedOrderRow>, DomainError> {
        let (cond_sql, bind_kw, bind_pid) = build_cond_sql(filter);
        // 排序：点击"订单日期"列头时按订单日期升/降序，否则默认按最近调整日
        let sort_sql = if filter.sort_order == "asc" || filter.sort_order == "desc" {
            format!(
                "ORDER BY so.order_date {}, MAX(osi.allocate_date) DESC, so.order_no DESC",
                filter.sort_order
            )
        } else {
            "ORDER BY MAX(osi.allocate_date) DESC, so.order_no DESC".to_string()
        };
        let offset = (filter.page - 1) * filter.page_size;
        let list_sql = format!(
            "SELECT so.id, so.order_no, so.order_date, so.total_amount,
                    p.name as purchaser_name, so.purchaser_id,
                    COALESCE(SUM(osi.amount), 0.0) as adjust_amount,
                    COUNT(osi.id) as adjust_count,
                    MAX(osi.allocate_date) as last_adjust_date
             FROM sales_order so
             INNER JOIN order_supplement_item osi ON osi.target_order_id = so.id
             LEFT JOIN purchaser p ON so.purchaser_id = p.id
             WHERE 1=1{}
             GROUP BY so.id, so.order_no, so.order_date, so.total_amount, p.name, so.purchaser_id
             {}
             LIMIT ? OFFSET ?", cond_sql, sort_sql
        );
        let mut list_q = sqlx::query(AssertSqlSafe(list_sql.as_str()));
        for kw in &bind_kw {
            list_q = list_q.bind(kw);
        }
        for pid in &bind_pid {
            list_q = list_q.bind(pid);
        }
        let rows = list_q
            .bind(filter.page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| AdjustedOrderRow {
                id: row.get("id"),
                order_no: row.get("order_no"),
                order_date: row.get("order_date"),
                purchaser_name: row.get("purchaser_name"),
                purchaser_id: row.get("purchaser_id"),
                total_amount: row.get("total_amount"),
                adjust_amount: row.get("adjust_amount"),
                adjust_count: row.get("adjust_count"),
                last_adjust_date: row.get("last_adjust_date"),
            })
            .collect())
    }
}
