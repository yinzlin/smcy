//! 采购查询仓储实现：SQL 与旧 api_query_purchase_* / api_query_product_price_trend handler 逐字一致
//!
//! 行为保持：count 查询的错误由用例层 unwrap()（保持旧 panic 语义）；明细/汇总查询
//! 的错误由用例层 unwrap_or_default() 吞错；商品信息查询由用例层 unwrap_or(None) 吞错。

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::query_purchase::{
    PriceCompareRow, PriceExportRow, PriceItemRow, ProductBaseRow, ProductInfoRow,
    PurchaseOrderQueryRow, QueryPurchaseRepo, SummaryProductRow, SummarySupplierRow,
    SupplierTodayPriceRow, TrendPurchaseRow, TrendSellingRow,
};

#[derive(Clone)]
pub struct SqliteQueryPurchaseRepo {
    pool: SqlitePool,
}

impl SqliteQueryPurchaseRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl QueryPurchaseRepo for SqliteQueryPurchaseRepo {
    async fn count_orders(
        &self,
        supplier_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
    ) -> Result<i64, DomainError> {
        let mut count_sql = String::from(
            "SELECT COUNT(*) as count FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();
        if !supplier_id.is_empty() {
            count_sql.push_str(" AND po.supplier_id = ?");
            binds.push(supplier_id.to_string());
        }
        if !start_date.is_empty() {
            count_sql.push_str(" AND po.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            count_sql.push_str(" AND po.order_date <= ?");
            binds.push(end_date.to_string());
        }
        if !status.is_empty() {
            count_sql.push_str(" AND po.status = ?");
            binds.push(status.to_string());
        }
        let mut count_query = sqlx::query(AssertSqlSafe(count_sql.as_str()));
        for b in &binds {
            count_query = count_query.bind(b);
        }
        let row = count_query.fetch_one(&self.pool).await.map_err(storage)?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn list_orders(
        &self,
        supplier_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<Vec<PurchaseOrderQueryRow>, DomainError> {
        let mut sql = String::from(
            "SELECT po.id, po.order_no, po.order_date, po.total_amount, po.final_amount, po.status, po.remark, s.name as supplier_name 
             FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();
        if !supplier_id.is_empty() {
            sql.push_str(" AND po.supplier_id = ?");
            binds.push(supplier_id.to_string());
        }
        if !start_date.is_empty() {
            sql.push_str(" AND po.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            sql.push_str(" AND po.order_date <= ?");
            binds.push(end_date.to_string());
        }
        if !status.is_empty() {
            sql.push_str(" AND po.status = ?");
            binds.push(status.to_string());
        }
        sql.push_str(" ORDER BY po.id DESC LIMIT ? OFFSET ?");

        let mut query = sqlx::query(AssertSqlSafe(sql.as_str()));
        for b in &binds {
            query = query.bind(b);
        }
        query = query.bind(page_size).bind(offset);

        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| PurchaseOrderQueryRow {
                id: row.get("id"),
                order_no: row.get("order_no"),
                order_date: row.get("order_date"),
                total_amount: row.get("total_amount"),
                final_amount: row.get("final_amount"),
                status: row.get("status"),
                remark: row.get("remark"),
                supplier_name: row.get("supplier_name"),
            })
            .collect())
    }

    async fn list_orders_for_export(
        &self,
        supplier_id: &str,
        start_date: &str,
        end_date: &str,
        status: &str,
    ) -> Result<Vec<PurchaseOrderQueryRow>, DomainError> {
        let mut sql = String::from(
            "SELECT po.id, po.order_no, po.order_date, po.total_amount, po.final_amount, po.status, po.remark, s.name as supplier_name 
             FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();
        if !supplier_id.is_empty() {
            sql.push_str(" AND po.supplier_id = ?");
            binds.push(supplier_id.to_string());
        }
        if !start_date.is_empty() {
            sql.push_str(" AND po.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            sql.push_str(" AND po.order_date <= ?");
            binds.push(end_date.to_string());
        }
        if !status.is_empty() {
            sql.push_str(" AND po.status = ?");
            binds.push(status.to_string());
        }
        sql.push_str(" ORDER BY po.id DESC");

        let mut query = sqlx::query(AssertSqlSafe(sql.as_str()));
        for b in &binds {
            query = query.bind(b);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| PurchaseOrderQueryRow {
                id: row.get("id"),
                order_no: row.get("order_no"),
                order_date: row.get("order_date"),
                total_amount: row.get("total_amount"),
                final_amount: row.get("final_amount"),
                status: row.get("status"),
                remark: row.get("remark"),
                supplier_name: row.get("supplier_name"),
            })
            .collect())
    }

    async fn count_price_items(&self, product_name: &str, supplier_id: &str) -> Result<i64, DomainError> {
        let mut base_sql = String::from(
            " FROM purchase_order_item poi
         JOIN purchase_order po ON poi.order_id = po.id
         JOIN supplier s ON po.supplier_id = s.id
         JOIN product p ON poi.product_id = p.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !product_name.is_empty() {
            base_sql.push_str(" AND poi.product_name LIKE ?");
            binds.push(format!("%{}%", product_name));
        }
        if !supplier_id.is_empty() {
            base_sql.push_str(" AND po.supplier_id = ?");
            binds.push(supplier_id.to_string());
        }

        let count_query = format!("SELECT COUNT(*){count_sql}", count_sql = base_sql);
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
        supplier_id: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<Vec<PriceItemRow>, DomainError> {
        let mut base_sql = String::from(
            " FROM purchase_order_item poi
         JOIN purchase_order po ON poi.order_id = po.id
         JOIN supplier s ON po.supplier_id = s.id
         JOIN product p ON poi.product_id = p.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !product_name.is_empty() {
            base_sql.push_str(" AND poi.product_name LIKE ?");
            binds.push(format!("%{}%", product_name));
        }
        if !supplier_id.is_empty() {
            base_sql.push_str(" AND po.supplier_id = ?");
            binds.push(supplier_id.to_string());
        }

        let data_sql = format!(
            "SELECT poi.product_name, poi.unit, poi.unit_price, poi.quantity, poi.amount, poi.production_date, poi.remark, p.shelf_life, po.order_date, s.name as supplier_name, s.phone as contact_phone {data_sql} ORDER BY po.order_date ASC, poi.id ASC LIMIT ? OFFSET ?",
            data_sql = base_sql
        );
        let mut query = sqlx::query(AssertSqlSafe(data_sql.as_str()));
        for b in &binds {
            query = query.bind(b);
        }
        query = query.bind(page_size).bind(offset);

        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| PriceItemRow {
                product_name: row.get("product_name"),
                unit: row.get("unit"),
                unit_price: row.get("unit_price"),
                quantity: row.get("quantity"),
                amount: row.get("amount"),
                production_date: row.get("production_date"),
                remark: row.get("remark"),
                shelf_life: row.get("shelf_life"),
                order_date: row.get("order_date"),
                supplier_name: row.get("supplier_name"),
                contact_phone: row.get("contact_phone"),
            })
            .collect())
    }

    async fn summary_by_supplier(&self, start_date: &str, end_date: &str) -> Result<Vec<SummarySupplierRow>, DomainError> {
        let mut supplier_sql = String::from(
            "SELECT s.name, SUM(poi.quantity) as quantity, SUM(poi.amount) as amount 
         FROM purchase_order_item poi 
         JOIN purchase_order po ON poi.order_id = po.id 
         JOIN supplier s ON po.supplier_id = s.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !start_date.is_empty() {
            supplier_sql.push_str(" AND po.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            supplier_sql.push_str(" AND po.order_date <= ?");
            binds.push(end_date.to_string());
        }
        supplier_sql.push_str(" GROUP BY s.id ORDER BY amount DESC");

        let mut query1 = sqlx::query(AssertSqlSafe(supplier_sql.as_str()));
        for b in &binds {
            query1 = query1.bind(b);
        }
        let rows = query1.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SummarySupplierRow {
                name: row.get("name"),
                quantity: row.get("quantity"),
                amount: row.get("amount"),
            })
            .collect())
    }

    async fn summary_by_product(&self, start_date: &str, end_date: &str) -> Result<Vec<SummaryProductRow>, DomainError> {
        let mut product_sql = String::from(
            "SELECT poi.product_name, poi.spec, SUM(poi.quantity) as quantity, SUM(poi.amount) as amount 
         FROM purchase_order_item poi 
         JOIN purchase_order po ON poi.order_id = po.id WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if !start_date.is_empty() {
            product_sql.push_str(" AND po.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            product_sql.push_str(" AND po.order_date <= ?");
            binds.push(end_date.to_string());
        }
        product_sql.push_str(" GROUP BY poi.product_name, poi.spec ORDER BY amount DESC");

        let mut query2 = sqlx::query(AssertSqlSafe(product_sql.as_str()));
        for b in &binds {
            query2 = query2.bind(b);
        }
        let rows = query2.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SummaryProductRow {
                product_name: row.get("product_name"),
                spec: row.get("spec"),
                quantity: row.get("quantity"),
                amount: row.get("amount"),
            })
            .collect())
    }

    async fn fetch_product_info(&self, product_id: i64) -> Result<Option<ProductInfoRow>, DomainError> {
        let row = sqlx::query("SELECT id, name, spec, base_unit FROM product WHERE id = ?")
            .bind(product_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| ProductInfoRow {
            id: r.get("id"),
            name: r.get("name"),
            spec: r.get("spec"),
            base_unit: r.get("base_unit"),
        }))
    }

    async fn list_same_base_product_ids(&self, name: &str, base_unit: &str) -> Result<Vec<i64>, DomainError> {
        let rows = sqlx::query("SELECT id FROM product WHERE name = ? AND base_unit = ?")
            .bind(name)
            .bind(base_unit)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows.iter().map(|r| r.get::<i64, _>("id")).collect())
    }

    async fn list_trend_purchase(
        &self,
        product_ids: &[i64],
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<TrendPurchaseRow>, DomainError> {
        let placeholders = product_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let purchase_sql = format!(
            "SELECT po.order_date as date, poi.unit_price as price, poi.unit as unit,
                poi.base_quantity as bq, poi.quantity as qty, poi.amount as amount,
                s.name as supplier_name
         FROM purchase_order_item poi
         JOIN purchase_order po ON poi.order_id = po.id
         JOIN supplier s ON po.supplier_id = s.id
         WHERE poi.product_id IN ({})
         {} {}
         ORDER BY po.order_date ASC",
            placeholders,
            if start_date.is_empty() { String::new() } else { format!("AND po.order_date >= '{}'", start_date.replace('\'', "''")) },
            if end_date.is_empty() { String::new() } else { format!("AND po.order_date <= '{}'", end_date.replace('\'', "''")) },
        );
        let mut purchase_q = sqlx::query(AssertSqlSafe(purchase_sql));
        for pid in product_ids {
            purchase_q = purchase_q.bind(pid);
        }
        let rows = purchase_q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| TrendPurchaseRow {
                date: r.get("date"),
                price: r.get("price"),
                unit: r.get("unit"),
                bq: r.get("bq"),
                qty: r.get("qty"),
                amount: r.get("amount"),
                supplier_name: r.get("supplier_name"),
            })
            .collect())
    }

    async fn list_trend_selling(
        &self,
        product_ids: &[i64],
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<TrendSellingRow>, DomainError> {
        let placeholders = product_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let selling_sql = format!(
            "SELECT so.order_date as date, soi.unit_price as price, soi.unit as unit,
                soi.base_quantity as bq, soi.quantity as qty, soi.amount as amount,
                p.name as purchaser_name
         FROM sales_order_item soi
         JOIN sales_order so ON soi.order_id = so.id
         JOIN purchaser p ON so.purchaser_id = p.id
         WHERE soi.product_id IN ({})
         {} {}
         ORDER BY so.order_date ASC",
            placeholders,
            if start_date.is_empty() { String::new() } else { format!("AND so.order_date >= '{}'", start_date.replace('\'', "''")) },
            if end_date.is_empty() { String::new() } else { format!("AND so.order_date <= '{}'", end_date.replace('\'', "''")) },
        );
        let mut selling_q = sqlx::query(AssertSqlSafe(selling_sql));
        for pid in product_ids {
            selling_q = selling_q.bind(pid);
        }
        let rows = selling_q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| TrendSellingRow {
                date: r.get("date"),
                price: r.get("price"),
                unit: r.get("unit"),
                bq: r.get("bq"),
                qty: r.get("qty"),
                amount: r.get("amount"),
                purchaser_name: r.get("purchaser_name"),
            })
            .collect())
    }

    async fn fetch_product_base(&self, product_id: i64) -> Result<Option<ProductBaseRow>, DomainError> {
        let row = sqlx::query("SELECT name, base_unit FROM product WHERE id = ?")
            .bind(product_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| ProductBaseRow {
            name: r.get("name"),
            base_unit: r.get("base_unit"),
        }))
    }

    async fn list_price_history(&self, product_id: i64) -> Result<Vec<PriceCompareRow>, DomainError> {
        let sql = "SELECT po.order_date as date, poi.unit_price as price, poi.unit as unit,
                     poi.base_quantity as bq, poi.quantity as qty, poi.amount as amount,
                     s.id as supplier_id, s.name as supplier_name
              FROM purchase_order_item poi
              JOIN purchase_order po ON poi.order_id = po.id
              JOIN supplier s ON po.supplier_id = s.id
              WHERE poi.product_id = ?
              ORDER BY po.order_date DESC";
        let rows = sqlx::query(sql)
            .bind(product_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| PriceCompareRow {
                date: r.get("date"),
                price: r.get("price"),
                unit: r.get("unit"),
                bq: r.get("bq"),
                qty: r.get("qty"),
                amount: r.get("amount"),
                supplier_id: r.get("supplier_id"),
                supplier_name: r.get("supplier_name"),
            })
            .collect())
    }

