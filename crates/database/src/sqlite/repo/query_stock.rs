//! 库存查询仓储实现：SQL 与旧 api_query_stock_balance* / api_query_stock_flow* /
//! api_query_stock_warning* / api_query_slow_stock* handler 逐字一致
//!
//! 行为保持：全部查询的错误由用例层 unwrap_or_default()/unwrap_or(0) 吞错；
//! 流水数值列 try_get + unwrap_or 吞错、production_date/batch_no try_get 吞错（与旧 handler 一致）。

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::query_stock::{
    QueryStockRepo, SlowStockExportRow, SlowStockRow, StockBalanceHistoryRow,
    StockBalanceRealtimeRow, StockFlowOpeningRow, StockFlowRow, StockSummaryPurchaseRow,
    StockSummarySalesRow, StockWarningRow,
};

#[derive(Clone)]
pub struct SqliteQueryStockRepo {
    pool: SqlitePool,
}

impl SqliteQueryStockRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

/// 库存流水基础查询：按订单最终状态聚合成组（同一订单同一商品的审核/反审核流水相互抵消）。
/// 数量统一换算为基础单位口径，单位显示商品基础单位。
/// src/sid 作为稳定排序键：0=采购、1=销售，同日内先入后出。
/// 余额列此处固定返回 0.0 占位：balance_after 快照是物理写入时刻的库存值，
/// 反审核/再审核后无法对应单据日期顺序下的逻辑余额，故余额由调用方在应用层
/// 按 (商品, order_date, src, sid) 顺序用净额累计计算。
/// 返回的 SQL 不含外层 ORDER BY / LIMIT，由调用方追加。
fn stock_flow_base_sql(where_clause: &str) -> String {
    // 按订单最终状态聚合：同一订单同一商品同一明细的审核/反审核流水相互抵消，
    // 净额为 0（最终未审核）的整组不显示，台账只保留最终生效的一条，不产生出入库虚数。
    // 审核与冲销归属同一 order_date，日期过滤不会拆散配对。
    // 分组带 ref_item_id：同单同商品多行（补采/分行验收）各自成行，批次不同分行显示；历史行该列为 NULL 仍聚合成一行。
    format!(
        "SELECT * FROM (
            SELECT sm.order_date AS create_time,
                   sm.movement_type,
                   CASE WHEN sm.movement_type='purchase' THEN '采购入库' ELSE '销售出库' END AS summary,
                   CASE WHEN sm.movement_type='purchase' THEN 'in' ELSE 'out' END AS direction,
                   sm.product_id,
                   p.name AS product_name,
                   p.spec,
                   COALESCE(NULLIF(p.base_unit, ''), p.unit) AS unit,
                   CASE WHEN sm.movement_type='purchase'
                        THEN SUM(CASE WHEN sm.direction='in' THEN sm.base_quantity ELSE -sm.base_quantity END)
                        ELSE 0 END AS in_quantity,
                   CASE WHEN sm.movement_type='sales'
                        THEN SUM(CASE WHEN sm.direction='out' THEN sm.base_quantity ELSE -sm.base_quantity END)
                        ELSE 0 END AS out_quantity,
                   ABS(SUM(CASE WHEN sm.direction=(CASE WHEN sm.movement_type='purchase' THEN 'in' ELSE 'out' END)
                           THEN sm.orig_quantity ELSE -sm.orig_quantity END)) AS orig_quantity,
                   MAX(sm.orig_unit) AS orig_unit,
                   MAX(sm.ref_no) AS ref_no,
                   MAX(sm.party_name) AS party_name,
                   MAX(sm.production_date) AS production_date,
                   MAX(sm.batch_no) AS batch_no,
                   CASE WHEN sm.movement_type='purchase' THEN 0 ELSE 1 END AS src,
                   MAX(sm.id) AS sid,
                   0.0 AS balance
            FROM stock_movement sm
            JOIN product p ON sm.product_id = p.id
            {}
            GROUP BY sm.ref_type, sm.ref_id, sm.product_id, sm.movement_type, sm.order_date, sm.ref_item_id
        ) t
        WHERE t.in_quantity > 0.001 OR t.out_quantity > 0.001",
        where_clause
    )
}

