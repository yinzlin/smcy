//! SQLite 销售订单仓储实现
//!
//! SQL 与旧 api_sales_order_* handler 逐字一致；乐观锁更新 / 状态流转（FIFO 批次
//! 快照 + 库存流水）/ 反审核冲销的多步事务序列在实现内部 begin/commit，错误文案
//! 按旧 handler 逐字预置。

use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::sales_order::*;
use domain::{SalesOrderItemReq, SalesOrderReq};

use super::stock_writer;

#[derive(Clone)]
pub struct SqliteSalesOrderRepo {
    pool: sqlx::SqlitePool,
}

impl SqliteSalesOrderRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

/// 明细批量 INSERT（创建走 pool / 更新走事务共用拼装）：15 列，数量兜底已在用例层完成
fn build_items_insert(items: &[SalesOrderItemReq]) -> String {
    let placeholders: Vec<String> = items.iter()
        .map(|_| "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)".to_string())
        .collect();
    format!(
        "INSERT INTO sales_order_item(order_id, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, base_quantity, amount, pre_sale_quantity, supplier_id, supplier_name, remark) VALUES {}",
        placeholders.join(", ")
    )
}

fn bind_items<'a>(
    mut query: sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>,
    order_id: Option<i64>,
    items: &'a [SalesOrderItemReq],
    base_qtys: &[f64],
) -> sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    for (idx, item) in items.iter().enumerate() {
        query = query
            .bind(order_id)
            .bind(item.product_id)
            .bind(&item.product_name)
            .bind(&item.alias1)
            .bind(&item.alias2)
            .bind(&item.spec)
            .bind(&item.unit)
            .bind(item.unit_price)
            .bind(item.quantity)
            .bind(base_qtys[idx])
            .bind(item.amount)
            .bind(item.pre_sale_quantity.unwrap_or(0.0))
            .bind(item.supplier_id)
            .bind(&item.supplier_name)
            .bind(&item.remark);
    }
    query
}

