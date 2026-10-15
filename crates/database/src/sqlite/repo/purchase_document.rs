//! 采购单据仓储实现：SQL 与旧 api_purchase_document_* handler 逐字一致
//!
//! 行为保持：列表查询的吞错（unwrap_or_default）由用例层执行；
//! 删除前置读的 fetch_optional 吞错（unwrap_or(None)）在本仓储内保持。

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::purchase_document::{
    PurchaseDocumentExportRow, PurchaseDocumentFilter, PurchaseDocumentRepo, PurchaseDocumentRow,
};

#[derive(Clone)]
pub struct SqlitePurchaseDocumentRepo {
    pool: SqlitePool,
}

impl SqlitePurchaseDocumentRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl PurchaseDocumentRepo for SqlitePurchaseDocumentRepo {
    async fn list(&self, filter: &PurchaseDocumentFilter) -> Result<Vec<PurchaseDocumentRow>, DomainError> {
        let mut sql = "SELECT id, supplier_id, supplier_name, document_date, image_url, remark, create_at FROM purchase_document WHERE 1=1".to_string();
        let rows = match (filter.supplier_id, filter.document_date.is_empty()) {
            (Some(sid), false) => {
                sql.push_str(" AND supplier_id = ? AND document_date = ?");
                sql.push_str(" ORDER BY create_at DESC, id DESC");
                sqlx::query(AssertSqlSafe(sql.as_str()))
                    .bind(sid)
                    .bind(filter.document_date.as_str())
                    .fetch_all(&self.pool)
                    .await
                    .map_err(storage)?
            },
            (Some(sid), true) => {
                sql.push_str(" AND supplier_id = ? ORDER BY create_at DESC, id DESC");
                sqlx::query(AssertSqlSafe(sql.as_str()))
                    .bind(sid)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(storage)?
            },
            (None, false) => {
                sql.push_str(" AND document_date = ? ORDER BY create_at DESC, id DESC");
                sqlx::query(AssertSqlSafe(sql.as_str()))
                    .bind(filter.document_date.as_str())
                    .fetch_all(&self.pool)
                    .await
                    .map_err(storage)?
            },
            (None, true) => {
                sql.push_str(" ORDER BY create_at DESC, id DESC");
                sqlx::query(AssertSqlSafe(sql.as_str()))
                    .fetch_all(&self.pool)
                    .await
                    .map_err(storage)?
            },
        };
        Ok(rows
            .iter()
            .map(|row| PurchaseDocumentRow {
                id: row.get::<i64, _>("id"),
                supplier_id: row.get::<i64, _>("supplier_id"),
                supplier_name: row.get::<String, _>("supplier_name"),
                document_date: row.get::<String, _>("document_date"),
                image_url: row.get::<String, _>("image_url"),
                remark: row.get::<Option<String>, _>("remark"),
                create_at: row.get::<String, _>("create_at"),
            })
            .collect())
    }

    async fn list_export(&self, filter: &PurchaseDocumentFilter) -> Result<Vec<PurchaseDocumentExportRow>, DomainError> {
        let mut sql = "SELECT id,supplier_id,supplier_name,document_date,remark,create_at FROM purchase_document WHERE 1=1".to_string();
        let rows = match (filter.supplier_id, filter.document_date.is_empty()) {
            (Some(sid), false) => {
                sql.push_str(" AND supplier_id=? AND document_date=? ORDER BY create_at DESC, id DESC");
                sqlx::query(AssertSqlSafe(sql.as_str()))
                    .bind(sid)
                    .bind(filter.document_date.as_str())
                    .fetch_all(&self.pool)
                    .await
                    .map_err(storage)?
            },
            (Some(sid), true) => {
                sql.push_str(" AND supplier_id=? ORDER BY create_at DESC, id DESC");
                sqlx::query(AssertSqlSafe(sql.as_str()))
                    .bind(sid)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(storage)?
            },
            (None, false) => {
                sql.push_str(" AND document_date=? ORDER BY create_at DESC, id DESC");
                sqlx::query(AssertSqlSafe(sql.as_str()))
                    .bind(filter.document_date.as_str())
                    .fetch_all(&self.pool)
                    .await
                    .map_err(storage)?
            },
            (None, true) => {
                sql.push_str(" ORDER BY create_at DESC, id DESC");
                sqlx::query(AssertSqlSafe(sql.as_str()))
                    .fetch_all(&self.pool)
                    .await
                    .map_err(storage)?
            },
        };
        Ok(rows
            .iter()
            .map(|row| PurchaseDocumentExportRow {
                id: row.get::<i64, _>("id"),
                supplier_id: row.get::<i64, _>("supplier_id"),
                supplier_name: row.get::<String, _>("supplier_name"),
                document_date: row.get::<String, _>("document_date"),
                remark: row.get::<Option<String>, _>("remark"),
                create_at: row.get::<String, _>("create_at"),
            })
            .collect())
    }

    async fn insert(&self, supplier_id: i64, supplier_name: &str, document_date: &str, image_url: &str, remark: Option<&str>) -> Result<i64, DomainError> {
        let r = sqlx::query(
            "INSERT INTO purchase_document(supplier_id, supplier_name, document_date, image_url, remark) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(supplier_id)
        .bind(supplier_name)
        .bind(document_date)
        .bind(image_url)
        .bind(remark)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(r.last_insert_rowid())
    }

    async fn get_image_url_and_supplier(&self, id: i64) -> Option<(String, i64)> {
        let row = sqlx::query("SELECT image_url, supplier_id FROM purchase_document WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .unwrap_or(None)?;
        Some((row.get("image_url"), row.get::<i64, _>("supplier_id")))
    }

    async fn delete(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM purchase_document WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
}
