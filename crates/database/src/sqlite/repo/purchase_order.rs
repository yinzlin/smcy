//! SQLite 采购订单仓储实现
//!
//! SQL 与旧 api_purchase_order_* handler 逐字一致；乐观锁更新/审核/反审核的
//! 多步事务序列在实现内部 begin/commit，错误文案按旧 handler 逐字预置。

use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::purchase_order::*;
use domain::{PurchaseOrderItemReq, PurchaseOrderReq};

use super::stock_writer;

#[derive(Clone)]
pub struct SqlitePurchaseOrderRepo {
    pool: sqlx::SqlitePool,
}

impl SqlitePurchaseOrderRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

/// 采购单明细单条 INSERT（更新流程中新增明细/兜底插入共用）：
/// source_sales_order_id 用主表 source 兜底，保证新增明细归属到生成它的销售单。
async fn insert_purchase_item(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    order_id: i64,
    item: &PurchaseOrderItemReq,
    fallback_source: Option<i64>,
    base_qty: f64,
) {
    let _ = sqlx::query(
        "INSERT INTO purchase_order_item(order_id, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, base_quantity, amount, ordered_quantity, remark, warehouse_id, warehouse_name, sales_unit, source_sales_order_id, production_date, batch_no) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
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
    .bind(base_qty)
    .bind(item.amount)
    .bind(item.ordered_quantity.unwrap_or(0.0))
    .bind(&item.remark)
    .bind(item.warehouse_id.unwrap_or(0))
    .bind(&item.warehouse_name.clone().unwrap_or_default())
    .bind(&item.unit.clone().unwrap_or_default())
    .bind(fallback_source)
    .bind(&item.production_date)
    .bind(&item.batch_no)
    .execute(&mut **tx)
    .await
    .ok();
}

impl PurchaseOrderRepo for SqlitePurchaseOrderRepo {
    async fn supplier_audit_status(&self, supplier_id: i64) -> Result<Option<String>, DomainError> {
        sqlx::query_scalar::<_, String>("SELECT audit_status FROM supplier WHERE id = ?")
            .bind(supplier_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }

    async fn insert_order(&self, req: &PurchaseOrderReq, main_wh_id: i64, main_wh_name: &str) -> Result<i64, DomainError> {
        let res = sqlx::query(
            "INSERT INTO purchase_order(supplier_id, order_no, order_date, total_amount, discount_rate, amount_reduction, final_amount, warehouse_id, warehouse_name, user_id, handler_phone, remark, is_settled) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(req.supplier_id)
        .bind(&req.order_no)
        .bind(&req.order_date)
        .bind(req.total_amount)
        .bind(req.discount_rate)
        .bind(req.amount_reduction)
        .bind(req.final_amount)
        .bind(main_wh_id)
        .bind(main_wh_name)
        .bind(req.user_id.unwrap_or(0))
        .bind(&req.handler_phone.clone().unwrap_or_default())
        .bind(&req.remark)
        .bind(req.is_settled.unwrap_or(0))
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.last_insert_rowid())
    }

    async fn insert_items_batch(&self, order_id: i64, items: &[PurchaseOrderItemReq], base_qtys: &[f64]) -> Result<(), DomainError> {
        let placeholders: Vec<String> = items.iter()
            .map(|_| "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)".to_string())
            .collect();
        let sql = format!(
            "INSERT INTO purchase_order_item(order_id, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, base_quantity, amount, ordered_quantity, remark, warehouse_id, warehouse_name, production_date, batch_no) VALUES {}",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(AssertSqlSafe(sql.as_str()));
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
                .bind(item.ordered_quantity.unwrap_or(0.0))
                .bind(&item.remark)
                .bind(item.warehouse_id.unwrap_or(0))
                .bind(&item.warehouse_name.clone().unwrap_or_default())
                .bind(&item.production_date)
                .bind(&item.batch_no);
        }
        query.execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }

    async fn count_list(&self, filter: &PurchaseOrderListFilter) -> Result<i64, DomainError> {
        let keyword_pattern = &filter.keyword_pattern;
        let (total_sql, total_params) = match (filter.supplier_id, filter.is_settled.as_ref()) {
            (Some(sid), Some(isf)) => (
                "SELECT COUNT(*) as count FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                 WHERE po.supplier_id = ? AND po.is_settled = ? AND (po.order_no LIKE ? OR s.name LIKE ? OR po.order_date LIKE ?)",
                vec![sid.to_string(), isf.clone(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (Some(sid), None) => (
                "SELECT COUNT(*) as count FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                 WHERE po.supplier_id = ? AND (po.order_no LIKE ? OR s.name LIKE ? OR po.order_date LIKE ?)",
                vec![sid.to_string(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (None, Some(isf)) => (
                "SELECT COUNT(*) as count FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                 WHERE po.is_settled = ? AND (po.order_no LIKE ? OR s.name LIKE ? OR po.order_date LIKE ?)",
                vec![isf.clone(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (None, None) => (
                "SELECT COUNT(*) as count FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                 WHERE po.order_no LIKE ? OR s.name LIKE ? OR po.order_date LIKE ?",
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

    async fn list(&self, filter: &PurchaseOrderListFilter, order_clause: &str) -> Result<Vec<PurchaseOrderListRow>, DomainError> {
        let keyword_pattern = &filter.keyword_pattern;
        let offset = (filter.page - 1) * filter.page_size;
        let (sql, query_params) = match (filter.supplier_id, filter.is_settled.as_ref()) {
            (Some(sid), Some(isf)) => (
                format!(
                    "SELECT po.id, po.order_no, po.order_date, po.total_amount, po.discount_rate, po.amount_reduction, po.final_amount, po.status, po.remark, po.warehouse_id, po.warehouse_name, po.is_settled, s.name as supplier_name,
                            (SELECT GROUP_CONCAT(DISTINCT NULLIF(TRIM(poi.warehouse_name), ''))
                             FROM purchase_order_item poi
                             WHERE poi.order_id = po.id) as item_warehouse_names,
                            (SELECT COUNT(DISTINCT NULLIF(TRIM(poi.warehouse_name), ''))
                             FROM purchase_order_item poi
                             WHERE poi.order_id = po.id) as item_warehouse_count
                     FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                     WHERE po.supplier_id = ? AND po.is_settled = ? AND (po.order_no LIKE ? OR s.name LIKE ? OR po.order_date LIKE ?)
                     ORDER BY {} LIMIT ? OFFSET ?",
                    order_clause
                ),
                vec![sid.to_string(), isf.clone(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (Some(sid), None) => (
                format!(
                    "SELECT po.id, po.order_no, po.order_date, po.total_amount, po.discount_rate, po.amount_reduction, po.final_amount, po.status, po.remark, po.warehouse_id, po.warehouse_name, po.is_settled, s.name as supplier_name,
                            (SELECT GROUP_CONCAT(DISTINCT NULLIF(TRIM(poi.warehouse_name), ''))
                             FROM purchase_order_item poi
                             WHERE poi.order_id = po.id) as item_warehouse_names,
                            (SELECT COUNT(DISTINCT NULLIF(TRIM(poi.warehouse_name), ''))
                             FROM purchase_order_item poi
                             WHERE poi.order_id = po.id) as item_warehouse_count
                     FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                     WHERE po.supplier_id = ? AND (po.order_no LIKE ? OR s.name LIKE ? OR po.order_date LIKE ?)
                     ORDER BY {} LIMIT ? OFFSET ?",
                    order_clause
                ),
                vec![sid.to_string(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (None, Some(isf)) => (
                format!(
                    "SELECT po.id, po.order_no, po.order_date, po.total_amount, po.discount_rate, po.amount_reduction, po.final_amount, po.status, po.remark, po.warehouse_id, po.warehouse_name, po.is_settled, s.name as supplier_name,
                            (SELECT GROUP_CONCAT(DISTINCT NULLIF(TRIM(poi.warehouse_name), ''))
                             FROM purchase_order_item poi
                             WHERE poi.order_id = po.id) as item_warehouse_names,
                            (SELECT COUNT(DISTINCT NULLIF(TRIM(poi.warehouse_name), ''))
                             FROM purchase_order_item poi
                             WHERE poi.order_id = po.id) as item_warehouse_count
                     FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                     WHERE po.is_settled = ? AND (po.order_no LIKE ? OR s.name LIKE ? OR po.order_date LIKE ?)
                     ORDER BY {} LIMIT ? OFFSET ?",
                    order_clause
                ),
                vec![isf.clone(), keyword_pattern.clone(), keyword_pattern.clone(), keyword_pattern.clone()]
            ),
            (None, None) => (
                format!(
                    "SELECT po.id, po.order_no, po.order_date, po.total_amount, po.discount_rate, po.amount_reduction, po.final_amount, po.status, po.remark, po.warehouse_id, po.warehouse_name, po.is_settled, s.name as supplier_name,
                            (SELECT GROUP_CONCAT(DISTINCT NULLIF(TRIM(poi.warehouse_name), ''))
                             FROM purchase_order_item poi
                             WHERE poi.order_id = po.id) as item_warehouse_names,
                            (SELECT COUNT(DISTINCT NULLIF(TRIM(poi.warehouse_name), ''))
                             FROM purchase_order_item poi
                             WHERE poi.order_id = po.id) as item_warehouse_count
                     FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                     WHERE po.order_no LIKE ? OR s.name LIKE ? OR po.order_date LIKE ?
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

        Ok(rows
            .iter()
            .map(|row| PurchaseOrderListRow {
                id: row.get::<i64, _>("id"),
                order_no: row.get::<String, _>("order_no"),
                order_date: row.get::<String, _>("order_date"),
                total_amount: row.get::<f64, _>("total_amount"),
                discount_rate: row.get::<f64, _>("discount_rate"),
                amount_reduction: row.get::<f64, _>("amount_reduction"),
                final_amount: row.get::<f64, _>("final_amount"),
                status: row.get::<String, _>("status"),
                remark: row.get::<Option<String>, _>("remark"),
                warehouse_id: row.get::<i64, _>("warehouse_id"),
                warehouse_name: row.try_get::<Option<String>, _>("warehouse_name").unwrap_or(None),
                is_settled: row.get::<i64, _>("is_settled"),
                supplier_name: row.get::<String, _>("supplier_name"),
                item_warehouse_names: row.try_get::<Option<String>, _>("item_warehouse_names").unwrap_or(None),
                item_warehouse_count: row.get::<i64, _>("item_warehouse_count"),
            })
            .collect())
    }

    async fn get_order_detail(&self, id: i64) -> Result<Option<PurchaseOrderDetailRow>, DomainError> {
        let row = sqlx::query(
            "SELECT po.id, po.supplier_id, po.order_no, po.order_date, po.total_amount, po.discount_rate, po.amount_reduction, po.final_amount, po.status, po.remark, po.warehouse_id, po.warehouse_name, po.user_id, po.version, po.is_settled, s.name as supplier_name
             FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id WHERE po.id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.map(|row| PurchaseOrderDetailRow {
            id: row.get::<i64, _>("id"),
            supplier_id: row.get::<i64, _>("supplier_id"),
            order_no: row.get::<String, _>("order_no"),
            order_date: row.get::<String, _>("order_date"),
            total_amount: row.get::<f64, _>("total_amount"),
            discount_rate: row.get::<f64, _>("discount_rate"),
            amount_reduction: row.get::<f64, _>("amount_reduction"),
            final_amount: row.get::<f64, _>("final_amount"),
            status: row.get::<String, _>("status"),
            remark: row.get::<Option<String>, _>("remark"),
            warehouse_id: row.get::<i64, _>("warehouse_id"),
            warehouse_name: row.get::<Option<String>, _>("warehouse_name"),
            user_id: row.get::<i64, _>("user_id"),
            version: row.get::<i64, _>("version"),
            is_settled: row.get::<i64, _>("is_settled"),
            supplier_name: row.get::<String, _>("supplier_name"),
        }))
    }

    async fn list_items(&self, order_id: i64) -> Result<Vec<PurchaseOrderItemRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, base_quantity, amount, ordered_quantity, remark, warehouse_id, warehouse_name, production_date, batch_no FROM purchase_order_item WHERE order_id = ?"
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| PurchaseOrderItemRow {
                id: r.get::<i64, _>("id"),
                product_id: r.get::<i64, _>("product_id"),
                product_name: r.get::<String, _>("product_name"),
                alias1: r.get::<Option<String>, _>("alias1"),
                alias2: r.get::<Option<String>, _>("alias2"),
                spec: r.get::<Option<String>, _>("spec"),
                unit: r.get::<Option<String>, _>("unit"),
                unit_price: r.get::<f64, _>("unit_price"),
                quantity: r.get::<f64, _>("quantity"),
                base_quantity: r.get::<Option<f64>, _>("base_quantity"),
                amount: r.get::<f64, _>("amount"),
                ordered_quantity: r.get::<Option<f64>, _>("ordered_quantity"),
                remark: r.get::<Option<String>, _>("remark"),
                warehouse_id: r.get::<i64, _>("warehouse_id"),
                warehouse_name: r.get::<Option<String>, _>("warehouse_name"),
                production_date: r.get::<Option<String>, _>("production_date"),
                batch_no: r.get::<Option<String>, _>("batch_no"),
            })
            .collect())
    }

    async fn get_supplier_status(&self, id: i64) -> Result<Option<(i64, String)>, DomainError> {
        let row = sqlx::query_as::<_, (i64, String)>("SELECT supplier_id, status FROM purchase_order WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row)
    }

    async fn update_order_with_items(
        &self,
        req: &PurchaseOrderReq,
        main_wh_id: i64,
        main_wh_name: &str,
        ver: i64,
        base_qtys: &[f64],
    ) -> Result<(), DomainError> {
        // 用 BEGIN IMMEDIATE 启动写事务，立即获得 SQLite 写锁（RESERVED）。
        // 作用：把"主表 UPDATE + 明细同步"整体串行化，
        // 杜绝连点两次保存时两个事务都读到同一 version、都通过乐观锁、都把主表 version 自增、
        // 然后后提交的事务用全量 items 把先提交的事务的明细覆盖/丢失。
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await
            .map_err(|_| DomainError::Storage("更新失败：事务启动失败".to_string()))?;

        // 事务内做"主表 UPDATE + version 自增"：WHERE 同时校验 id 和 version。
        // 如果行不存在或 version 已被他人递增，rows_affected = 0 → 回滚事务 + 返回 409。
        let upd = sqlx::query(
            "UPDATE purchase_order SET supplier_id = ?, order_no = ?, order_date = ?, total_amount = ?, discount_rate = ?, amount_reduction = ?, final_amount = ?, warehouse_id = ?, warehouse_name = ?, user_id = ?, handler_phone = ?, remark = ?, is_settled = ?, version = version + 1 WHERE id = ? AND version = ?"
        )
        .bind(req.supplier_id)
        .bind(&req.order_no)
        .bind(&req.order_date)
        .bind(req.total_amount)
        .bind(req.discount_rate)
        .bind(req.amount_reduction)
        .bind(req.final_amount)
        .bind(main_wh_id)
        .bind(main_wh_name)
        .bind(req.user_id.unwrap_or(0))
        .bind(&req.handler_phone.clone().unwrap_or_default())
        .bind(&req.remark)
        .bind(req.is_settled.unwrap_or(0))
        .bind(req.id)
        .bind(ver)
        .execute(&mut *tx)
        .await;

        let upd_res = match upd {
            Ok(r) => r,
            Err(e) => {
                // 出现异常，让事务 drop 时自动回滚
                return Err(DomainError::Storage(format!("更新失败: {}", e)));
            }
        };
        if upd_res.rows_affected() == 0 {
            // version 已被他人递增（极端并发）→ 整个事务回滚（其实只 UPDATE 了一行 ROLLBACK 也无害）
            return Err(DomainError::Conflict("订单版本冲突".to_string()));
        }

        // 按 id 精确同步明细（保留每条明细的 source_sales_order_id 归属）
        let po_main_source: Option<i64> = sqlx::query("SELECT source_sales_order_id FROM purchase_order WHERE id = ?")
            .bind(req.id)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten()
            .and_then(|r| r.try_get("source_sales_order_id").ok().flatten());

        let existing_by_id: std::collections::HashMap<i64, Option<i64>> = sqlx::query(
            "SELECT id, source_sales_order_id FROM purchase_order_item WHERE order_id = ?"
        )
        .bind(req.id)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let id: i64 = r.get("id");
            let src: Option<i64> = r.try_get("source_sales_order_id").ok().flatten();
            (id, src)
        })
        .collect();

        let mut req_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
        for item in &req.items {
            if let Some(iid) = item.id {
                if iid > 0 {
                    req_ids.insert(iid);
                }
            }
        }
        for (db_id, _src) in &existing_by_id {
            if !req_ids.contains(db_id) {
                let _ = sqlx::query("DELETE FROM purchase_order_item WHERE id = ? AND order_id = ?")
                    .bind(db_id)
                    .bind(req.id)
                    .execute(&mut *tx)
                    .await;
            }
        }

        for (idx, item) in req.items.iter().enumerate() {
            if let Some(iid) = item.id {
                if iid > 0 && existing_by_id.contains_key(&iid) {
                    let _ = sqlx::query(
                        "UPDATE purchase_order_item SET product_id = ?, product_name = ?, alias1 = ?, alias2 = ?, spec = ?, unit = ?, unit_price = ?, quantity = ?, base_quantity = ?, amount = ?, ordered_quantity = ?, remark = ?, warehouse_id = ?, warehouse_name = ?, production_date = ?, batch_no = ? WHERE id = ? AND order_id = ?"
                    )
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
                    .bind(item.ordered_quantity.unwrap_or(0.0))
                    .bind(&item.remark)
                    .bind(item.warehouse_id.unwrap_or(0))
                    .bind(&item.warehouse_name.clone().unwrap_or_default())
                    .bind(&item.production_date)
                    .bind(&item.batch_no)
                    .bind(iid)
                    .bind(req.id)
                    .execute(&mut *tx)
                    .await
                    .ok();
                } else {
                    insert_purchase_item(&mut tx, req.id.unwrap_or(0), item, po_main_source, base_qtys[idx]).await;
                }
            } else {
                insert_purchase_item(&mut tx, req.id.unwrap_or(0), item, po_main_source, base_qtys[idx]).await;
            }
        }
        tx.commit().await
            .map_err(|_| DomainError::Storage("更新失败：事务提交失败".to_string()))?;
        Ok(())
    }

    async fn distinct_product_ids(&self, order_id: i64) -> Result<Vec<i64>, DomainError> {
        let rows = sqlx::query("SELECT DISTINCT product_id FROM purchase_order_item WHERE order_id = ? AND product_id > 0")
            .bind(order_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        let mut ids = Vec::new();
        for r in rows {
            if let Ok(pid) = r.try_get::<i64, _>("product_id") {
                ids.push(pid);
            }
        }
        Ok(ids)
    }

    async fn delete_items_of_order(&self, order_id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM purchase_order_item WHERE order_id = ?")
            .bind(order_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_order(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM purchase_order WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn set_settled(&self, id: i64, is_settled: i64) -> Result<u64, DomainError> {
        let res = sqlx::query("UPDATE purchase_order SET is_settled = ? WHERE id = ?")
            .bind(is_settled)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(res.rows_affected())
    }

    async fn approve_tx(&self, id: i64) -> Result<(), DomainError> {
        // 用 BEGIN IMMEDIATE 事务：主表 UPDATE + 写库存流水 + 更新 inventory.quantity 三步原子化
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await
            .map_err(|_| DomainError::Storage("审核失败：事务启动失败".to_string()))?;
        let upd = sqlx::query("UPDATE purchase_order SET status = 'confirmed', version = version + 1 WHERE id = ? AND status = 'pending'")
            .bind(id)
            .execute(&mut *tx)
            .await;

        match upd {
            Ok(res) if res.rows_affected() > 0 => {
                // 写库存流水 + 更新 inventory
                if let Err(e) = stock_writer::write_stock_movements_for_audit(
                    &mut tx, id, "purchase_order_item", "in", "purchase", "purchase", "采购入库", "审核"
                ).await {
                    let _ = tx.rollback().await;
                    return Err(DomainError::Storage(format!("审核失败：写库存流水失败 {}", e)));
                }
                match tx.commit().await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(DomainError::Storage(format!("审核失败：提交事务失败 {}", e))),
                }
            }
            _ => {
                let _ = tx.rollback().await;
                Err(DomainError::Conflict("订单状态已变化".to_string()))
            }
        }
    }

    async fn unapprove_tx(&self, id: i64, is_super_admin: bool) -> Result<(), DomainError> {
        let update_sql = if is_super_admin {
            "UPDATE purchase_order SET status = 'pending', version = version + 1 WHERE id = ?"
        } else {
            "UPDATE purchase_order SET status = 'pending', version = version + 1 WHERE id = ? AND status = 'confirmed'"
        };
        // 用 BEGIN IMMEDIATE 事务：主表 UPDATE + 写冲销流水 + 回滚 inventory.quantity 三步原子化
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await
            .map_err(|_| DomainError::Storage("反审核失败：事务启动失败".to_string()))?;
        let result = sqlx::query(AssertSqlSafe(update_sql.to_string()))
            .bind(id)
            .execute(&mut *tx)
            .await;

        match result {
            Ok(res) if res.rows_affected() > 0 => {
                // 写冲销流水（原方向 in → 冲销方向 out）+ 同步 inventory
                if let Err(e) = stock_writer::write_stock_movements_for_unaudit(
                    &mut tx, id, "purchase_order_item", "in", "purchase", "purchase", "采购入库", "反审核冲销"
                ).await {
                    let _ = tx.rollback().await;
                    return Err(DomainError::Storage(format!("反审核失败：写冲销流水失败 {}", e)));
                }
                match tx.commit().await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(DomainError::Storage(format!("反审核失败：提交事务失败 {}", e))),
                }
            }
            _ => {
                let _ = tx.rollback().await;
                Err(DomainError::Conflict("订单状态已变化".to_string()))
            }
        }
    }

    async fn export_rows(&self, keyword_pattern: &str, supplier_id: Option<i64>) -> Result<Vec<PurchaseOrderExportRow>, DomainError> {
        // 导出按订单ID与明细ID顺序排列，便于阅读核对
        let base_sql = "SELECT po.id, po.order_no, po.order_date, po.total_amount, po.discount_rate, po.final_amount, po.status, po.remark, po.warehouse_id, po.warehouse_name, s.name as supplier_name,
                               (SELECT GROUP_CONCAT(DISTINCT NULLIF(TRIM(poi2.warehouse_name), ''))
                                FROM purchase_order_item poi2
                                WHERE poi2.order_id = po.id) as item_warehouse_names,
                               (SELECT COUNT(DISTINCT NULLIF(TRIM(poi2.warehouse_name), ''))
                                FROM purchase_order_item poi2
                                WHERE poi2.order_id = po.id) as item_warehouse_count,
                               poi.product_name, poi.alias1, poi.alias2, poi.spec, poi.unit, poi.ordered_quantity, poi.quantity, poi.unit_price, poi.base_quantity, poi.amount, poi.remark as item_remark, poi.warehouse_name as item_warehouse_name
                        FROM purchase_order po
                        JOIN supplier s ON po.supplier_id = s.id
                        LEFT JOIN purchase_order_item poi ON po.id = poi.order_id
                        WHERE (po.order_no LIKE ? OR s.name LIKE ? OR po.order_date LIKE ?)";
        let rows = if let Some(sid) = supplier_id {
            let sql = format!("{} AND po.supplier_id = ? ORDER BY po.id, poi.id", base_sql);
            sqlx::query(AssertSqlSafe(sql.as_str()))
                .bind(keyword_pattern).bind(keyword_pattern).bind(keyword_pattern)
                .bind(sid)
                .fetch_all(&self.pool).await.map_err(storage)?
        } else {
            let sql = format!("{} ORDER BY po.id, poi.id", base_sql);
            sqlx::query(AssertSqlSafe(sql.as_str()))
                .bind(keyword_pattern).bind(keyword_pattern).bind(keyword_pattern)
                .fetch_all(&self.pool).await.map_err(storage)?
        };
        Ok(rows
            .iter()
            .map(|row| PurchaseOrderExportRow {
                id: row.get::<i64, _>("id"),
                order_no: row.get::<String, _>("order_no"),
                order_date: row.get::<String, _>("order_date"),
                supplier_name: row.get::<String, _>("supplier_name"),
                total_amount: row.get::<f64, _>("total_amount"),
                discount_rate: row.get::<f64, _>("discount_rate"),
                final_amount: row.get::<f64, _>("final_amount"),
                status: row.get::<String, _>("status"),
                remark: row.get::<Option<String>, _>("remark"),
                product_name: row.get::<Option<String>, _>("product_name"),
                alias1: row.get::<Option<String>, _>("alias1"),
                alias2: row.get::<Option<String>, _>("alias2"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                ordered_quantity: row.get::<Option<f64>, _>("ordered_quantity"),
                quantity: row.get::<Option<f64>, _>("quantity"),
                unit_price: row.get::<Option<f64>, _>("unit_price"),
                base_quantity: row.get::<Option<f64>, _>("base_quantity"),
                amount: row.get::<Option<f64>, _>("amount"),
                item_remark: row.get::<Option<String>, _>("item_remark"),
                item_warehouse_name: row.try_get::<Option<String>, _>("item_warehouse_name").unwrap_or(None),
            })
            .collect())
    }

    async fn get_print_order(&self, id: i64) -> Result<Option<PurchaseOrderPrintRow>, DomainError> {
        let row = sqlx::query(
            "SELECT po.order_no, po.order_date, po.total_amount, po.amount_reduction, po.final_amount,
                    po.remark,
                    s.name, s.phone, s.address,
                    u.nickname, po.handler_phone
             FROM purchase_order po
             JOIN supplier s ON po.supplier_id = s.id
             LEFT JOIN user_account u ON po.user_id = u.id
             WHERE po.id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.map(|r| PurchaseOrderPrintRow {
            order_no: r.get::<String, _>("order_no"),
            order_date: r.get::<String, _>("order_date"),
            total_amount: r.get::<f64, _>("total_amount"),
            amount_reduction: r.get::<f64, _>("amount_reduction"),
            final_amount: r.get::<f64, _>("final_amount"),
            remark: r.get::<Option<String>, _>("remark"),
            supplier_name: r.get::<Option<String>, _>("name"),
            supplier_phone: r.get::<Option<String>, _>("phone"),
            supplier_address: r.get::<Option<String>, _>("address"),
            user_name: r.get::<Option<String>, _>("nickname"),
            handler_phone: r.get::<Option<String>, _>("handler_phone"),
        }))
    }

    async fn get_print_items(&self, id: i64) -> Result<Vec<PurchaseOrderPrintItemRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT warehouse_name, product_name, spec, unit, quantity, unit_price, amount, remark FROM purchase_order_item WHERE order_id = ? ORDER BY warehouse_name, id"
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| PurchaseOrderPrintItemRow {
                warehouse_name: r.get::<Option<String>, _>("warehouse_name").unwrap_or_default().trim().to_string(),
                product_name: r.get::<String, _>("product_name"),
                spec: r.get::<Option<String>, _>("spec"),
                unit: r.get::<Option<String>, _>("unit"),
                quantity: r.get::<f64, _>("quantity"),
                unit_price: r.get::<f64, _>("unit_price"),
                amount: r.get::<f64, _>("amount"),
                remark: r.get::<Option<String>, _>("remark"),
            })
            .collect())
    }

    async fn user_nickname_phone(&self, user_id: i64) -> Result<Option<(String, String)>, DomainError> {
        let row = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT nickname, COALESCE(NULLIF(phone, ''), '') as phone FROM user_account WHERE id = ?"
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.map(|(nickname, phone)| (nickname, phone.unwrap_or_default())))
    }

    async fn find_supplier_id_by_name(&self, name: &str) -> Result<Option<i64>, DomainError> {
        let row = sqlx::query("SELECT id FROM supplier WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| r.get::<i64, _>("id")))
    }

    async fn insert_import_order(&self, main: &PurchaseOrderImportMain) -> Result<(i64, u64), DomainError> {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO purchase_order(order_no, supplier_id, order_date, total_amount, discount_rate, final_amount, remark, status) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')"
        )
        .bind(&main.order_no)
        .bind(main.supplier_id)
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

    async fn insert_import_item(&self, order_id: i64, item: &PurchaseOrderImportItem) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO purchase_order_item(order_id, product_id, product_name, alias1, alias2, spec, unit, unit_price, quantity, base_quantity, amount, ordered_quantity, remark) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
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
        .ok();
        Ok(())
    }

    async fn max_seq_for_date(&self, order_date: &str) -> Result<i64, DomainError> {
        // 行为保持：查询失败旧 unwrap_or(0) → 序号从 1 起
        Ok(sqlx::query_scalar(
            "SELECT COALESCE(MAX(CAST(SUBSTR(order_no, 11, 3) AS INTEGER)), 0) FROM purchase_order WHERE order_date = ?",
        )
        .bind(order_date)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0))
    }

    async fn compute_base_quantities(&self, items: &[(i64, String, f64, f64)]) -> Vec<f64> {
        stock_writer::compute_base_quantities(&self.pool, items).await
    }
}
