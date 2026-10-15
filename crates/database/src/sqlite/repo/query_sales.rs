//! 销售查询仓储实现：SQL 与旧 api_query_sales_* / api_query_reimburse_summary* / api_query_product_rank* handler 逐字一致
//!
//! 行为保持：count 查询的错误由用例层 unwrap()（保持旧 panic 语义）；明细/汇总/聚合查询
//! 的错误由用例层 unwrap_or_default() 吞错；呆滞商品行的 stock_quantity/last_sale_date
//! 用 try_get + unwrap_or 吞错（与旧 handler 一致）。

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::query_sales::{
    QuerySalesRepo, RankSlowRow, RankTopRow, ReimburseOrderRow, ReimburseProductRealRow,
    ReimburseProductSuppRow, ReimbursePurchaserRow, SalesOrderQueryRow, SalesPriceExportRow,
    SalesPriceItemRow, SalesSummaryProductRow, SalesSummaryPurchaserRow,
};

#[derive(Clone)]
pub struct SqliteQuerySalesRepo {
    pool: SqlitePool,
}

impl SqliteQuerySalesRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

/// 报销口径共用的日期条件片段（与旧 handler 的 date_cond/binds 构造逐字一致）
fn reimburse_date_cond(start_date: &str, end_date: &str) -> (String, Vec<String>) {
    let mut date_cond = String::from("");
    let mut binds: Vec<String> = Vec::new();
    if !start_date.is_empty() {
        date_cond.push_str(" AND so.order_date >= ?");
        binds.push(start_date.to_string());
    }
    if !end_date.is_empty() {
        date_cond.push_str(" AND so.order_date <= ?");
        binds.push(end_date.to_string());
    }
    (date_cond, binds)
}