impl SalesOrderRepo for SqliteSalesOrderRepo {
    async fn get_purchaser_status(&self, id: i64) -> Result<Option<(i64, String)>, DomainError> {
        sqlx::query_as::<_, (i64, String)>("SELECT purchaser_id, status FROM sales_order WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }

    async fn get_detail_purchaser_id(&self, id: i64) -> Option<i64> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT purchaser_id FROM sales_order WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .unwrap_or(None);
        row.map(|(pid,)| pid)
    }

    async fn purchaser_audit_status(&self, id: i64) -> Result<Option<String>, DomainError> {
        sqlx::query_scalar::<_, String>("SELECT audit_status FROM purchaser WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }

    async fn product_audit_status(&self, id: i64) -> Result<Option<String>, DomainError> {
        sqlx::query_scalar::<_, String>("SELECT audit_status FROM product WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }

    async fn get_order_detail(&self, id: i64) -> Result<Option<SalesOrderDetailRow>, DomainError> {
        let row = sqlx::query(
            "SELECT so.id, so.purchaser_id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.amount_reduction, so.final_amount, so.status, so.version, so.remark, so.warehouse_id, so.warehouse_name, so.customer_order_image, so.signed_order_image, so.supplier_company, so.truck_plate, so.is_settled, COALESCE(p.name, '') as purchaser_name
             FROM sales_order so LEFT JOIN purchaser p ON so.purchaser_id = p.id WHERE so.id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.map(|r| SalesOrderDetailRow {
            id: r.get("id"),
            purchaser_id: r.get("purchaser_id"),
            order_no: r.get("order_no"),
            order_date: r.get("order_date"),
            total_amount: r.get("total_amount"),
            discount_rate: r.get("discount_rate"),
            amount_reduction: r.get("amount_reduction"),
            final_amount: r.get("final_amount"),
            status: r.get("status"),
            version: r.get("version"),
            remark: r.get("remark"),
            warehouse_id: r.get("warehouse_id"),
            warehouse_name: r.get("warehouse_name"),
            customer_order_image: r.get("customer_order_image"),
            signed_order_image: r.get("signed_order_image"),
            supplier_company: r.get("supplier_company"),
            truck_plate: r.get("truck_plate"),
            is_settled: r.get("is_settled"),
            purchaser_name: r.get("purchaser_name"),
        }))
    }

    async fn list_items(&self, order_id: i64) -> Result<Vec<SalesOrderItemRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, base_quantity, amount, pre_sale_quantity, supplier_id, supplier_name, production_date, batch_no, remark FROM sales_order_item WHERE order_id = ?"
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows.iter().map(|r| SalesOrderItemRow {
            id: r.get("id"),
            product_id: r.get("product_id"),
            product_name: r.get("product_name"),
            alias1: r.get("alias1"),
            alias2: r.get("alias2"),
            spec: r.get("spec"),
            unit: r.get("unit"),
            unit_price: r.get("unit_price"),
            quantity: r.get("quantity"),
            base_quantity: r.get("base_quantity"),
            amount: r.get("amount"),
            pre_sale_quantity: r.get("pre_sale_quantity"),
            supplier_id: r.get("supplier_id"),
            supplier_name: r.get("supplier_name"),
            production_date: r.get("production_date"),
            batch_no: r.get("batch_no"),
            remark: r.get("remark"),
        }).collect())
    }

    async fn count_list(&self, filter: &SalesOrderListFilter) -> Result<i64, DomainError> {
        let keyword_pattern = &filter.keyword_pattern;
        let (total_sql, total_params) = match (filter.purchaser_id, filter.is_settled.as_ref()) {
            (Some(pid), Some(isf)) => (
                "SELECT COUNT(*) as count FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
                 WHERE so.purchaser_id = ? AND so.is_settled = ? AND (so.order_no LIKE ? OR p.name LIKE ? OR so.order_date LIKE ?)",
                vec![pid.to_string(), isf.clone(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (Some(pid), None) => (
                "SELECT COUNT(*) as count FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
                 WHERE so.purchaser_id = ? AND (so.order_no LIKE ? OR p.name LIKE ? OR so.order_date LIKE ?)",
                vec![pid.to_string(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (None, Some(isf)) => (
                "SELECT COUNT(*) as count FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
                 WHERE so.is_settled = ? AND (so.order_no LIKE ? OR p.name LIKE ? OR so.order_date LIKE ?)",
                vec![isf.clone(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (None, None) => (
                "SELECT COUNT(*) as count FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
                 WHERE so.order_no LIKE ? OR p.name LIKE ? OR so.order_date LIKE ?",
                vec![keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
        };

        let mut total_query = sqlx::query(total_sql);
        for p in &total_params {
            total_query = total_query.bind(p);
        }
        let total_rows = total_query.fetch_one(&self.pool).await.map_err(storage)?;
        Ok(total_rows.get("count"))
    }

    async fn list(&self, filter: &SalesOrderListFilter, order_clause: &str) -> Result<Vec<SalesOrderListRow>, DomainError> {
        let keyword_pattern = &filter.keyword_pattern;
        let offset = (filter.page - 1) * filter.page_size;
        let (sql, query_params) = match (filter.purchaser_id, filter.is_settled.as_ref()) {
            (Some(pid), Some(isf)) => (
                format!(
                    "SELECT so.id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.amount_reduction, so.final_amount, so.status, so.remark, so.warehouse_id, so.warehouse_name, so.is_settled, p.name as purchaser_name,
                            (SELECT COUNT(*) FROM order_supplement_item osi WHERE osi.target_order_id = so.id) as supplement_count
                     FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
                     WHERE so.purchaser_id = ? AND so.is_settled = ? AND (so.order_no LIKE ? OR p.name LIKE ? OR so.order_date LIKE ?)
                     ORDER BY {} LIMIT ? OFFSET ?",
                    order_clause
                ),
                vec![pid.to_string(), isf.clone(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (Some(pid), None) => (
                format!(
                    "SELECT so.id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.amount_reduction, so.final_amount, so.status, so.remark, so.warehouse_id, so.warehouse_name, so.is_settled, p.name as purchaser_name,
                            (SELECT COUNT(*) FROM order_supplement_item osi WHERE osi.target_order_id = so.id) as supplement_count
                     FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
                     WHERE so.purchaser_id = ? AND (so.order_no LIKE ? OR p.name LIKE ? OR so.order_date LIKE ?)
                     ORDER BY {} LIMIT ? OFFSET ?",
                    order_clause
                ),
                vec![pid.to_string(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (None, Some(isf)) => (
                format!(
                    "SELECT so.id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.amount_reduction, so.final_amount, so.status, so.remark, so.warehouse_id, so.warehouse_name, so.is_settled, p.name as purchaser_name,
                            (SELECT COUNT(*) FROM order_supplement_item osi WHERE osi.target_order_id = so.id) as supplement_count
                     FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
                     WHERE so.is_settled = ? AND (so.order_no LIKE ? OR p.name LIKE ? OR so.order_date LIKE ?)
                     ORDER BY {} LIMIT ? OFFSET ?",
                    order_clause
                ),
                vec![isf.clone(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (None, None) => (
                format!(
                    "SELECT so.id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.amount_reduction, so.final_amount, so.status, so.remark, so.warehouse_id, so.warehouse_name, so.is_settled, p.name as purchaser_name,
                            (SELECT COUNT(*) FROM order_supplement_item osi WHERE osi.target_order_id = so.id) as supplement_count
                     FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
                     WHERE so.order_no LIKE ? OR p.name LIKE ? OR so.order_date LIKE ?
                     ORDER BY {} LIMIT ? OFFSET ?",
                    order_clause
                ),
                vec![keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
        };

        let mut query = sqlx::query(AssertSqlSafe(sql.as_str()));
        for p in &query_params {
            query = query.bind(p);
        }
        query = query.bind(filter.page_size).bind(offset);
        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(|row| SalesOrderListRow {
            id: row.get("id"),
            order_no: row.get("order_no"),
            order_date: row.get("order_date"),
            total_amount: row.get("total_amount"),
            discount_rate: row.get("discount_rate"),
            amount_reduction: row.get("amount_reduction"),
            final_amount: row.get("final_amount"),
            status: row.get("status"),
            remark: row.get("remark"),
            warehouse_id: row.get("warehouse_id"),
            warehouse_name: row.get("warehouse_name"),
            is_settled: row.get("is_settled"),
            purchaser_name: row.get("purchaser_name"),
            supplement_count: row.get("supplement_count"),
        }).collect())
    }

    async fn list_by_purchaser(&self, purchaser_id: i64) -> Result<Vec<SalesOrderByPurchaserRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT so.id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.final_amount as discount_total, so.status,
                    p.name as purchaser_name, so.purchaser_id
             FROM sales_order so LEFT JOIN purchaser p ON so.purchaser_id = p.id
             WHERE so.purchaser_id = ? ORDER BY so.order_date DESC"
        )
        .bind(purchaser_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows.iter().map(|row| SalesOrderByPurchaserRow {
            id: row.get("id"),
            order_no: row.get("order_no"),
            order_date: row.get("order_date"),
            total_amount: row.get("total_amount"),
            discount_rate: row.get("discount_rate"),
            discount_total: row.get("discount_total"),
            status: row.get("status"),
            purchaser_name: row.get("purchaser_name"),
            purchaser_id: row.get("purchaser_id"),
        }).collect())
    }

    async fn allocation_status_rows(&self, order_ids: &[i64]) -> Result<Vec<(i64, i64)>, DomainError> {
        if order_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = order_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT source_order_id, status FROM consumable_allocation WHERE source_order_id IN ({}) ORDER BY id ASC",
            placeholders
        );
        let mut query = sqlx::query(AssertSqlSafe(sql.as_str()));
        for id in order_ids {
            query = query.bind(*id);
        }
        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(|row| (row.get::<i64, _>("source_order_id"), row.get::<i64, _>("status"))).collect())
    }

    async fn insert_order(&self, req: &SalesOrderReq) -> Result<i64, DomainError> {
        let res = sqlx::query(
            "INSERT INTO sales_order(purchaser_id, order_no, order_date, total_amount, discount_rate, amount_reduction, final_amount, warehouse_id, warehouse_name, remark, supplier_company, truck_plate, is_settled) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(req.purchaser_id)
        .bind(&req.order_no)
        .bind(&req.order_date)
        .bind(req.total_amount)
        .bind(req.discount_rate)
        .bind(req.amount_reduction)
        .bind(req.final_amount)
        .bind(req.warehouse_id)
        .bind(&req.warehouse_name)
        .bind(&req.remark)
        .bind(req.supplier_company.as_deref().unwrap_or(""))
        .bind(req.truck_plate.as_deref().unwrap_or(""))
        .bind(req.is_settled.unwrap_or(0))
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.last_insert_rowid())
    }

    async fn insert_items_batch(&self, order_id: i64, items: &[SalesOrderItemReq], base_qtys: &[f64]) -> Result<(), DomainError> {
        let sql = build_items_insert(items);
        let query = sqlx::query(AssertSqlSafe(sql.as_str()));
        let query = bind_items(query, Some(order_id), items, base_qtys);
        query.execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }

    async fn update_order_with_items(&self, req: &SalesOrderReq, ver: i64, base_qtys: &[f64]) -> Result<SalesOrderUpdateTx, DomainError> {
        // 用 BEGIN IMMEDIATE 启动写事务，立即获得 SQLite 写锁（RESERVED）。
        // 作用：把"主表 UPDATE + 明细全量重写"整体串行化，
        // 杜绝连点两次保存时两个事务都读到同一 version、都通过乐观锁、然后后提交的事务用全量 items
        // 把先提交的事务的明细覆盖/丢失。
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await
            .map_err(|_| DomainError::Storage("更新失败：事务启动失败".to_string()))?;

        // 事务内做"主表 UPDATE + version 自增"：WHERE 同时校验 id 和 version。
        let upd = sqlx::query(
            "UPDATE sales_order SET purchaser_id = ?, order_no = ?, order_date = ?, total_amount = ?, discount_rate = ?, amount_reduction = ?, final_amount = ?, warehouse_id = ?, warehouse_name = ?, remark = ?, supplier_company = ?, truck_plate = ?, is_settled = ?, version = version + 1 WHERE id = ? AND version = ?"
        )
        .bind(req.purchaser_id)
        .bind(&req.order_no)
        .bind(&req.order_date)
        .bind(req.total_amount)
        .bind(req.discount_rate)
        .bind(req.amount_reduction)
        .bind(req.final_amount)
        .bind(req.warehouse_id)
        .bind(&req.warehouse_name)
        .bind(&req.remark)
        .bind(req.supplier_company.as_deref().unwrap_or(""))
        .bind(req.truck_plate.as_deref().unwrap_or(""))
        .bind(req.is_settled.unwrap_or(0))
        .bind(req.id)
        .bind(ver)
        .execute(&mut *tx)
        .await;

        let upd_res = match upd {
            Ok(r) => r,
            Err(e) => {
                return Err(DomainError::Storage(format!("更新失败: {}", e)));
            }
        };
        if upd_res.rows_affected() == 0 {
            return Ok(SalesOrderUpdateTx::VersionConflict);
        }

        sqlx::query("DELETE FROM sales_order_item WHERE order_id = ?")
            .bind(req.id)
            .execute(&mut *tx)
            .await
            .ok();

        if !req.items.is_empty() {
            let sql = build_items_insert(&req.items);
            let query = sqlx::query(AssertSqlSafe(sql.as_str()));
            let query = bind_items(query, req.id, &req.items, base_qtys);
            let _ = query.execute(&mut *tx).await;
        }
        if tx.commit().await.is_err() {
            return Err(DomainError::Storage("更新失败：事务提交失败".to_string()));
        }
        Ok(SalesOrderUpdateTx::Committed)
    }

    async fn delete_items(&self, order_id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM sales_order_item WHERE order_id = ?")
            .bind(order_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_order(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM sales_order WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn set_settled(&self, id: i64, is_settled: i64) -> Result<u64, DomainError> {
        let res = sqlx::query("UPDATE sales_order SET is_settled = ? WHERE id = ?")
            .bind(is_settled)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(res.rows_affected())
    }

    async fn approve(&self, id: i64) -> Result<u64, DomainError> {
        // 出库时点已后移到「确认验收」，审核只做状态原子转换，不写库存流水、不动 inventory。
        let res = sqlx::query("UPDATE sales_order SET status = 'confirmed', version = version + 1 WHERE id = ? AND status = 'pending'")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(res.rows_affected())
    }

    async fn unapprove(&self, id: i64, is_super_admin: bool, was_accepted: bool) -> Result<SalesOrderUnapproveTx, DomainError> {
        let update_sql = if is_super_admin {
            "UPDATE sales_order SET status = 'pending', version = version + 1 WHERE id = ?"
        } else {
            "UPDATE sales_order SET status = 'pending', version = version + 1 WHERE id = ? AND status = 'confirmed'"
        };
        // 用 BEGIN IMMEDIATE 事务：主表 UPDATE + （仅已出库时）写冲销流水 + 回滚 inventory
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await
            .map_err(|_| DomainError::Storage("反审核失败：事务启动失败".to_string()))?;
        let result = sqlx::query(AssertSqlSafe(update_sql.to_string()))
            .bind(id)
            .execute(&mut *tx)
            .await;

        match result {
            Ok(res) if res.rows_affected() > 0 => {
                // 出库时点在验收：只有原状态为 accepted（已出库）才需写冲销（in）；
                // confirmed 等未出库状态反审核只改状态，不动库存。
                if was_accepted {
                    if let Err(e) = stock_writer::write_stock_movements_for_unaudit(
                        &mut tx, id, "sales_order_item", "out", "sales", "sales", "销售出库", "反审核冲销"
                    ).await {
                        let _ = tx.rollback().await;
                        return Err(DomainError::Storage(format!("反审核失败：写冲销流水失败 {}", e)));
                    }
                }
                match tx.commit().await {
                    Ok(_) => Ok(SalesOrderUnapproveTx::Committed),
                    Err(e) => Err(DomainError::Storage(format!("反审核失败：提交事务失败 {}", e))),
                }
            }
            _ => {
                let _ = tx.rollback().await;
                Ok(SalesOrderUnapproveTx::StateConflict)
            }
        }
    }

    async fn transition_status(&self, id: i64, current_status: &str, new_status: &str) -> Result<SalesOrderTransitionTx, DomainError> {
        // 原子状态转换 + 版本递增：WHERE 校验当前状态，防止并发重复流转。
        // 用 BEGIN IMMEDIATE 事务，在「审核/反审核」这两个真正影响库存的节点同步写流水+更新库存。
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await
            .map_err(|_| DomainError::Storage("状态更新失败：事务启动失败".to_string()))?;
        let result = sqlx::query("UPDATE sales_order SET status = ?, version = version + 1 WHERE id = ? AND status = ?")
            .bind(new_status)
            .bind(id)
            .bind(current_status)
            .execute(&mut *tx)
            .await;

        let updated = match result {
            Ok(res) => res.rows_affected() > 0,
            Err(_) => {
                let _ = tx.rollback().await;
                return Err(DomainError::Storage("状态更新失败".to_string()));
            }
        };
        if !updated {
            let _ = tx.rollback().await;
            return Ok(SalesOrderTransitionTx::StateConflict);
        }

        // 出库时点口径：库存变动发生在「确认验收」而非「审核」：
        //   进入 accepted（confirmed/delivered -> accepted）：写销售出库（out）
        //   离开 accepted（accepted -> delivered，撤销验收）  ：写冲销（in）
        // pending -> confirmed（审核）、confirmed -> pending（反审核）均不动库存。

        // FIFO 快照必须在写出库流水之前完成：流水行要携带本次出库实际消耗批次（销售明细快照）。
        // 撤销验收时快照在冲销流水之后才清空（冲销行沿用原批次，保证与原出库行配对抵消）。
        if new_status == "accepted" {
            let items: Vec<(i64, i64)> = sqlx::query_as(
                "SELECT id, COALESCE(product_id, 0) FROM sales_order_item WHERE order_id = ?"
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await
            .unwrap_or_default();
            for (item_id, product_id) in items {
                if product_id <= 0 { continue; }
                let (prod, batch) = stock_writer::fifo_earliest_batch(&self.pool, product_id).await;
                let _ = sqlx::query(
                    "UPDATE sales_order_item SET production_date = ?, batch_no = ? WHERE id = ?"
                )
                .bind(&prod)
                .bind(&batch)
                .bind(item_id)
                .execute(&mut *tx)
                .await;
            }
        }

        let mv_res: Result<(), sqlx::Error> = if new_status == "accepted" {
            stock_writer::write_stock_movements_for_audit(
                &mut tx, id, "sales_order_item", "out", "sales", "sales", "销售出库", "确认验收"
            ).await
        } else if current_status == "accepted" {
            stock_writer::write_stock_movements_for_unaudit(
                &mut tx, id, "sales_order_item", "out", "sales", "sales", "销售出库", "撤销验收冲销"
            ).await
        } else {
            Ok(())
        };
        if let Err(e) = mv_res {
            let _ = tx.rollback().await;
            return Err(DomainError::Storage(format!("状态更新失败：写库存流水失败 {}", e)));
        }

        // 撤销验收清空批次快照（重新验收会重算）
        if current_status == "accepted" && new_status != "accepted" {
            let _ = sqlx::query(
                "UPDATE sales_order_item SET production_date = NULL, batch_no = NULL WHERE order_id = ?"
            )
            .bind(id)
            .execute(&mut *tx)
            .await;
        }

        match tx.commit().await {
            Ok(_) => Ok(SalesOrderTransitionTx::Committed),
            Err(e) => Err(DomainError::Storage(format!("状态更新失败：提交事务失败 {}", e))),
        }
    }

    async fn fetch_status_or_pending(&self, id: i64) -> String {
        let status: Option<String> = sqlx::query_scalar("SELECT status FROM sales_order WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .ok();
        status.unwrap_or_else(|| "pending".to_string())
    }

    async fn fetch_purchaser_id_or_neg1(&self, id: i64) -> i64 {
        sqlx::query_scalar("SELECT purchaser_id FROM sales_order WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .unwrap_or(-1)
    }

    async fn list_item_price_inputs(&self, order_id: i64) -> Vec<(i64, i64, f64)> {
        let rows = sqlx::query(
            "SELECT id, product_id, quantity, base_quantity FROM sales_order_item WHERE order_id = ?"
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        rows.iter().map(|r| (r.get::<i64, _>("id"), r.get::<i64, _>("product_id"), r.get::<f64, _>("quantity"))).collect()
    }

    async fn product_base_price(&self, product_id: i64) -> Option<f64> {
        let row: Option<(f64,)> = sqlx::query_as(
            "SELECT COALESCE(base_price, 0) FROM product WHERE id = ?"
        )
        .bind(product_id)
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None);
        row.map(|(p,)| p)
    }

    async fn fetch_discount_and_reduction(&self, id: i64) -> (f64, f64) {
        let discount_rate: f64 = sqlx::query_scalar(
            "SELECT COALESCE(discount_rate, 0) FROM sales_order WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0.0);

        let amount_reduction: f64 = sqlx::query_scalar(
            "SELECT COALESCE(amount_reduction, 0) FROM sales_order WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0.0);

        (discount_rate, amount_reduction)
    }

    async fn export_rows(&self, keyword_pattern: &str, purchaser_id: Option<i64>) -> Result<Vec<SalesOrderExportRow>, DomainError> {
        let base_sql = "SELECT so.id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.final_amount, so.status, so.remark, p.name as purchaser_name,
                               soi.product_name, soi.alias1, soi.alias2, soi.spec, soi.unit, soi.pre_sale_quantity, soi.quantity, soi.unit_price, soi.base_quantity, soi.amount, soi.remark as item_remark
                        FROM sales_order so
                        JOIN purchaser p ON so.purchaser_id = p.id
                        LEFT JOIN sales_order_item soi ON so.id = soi.order_id
                        WHERE (so.order_no LIKE ? OR p.name LIKE ? OR so.order_date LIKE ?)";
        let rows = if let Some(pid) = purchaser_id {
            let sql = format!("{} AND so.purchaser_id = ? ORDER BY so.id, soi.id", base_sql);
            sqlx::query(AssertSqlSafe(sql.as_str()))
                .bind(keyword_pattern).bind(keyword_pattern).bind(keyword_pattern)
                .bind(pid)
                .fetch_all(&self.pool).await.map_err(storage)?
        } else {
            let sql = format!("{} ORDER BY so.id, soi.id", base_sql);
            sqlx::query(AssertSqlSafe(sql.as_str()))
                .bind(keyword_pattern).bind(keyword_pattern).bind(keyword_pattern)
                .fetch_all(&self.pool).await.map_err(storage)?
        };
        Ok(rows
            .iter()
            .map(|row| SalesOrderExportRow {
                id: row.get::<i64, _>("id"),
                order_no: row.get::<String, _>("order_no"),
                order_date: row.get::<String, _>("order_date"),
                total_amount: row.get::<f64, _>("total_amount"),
                discount_rate: row.get::<f64, _>("discount_rate"),
                final_amount: row.get::<f64, _>("final_amount"),
                status: row.get::<String, _>("status"),
                remark: row.get::<Option<String>, _>("remark"),
                purchaser_name: row.get::<String, _>("purchaser_name"),
                product_name: row.get::<Option<String>, _>("product_name"),
                alias1: row.get::<Option<String>, _>("alias1"),
                alias2: row.get::<Option<String>, _>("alias2"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                pre_sale_quantity: row.get::<Option<f64>, _>("pre_sale_quantity"),
                quantity: row.get::<Option<f64>, _>("quantity"),
                unit_price: row.get::<Option<f64>, _>("unit_price"),
                base_quantity: row.get::<Option<f64>, _>("base_quantity"),
                amount: row.get::<Option<f64>, _>("amount"),
                item_remark: row.get::<Option<String>, _>("item_remark"),
            })
            .collect())
    }

    async fn find_purchaser_id_by_name(&self, name: &str) -> Result<Option<i64>, DomainError> {
        let row = sqlx::query("SELECT id FROM purchaser WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| r.get::<i64, _>("id")))
    }

    async fn insert_import_order(&self, main: &SalesOrderImportMain) -> Result<(i64, u64), DomainError> {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO sales_order(order_no, purchaser_id, order_date, total_amount, discount_rate, final_amount, remark, status) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')"
        )
        .bind(&main.order_no)
        .bind(main.purchaser_id)
        .bind(&main.order_date)
        .bind(main.total_amount)
        .bind(main.discount_rate)
        .bind(main.final_amount)
        .bind(&main.remark)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok((res.last_insert_rowid(), res.rows_affected()))
    }

    async fn find_product_id_by_name_spec(&self, name: &str, spec: &str) -> Result<Option<i64>, DomainError> {
        let row = sqlx::query("SELECT id FROM product WHERE name = ? AND (spec IS NULL OR spec = ?)")
            .bind(name)
            .bind(spec)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| r.get::<i64, _>("id")))
    }

    async fn unit_ratio(&self, product_id: i64, unit: &str) -> Result<Option<f64>, DomainError> {
        let row = sqlx::query("SELECT ratio FROM product_unit WHERE product_id = ? AND unit_name = ?")
            .bind(product_id)
            .bind(unit)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| r.get::<f64, _>("ratio")))
    }

    async fn insert_import_item(&self, order_id: i64, item: &SalesOrderImportItem) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO sales_order_item(order_id, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, base_quantity, amount, pre_sale_quantity, remark) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(order_id)
        .bind(item.product_id)
        .bind(&item.product_name)
        .bind(&item.alias1)
        .bind(&item.alias2)
        .bind(&item.spec)
        .bind(&item.unit)
        .bind(item.unit_price)
        .bind(item.quantity)
        .bind(item.base_quantity)
        .bind(item.amount)
        .bind(0.0f64)
        .bind(&item.remark)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(())
    }
}