/// 历史时点余额 = 当前 inventory 余额 − 单据日期在时点之后的净流水（in 正 / out 负）；
/// 冲销流水 direction 与原方向相反且归属同一单据日期，自动计入。相关子查询走 idx_sm_order_date 索引。
fn stock_balance_history_inner(category_id: &str) -> String {
    let cat_cond = if category_id.is_empty() {
        String::new()
    } else {
        " AND p.category_id = ?".to_string()
    };
    format!(
        "SELECT i.product_id, p.name AS product_name, p.spec, COALESCE(NULLIF(p.base_unit, ''), p.unit) AS unit, p.base_price,
                (i.quantity - COALESCE((SELECT SUM(CASE WHEN sm2.direction='in' THEN sm2.base_quantity ELSE -sm2.base_quantity END) FROM stock_movement sm2 WHERE sm2.product_id=i.product_id AND sm2.order_date >= ?), 0.0)) AS opening_qty,
                COALESCE((SELECT SUM(sm2.base_quantity) FROM stock_movement sm2 WHERE sm2.product_id=i.product_id AND sm2.direction='in' AND sm2.order_date >= ? AND sm2.order_date <= ?), 0.0) AS period_in,
                COALESCE((SELECT SUM(sm2.base_quantity) FROM stock_movement sm2 WHERE sm2.product_id=i.product_id AND sm2.direction='out' AND sm2.order_date >= ? AND sm2.order_date <= ?), 0.0) AS period_out,
                (i.quantity - COALESCE((SELECT SUM(CASE WHEN sm2.direction='in' THEN sm2.base_quantity ELSE -sm2.base_quantity END) FROM stock_movement sm2 WHERE sm2.product_id=i.product_id AND sm2.order_date > ?), 0.0)) AS closing_qty
         FROM inventory i JOIN product p ON i.product_id=p.id
         WHERE p.name LIKE ?{}",
        cat_cond
    )
}

/// ? 绑定顺序（历史）：opening start、pin start/end、pout start/end、closing end、pattern、[cat]
fn bind_stock_balance_history<'q>(
    q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>,
    start_bound: &str,
    end_bound: &str,
    pattern: &str,
    category_id: &str,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    let mut q = q
        .bind(start_bound).bind(start_bound).bind(end_bound)
        .bind(start_bound).bind(end_bound).bind(end_bound)
        .bind(pattern);
    if !category_id.is_empty() {
        let cat: i64 = category_id.parse().unwrap_or(0);
        q = q.bind(cat);
    }
    q
}

impl QueryStockRepo for SqliteQueryStockRepo {
    async fn count_stock_balance_history(&self, start_bound: &str, end_bound: &str, pattern: &str, category_id: &str) -> Result<i64, DomainError> {
        // 过滤掉期初/期间入出/期末全为 0 的商品，减少噪音
        let nonzero = "ABS(opening_qty)>0.001 OR ABS(period_in)>0.001 OR ABS(period_out)>0.001 OR ABS(closing_qty)>0.001";
        let count_sql = format!("SELECT COUNT(*) FROM ({}) t WHERE {}", stock_balance_history_inner(category_id), nonzero);
        let q = sqlx::query(AssertSqlSafe(count_sql.as_str()));
        let q = bind_stock_balance_history(q, start_bound, end_bound, pattern, category_id);
        let row = q.fetch_one(&self.pool).await.map_err(storage)?;
        Ok(row.get::<i64, _>(0))
    }

