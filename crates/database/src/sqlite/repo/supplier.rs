//! supplier 仓储实现：SQL 与旧 api_supplier_* handler 逐字一致

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::supplier::{ImportSupplier, SupplierFilter, SupplierRepo, SupplierRow, UpsertSupplier};

#[derive(Clone)]
pub struct SqliteSupplierRepo {
    pool: SqlitePool,
}

impl SqliteSupplierRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

fn row_to_supplier(r: &sqlx::sqlite::SqliteRow) -> SupplierRow {
    SupplierRow {
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
        is_designated: r.get::<Option<i64>, _>("is_designated").unwrap_or(0),
        designated_no: r.get("designated_no"),
        legal_person: r.get("legal_person"),
        credit_code: r.get("credit_code"),
        license_no: r.get("license_no"),
        license_expire: r.get("license_expire"),
        contract_no: r.get("contract_no"),
        contract_start: r.get("contract_start"),
        contract_end: r.get("contract_end"),
    }
}

const SELECT_COLS: &str = "SELECT s.id, s.name, s.contact, s.phone, s.address, s.business_scope, s.remark, s.category_id, s.audit_status, c.name as category_name,\n                    s.is_designated, s.designated_no, s.legal_person, s.credit_code, s.license_no, s.license_expire,\n                    s.contract_no, s.contract_start, s.contract_end\n             FROM supplier s LEFT JOIN category c ON s.category_id = c.id";

impl SupplierRepo for SqliteSupplierRepo {
    async fn list(&self, filter: &SupplierFilter) -> Result<Vec<SupplierRow>, DomainError> {
        let rows = if let Some(cid) = filter.category_id {
            sqlx::query(AssertSqlSafe(format!(
                "{}\n             WHERE s.category_id IN (\n                 WITH RECURSIVE cat_tree(id) AS (\n                     SELECT id FROM category WHERE id = ?\n                     UNION ALL\n                     SELECT c.id FROM category c \n                     JOIN cat_tree ct ON c.parent_id = ct.id\n                 )\n                 SELECT id FROM cat_tree\n             )\n             AND s.name LIKE ?\n             ORDER BY s.id DESC",
                SELECT_COLS
            )))
            .bind(cid)
            .bind(&filter.keyword_pattern)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
        } else {
            sqlx::query(AssertSqlSafe(format!(
                "{}\n             WHERE s.name LIKE ?\n             ORDER BY s.id DESC",
                SELECT_COLS
            )))
            .bind(&filter.keyword_pattern)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
        };
        Ok(rows.iter().map(row_to_supplier).collect())
    }

    async fn list_for_export(&self) -> Result<Vec<SupplierRow>, DomainError> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "{} ORDER BY s.id",
            SELECT_COLS
        )))
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows.iter().map(row_to_supplier).collect())
    }

    async fn create(&self, s: &UpsertSupplier) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO supplier(name, contact, phone, address, business_scope, remark, category_id,\n                              is_designated, designated_no, legal_person, credit_code, license_no, license_expire,\n                              contract_no, contract_start, contract_end)\n         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&s.name)
        .bind(&s.contact)
        .bind(&s.phone)
        .bind(&s.address)
        .bind(&s.business_scope)
        .bind(&s.remark)
        .bind(&s.category_id)
        .bind(s.is_designated)
        .bind(&s.designated_no)
        .bind(&s.legal_person)
        .bind(&s.credit_code)
        .bind(&s.license_no)
        .bind(&s.license_expire)
        .bind(&s.contract_no)
        .bind(&s.contract_start)
        .bind(&s.contract_end)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn update(&self, id: i64, s: &UpsertSupplier) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE supplier SET name=?, contact=?, phone=?, address=?, business_scope=?, remark=?, category_id=?,\n                              is_designated=?, designated_no=?, legal_person=?, credit_code=?, license_no=?, license_expire=?,\n                              contract_no=?, contract_start=?, contract_end=?, audit_status='pending' WHERE id=?",
        )
        .bind(&s.name)
        .bind(&s.contact)
        .bind(&s.phone)
        .bind(&s.address)
        .bind(&s.business_scope)
        .bind(&s.remark)
        .bind(&s.category_id)
        .bind(s.is_designated)
        .bind(&s.designated_no)
        .bind(&s.legal_person)
        .bind(&s.credit_code)
        .bind(&s.license_no)
        .bind(&s.license_expire)
        .bind(&s.contract_no)
        .bind(&s.contract_start)
        .bind(&s.contract_end)
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn delete(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM supplier WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn get_audit_status(&self, id: i64) -> Result<Option<String>, DomainError> {
        sqlx::query_scalar("SELECT audit_status FROM supplier WHERE id = ?")
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

    async fn import_row(&self, s: &ImportSupplier) -> Result<bool, DomainError> {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO supplier(name, contact, phone, address, business_scope, remark, category_id) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&s.name)
        .bind(&s.contact)
        .bind(&s.phone)
        .bind(&s.address)
        .bind(&s.business_scope)
        .bind(&s.remark)
        .bind(&s.category_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.rows_affected() > 0)
    }
}