impl QuerySalesRepo for SqliteQuerySalesRepo {
    async fn count_orders(
        &self,
        purchaser_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
    ) -> Result<i64, DomainError> {
        let mut base_sql = String::from(
            " FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !purchaser_id.is_empty() {
            base_sql.push_str(" AND so.purchaser_id = ?");
            binds.push(purchaser_id.to_string());
        }
        if !start_date.is_empty() {
            base_sql.push_str(" AND so.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            base_sql.push_str(" AND so.order_date <= ?");
            binds.push(end_date.to_string());
        }
        if !status.is_empty() {
            base_sql.push_str(" AND so.status = ?");
            binds.push(status.to_string());
        }

        let count_query = format!("SELECT COUNT(*){}", base_sql);
        let mut count_q = sqlx::query(AssertSqlSafe(count_query.as_str()));
        for b in &binds {
            count_q = count_q.bind(b);
        }
        let row = count_q.fetch_one(&self.pool).await.map_err(storage)?;
        Ok(row.get::<i64, _>("COUNT(*)"))
    }

    async fn list_orders(
        &self,
        purchaser_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
        order_clause: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<Vec<SalesOrderQueryRow>, DomainError> {
        let mut base_sql = String::from(
            " FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !purchaser_id.is_empty() {
            base_sql.push_str(" AND so.purchaser_id = ?");
            binds.push(purchaser_id.to_string());
        }
        if !start_date.is_empty() {
            base_sql.push_str(" AND so.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            base_sql.push_str(" AND so.order_date <= ?");
            binds.push(end_date.to_string());
        }
        if !status.is_empty() {
            base_sql.push_str(" AND so.status = ?");
            binds.push(status.to_string());
        }

        let data_sql = format!(
            "SELECT so.id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.final_amount, so.status, so.remark, p.name as purchaser_name {} ORDER BY {} LIMIT ? OFFSET ?",
            base_sql, order_clause
        );
        let mut query = sqlx::query(AssertSqlSafe(data_sql.as_str()));
        for b in &binds {
            query = query.bind(b);
        }
        query = query.bind(page_size).bind(offset);

        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SalesOrderQueryRow {
                id: row.get::<i64, _>("id"),
                order_no: row.get::<String, _>("order_no"),
                order_date: row.get::<String, _>("order_date"),
                total_amount: row.get("total_amount"),
                discount_rate: row.get("discount_rate"),
                final_amount: row.get("final_amount"),
                status: row.get::<String, _>("status"),
                remark: row.get::<Option<String>, _>("remark"),
                purchaser_name: row.get::<String, _>("purchaser_name"),
            })
            .collect())
    }

    async fn list_orders_for_export(
        &self,
        purchaser_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
    ) -> Result<Vec<SalesOrderQueryRow>, DomainError> {
        let mut sql = String::from(
            "SELECT so.id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.final_amount, so.status, so.remark, p.name as purchaser_name
             FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !purchaser_id.is_empty() {
            sql.push_str(" AND so.purchaser_id = ?");
            binds.push(purchaser_id.to_string());
        }
        if !start_date.is_empty() {
            sql.push_str(" AND so.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            sql.push_str(" AND so.order_date <= ?");
            binds.push(end_date.to_string());
        }
        if !status.is_empty() {
            sql.push_str(" AND so.status = ?");
            binds.push(status.to_string());
        }
        sql.push_str(" ORDER BY so.id DESC");

        let mut query = sqlx::query(AssertSqlSafe(sql.as_str()));
        for b in &binds {
            query = query.bind(b);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SalesOrderQueryRow {
                id: row.get::<i64, _>("id"),
                order_no: row.get::<String, _>("order_no"),
                order_date: row.get::<String, _>("order_date"),
                total_amount: row.get("total_amount"),
                discount_rate: row.get("discount_rate"),
                final_amount: row.get("final_amount"),
                status: row.get::<String, _>("status"),
                remark: row.get::<Option<String>, _>("remark"),
                purchaser_name: row.get::<String, _>("purchaser_name"),
            })
            .collect())
    }

    async fn count_price_items(
        &self,
        product_name: &str,
        product_id: &str,
        purchaser_id: &str,
    ) -> Result<i64, DomainError> {
        let mut base_sql = String::from(
            " FROM sales_order_item soi
         JOIN sales_order so ON soi.order_id = so.id
         JOIN purchaser p ON so.purchaser_id = p.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !product_name.is_empty() {
            base_sql.push_str(" AND soi.product_name LIKE ?");
            binds.push(format!("%{}%", product_name));
        }
        if !product_id.is_empty() {
            base_sql.push_str(" AND soi.product_id = ?");
            binds.push(product_id.to_string());
        }
        if !purchaser_id.is_empty() {
            base_sql.push_str(" AND so.purchaser_id = ?");
            binds.push(purchaser_id.to_string());
        }

        let count_query = format!("SELECT COUNT(*){}", base_sql);
        let mut count_q = sqlx::query(AssertSqlSafe(count_query.as_str()));
        for b in &binds {
            count_q = count_q.bind(b);
        }
        let row = count_q.fetch_one(&self.pool).await.map_err(storage)?;
        Ok(row.get::<i64, _>("COUNT(*)"))
    }

    async fn list_price_items(
        &self,
        product_name: &str,
        product_id: &str,
        purchaser_id: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<Vec<SalesPriceItemRow>, DomainError> {
        let mut base_sql = String::from(
            " FROM sales_order_item soi
         JOIN sales_order so ON soi.order_id = so.id
         JOIN purchaser p ON so.purchaser_id = p.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !product_name.is_empty() {
            base_sql.push_str(" AND soi.product_name LIKE ?");
            binds.push(format!("%{}%", product_name));
        }
        if !product_id.is_empty() {
            base_sql.push_str(" AND soi.product_id = ?");
            binds.push(product_id.to_string());
        }
        if !purchaser_id.is_empty() {
            base_sql.push_str(" AND so.purchaser_id = ?");
            binds.push(purchaser_id.to_string());
        }

        let data_sql = format!(
            "SELECT soi.product_name, soi.spec, soi.unit_price, soi.quantity, soi.unit, so.order_date, p.name as purchaser_name {} ORDER BY so.order_date DESC LIMIT ? OFFSET ?",
            base_sql
        );
        let mut query = sqlx::query(AssertSqlSafe(data_sql.as_str()));
        for b in &binds {
            query = query.bind(b);
        }
        query = query.bind(page_size).bind(offset);

        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SalesPriceItemRow {
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                purchaser_name: row.get::<String, _>("purchaser_name"),
                unit_price: row.get("unit_price"),
                order_date: row.get::<String, _>("order_date"),
                quantity: row.get("quantity"),
            })
            .collect())
    }

    async fn summary_by_purchaser(&self, start_date: &str, end_date: &str) -> Result<Vec<SalesSummaryPurchaserRow>, DomainError> {
        let mut purchaser_sql = String::from(
            "SELECT p.name, SUM(soi.quantity) as quantity, SUM(soi.amount) as sales_amount
         FROM sales_order_item soi
         JOIN sales_order so ON soi.order_id = so.id
         JOIN purchaser p ON so.purchaser_id = p.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !start_date.is_empty() {
            purchaser_sql.push_str(" AND so.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            purchaser_sql.push_str(" AND so.order_date <= ?");
            binds.push(end_date.to_string());
        }
        purchaser_sql.push_str(" GROUP BY p.id ORDER BY sales_amount DESC");

        let mut query1 = sqlx::query(AssertSqlSafe(purchaser_sql.as_str()));
        for b in &binds {
            query1 = query1.bind(b);
        }
        let rows = query1.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SalesSummaryPurchaserRow {
                name: row.get::<String, _>("name"),
                quantity: row.get("quantity"),
                sales_amount: row.get("sales_amount"),
            })
            .collect())
    }

    async fn summary_by_product(&self, start_date: &str, end_date: &str) -> Result<Vec<SalesSummaryProductRow>, DomainError> {
        let mut product_sql = String::from(
            "SELECT soi.product_name, soi.spec, SUM(soi.quantity) as quantity, SUM(soi.amount) as sales_amount
         FROM sales_order_item soi
         JOIN sales_order so ON soi.order_id = so.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !start_date.is_empty() {
            product_sql.push_str(" AND so.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            product_sql.push_str(" AND so.order_date <= ?");
            binds.push(end_date.to_string());
        }
        product_sql.push_str(" GROUP BY soi.product_name, soi.spec ORDER BY sales_amount DESC");

        let mut query2 = sqlx::query(AssertSqlSafe(product_sql.as_str()));
        for b in &binds {
            query2 = query2.bind(b);
        }
        let rows = query2.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SalesSummaryProductRow {
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                quantity: row.get("quantity"),
                sales_amount: row.get("sales_amount"),
            })
            .collect())
    }

    async fn list_allocation_source_ids(&self) -> Result<Vec<i64>, DomainError> {
        let ids = sqlx::query_scalar::<_, i64>("SELECT DISTINCT source_order_id FROM consumable_allocation")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(ids)
    }

    async fn reimburse_real_by_purchaser(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimbursePurchaserRow>, DomainError> {
        let (date_cond, binds) = reimburse_date_cond(start_date, end_date);
        let real_purchaser_sql = format!(
            "SELECT so.purchaser_id, p.name, SUM(soi.amount) as amount
         FROM sales_order_item soi
         JOIN sales_order so ON soi.order_id = so.id
         JOIN purchaser p ON so.purchaser_id = p.id
         WHERE 1=1 {}
         GROUP BY so.purchaser_id", date_cond
        );
        let mut q = sqlx::query(AssertSqlSafe(real_purchaser_sql.as_str()));
        for b in &binds { q = q.bind(b); }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| ReimbursePurchaserRow {
                purchaser_id: r.get::<i64, _>("purchaser_id"),
                name: r.get::<String, _>("name"),
                amount: r.get::<f64, _>("amount"),
            })
            .collect())
    }

    async fn reimburse_supp_by_purchaser(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimbursePurchaserRow>, DomainError> {
        let (date_cond, binds) = reimburse_date_cond(start_date, end_date);
        let supp_target_sql = format!(
            "SELECT so.purchaser_id, p.name, SUM(osi.amount) as supp_amount
         FROM order_supplement_item osi
         JOIN sales_order so ON osi.target_order_id = so.id
         JOIN purchaser p ON so.purchaser_id = p.id
         WHERE 1=1 {}
         GROUP BY so.purchaser_id", date_cond
        );
        let mut q = sqlx::query(AssertSqlSafe(supp_target_sql.as_str()));
        for b in &binds { q = q.bind(b); }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| ReimbursePurchaserRow {
                purchaser_id: r.get::<i64, _>("purchaser_id"),
                name: r.get::<String, _>("name"),
                amount: r.get::<f64, _>("supp_amount"),
            })
            .collect())
    }

    async fn reimburse_real_by_order(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimburseOrderRow>, DomainError> {
        let (date_cond, binds) = reimburse_date_cond(start_date, end_date);
        let source_real_sql = format!(
            "SELECT so.purchaser_id, p.name, so.id as order_id, SUM(soi.amount) as amount
         FROM sales_order_item soi
         JOIN sales_order so ON soi.order_id = so.id
         JOIN purchaser p ON so.purchaser_id = p.id
         WHERE 1=1 {}
         GROUP BY so.id", date_cond
        );
        let mut q = sqlx::query(AssertSqlSafe(source_real_sql.as_str()));
        for b in &binds { q = q.bind(b); }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| ReimburseOrderRow {
                purchaser_id: r.get::<i64, _>("purchaser_id"),
                name: r.get::<String, _>("name"),
                order_id: r.get::<i64, _>("order_id"),
                amount: r.get::<f64, _>("amount"),
            })
            .collect())
    }