    async fn count_stock_balance_realtime(&self, pattern: &str, category_id: &str) -> Result<i64, DomainError> {
        let where_sql = if category_id.is_empty() {
            "WHERE p.name LIKE ?"
        } else {
            "WHERE p.name LIKE ? AND p.category_id = ?"
        };
        let count_sql = format!(
            "SELECT COUNT(*) FROM inventory i JOIN product p ON i.product_id = p.id {}",
            where_sql
        );
        let mut q = sqlx::query_scalar::<_, i64>(AssertSqlSafe(count_sql.as_str()));
        q = q.bind(pattern);
        if !category_id.is_empty() {
            let cat: i64 = category_id.parse().unwrap_or(0);
            q = q.bind(cat);
        }
        Ok(q.fetch_one(&self.pool).await.map_err(storage)?)
    }

    async fn list_stock_balance_history(&self, start_bound: &str, end_bound: &str, pattern: &str, category_id: &str, page_size: i64, offset: i64) -> Result<Vec<StockBalanceHistoryRow>, DomainError> {
        // 过滤掉期初/期间入出/期末全为 0 的商品，减少噪音
        let nonzero = "ABS(opening_qty)>0.001 OR ABS(period_in)>0.001 OR ABS(period_out)>0.001 OR ABS(closing_qty)>0.001";
        let data_sql = format!(
            "SELECT * FROM ({}) t WHERE {} ORDER BY product_name LIMIT ? OFFSET ?",
            stock_balance_history_inner(category_id), nonzero
        );
        let q = sqlx::query(AssertSqlSafe(data_sql.as_str()));
        let q = bind_stock_balance_history(q, start_bound, end_bound, pattern, category_id)
            .bind(page_size).bind(offset);
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| StockBalanceHistoryRow {
                product_id: row.get::<i64, _>("product_id"),
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                // 行为保持：旧 handler 对这些数值列 try_get + unwrap_or 吞错
                opening_qty: row.try_get::<f64, _>("opening_qty").unwrap_or(0.0),
                period_in: row.try_get::<f64, _>("period_in").unwrap_or(0.0),
                period_out: row.try_get::<f64, _>("period_out").unwrap_or(0.0),
                closing_qty: row.try_get::<f64, _>("closing_qty").unwrap_or(0.0),
                base_price: row.try_get::<f64, _>("base_price").unwrap_or(0.0),
            })
            .collect())
    }

    async fn list_stock_balance_realtime(&self, pattern: &str, category_id: &str, page_size: i64, offset: i64) -> Result<Vec<StockBalanceRealtimeRow>, DomainError> {
        let where_sql = if category_id.is_empty() {
            "WHERE p.name LIKE ?"
        } else {
            "WHERE p.name LIKE ? AND p.category_id = ?"
        };
        let data_sql = format!(
            "SELECT i.id, i.product_id, i.warehouse_id, i.quantity, i.min_stock, i.max_stock,
                    p.name as product_name, p.spec, COALESCE(NULLIF(p.base_unit, ''), p.unit) AS unit, p.base_price,
                    (i.quantity * p.base_price) as amount
             FROM inventory i JOIN product p ON i.product_id=p.id
             {} ORDER BY p.name LIMIT ? OFFSET ?",
            where_sql
        );
        let mut q = sqlx::query(AssertSqlSafe(data_sql.as_str()));
        q = q.bind(pattern);
        if !category_id.is_empty() {
            let cat: i64 = category_id.parse().unwrap_or(0);
            q = q.bind(cat);
        }
        let rows = q.bind(page_size).bind(offset).fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| StockBalanceRealtimeRow {
                product_id: row.get::<i64, _>("product_id"),
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                // 行为保持：旧 handler 对这些数值列 try_get + unwrap_or 吞错
                quantity: row.try_get::<f64, _>("quantity").unwrap_or(0.0),
                amount: row.try_get::<f64, _>("amount").unwrap_or(0.0),
                min_stock: row.try_get::<f64, _>("min_stock").unwrap_or(0.0),
                max_stock: row.try_get::<f64, _>("max_stock").unwrap_or(0.0),
            })
            .collect())
    }

    async fn list_stock_balance_history_export(&self, start_bound: &str, end_bound: &str, pattern: &str, category_id: &str) -> Result<Vec<StockBalanceHistoryRow>, DomainError> {
        let nonzero = "ABS(opening_qty)>0.001 OR ABS(period_in)>0.001 OR ABS(period_out)>0.001 OR ABS(closing_qty)>0.001";
        let sql = format!("SELECT * FROM ({}) t WHERE {} ORDER BY product_name", stock_balance_history_inner(category_id), nonzero);
        let q = sqlx::query(AssertSqlSafe(sql.as_str()));
        let q = bind_stock_balance_history(q, start_bound, end_bound, pattern, category_id);
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| StockBalanceHistoryRow {
                product_id: row.get::<i64, _>("product_id"),
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                // 行为保持：旧 handler 对这些数值列 try_get + unwrap_or 吞错
                opening_qty: row.try_get::<f64, _>("opening_qty").unwrap_or(0.0),
                period_in: row.try_get::<f64, _>("period_in").unwrap_or(0.0),
                period_out: row.try_get::<f64, _>("period_out").unwrap_or(0.0),
                closing_qty: row.try_get::<f64, _>("closing_qty").unwrap_or(0.0),
                base_price: row.try_get::<f64, _>("base_price").unwrap_or(0.0),
            })
            .collect())
    }

    async fn list_stock_balance_realtime_export(&self, pattern: &str, category_id: &str) -> Result<Vec<StockBalanceRealtimeRow>, DomainError> {
        // 行为保持：category_id 非空时与旧 handler 一致——SQL 内联 category_id 且仍绑定 cid（多余绑定由 unwrap_or_default 吞错）
        let sql = if category_id.is_empty() {
            "SELECT i.id,i.product_id,i.quantity,i.min_stock,i.max_stock,p.name as product_name,p.spec,COALESCE(NULLIF(p.base_unit, ''), p.unit) as unit,p.base_price,(i.quantity*p.base_price) as amount FROM inventory i JOIN product p ON i.product_id=p.id WHERE p.name LIKE ? ORDER BY p.name, p.id".to_string()
        } else {
            format!("SELECT i.id,i.product_id,i.quantity,i.min_stock,i.max_stock,p.name as product_name,p.spec,COALESCE(NULLIF(p.base_unit, ''), p.unit) as unit,p.base_price,(i.quantity*p.base_price) as amount FROM inventory i JOIN product p ON i.product_id=p.id WHERE p.name LIKE ? AND p.category_id={} ORDER BY p.name, p.id", category_id)
        };
        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        q = q.bind(pattern);
        if !category_id.is_empty() {
            let cid: i64 = category_id.parse().unwrap_or(0);
            q = q.bind(cid);
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| StockBalanceRealtimeRow {
                product_id: row.get::<i64, _>("product_id"),
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                // 行为保持：旧 handler 对这些数值列 try_get + unwrap_or 吞错
                quantity: row.try_get::<f64, _>("quantity").unwrap_or(0.0),
                amount: row.try_get::<f64, _>("amount").unwrap_or(0.0),
                min_stock: row.try_get::<f64, _>("min_stock").unwrap_or(0.0),
                max_stock: row.try_get::<f64, _>("max_stock").unwrap_or(0.0),
            })
            .collect())
    }

    async fn count_stock_flow(&self, where_clause: &str, bind_pattern: bool, pattern: &str) -> Result<i64, DomainError> {
        // 总条数：按订单最终状态聚合后的行数（与 data_sql 同口径）
        let count_sql = format!("SELECT COUNT(*) FROM ({})", stock_flow_base_sql(where_clause));
        let mut q = sqlx::query_scalar::<_, i64>(AssertSqlSafe(count_sql.as_str()));
        if bind_pattern {
            q = q.bind(pattern);
        }
        Ok(q.fetch_one(&self.pool).await.map_err(storage)?)
    }

    async fn list_stock_flow(&self, where_clause: &str, order_by: &str, fetch_limit: i64, bind_pattern: bool, pattern: &str) -> Result<Vec<StockFlowRow>, DomainError> {
        let data_sql = format!(
            "SELECT create_time, movement_type, summary, direction, product_id, product_name, spec, unit,
                    in_quantity, out_quantity, orig_quantity, orig_unit,
                    ref_no, party_name, production_date, batch_no
             FROM ({})
             ORDER BY {} LIMIT {}",
            stock_flow_base_sql(where_clause), order_by, fetch_limit
        );
        let mut q = sqlx::query(AssertSqlSafe(data_sql.as_str()));
        if bind_pattern {
            q = q.bind(pattern);
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(map_stock_flow_row).collect())
    }

    async fn list_stock_flow_export_timeline(&self, where_clause: &str, bind_pattern: bool, pattern: &str) -> Result<Vec<StockFlowRow>, DomainError> {
        let sql=format!("SELECT create_time,movement_type,summary,direction,product_id,product_name,spec,unit,in_quantity,out_quantity,orig_quantity,orig_unit,ref_no,party_name,production_date,batch_no FROM ({}) ORDER BY create_time,src,sid",stock_flow_base_sql(where_clause));
        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        if bind_pattern {
            q = q.bind(pattern);
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(map_stock_flow_row).collect())
    }

    async fn list_stock_flow_export_ledger(&self, where_clause: &str, bind_pattern: bool, pattern: &str) -> Result<Vec<StockFlowRow>, DomainError> {
        let sql=format!("SELECT create_time,movement_type,summary,direction,product_id,product_name,spec,unit,in_quantity,out_quantity,orig_quantity,orig_unit,ref_no,party_name,production_date,batch_no FROM ({}) ORDER BY product_id,create_time,src,sid",stock_flow_base_sql(where_clause));
        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        if bind_pattern {
            q = q.bind(pattern);
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(map_stock_flow_row).collect())
    }

    async fn list_stock_flow_opening(&self, pid_list: &str, base_date: &str) -> Result<Vec<StockFlowOpeningRow>, DomainError> {
        // 期初库存必须与 stock_flow_base_sql 同一聚合口径（按订单分组抵消反审核），
        // 否则反审核冲销记录会被重复计入，导致期初余额错误。
        let opening_where = format!(
            "WHERE sm.product_id IN ({}) AND sm.order_date < '{}'",
            pid_list, base_date
        );
        let opening_sql = format!(
            "SELECT product_id, SUM(in_quantity - out_quantity) AS net FROM ({}) GROUP BY product_id",
            stock_flow_base_sql(&opening_where)
        );
        let rows = sqlx::query(AssertSqlSafe(opening_sql.as_str()))
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| StockFlowOpeningRow {
                product_id: r.get::<i64, _>("product_id"),
                net: r.get::<f64, _>("net"),
            })
            .collect())
    }

    async fn main_warehouse_name(&self) -> Result<String, DomainError> {
        let name: String = sqlx::query_scalar("SELECT name FROM warehouse WHERE id=1")
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        Ok(name)
    }

    async fn list_stock_warning_low(&self) -> Result<Vec<StockWarningRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT p.name as product_name, p.spec, p.unit, i.quantity as current_stock, i.min_stock
             FROM inventory i JOIN product p ON i.product_id = p.id
             WHERE i.quantity < i.min_stock ORDER BY (i.min_stock - i.quantity) DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows.iter().map(|r| map_stock_warning_row(r, "min_stock")).collect())
    }

    async fn list_stock_warning_high(&self) -> Result<Vec<StockWarningRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT p.name as product_name, p.spec, p.unit, i.quantity as current_stock, i.max_stock
             FROM inventory i JOIN product p ON i.product_id = p.id
             WHERE i.quantity > i.max_stock ORDER BY (i.quantity - i.max_stock) DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows.iter().map(|r| map_stock_warning_row(r, "max_stock")).collect())
    }

    async fn list_stock_warning_low_export(&self) -> Result<Vec<StockWarningRow>, DomainError> {
        let rows=sqlx::query("SELECT p.name as product_name,p.spec,p.unit,i.quantity as current_stock,i.min_stock FROM inventory i JOIN product p ON i.product_id=p.id WHERE i.quantity<i.min_stock ORDER BY (i.min_stock-i.quantity) DESC, p.id ASC").fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(|r| map_stock_warning_row(r, "min_stock")).collect())
    }

    async fn list_stock_warning_high_export(&self) -> Result<Vec<StockWarningRow>, DomainError> {
        let rows=sqlx::query("SELECT p.name as product_name,p.spec,p.unit,i.quantity as current_stock,i.max_stock FROM inventory i JOIN product p ON i.product_id=p.id WHERE i.quantity>i.max_stock ORDER BY (i.quantity-i.max_stock) DESC, p.id ASC").fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(|r| map_stock_warning_row(r, "max_stock")).collect())
    }

    async fn list_slow_stock(&self, days: i64) -> Result<Vec<SlowStockRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT p.id, p.name as product_name, p.spec, p.unit, i.quantity as current_stock,
                    COALESCE(soi.last_sale_date, '无销售记录') as last_sale_date
             FROM inventory i
             JOIN product p ON i.product_id = p.id
             LEFT JOIN (
                 SELECT soi.product_id, MAX(so.order_date) as last_sale_date
                 FROM sales_order_item soi
                 JOIN sales_order so ON soi.order_id = so.id
                 GROUP BY soi.product_id
             ) soi ON i.product_id = soi.product_id
             WHERE soi.last_sale_date IS NULL
                OR julianday('now') - julianday(soi.last_sale_date) > ?
             ORDER BY soi.last_sale_date ASC NULLS FIRST"
        )
        .bind(days)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SlowStockRow {
                id: row.get::<i64, _>("id"),
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                // 行为保持：旧 handler 对该列 try_get + unwrap_or 吞错
                current_stock: row.try_get::<f64, _>("current_stock").unwrap_or(0.0),
                last_sale_date: row.get::<String, _>("last_sale_date"),
            })
            .collect())
    }

    async fn list_slow_stock_export(&self, days: i64) -> Result<Vec<SlowStockExportRow>, DomainError> {
        let rows=sqlx::query("SELECT p.id,p.name as product_name,p.spec,p.unit,i.quantity as current_stock,i.quantity*p.base_price as amount,COALESCE(soi.last_sale_date,'无') as last_sale_date FROM inventory i JOIN product p ON i.product_id=p.id LEFT JOIN (SELECT soi.product_id,MAX(so.order_date) as last_sale_date FROM sales_order_item soi JOIN sales_order so ON soi.order_id=so.id GROUP BY soi.product_id) soi ON i.product_id=soi.product_id WHERE soi.last_sale_date IS NULL OR julianday('now')-julianday(soi.last_sale_date)>? ORDER BY soi.last_sale_date ASC NULLS FIRST, p.id ASC").bind(days).fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SlowStockExportRow {
                id: row.get::<i64, _>("id"),
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                // 行为保持：旧 handler 对这些数值列 try_get + unwrap_or 吞错
                current_stock: row.try_get::<f64, _>("current_stock").unwrap_or(0.0),
                amount: row.try_get::<f64, _>("amount").unwrap_or(0.0),
                last_sale_date: row.get::<String, _>("last_sale_date"),
            })
            .collect())
    }

    async fn stock_summary_purchase_rows(&self, start_date: &str, end_date: &str) -> Vec<StockSummaryPurchaseRow> {
        // 行为保持：查询失败旧 unwrap_or_default() → 空
        let mut purchase_where = String::from("WHERE 1=1");
        if !start_date.is_empty() {
            purchase_where.push_str(&format!(" AND po.order_date >= '{}'", start_date));
        }
        if !end_date.is_empty() {
            purchase_where.push_str(&format!(" AND po.order_date <= '{}'", end_date));
        }
        let purchase_sql = format!(
            "WITH src AS (
                SELECT po.order_date as day,
                       COALESCE(poi.warehouse_id, po.warehouse_id, 0) as warehouse_id,
                       COALESCE(NULLIF(TRIM(poi.warehouse_name), ''), po.warehouse_name, '未指定') as warehouse_name,
                       poi.amount as amount,
                       po.id as po_id
                FROM purchase_order po
                JOIN purchase_order_item poi ON poi.order_id = po.id
                {}
             )
             SELECT day,
                    warehouse_id,
                    warehouse_name,
                    COALESCE(SUM(amount), 0.0) as in_amount,
                    COUNT(*) as in_item_count,
                    COUNT(DISTINCT po_id) as in_order_count
             FROM src
             GROUP BY day, warehouse_id, warehouse_name",
            purchase_where
        );
        let rows = sqlx::query(AssertSqlSafe(purchase_sql.as_str()))
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        rows.iter()
            .map(|row| StockSummaryPurchaseRow {
                day: row.get("day"),
                warehouse_id: row.get::<i64, _>("warehouse_id"),
                warehouse_name: row.get::<String, _>("warehouse_name"),
                amount: row.get::<f64, _>("in_amount"),
                item_count: row.get::<i64, _>("in_item_count"),
                order_count: row.get::<i64, _>("in_order_count"),
            })
            .collect()
    }

    async fn stock_summary_sales_rows(&self, start_date: &str, end_date: &str) -> Vec<StockSummarySalesRow> {
        // 行为保持：查询失败旧 unwrap_or_default() → 空
        let mut sales_where = String::from("WHERE 1=1");
        if !start_date.is_empty() {
            sales_where.push_str(&format!(" AND so.order_date >= '{}'", start_date));
        }
        if !end_date.is_empty() {
            sales_where.push_str(&format!(" AND so.order_date <= '{}'", end_date));
        }
        let sales_sql = format!(
            "SELECT so.order_date as day,
                    COALESCE(so.warehouse_id, 0) as warehouse_id,
                    COALESCE(so.warehouse_name, '未指定') as warehouse_name,
                    soi.amount as out_amount,
                    so.id as order_id,
                    COALESCE(so.discount_rate, 0) as discount_rate
             FROM sales_order so
             JOIN sales_order_item soi ON soi.order_id = so.id
             {}",
            sales_where
        );
        let rows = sqlx::query(AssertSqlSafe(sales_sql.as_str()))
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        rows.iter().map(map_stock_summary_sales_row).collect()
    }

    async fn stock_summary_adj_target_rows(&self, start_date: &str, end_date: &str) -> Vec<StockSummarySalesRow> {
        // 行为保持：查询失败旧 unwrap_or_default() → 空
        let mut sales_where = String::from("WHERE 1=1");
        if !start_date.is_empty() {
            sales_where.push_str(&format!(" AND so.order_date >= '{}'", start_date));
        }
        if !end_date.is_empty() {
            sales_where.push_str(&format!(" AND so.order_date <= '{}'", end_date));
        }
        let adj1_sql = format!(
            "SELECT so.order_date as day,
                    COALESCE(so.warehouse_id, 0) as warehouse_id,
                    COALESCE(so.warehouse_name, '未指定') as warehouse_name,
                    osi.amount as out_amount,
                    COALESCE(so.discount_rate, 0) as discount_rate
             FROM order_supplement_item osi
             JOIN sales_order so ON osi.target_order_id = so.id
             {}",
            sales_where
        );
        let rows = sqlx::query(AssertSqlSafe(adj1_sql.as_str()))
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        rows.iter()
            .map(|row| map_stock_summary_sales_row_adj(row))
            .collect()
    }

    async fn stock_summary_adj_source_rows(&self, start_date: &str, end_date: &str) -> Vec<StockSummarySalesRow> {
        // 行为保持：查询失败旧 unwrap_or_default() → 空
        let mut sales_where = String::from("WHERE 1=1");
        if !start_date.is_empty() {
            sales_where.push_str(&format!(" AND so.order_date >= '{}'", start_date));
        }
        if !end_date.is_empty() {
            sales_where.push_str(&format!(" AND so.order_date <= '{}'", end_date));
        }
        let adj2_sql = format!(
            "SELECT so.order_date as day,
                    COALESCE(so.warehouse_id, 0) as warehouse_id,
                    COALESCE(so.warehouse_name, '未指定') as warehouse_name,
                    soi.amount as out_amount,
                    so.id as order_id,
                    COALESCE(so.discount_rate, 0) as discount_rate
             FROM sales_order so
             JOIN sales_order_item soi ON soi.order_id = so.id
             {} AND so.id IN (SELECT DISTINCT source_order_id FROM consumable_allocation)",
            sales_where
        );
        let rows = sqlx::query(AssertSqlSafe(adj2_sql.as_str()))
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        rows.iter().map(map_stock_summary_sales_row).collect()
    }
}

