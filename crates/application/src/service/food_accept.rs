//! 验收单用例：创建 / 列表
//!
//! 行为保持：明细插入失败吞错（repo 内 .ok()）；列表查询失败吞错返回空数组。

use domain::port::food_accept::FoodAcceptRepo;
use domain::AcceptReq;

use crate::error::UseCaseError;
use crate::port::food_accept::FoodAcceptUseCase;

#[derive(Clone)]
pub struct FoodAcceptService<R: FoodAcceptRepo> {
    repo: R,
}

impl<R: FoodAcceptRepo> FoodAcceptService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: FoodAcceptRepo> FoodAcceptUseCase for FoodAcceptService<R> {
    async fn create(&self, req: AcceptReq) -> Result<(), UseCaseError> {
        // 旧行为：主记录 Err → 仅 500 状态码；Ok → 仅 200 状态码（无响应体）
        self.repo.create(&req).await.map(|_| ()).map_err(|_| UseCaseError::Storage(String::new()))
    }

    async fn list(&self) -> Result<String, UseCaseError> {
        // 行为保持：查询失败返回空数组
        let rows = self.repo.list().await.unwrap_or_default();

        let accepts: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "id": row.id,
                    "supplier_id": row.supplier_id,
                    "purchaser_id": row.purchaser_id,
                    "car_no": row.car_no,
                    "supply_time": row.supply_time,
                    "total_price": row.total_price,
                    "discount_rate": row.discount_rate,
                    "final_price": row.final_price,
                    "status": row.status,
                    "supplier_name": row.supplier_name,
                    "purchaser_name": row.purchaser_name,
                })
            })
            .collect();

        Ok(serde_json::to_string(&accepts).unwrap())
    }
}
