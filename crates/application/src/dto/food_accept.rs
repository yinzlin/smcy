//! 验收单入站契约 + 到 domain 端口输入类型的映射

use serde::{Deserialize, Serialize};

use domain::port::food_accept::{AcceptInput, FoodItemInput};

#[derive(Deserialize, Serialize)]
pub struct AcceptReq {
    pub supplier_id: i64,
    pub purchaser_id: i64,
    pub car_no: Option<String>,
    pub supply_time: String,
    pub total_price: f64,
    pub discount_rate: f64,
    pub final_price: f64,
    pub items: Vec<FoodItemReq>,
}

#[derive(Deserialize, Serialize)]
pub struct FoodItemReq {
    pub food_name: String,
    pub spec: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub sub_total: f64,
    pub produce_batch: Option<String>,
    pub shelf_life: Option<String>,
    pub has_veg_report: bool,
    pub has_meat_quarantine: bool,
    pub has_abnormal: bool,
    pub pass_check: bool,
    pub remark: Option<String>,
}

impl From<FoodItemReq> for FoodItemInput {
    fn from(r: FoodItemReq) -> Self {
        Self {
            food_name: r.food_name,
            spec: r.spec,
            unit_price: r.unit_price,
            quantity: r.quantity,
            sub_total: r.sub_total,
            produce_batch: r.produce_batch,
            shelf_life: r.shelf_life,
            has_veg_report: r.has_veg_report,
            has_meat_quarantine: r.has_meat_quarantine,
            has_abnormal: r.has_abnormal,
            pass_check: r.pass_check,
            remark: r.remark,
        }
    }
}

impl From<AcceptReq> for AcceptInput {
    fn from(r: AcceptReq) -> Self {
        Self {
            supplier_id: r.supplier_id,
            purchaser_id: r.purchaser_id,
            car_no: r.car_no,
            supply_time: r.supply_time,
            total_price: r.total_price,
            discount_rate: r.discount_rate,
            final_price: r.final_price,
            items: r.items.into_iter().map(Into::into).collect(),
        }
    }
}
