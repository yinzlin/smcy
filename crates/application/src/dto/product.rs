//! 商品域入站契约

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ProductReq {
    pub id: Option<i64>,
    pub name: String,
    pub spec: Option<String>,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub unit: Option<String>,
    pub base_unit: Option<String>,
    pub base_price: Option<f64>,
    pub purchase_price: Option<f64>,
    pub image_url: Option<String>,
    pub category_id: Option<i64>,
    /// 保质期（商品自身属性，如 "7天"、"180天"）
    pub shelf_life: Option<String>,
    /// 政采平台状态：0=非政采商品 1=政采商品（有指导价） 2=政采商品（无指导价）
    pub gov_status: Option<i64>,
}

#[derive(Deserialize, Serialize)]
pub struct ProductUnitReq {
    pub product_id: i64,
    pub unit_name: String,
    pub ratio: f64,
    pub unit_price: Option<f64>,
    pub purchase_price: Option<f64>,
    pub sort_order: Option<i32>,
}

#[derive(Deserialize, Serialize)]
pub struct ProductPriceReq {
    pub product_id: i64,
    pub price_type: String,
    pub price: Option<f64>,
    pub collected_at: Option<String>,
    pub source: Option<String>,
}

#[derive(Deserialize)]
pub struct ProductUpdateReq {
    pub id: i64,
    pub name: String,
    pub spec: Option<String>,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub unit: Option<String>,
    pub base_unit: Option<String>,
    pub base_price: Option<f64>,
    pub purchase_price: Option<f64>,
    pub image_url: Option<String>,
    pub category_id: Option<i64>,
    pub markup_rate: Option<f64>,
    pub auto_update_price: Option<i64>,
    /// 保质期（商品自身属性，如 "7天"、"180天"）
    pub shelf_life: Option<String>,
    /// 政采平台状态：0=非政采商品 1=政采商品（有指导价） 2=政采商品（无指导价）
    pub gov_status: Option<i64>,
}
