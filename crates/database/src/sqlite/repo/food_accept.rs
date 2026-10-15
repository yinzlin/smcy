//! food_accept 仓储实现：SQL 与旧 api_accept_* handler 逐字一致

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use domain::error::DomainError;
use domain::port::food_accept::{AcceptInput, FoodAcceptRepo, FoodAcceptRow};

#[derive(Clone)]
pub struct SqliteFoodAcceptRepo {
    pool: SqlitePool,
}

impl SqliteFoodAcceptRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl FoodAcceptRepo for SqliteFoodAcceptRepo {
    async fn create(&self, req: &AcceptInput) -> Result<i64, DomainError> {
        let res = sqlx::query(
            "INSERT INTO food_accept(supplier_id, purchaser_id, car_no, supply_time, total_price, discount_rate, final_price) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(req.supplier_id)
        .bind(req.purchaser_id)
        .bind(&req.car_no)
        .bind(&req.supply_time)
        .bind(req.total_price)
        .bind(req.discount_rate)
        .bind(req.final_price)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        let accept_id = res.last_insert_rowid();
        // 行为保持：明细逐条插入，单条失败吞错（旧 .ok()）
        for item in &req.items {
            sqlx::query(
                "INSERT INTO food_item(accept_id, food_name, spec, unit_price, quantity, sub_total, produce_batch, shelf_life, has_veg_report, has_meat_quarantine, has_abnormal, pass_check, remark) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(accept_id)
            .bind(&item.food_name)
            .bind(&item.spec)
            .bind(item.unit_price)
            .bind(item.quantity)
            .bind(item.sub_total)
            .bind(&item.produce_batch)
            .bind(&item.shelf_life)
            .bind(item.has_veg_report)
            .bind(item.has_meat_quarantine)
            .bind(item.has_abnormal)
            .bind(item.pass_check)
            .bind(&item.remark)
            .execute(&self.pool)
            .await
            .ok();
        }
        Ok(accept_id)
    }

    async fn list(&self) -> Result<Vec<FoodAcceptRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT fa.id, fa.supplier_id, fa.purchaser_id, fa.car_no, fa.supply_time, fa.total_price, fa.discount_rate, fa.final_price, fa.status,
                s.name as supplier_name, p.name as purchaser_name
             FROM food_accept fa
             JOIN supplier s ON fa.supplier_id = s.id
             JOIN purchaser p ON fa.purchaser_id = p.id
             ORDER BY fa.id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| FoodAcceptRow {
                id: row.get("id"),
                supplier_id: row.get("supplier_id"),
                purchaser_id: row.get("purchaser_id"),
                car_no: row.get("car_no"),
                supply_time: row.get("supply_time"),
                total_price: row.get("total_price"),
                discount_rate: row.get("discount_rate"),
                final_price: row.get("final_price"),
                status: row.get("status"),
                supplier_name: row.get("supplier_name"),
                purchaser_name: row.get("purchaser_name"),
            })
            .collect())
    }
}