    async fn list_today_supplier_prices(&self, product_id: i64) -> Result<Vec<SupplierTodayPriceRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT psp.supplier_id, s.name as supplier_name, psp.price, psp.updated_at
         FROM product_supplier_price psp
         JOIN supplier s ON psp.supplier_id = s.id
         WHERE psp.product_id = ?",
        )
        .bind(product_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| SupplierTodayPriceRow {
                supplier_id: r.get("supplier_id"),
                supplier_name: r.get("supplier_name"),
                price: r.get("price"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    async fn list_price_items_for_export(&self, product_name: &str, supplier_id: &str) -> Result<Vec<PriceExportRow>, DomainError> {
        let mut base_sql = String::from(" FROM purchase_order_item poi JOIN purchase_order po ON poi.order_id = po.id JOIN supplier s ON po.supplier_id = s.id JOIN product p ON poi.product_id = p.id WHERE 1=1");
        let mut binds: Vec<String> = Vec::new();
        if !product_name.is_empty() { base_sql.push_str(" AND poi.product_name LIKE ?"); binds.push(format!("%{}%", product_name)); }
        if !supplier_id.is_empty() { base_sql.push_str(" AND po.supplier_id = ?"); binds.push(supplier_id.to_string()); }
        let data_sql = format!("SELECT poi.product_name, poi.unit, poi.unit_price, poi.quantity, poi.amount, poi.production_date, poi.remark, p.shelf_life, po.order_date, s.name as supplier_name {} ORDER BY po.order_date ASC, poi.id ASC", base_sql);
        let mut query = sqlx::query(AssertSqlSafe(data_sql.as_str())); for b in &binds { query = query.bind(b); }
        let rows = query.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| PriceExportRow {
                product_name: row.get("product_name"),
                unit: row.get("unit"),
                unit_price: row.get("unit_price"),
                quantity: row.get("quantity"),
                amount: row.get("amount"),
                production_date: row.get("production_date"),
                remark: row.get("remark"),
                shelf_life: row.get("shelf_life"),
                order_date: row.get("order_date"),
                supplier_name: row.get("supplier_name"),
            })
            .collect())
    }

    async fn summary_export_by_supplier(&self, start_date: &str, end_date: &str) -> Result<Vec<SummarySupplierRow>, DomainError> {
        let mut supplier_sql = String::from("SELECT s.name, SUM(poi.quantity) as quantity, SUM(poi.amount) as amount FROM purchase_order_item poi JOIN purchase_order po ON poi.order_id = po.id JOIN supplier s ON po.supplier_id = s.id WHERE 1=1");
        let mut binds: Vec<String> = Vec::new();
        if !start_date.is_empty() { supplier_sql.push_str(" AND po.order_date >= ?"); binds.push(start_date.to_string()); }
        if !end_date.is_empty() { supplier_sql.push_str(" AND po.order_date <= ?"); binds.push(end_date.to_string()); }
        supplier_sql.push_str(" GROUP BY s.id ORDER BY amount DESC, s.id ASC");
        let mut q1 = sqlx::query(AssertSqlSafe(supplier_sql.as_str()));
        for b in &binds { q1 = q1.bind(b); }
        let rows = q1.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SummarySupplierRow {
                name: row.get("name"),
                quantity: row.get("quantity"),
                amount: row.get("amount"),
            })
            .collect())
    }

    async fn summary_export_by_product(&self, start_date: &str, end_date: &str) -> Result<Vec<SummaryProductRow>, DomainError> {
        let mut product_sql = String::from("SELECT poi.product_name, poi.spec, SUM(poi.quantity) as quantity, SUM(poi.amount) as amount FROM purchase_order_item poi JOIN purchase_order po ON poi.order_id = po.id WHERE 1=1");
        let mut binds: Vec<String> = Vec::new();
        if !start_date.is_empty() { product_sql.push_str(" AND po.order_date >= ?"); binds.push(start_date.to_string()); }
        if !end_date.is_empty() { product_sql.push_str(" AND po.order_date <= ?"); binds.push(end_date.to_string()); }
        product_sql.push_str(" GROUP BY poi.product_name, poi.spec ORDER BY amount DESC, poi.product_name ASC, poi.spec ASC");
        let mut q2 = sqlx::query(AssertSqlSafe(product_sql.as_str()));
        for b in &binds { q2 = q2.bind(b); }
        let rows = q2.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SummaryProductRow {
                product_name: row.get("product_name"),
                spec: row.get("spec"),
                quantity: row.get("quantity"),
                amount: row.get("amount"),
            })
            .collect())
    }
}
