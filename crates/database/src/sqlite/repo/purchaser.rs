//! purchaser 仓储实现：SQL 与旧 api_purchaser_* handler 逐字一致

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::purchaser::{ImportPurchaser, PurchaserFilter, PurchaserRepo, PurchaserRow, UpsertPurchaser};

#[derive(Clone)]
pub struct SqlitePurchaserRepo {
    pool: SqlitePool,
}

impl SqlitePurchaserRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

fn row_to_purchaser(r: &sqlx::sqlite::SqliteRow) -> PurchaserRow {
    PurchaserRow {
        id: r.get("id"),
        name: r.get("name"),
        contact: r.get("contact"),
        phone: r.get("phone"),
        address: r.get("address"),
        business_scope: r.get("business_scope"),
        remark: r.get("remark"),
        category_id: r.get("category_id"),
        category_name: r.get("category_name"),
        audit_status: r.get("audit_status"),
    }
}

const SELECT_COLS: &str = "SELECT p.id, p.name, p.contact, p.phone, p.address, p.business_scope, p.remark, p.category_id, p.audit_status, c.name as category_name\n             FROM purchaser p LEFT JOIN category c ON p.category_id = c.id";

impl PurchaserRepo for SqlitePurchaserRepo {
    async fn list(&self, filter: &PurchaserFilter) -> Result<Vec<PurchaserRow>, DomainError> {
        let rows = if let Some(cid) = filter.category_id {
            sqlx::query(AssertSqlSafe(format!(
                "{}\n             WHERE p.category_id IN (\n                 WITH RECURSIVE cat_tree(id) AS (\n                     SELECT id FROM category WHERE id = ?\n                     UNION ALL\n                     SELECT c.id FROM category c\n                     JOIN cat_tree ct ON c.parent_id = ct.id\n                 )\n                 SELECT id FROM cat_tree\n             )\n             AND p.name LIKE ?\n             ORDER BY p.id DESC",
                SELECT_COLS
            )))
            .bind(cid)
            .bind(&filter.keyword_pattern)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
        } else {
            sqlx::query(AssertSqlSafe(format!(
                "{}\n             WHERE p.name LIKE ?\n             ORDER BY p.id DESC",
                SELECT_COLS
            )))
            .bind(&filter.keyword_pattern)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
        };
        Ok(rows.iter().map(row_to_purchaser).collect())
    }

    async fn list_for_export(&self) -> Result<Vec<PurchaserRow>, DomainError> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "{} ORDER BY p.id",
            SELECT_COLS
        )))
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows.iter().map(row_to_purchaser).collect())
    }

    async fn create(&self, p: &UpsertPurchaser) -> Result<(), DomainError> {
        sqlx::query("INSERT INTO purchaser(name, contact, phone, address, business_scope, remark, category_id) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&p.name)
            .bind(&p.contact)
            .bind(&p.phone)
            .bind(&p.address)
            .bind(&p.business_scope)
            .bind(&p.remark)
            .bind(&p.category_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn update(&self, id: i64, p: &UpsertPurchaser) -> Result<(), DomainError> {
        sqlx::query("UPDATE purchaser SET name=?, contact=?, phone=?, address=?, business_scope=?, remark=?, category_id=?, audit_status='pending' WHERE id=?")
            .bind(&p.name)
            .bind(&p.contact)
            .bind(&p.phone)
            .bind(&p.address)
            .bind(&p.business_scope)
            .bind(&p.remark)
            .bind(&p.category_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn delete(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM purchaser WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn get_audit_status(&self, id: i64) -> Result<Option<String>, DomainError> {
        sqlx::query_scalar("SELECT audit_status FROM purchaser WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }

    async fn find_category_id(&self, name: &str, entity_type: &str) -> Result<Option<i64>, DomainError> {
        sqlx::query("SELECT id FROM category WHERE name = ? AND entity_type = ?")
            .bind(name)
            .bind(entity_type)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| Ok(r.get::<i64, _>("id")))
            .transpose()
    }

    async fn import_row(&self, p: &ImportPurchaser) -> Result<bool, DomainError> {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO purchaser(name, contact, phone, address, business_scope, remark, category_id) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&p.name)
        .bind(&p.contact)
        .bind(&p.phone)
        .bind(&p.address)
        .bind(&p.business_scope)
        .bind(&p.remark)
        .bind(&p.category_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.rows_affected() > 0)
    }
}
