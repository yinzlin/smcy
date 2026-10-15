//! 基础数据域入站契约（供应商/采购方/分类/仓库/通用删除）
//!
//! 由 handler 反序列化后映射为各域 `*Cmd`/`Upsert*` 入站命令，本身不进入 domain。

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct SupplierReq {
    pub id: Option<i64>,
    pub name: String,
    pub contact: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub business_scope: Option<String>,
    pub remark: Option<String>,
    pub category_id: Option<i64>,
    // 定点供应商档案字段
    #[serde(default)]
    pub is_designated: Option<i64>,
    #[serde(default)]
    pub designated_no: Option<String>,
    #[serde(default)]
    pub legal_person: Option<String>,
    #[serde(default)]
    pub credit_code: Option<String>,
    #[serde(default)]
    pub license_no: Option<String>,
    #[serde(default)]
    pub license_expire: Option<String>,
    #[serde(default)]
    pub contract_no: Option<String>,
    #[serde(default)]
    pub contract_start: Option<String>,
    #[serde(default)]
    pub contract_end: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct PurchaserReq {
    pub id: Option<i64>,
    pub name: String,
    pub contact: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub business_scope: Option<String>,
    pub remark: Option<String>,
    pub category_id: Option<i64>,
}

#[derive(Deserialize, Serialize)]
pub struct DeleteReq {
    pub id: i64,
}

#[derive(Deserialize, Serialize)]
pub struct CategoryReq {
    pub name: String,
    pub parent_id: Option<i64>,
    pub entity_type: String,
    pub sort_order: Option<i32>,
}

#[derive(Deserialize)]
pub struct CategoryRenameReq {
    pub id: i64,
    pub name: String,
}

#[derive(Deserialize)]
pub struct WarehouseCreateReq {
    pub name: String,
    pub code: Option<String>,
    pub address: Option<String>,
    pub contact: Option<String>,
    pub phone: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Deserialize)]
pub struct WarehouseUpdateReq {
    pub id: i64,
    pub name: String,
    pub code: Option<String>,
    pub address: Option<String>,
    pub contact: Option<String>,
    pub phone: Option<String>,
    pub status: Option<i32>,
    pub sort_order: Option<i32>,
}
