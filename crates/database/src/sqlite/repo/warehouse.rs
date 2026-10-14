//! warehouse 仓储实现：SQL 与旧 api_warehouse_* handler 逐字一致
//!
//! 唯一性冲突（名称/编号 UNIQUE）映射为 DomainError::Conflict，
//! 由用例层转换为旧文案"仓库名称或编号已存在"。

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use domain::error::DomainError;
use domain::port::warehouse::{NewWarehouse, UpdateWarehouse, WarehouseRepo, WarehouseRow};

#[derive(Clone)]
pub struct SqliteWarehouseRepo {
    pool: SqlitePool,
}

impl SqliteWarehouseRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

fn map_write_err(e: sqlx::Error) -> DomainError {
    if e.to_string().contains("UNIQUE constraint failed") {
        DomainError::Conflict(e.to_string())
    } else {
        DomainError::Storage(e.to_string())
    }
}

impl WarehouseRepo for SqliteWarehouseRepo {
    async fn list_all(&self) -> Result<Vec<WarehouseRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, name, code, address, contact, phone, status, sort_order, audit_status, create_at, update_at FROM warehouse ORDER BY sort_order, id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| WarehouseRow {
                id: r.get("id"),
                name: r.get("name"),
                code: r.get("code"),
                address: r.get("address"),
                contact: r.get("contact"),
                phone: r.get("phone"),
                status: r.get("status"),
                sort_order: r.get("sort_order"),
                audit_status: r.get("audit_status"),
                create_at: r.get("create_at"),
                update_at: r.get("update_at"),
            })
            .collect())
    }

    async fn create(&self, w: &NewWarehouse) -> Result<(), DomainError> {
        sqlx::query("INSERT INTO warehouse (name, code, address, contact, phone, sort_order) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&w.name)
            .bind(&w.code)
            .bind(&w.address)
            .bind(&w.contact)
            .bind(&w.phone)
            .bind(w.sort_order)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(map_write_err)
    }

    async fn update(&self, w: &UpdateWarehouse) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE warehouse SET name = ?, code = ?, address = ?, contact = ?, phone = ?, status = ?, sort_order = ?, audit_status='pending', update_at = datetime('now','localtime') WHERE id = ?",
        )
        .bind(&w.name)
        .bind(&w.code)
        .bind(&w.address)
        .bind(&w.contact)
        .bind(&w.phone)
        .bind(w.status)
        .bind(w.sort_order)
        .bind(w.id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(map_write_err)
    }

    async fn delete(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM warehouse WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn count_inventory(&self, id: i64) -> Result<i64, DomainError> {
        sqlx::query("SELECT COUNT(*) FROM inventory WHERE warehouse_id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map(|r| r.get(0))
            .map_err(storage)
    }

    async fn get_audit_status(&self, id: i64) -> Result<Option<String>, DomainError> {
        sqlx::query_scalar("SELECT audit_status FROM warehouse WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }
}