/// 出入库统计：销售明细行映射（adj 目标单口径无 order_id 列，固定 0）
fn map_stock_summary_sales_row(row: &sqlx::sqlite::SqliteRow) -> StockSummarySalesRow {
    StockSummarySalesRow {
        day: row.get("day"),
        warehouse_id: row.get::<i64, _>("warehouse_id"),
        warehouse_name: row.get::<String, _>("warehouse_name"),
        amount: row.get::<f64, _>("out_amount"),
        order_id: row.get::<i64, _>("order_id"),
        discount_rate: row.get::<f64, _>("discount_rate"),
    }
}

/// 出入库统计：分摊目标单明细行映射（SQL 未选 order_id，旧行为不参与去重计数）
fn map_stock_summary_sales_row_adj(row: &sqlx::sqlite::SqliteRow) -> StockSummarySalesRow {
    StockSummarySalesRow {
        day: row.get("day"),
        warehouse_id: row.get::<i64, _>("warehouse_id"),
        warehouse_name: row.get::<String, _>("warehouse_name"),
        amount: row.get::<f64, _>("out_amount"),
        order_id: 0,
        discount_rate: row.get::<f64, _>("discount_rate"),
    }
}

fn map_stock_flow_row(row: &sqlx::sqlite::SqliteRow) -> StockFlowRow {
    StockFlowRow {
        create_time: row.get::<String, _>("create_time"),
        movement_type: row.get::<String, _>("movement_type"),
        summary: row.get::<String, _>("summary"),
        direction: row.get::<String, _>("direction"),
        product_id: row.get::<i64, _>("product_id"),
        product_name: row.get::<String, _>("product_name"),
        spec: row.get::<Option<String>, _>("spec"),
        unit: row.get::<Option<String>, _>("unit"),
        // 行为保持：旧 handler 对这三列 try_get + unwrap_or 吞错
        in_quantity: row.try_get::<f64, _>("in_quantity").unwrap_or(0.0),
        out_quantity: row.try_get::<f64, _>("out_quantity").unwrap_or(0.0),
        ref_no: row.get::<Option<String>, _>("ref_no"),
        party_name: row.get::<Option<String>, _>("party_name"),
        // 行为保持：导出路径对这两列 try_get + unwrap_or 吞错（TEXT/NULL 列与 get 行为一致）
        production_date: row.try_get::<Option<String>, _>("production_date").unwrap_or(None),
        batch_no: row.try_get::<Option<String>, _>("batch_no").unwrap_or(None),
        orig_quantity: row.try_get::<f64, _>("orig_quantity").unwrap_or(0.0),
        orig_unit: row.get::<Option<String>, _>("orig_unit"),
    }
}

fn map_stock_warning_row(row: &sqlx::sqlite::SqliteRow, limit_col: &str) -> StockWarningRow {
    StockWarningRow {
        product_name: row.get::<String, _>("product_name"),
        spec: row.get::<Option<String>, _>("spec"),
        unit: row.get::<Option<String>, _>("unit"),
        // 行为保持：旧 handler 对这两列 try_get + unwrap_or 吞错
        current_stock: row.try_get::<f64, _>("current_stock").unwrap_or(0.0),
        limit_stock: row.try_get::<f64, _>(limit_col).unwrap_or(0.0),
    }
}
