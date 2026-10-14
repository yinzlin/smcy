//! SQLite 库存仓储实现

use sqlx::Row;

use domain::error::DomainError;
use domain::port::inventory::{InventoryRepo, InventoryRow};

#[derive(Clone)]
pub struct SqliteInventoryRepo {
    pool: sqlx::SqlitePool,
}

impl SqliteInventoryRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl InventoryRepo for SqliteInventoryRepo {
    async fn list_all(&self) -> Result<Vec<InventoryRow>, DomainError> {
        // SQL 逐字复刻旧 api_inventory_list
        let rows = sqlx::query(
            "SELECT i.id, i.product_id, i.warehouse_id, p.name, p.spec, i.quantity, i.min_stock, i.max_stock, w.name as warehouse_name
             FROM inventory i JOIN product p ON i.product_id = p.id LEFT JOIN warehouse w ON i.warehouse_id = w.id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| InventoryRow {
                id: row.get::<i64, _>("id"),
                product_id: row.get::<i64, _>("product_id"),
                warehouse_id: row.get::<i64, _>("warehouse_id"),
                warehouse_name: row.get::<Option<String>, _>("warehouse_name"),
                name: row.get::<String, _>("name"),
                spec: row.get::<Option<String>, _>("spec"),
                quantity: row.get::<f64, _>("quantity"),
                min_stock: row.get::<f64, _>("min_stock"),
                max_stock: row.get::<f64, _>("max_stock"),
            })
            .collect())
    }
}
