//! 统计概览仓储实现：SQL 与旧 api_query_overview* / api_query_category_stats* /
//! api_query_document_summary* handler 逐字一致（JSON 版多行 SQL 与导出版紧凑 SQL 分开保留）
//!
//! 行为保持：全部查询的错误由用例层 unwrap_or(0.0)/unwrap_or_default() 吞错；
//! 行列读取 get 与旧 handler 一致（列类型不符时 panic 行为相同）。

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use domain::error::DomainError;
use domain::port::query_stats::{
    CategoryStatsRow, DocumentSummaryRow, NameAmountQuantityRow, QueryStatsRepo,
};

#[derive(Clone)]
pub struct SqliteQueryStatsRepo {
    pool: SqlitePool,
}

impl SqliteQueryStatsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl QueryStatsRepo for SqliteQueryStatsRepo {
    async fn sum_purchase_total(&self, month: &str) -> Result<f64, DomainError> {
        let total: f64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(po.total_amount), 0.0) FROM purchase_order po WHERE strftime('%Y-%m', po.order_date) = ?"
        )
        .bind(month)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        Ok(total)
    }

    async fn sum_sales_total(&self, month: &str) -> Result<f64, DomainError> {
        let total: f64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(so.total_amount), 0.0) FROM sales_order so WHERE strftime('%Y-%m', so.order_date) = ?"
        )
        .bind(month)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        Ok(total)
    }

    async fn sum_stock_total(&self) -> Result<f64, DomainError> {
        let total: f64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(i.quantity * pr.selling_price), 0.0) FROM inventory i JOIN product pr ON i.product_id = pr.id"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        Ok(total)
    }

    async fn list_purchase_by_supplier(&self, month: &str) -> Result<Vec<NameAmountQuantityRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT s.name, COALESCE(SUM(poi.amount), 0.0) as amount, COALESCE(SUM(poi.quantity), 0.0) as quantity
             FROM purchase_order_item poi
             JOIN purchase_order po ON poi.order_id = po.id
             JOIN supplier s ON po.supplier_id = s.id
             WHERE strftime('%Y-%m', po.order_date) = ?
             GROUP BY s.id, s.name
             ORDER BY amount DESC"
        )
        .bind(month)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| NameAmountQuantityRow {
                name: row.get::<String, _>("name"),
                amount: row.get::<f64, _>("amount"),
                quantity: row.get::<f64, _>("quantity"),
            })
            .collect())
    }

    async fn list_sales_by_purchaser(&self, month: &str) -> Result<Vec<NameAmountQuantityRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT p.name, COALESCE(SUM(soi.amount), 0.0) as amount, COALESCE(SUM(soi.quantity), 0.0) as quantity
             FROM sales_order_item soi
             JOIN sales_order so ON soi.order_id = so.id
             JOIN purchaser p ON so.purchaser_id = p.id
             WHERE strftime('%Y-%m', so.order_date) = ?
             GROUP BY p.id, p.name
             ORDER BY amount DESC"
        )
        .bind(month)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| NameAmountQuantityRow {
                name: row.get::<String, _>("name"),
                amount: row.get::<f64, _>("amount"),
                quantity: row.get::<f64, _>("quantity"),
            })
            .collect())
    }

    async fn sum_purchase_total_export(&self, month: &str) -> Result<f64, DomainError> {
        let total: f64 = sqlx::query_scalar("SELECT COALESCE(SUM(po.total_amount),0.0) FROM purchase_order po WHERE strftime('%Y-%m',po.order_date)=?")
            .bind(month)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        Ok(total)
    }

    async fn sum_sales_total_export(&self, month: &str) -> Result<f64, DomainError> {
        let total: f64 = sqlx::query_scalar("SELECT COALESCE(SUM(so.total_amount),0.0) FROM sales_order so WHERE strftime('%Y-%m',so.order_date)=?")
            .bind(month)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        Ok(total)
    }

    async fn sum_stock_total_export(&self) -> Result<f64, DomainError> {
        let total: f64 = sqlx::query_scalar("SELECT COALESCE(SUM(i.quantity*pr.selling_price),0.0) FROM inventory i JOIN product pr ON i.product_id=pr.id")
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        Ok(total)
    }

    async fn list_purchase_by_supplier_export(&self, month: &str) -> Result<Vec<NameAmountQuantityRow>, DomainError> {
        let rows = sqlx::query("SELECT s.name,COALESCE(SUM(poi.amount),0.0) as amount,COALESCE(SUM(poi.quantity),0.0) as quantity FROM purchase_order_item poi JOIN purchase_order po ON poi.order_id=po.id JOIN supplier s ON po.supplier_id=s.id WHERE strftime('%Y-%m',po.order_date)=? GROUP BY s.id,s.name ORDER BY amount DESC, s.id ASC")
            .bind(month)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| NameAmountQuantityRow {
                name: row.get::<String, _>("name"),
                amount: row.get::<f64, _>("amount"),
                quantity: row.get::<f64, _>("quantity"),
            })
            .collect())
    }

    async fn list_sales_by_purchaser_export(&self, month: &str) -> Result<Vec<NameAmountQuantityRow>, DomainError> {
        let rows = sqlx::query("SELECT p.name,COALESCE(SUM(soi.amount),0.0) as amount,COALESCE(SUM(soi.quantity),0.0) as quantity FROM sales_order_item soi JOIN sales_order so ON soi.order_id=so.id JOIN purchaser p ON so.purchaser_id=p.id WHERE strftime('%Y-%m',so.order_date)=? GROUP BY p.id,p.name ORDER BY amount DESC, p.id ASC")
            .bind(month)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| NameAmountQuantityRow {
                name: row.get::<String, _>("name"),
                amount: row.get::<f64, _>("amount"),
                quantity: row.get::<f64, _>("quantity"),
            })
            .collect())
    }

    async fn list_category_stats(&self, start_date: &str, end_date: &str) -> Result<Vec<CategoryStatsRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT pc.id, pc.name as category_name,
                    COALESCE((SELECT SUM(poi.quantity) FROM purchase_order_item poi 
                              JOIN purchase_order po ON poi.order_id = po.id
                              JOIN product pr ON poi.product_id = pr.id
                              WHERE pr.category_id = pc.id AND po.order_date >= ? AND po.order_date <= ?), 0.0) as purchase_quantity,
                    COALESCE((SELECT SUM(poi.amount) FROM purchase_order_item poi 
                              JOIN purchase_order po ON poi.order_id = po.id
                              JOIN product pr ON poi.product_id = pr.id
                              WHERE pr.category_id = pc.id AND po.order_date >= ? AND po.order_date <= ?), 0.0) as purchase_amount,
                    COALESCE((SELECT SUM(soi.quantity) FROM sales_order_item soi 
                              JOIN sales_order so ON soi.order_id = so.id
                              JOIN product pr ON soi.product_id = pr.id
                              WHERE pr.category_id = pc.id AND so.order_date >= ? AND so.order_date <= ?), 0.0) as sales_quantity,
                    COALESCE((SELECT SUM(soi.amount) FROM sales_order_item soi 
                              JOIN sales_order so ON soi.order_id = so.id
                              JOIN product pr ON soi.product_id = pr.id
                              WHERE pr.category_id = pc.id AND so.order_date >= ? AND so.order_date <= ?), 0.0) as sales_amount,
                    COALESCE((SELECT SUM(i.quantity) FROM inventory i JOIN product pr ON i.product_id = pr.id WHERE pr.category_id = pc.id), 0.0) as stock_quantity,
                    COALESCE((SELECT SUM(i.quantity * pr.selling_price) FROM inventory i JOIN product pr ON i.product_id = pr.id WHERE pr.category_id = pc.id), 0.0) as stock_amount
             FROM category pc
             WHERE pc.entity_type = 'product' AND pc.parent_id IS NULL
             ORDER BY pc.id"
        )
        .bind(start_date)
        .bind(end_date)
        .bind(start_date)
        .bind(end_date)
        .bind(start_date)
        .bind(end_date)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| CategoryStatsRow {
                category_name: row.get::<String, _>("category_name"),
                purchase_quantity: row.get::<f64, _>("purchase_quantity"),
                purchase_amount: row.get::<f64, _>("purchase_amount"),
                sales_quantity: row.get::<f64, _>("sales_quantity"),
                sales_amount: row.get::<f64, _>("sales_amount"),
                stock_quantity: row.get::<f64, _>("stock_quantity"),
                stock_amount: row.get::<f64, _>("stock_amount"),
            })
            .collect())
    }

    async fn list_category_stats_export(&self, start_date: &str, end_date: &str) -> Result<Vec<CategoryStatsRow>, DomainError> {
        let rows = sqlx::query("SELECT pc.id,pc.name as category_name,COALESCE((SELECT SUM(poi.quantity) FROM purchase_order_item poi JOIN purchase_order po ON poi.order_id=po.id JOIN product pr ON poi.product_id=pr.id WHERE pr.category_id=pc.id AND po.order_date>=? AND po.order_date<=?),0.0) as purchase_quantity,COALESCE((SELECT SUM(poi.amount) FROM purchase_order_item poi JOIN purchase_order po ON poi.order_id=po.id JOIN product pr ON poi.product_id=pr.id WHERE pr.category_id=pc.id AND po.order_date>=? AND po.order_date<=?),0.0) as purchase_amount,COALESCE((SELECT SUM(soi.quantity) FROM sales_order_item soi JOIN sales_order so ON soi.order_id=so.id JOIN product pr ON soi.product_id=pr.id WHERE pr.category_id=pc.id AND so.order_date>=? AND so.order_date<=?),0.0) as sales_quantity,COALESCE((SELECT SUM(soi.amount) FROM sales_order_item soi JOIN sales_order so ON soi.order_id=so.id JOIN product pr ON soi.product_id=pr.id WHERE pr.category_id=pc.id AND so.order_date>=? AND so.order_date<=?),0.0) as sales_amount,COALESCE((SELECT SUM(i.quantity) FROM inventory i JOIN product pr ON i.product_id=pr.id WHERE pr.category_id=pc.id),0.0) as stock_quantity,COALESCE((SELECT SUM(i.quantity*pr.selling_price) FROM inventory i JOIN product pr ON i.product_id=pr.id WHERE pr.category_id=pc.id),0.0) as stock_amount FROM category pc WHERE pc.entity_type='product' AND pc.parent_id IS NULL ORDER BY pc.id")
            .bind(start_date).bind(end_date).bind(start_date).bind(end_date).bind(start_date).bind(end_date).bind(start_date).bind(end_date)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| CategoryStatsRow {
                category_name: row.get::<String, _>("category_name"),
                purchase_quantity: row.get::<f64, _>("purchase_quantity"),
                purchase_amount: row.get::<f64, _>("purchase_amount"),
                sales_quantity: row.get::<f64, _>("sales_quantity"),
                sales_amount: row.get::<f64, _>("sales_amount"),
                stock_quantity: row.get::<f64, _>("stock_quantity"),
                stock_amount: row.get::<f64, _>("stock_amount"),
            })
            .collect())
    }

    async fn list_document_summary(&self, month: &str) -> Result<Vec<DocumentSummaryRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT strftime('%Y-%m', po.order_date) as month,
                    COUNT(DISTINCT po.id) as purchase_count,
                    COALESCE(SUM(po.total_amount), 0.0) as purchase_amount,
                    COALESCE((SELECT COUNT(DISTINCT so.id) FROM sales_order so WHERE strftime('%Y-%m', so.order_date) = strftime('%Y-%m', po.order_date)), 0) as sales_count,
                    COALESCE((SELECT SUM(so.total_amount) FROM sales_order so WHERE strftime('%Y-%m', so.order_date) = strftime('%Y-%m', po.order_date)), 0.0) as sales_amount
             FROM purchase_order po
             WHERE strftime('%Y-%m', po.order_date) = ?
             GROUP BY strftime('%Y-%m', po.order_date)
             UNION ALL
             SELECT strftime('%Y-%m', so.order_date) as month,
                    COALESCE((SELECT COUNT(DISTINCT po.id) FROM purchase_order po WHERE strftime('%Y-%m', po.order_date) = strftime('%Y-%m', so.order_date)), 0) as purchase_count,
                    COALESCE((SELECT SUM(po.total_amount) FROM purchase_order po WHERE strftime('%Y-%m', po.order_date) = strftime('%Y-%m', so.order_date)), 0.0) as purchase_amount,
                    COUNT(DISTINCT so.id) as sales_count,
                    COALESCE(SUM(so.total_amount), 0.0) as sales_amount
             FROM sales_order so
             WHERE strftime('%Y-%m', so.order_date) = ?
             GROUP BY strftime('%Y-%m', so.order_date)"
        )
        .bind(month)
        .bind(month)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| DocumentSummaryRow {
                month: row.get::<String, _>("month"),
                purchase_count: row.get::<i64, _>("purchase_count"),
                purchase_amount: row.get::<f64, _>("purchase_amount"),
                sales_count: row.get::<i64, _>("sales_count"),
                sales_amount: row.get::<f64, _>("sales_amount"),
            })
            .collect())
    }

    async fn list_document_summary_export(&self, month: &str) -> Result<Vec<DocumentSummaryRow>, DomainError> {
        let rows = sqlx::query("SELECT strftime('%Y-%m',po.order_date) as month,COUNT(DISTINCT po.id) as purchase_count,COALESCE(SUM(po.total_amount),0.0) as purchase_amount,COALESCE((SELECT COUNT(DISTINCT so.id) FROM sales_order so WHERE strftime('%Y-%m',so.order_date)=strftime('%Y-%m',po.order_date)),0) as sales_count,COALESCE((SELECT SUM(so.total_amount) FROM sales_order so WHERE strftime('%Y-%m',so.order_date)=strftime('%Y-%m',po.order_date)),0.0) as sales_amount FROM purchase_order po WHERE strftime('%Y-%m',po.order_date)=? GROUP BY strftime('%Y-%m',po.order_date) UNION ALL SELECT strftime('%Y-%m',so.order_date) as month,COALESCE((SELECT COUNT(DISTINCT po.id) FROM purchase_order po WHERE strftime('%Y-%m',po.order_date)=strftime('%Y-%m',so.order_date)),0) as purchase_count,COALESCE((SELECT SUM(po.total_amount) FROM purchase_order po WHERE strftime('%Y-%m',po.order_date)=strftime('%Y-%m',so.order_date)),0.0) as purchase_amount,COUNT(DISTINCT so.id) as sales_count,COALESCE(SUM(so.total_amount),0.0) as sales_amount FROM sales_order so WHERE strftime('%Y-%m',so.order_date)=? GROUP BY strftime('%Y-%m',so.order_date)")
            .bind(month)
            .bind(month)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| DocumentSummaryRow {
                month: row.get::<String, _>("month"),
                purchase_count: row.get::<i64, _>("purchase_count"),
                purchase_amount: row.get::<f64, _>("purchase_amount"),
                sales_count: row.get::<i64, _>("sales_count"),
                sales_amount: row.get::<f64, _>("sales_amount"),
            })
            .collect())
    }
}
