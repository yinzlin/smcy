//! category 仓储实现：SQL 与旧 api_category_* handler 逐字一致

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use domain::error::DomainError;
use domain::port::category::{CategoryRepo, CategoryRow, NewCategory};

#[derive(Clone)]
pub struct SqliteCategoryRepo {
    pool: SqlitePool,
}

impl SqliteCategoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl CategoryRepo for SqliteCategoryRepo {
    async fn list_all(&self) -> Result<Vec<CategoryRow>, DomainError> {
        let rows = sqlx::query("SELECT id, name, parent_id, entity_type, sort_order FROM category ORDER BY entity_type, sort_order, id")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| CategoryRow {
                id: r.get("id"),
                name: r.get("name"),
                parent_id: r.get("parent_id"),
                entity_type: r.get("entity_type"),
                sort_order: r.get("sort_order"),
            })
            .collect())
    }

    async fn list_for_tree(&self) -> Result<Vec<CategoryRow>, DomainError> {
        let rows = sqlx::query("SELECT id, name, parent_id, entity_type, sort_order FROM category ORDER BY sort_order, id")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| CategoryRow {
                id: r.get("id"),
                name: r.get("name"),
                parent_id: r.get("parent_id"),
                entity_type: r.get("entity_type"),
                sort_order: r.get("sort_order"),
            })
            .collect())
    }

    async fn create(&self, c: &NewCategory) -> Result<(), DomainError> {
        sqlx::query("INSERT INTO category(name, parent_id, entity_type, sort_order) VALUES (?, ?, ?, ?)")
            .bind(&c.name)
            .bind(&c.parent_id)
            .bind(&c.entity_type)
            .bind(&c.sort_order)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn rename(&self, id: i64, name: &str) -> Result<(), DomainError> {
        sqlx::query("UPDATE category SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn delete(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM category WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn count_children(&self, id: i64) -> Result<i64, DomainError> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM category WHERE parent_id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    async fn count_supplier_refs(&self, id: i64) -> Result<i64, DomainError> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM supplier WHERE category_id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    async fn count_purchaser_refs(&self, id: i64) -> Result<i64, DomainError> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM purchaser WHERE category_id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    async fn count_product_refs(&self, id: i64) -> Result<i64, DomainError> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product WHERE category_id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }
}