    async fn reimburse_product_real(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimburseProductRealRow>, DomainError> {
        let (date_cond, binds) = reimburse_date_cond(start_date, end_date);
        let prod_real_sql = format!(
            "SELECT soi.product_name, COALESCE(soi.spec,'') as spec, soi.order_id, soi.quantity, soi.amount
         FROM sales_order_item soi
         JOIN sales_order so ON soi.order_id = so.id
         WHERE 1=1 {}", date_cond
        );
        let mut q = sqlx::query(AssertSqlSafe(prod_real_sql.as_str()));
        for b in &binds { q = q.bind(b); }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| ReimburseProductRealRow {
                product_name: r.get::<String, _>("product_name"),
                spec: r.get::<String, _>("spec"),
                order_id: r.get::<i64, _>("order_id"),
                quantity: r.get::<f64, _>("quantity"),
                amount: r.get::<f64, _>("amount"),
            })
            .collect())
    }

    async fn reimburse_product_supp(&self, start_date: &str, end_date: &str) -> Result<Vec<ReimburseProductSuppRow>, DomainError> {
        let (date_cond, binds) = reimburse_date_cond(start_date, end_date);
        let prod_supp_sql = format!(
            "SELECT osi.product_name, COALESCE(osi.spec,'') as spec, osi.quantity, osi.amount
         FROM order_supplement_item osi
         JOIN sales_order so ON osi.target_order_id = so.id
         WHERE 1=1 {}", date_cond
        );
        let mut q = sqlx::query(AssertSqlSafe(prod_supp_sql.as_str()));
        for b in &binds { q = q.bind(b); }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| ReimburseProductSuppRow {
                product_name: r.get::<String, _>("product_name"),
                spec: r.get::<String, _>("spec"),
                quantity: r.get::<f64, _>("quantity"),
                amount: r.get::<f64, _>("amount"),
            })
            .collect())
    }

    async fn rank_top(&self, start_date: &str, end_date: &str) -> Result<Vec<RankTopRow>, DomainError> {
        let mut top_sql = String::from(
            "SELECT soi.product_name, soi.spec, SUM(soi.quantity) as quantity, SUM(soi.amount) as amount
         FROM sales_order_item soi
         JOIN sales_order so ON soi.order_id = so.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !start_date.is_empty() {
            top_sql.push_str(" AND so.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            top_sql.push_str(" AND so.order_date <= ?");
            binds.push(end_date.to_string());
        }
        top_sql.push_str(" GROUP BY soi.product_name, soi.spec ORDER BY quantity DESC, soi.product_name ASC, soi.spec ASC LIMIT 10");

        let mut query1 = sqlx::query(AssertSqlSafe(top_sql.as_str()));
        for b in &binds {
            query1 = query1.bind(b);
        }
        let rows = query1.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| RankTopRow {
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                quantity: row.get("quantity"),
                amount: row.get("amount"),
            })
            .collect())
    }

    async fn rank_slow(&self, start_date: &str, end_date: &str) -> Result<Vec<RankSlowRow>, DomainError> {
        let slow_sql = String::from(
            "SELECT pr.id, pr.name as product_name, pr.spec, pr.stock_quantity,
                (SELECT MAX(so.order_date) FROM sales_order so
                 JOIN sales_order_item soi ON so.id = soi.order_id
                 WHERE soi.product_id = pr.id) as last_sale_date
         FROM product pr
         WHERE pr.status = 1
         AND pr.id NOT IN (
             SELECT DISTINCT soi.product_id
             FROM sales_order_item soi
             JOIN sales_order so ON soi.order_id = so.id
             WHERE 1=1",
        );
        let mut slow_sql_complete = slow_sql;
        let mut binds: Vec<String> = Vec::new();
        if !start_date.is_empty() {
            slow_sql_complete.push_str(" AND so.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            slow_sql_complete.push_str(" AND so.order_date <= ?");
            binds.push(end_date.to_string());
        }
        slow_sql_complete.push_str(") ORDER BY pr.id LIMIT 50");

        let mut query2 = sqlx::query(AssertSqlSafe(slow_sql_complete.as_str()));
        for b in &binds {
            query2 = query2.bind(b);
        }
        let rows = query2.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| RankSlowRow {
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                // 行为保持：旧 handler 对这两列 try_get + unwrap_or 吞错
                stock_quantity: row.try_get("stock_quantity").unwrap_or(0.0),
                last_sale_date: row.try_get("last_sale_date").unwrap_or(None),
            })
            .collect())
    }

    async fn list_price_for_export(&self, product_name: &str, purchaser_id: &str) -> Result<Vec<SalesPriceExportRow>, DomainError> {
        let mut base_sql = String::from(" FROM sales_order_item soi JOIN sales_order so ON soi.order_id = so.id JOIN purchaser p ON so.purchaser_id = p.id WHERE 1=1");
        let mut binds: Vec<String> = Vec::new();
        if !product_name.is_empty() { base_sql.push_str(" AND soi.product_name LIKE ?"); binds.push(format!("%{}%", product_name)); }
        if !purchaser_id.is_empty() { base_sql.push_str(" AND so.purchaser_id = ?"); binds.push(purchaser_id.to_string()); }
        let data_sql = format!("SELECT soi.product_name, soi.spec, soi.unit, soi.unit_price, soi.quantity, so.order_date, p.name as purchaser_name {} ORDER BY so.order_date DESC, so.id DESC, soi.id ASC", base_sql);
        let mut query = sqlx::query(AssertSqlSafe(data_sql.as_str())); for b in &binds { query = query.bind(b); }
        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SalesPriceExportRow {
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<Option<String>, _>("unit"),
                unit_price: row.get("unit_price"),
                quantity: row.get("quantity"),
                order_date: row.get::<String, _>("order_date"),
                purchaser_name: row.get::<String, _>("purchaser_name"),
            })
            .collect())
    }

    async fn summary_export_by_purchaser(&self, start_date: &str, end_date: &str) -> Result<Vec<SalesSummaryPurchaserRow>, DomainError> {
        let mut purchaser_sql = String::from("SELECT p.name, SUM(soi.quantity) as quantity, SUM(soi.amount) as sales_amount FROM sales_order_item soi JOIN sales_order so ON soi.order_id = so.id JOIN purchaser p ON so.purchaser_id = p.id WHERE 1=1");
        let mut binds: Vec<String> = Vec::new();
        if !start_date.is_empty() { purchaser_sql.push_str(" AND so.order_date >= ?"); binds.push(start_date.to_string()); }
        if !end_date.is_empty() { purchaser_sql.push_str(" AND so.order_date <= ?"); binds.push(end_date.to_string()); }
        purchaser_sql.push_str(" GROUP BY p.id ORDER BY sales_amount DESC, p.id ASC");
        let mut q1 = sqlx::query(AssertSqlSafe(purchaser_sql.as_str()));
        for b in &binds { q1 = q1.bind(b); }
        let rows = q1.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SalesSummaryPurchaserRow {
                name: row.get::<String, _>("name"),
                quantity: row.get("quantity"),
                sales_amount: row.get("sales_amount"),
            })
            .collect())
    }

    async fn summary_export_by_product(&self, start_date: &str, end_date: &str) -> Result<Vec<SalesSummaryProductRow>, DomainError> {
        let mut product_sql = String::from("SELECT soi.product_name, soi.spec, SUM(soi.quantity) as quantity, SUM(soi.amount) as sales_amount FROM sales_order_item soi JOIN sales_order so ON soi.order_id = so.id WHERE 1=1");
        let mut binds: Vec<String> = Vec::new();
        if !start_date.is_empty() { product_sql.push_str(" AND so.order_date >= ?"); binds.push(start_date.to_string()); }
        if !end_date.is_empty() { product_sql.push_str(" AND so.order_date <= ?"); binds.push(end_date.to_string()); }
        product_sql.push_str(" GROUP BY soi.product_name, soi.spec ORDER BY sales_amount DESC, soi.product_name ASC, soi.spec ASC");
        let mut q2 = sqlx::query(AssertSqlSafe(product_sql.as_str()));
        for b in &binds { q2 = q2.bind(b); }
        let rows = q2.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SalesSummaryProductRow {
                product_name: row.get::<String, _>("product_name"),
                spec: row.get::<Option<String>, _>("spec"),
                quantity: row.get("quantity"),
                sales_amount: row.get("sales_amount"),
            })
            .collect())
    }
}
