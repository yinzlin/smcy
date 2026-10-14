//! 今日进价采集仓储实现：SQL 与旧 api_product_today_price_* handler 逐字一致

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::today_price::{
    TodayPriceCategoryRow, TodayPriceExportRow, TodayPriceItemRow, TodayPriceRepo,
};

#[derive(Clone)]
pub struct SqliteTodayPriceRepo {
    pool: SqlitePool,
}

impl SqliteTodayPriceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl TodayPriceRepo for SqliteTodayPriceRepo {
    async fn fetch_items(&self, date: Option<&str>) -> Result<Vec<TodayPriceItemRow>, DomainError> {
        let has_date = date.is_some();
        let where_sql = if has_date {
            "WHERE so.order_date = ?"
        } else {
            "WHERE so.status IN ('pending', 'sorting')"
        };
        let sql = format!(
            "SELECT soi.product_id, MAX(soi.product_name) as product_name, MAX(COALESCE(soi.spec,'')) as spec,
                MAX(soi.supplier_id) as supplier_id, MAX(s.name) as supplier_name,
                MAX(p.base_unit) as base_unit,
                MAX(p.purchase_price) as purchase_price,
                MAX(p.max_purchase_price) as max_purchase_price,
                MAX(p.min_purchase_price) as min_purchase_price,
                MAX(psp.price) as supplier_price,
                COALESCE(NULLIF(TRIM(COALESCE(soi.unit, '')), ''), '') as sale_unit,
                SUM(soi.quantity) as unit_qty,
                SUM(CASE WHEN soi.base_quantity > 0 THEN soi.base_quantity ELSE soi.quantity END) as base_qty
         FROM sales_order_item soi
         LEFT JOIN sales_order so ON soi.order_id = so.id
         LEFT JOIN supplier s ON soi.supplier_id = s.id
         LEFT JOIN product p ON soi.product_id = p.id
         LEFT JOIN product_supplier_price psp ON psp.product_id = soi.product_id AND psp.supplier_id = soi.supplier_id
         {} AND soi.product_id > 0
         GROUP BY soi.supplier_id, soi.product_id, sale_unit
         ORDER BY MAX(s.name), soi.supplier_id, MAX(soi.product_name), soi.product_id, sale_unit",
            where_sql
        );
        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        if has_date {
            if let Some(d) = date {
                q = q.bind(d);
            }
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| TodayPriceItemRow {
                product_id: r.get::<i64, _>("product_id"),
                product_name: r.get::<Option<String>, _>("product_name"),
                spec: r.get::<Option<String>, _>("spec"),
                supplier_id: r.get::<i64, _>("supplier_id"),
                supplier_name: r.get::<Option<String>, _>("supplier_name"),
                base_unit: r.get::<Option<String>, _>("base_unit"),
                purchase_price: r.get::<Option<f64>, _>("purchase_price"),
                max_purchase_price: r.get::<Option<f64>, _>("max_purchase_price"),
                min_purchase_price: r.get::<Option<f64>, _>("min_purchase_price"),
                supplier_price: r.get::<Option<f64>, _>("supplier_price"),
                sale_unit: r.get::<String, _>("sale_unit"),
                unit_qty: r.get::<Option<f64>, _>("unit_qty"),
                base_qty: r.get::<Option<f64>, _>("base_qty"),
            })
            .collect())
    }

    async fn fetch_export_rows(
        &self,
        date: Option<&str>,
    ) -> Result<Vec<TodayPriceExportRow>, DomainError> {
        let has_date = date.is_some();
        let where_sql = if has_date {
            "WHERE so.order_date = ?"
        } else {
            "WHERE so.status IN ('pending', 'sorting')"
        };
        let sql = format!(
            "SELECT soi.product_id, MAX(soi.product_name) as product_name, MAX(COALESCE(soi.spec,'')) as spec,
                MAX(soi.supplier_id) as supplier_id, MAX(s.name) as supplier_name,
                MAX(p.base_unit) as base_unit,
                MAX(p.purchase_price) as purchase_price,
                MAX(p.max_purchase_price) as max_purchase_price,
                MAX(p.min_purchase_price) as min_purchase_price,
                MAX(psp.price) as supplier_price,
                SUM(CASE WHEN soi.base_quantity > 0 THEN soi.base_quantity ELSE soi.quantity END) as total_qty
         FROM sales_order_item soi
         LEFT JOIN sales_order so ON soi.order_id = so.id
         LEFT JOIN supplier s ON soi.supplier_id = s.id
         LEFT JOIN product p ON soi.product_id = p.id
         LEFT JOIN product_supplier_price psp ON psp.product_id = soi.product_id AND psp.supplier_id = soi.supplier_id
         {} AND soi.product_id > 0
         GROUP BY soi.supplier_id, soi.product_id
         ORDER BY MAX(s.name), soi.supplier_id, MAX(soi.product_name), soi.product_id",
            where_sql
        );
        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        if has_date {
            if let Some(d) = date {
                q = q.bind(d);
            }
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| TodayPriceExportRow {
                product_id: r.get::<i64, _>("product_id"),
                product_name: r.get::<Option<String>, _>("product_name"),
                spec: r.get::<Option<String>, _>("spec"),
                supplier_id: r.get::<i64, _>("supplier_id"),
                supplier_name: r.get::<Option<String>, _>("supplier_name"),
                base_unit: r.get::<Option<String>, _>("base_unit"),
                purchase_price: r.get::<Option<f64>, _>("purchase_price"),
                max_purchase_price: r.get::<Option<f64>, _>("max_purchase_price"),
                min_purchase_price: r.get::<Option<f64>, _>("min_purchase_price"),
                supplier_price: r.get::<Option<f64>, _>("supplier_price"),
                total_qty: r.get::<Option<f64>, _>("total_qty"),
            })
            .collect())
    }

    async fn fetch_category_rows(
        &self,
        date: Option<&str>,
    ) -> Result<Vec<TodayPriceCategoryRow>, DomainError> {
        let has_date = date.is_some();
        let where_sql = if has_date {
            "WHERE so.order_date = ?"
        } else {
            "WHERE so.status IN ('pending', 'sorting')"
        };
        let sql = format!(
            "SELECT soi.product_id, MAX(soi.product_name) as product_name, MAX(COALESCE(soi.spec,'')) as spec,
                MAX(p.base_unit) as base_unit,
                MAX(p.purchase_price) as purchase_price,
                MAX(p.max_purchase_price) as max_purchase_price,
                MAX(p.min_purchase_price) as min_purchase_price,
                SUM(CASE WHEN soi.base_quantity > 0 THEN soi.base_quantity ELSE soi.quantity END) as total_qty,
                MAX(c.name) as category_name,
                MAX(c2.name) as parent_name
         FROM sales_order_item soi
         LEFT JOIN sales_order so ON soi.order_id = so.id
         LEFT JOIN product p ON soi.product_id = p.id
         LEFT JOIN category c ON p.category_id = c.id
         LEFT JOIN category c2 ON c.parent_id = c2.id
         {} AND soi.product_id > 0
         GROUP BY soi.product_id
         ORDER BY MAX(c2.name), MAX(c.name), MAX(soi.product_name), soi.product_id",
            where_sql
        );
        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        if has_date {
            if let Some(d) = date {
                q = q.bind(d);
            }
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| TodayPriceCategoryRow {
                product_id: r.get::<i64, _>("product_id"),
                product_name: r.get::<Option<String>, _>("product_name"),
                spec: r.get::<Option<String>, _>("spec"),
                base_unit: r.get::<Option<String>, _>("base_unit"),
                purchase_price: r.get::<Option<f64>, _>("purchase_price"),
                max_purchase_price: r.get::<Option<f64>, _>("max_purchase_price"),
                min_purchase_price: r.get::<Option<f64>, _>("min_purchase_price"),
                total_qty: r.get::<Option<f64>, _>("total_qty"),
                category_name: r.get::<Option<String>, _>("category_name"),
                parent_name: r.get::<Option<String>, _>("parent_name"),
            })
            .collect())
    }

    async fn upsert_supplier_price(
        &self,
        product_id: i64,
        supplier_id: i64,
        price: f64,
    ) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO product_supplier_price(product_id, supplier_id, price, updated_at)
             VALUES (?, ?, ?, datetime('now','localtime'))
             ON CONFLICT(product_id, supplier_id) DO UPDATE SET price = excluded.price, updated_at = excluded.updated_at",
        )
        .bind(product_id)
        .bind(supplier_id)
        .bind(price)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn get_price_range(&self, product_id: i64) -> Result<Option<(f64, f64, f64)>, DomainError> {
        let row = sqlx::query(
            "SELECT purchase_price, max_purchase_price, min_purchase_price FROM product WHERE id = ?"
        )
        .bind(product_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.map(|r| {
            (r.get::<f64, _>("purchase_price"), r.get::<f64, _>("max_purchase_price"), r.get::<f64, _>("min_purchase_price"))
        }))
    }

    async fn update_price_range(
        &self,
        product_id: i64,
        price: f64,
        max_price: f64,
        min_price: f64,
    ) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE product SET purchase_price = ?, max_purchase_price = ?, min_purchase_price = ? WHERE id = ?"
        )
        .bind(price)
        .bind(max_price)
        .bind(min_price)
        .bind(product_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }
}
