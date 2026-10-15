//! 订单域入站契约（采购单/销售单/订单增补）+ 到 domain 端口输入类型的映射
//!
//! HTTP 契约（`*Req`）归本模块；出站端口出入参用 domain 自有类型，故在此实现 `From` 映射。

use serde::{Deserialize, Serialize};

use domain::port::order_supplement::OrderSupplementInput;
use domain::port::purchase_order::{PurchaseOrderInput, PurchaseOrderItemInput};
use domain::port::sales_order::{SalesOrderInput, SalesOrderItemInput};

#[derive(Deserialize, Serialize)]
pub struct PurchaseOrderReq {
    pub id: Option<i64>,
    pub supplier_id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub amount_reduction: f64,
    pub final_amount: f64,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub user_id: Option<i64>,
    pub handler_phone: Option<String>,
    pub items: Vec<PurchaseOrderItemReq>,
    pub remark: Option<String>,
    /// 乐观锁版本号：编辑时从详情接口取得，提交时校验，防止覆盖他人修改
    pub version: Option<i64>,
    /// 是否已结算（0=未结 1=已结），用于财务查询
    pub is_settled: Option<i64>,
}

#[derive(Deserialize, Serialize)]
pub struct PurchaseOrderItemReq {
    /// 明细主键 id：编辑回传时用于按 id 精确同步，保留 source_sales_order_id 归属
    pub id: Option<i64>,
    pub product_id: i64,
    pub product_name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub base_quantity: Option<f64>,
    pub amount: f64,
    pub ordered_quantity: Option<f64>,
    pub remark: Option<String>,
    /// 明细级仓库：同一订单各行可入不同仓库
    pub warehouse_id: Option<i64>,
    pub warehouse_name: Option<String>,
    /// 食材溯源：本批到货实物的生产日期（如 2026-09-28）
    pub production_date: Option<String>,
    /// 食材溯源：本批到货实物的批号
    pub batch_no: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct SalesOrderReq {
    pub id: Option<i64>,
    pub purchaser_id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub amount_reduction: f64,
    pub final_amount: f64,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub items: Vec<SalesOrderItemReq>,
    pub remark: Option<String>,
    /// 验收单/报销单导出时使用的供应商名称；可空，前端新建时默认填充"湖南食全味美餐饮管理有限公司"。
    pub supplier_company: Option<String>,
    /// 验收单/报销单导出时使用的供货车牌号；可空，前端新建时默认填充"湘A·BE9312"。
    pub truck_plate: Option<String>,
    /// 乐观锁版本号：编辑时从详情接口取得，提交时校验，防止覆盖他人修改
    pub version: Option<i64>,
    /// 是否已结算（0=未结 1=已结），用于财务查询
    pub is_settled: Option<i64>,
}

#[derive(Deserialize, Serialize)]
pub struct SalesOrderItemReq {
    pub product_id: i64,
    pub product_name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub base_quantity: Option<f64>,
    pub amount: f64,
    pub pre_sale_quantity: Option<f64>,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub category_id: Option<i64>,
    pub remark: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct OrderSupplementItemReq {
    pub target_order_id: i64,
    pub source_order_id: i64,
    pub source_remark: Option<String>,
    pub product_id: i64,
    pub product_name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: String,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub allocate_date: String,
    pub operation_type: String,
    pub target_order_item_id: Option<i64>,
}

// ---- 到 domain 端口输入类型的映射 ----

impl From<SalesOrderItemReq> for SalesOrderItemInput {
    fn from(r: SalesOrderItemReq) -> Self {
        Self {
            product_id: r.product_id,
            product_name: r.product_name,
            alias1: r.alias1,
            alias2: r.alias2,
            spec: r.spec,
            unit: r.unit,
            unit_price: r.unit_price,
            quantity: r.quantity,
            base_quantity: r.base_quantity,
            amount: r.amount,
            pre_sale_quantity: r.pre_sale_quantity,
            supplier_id: r.supplier_id,
            supplier_name: r.supplier_name,
            category_id: r.category_id,
            remark: r.remark,
        }
    }
}

impl From<SalesOrderReq> for SalesOrderInput {
    fn from(r: SalesOrderReq) -> Self {
        Self {
            id: r.id,
            purchaser_id: r.purchaser_id,
            order_no: r.order_no,
            order_date: r.order_date,
            total_amount: r.total_amount,
            discount_rate: r.discount_rate,
            amount_reduction: r.amount_reduction,
            final_amount: r.final_amount,
            warehouse_id: r.warehouse_id,
            warehouse_name: r.warehouse_name,
            items: r.items.into_iter().map(Into::into).collect(),
            remark: r.remark,
            supplier_company: r.supplier_company,
            truck_plate: r.truck_plate,
            version: r.version,
            is_settled: r.is_settled,
        }
    }
}

impl From<PurchaseOrderItemReq> for PurchaseOrderItemInput {
    fn from(r: PurchaseOrderItemReq) -> Self {
        Self {
            id: r.id,
            product_id: r.product_id,
            product_name: r.product_name,
            alias1: r.alias1,
            alias2: r.alias2,
            spec: r.spec,
            unit: r.unit,
            unit_price: r.unit_price,
            quantity: r.quantity,
            base_quantity: r.base_quantity,
            amount: r.amount,
            ordered_quantity: r.ordered_quantity,
            remark: r.remark,
            warehouse_id: r.warehouse_id,
            warehouse_name: r.warehouse_name,
            production_date: r.production_date,
            batch_no: r.batch_no,
        }
    }
}

impl From<PurchaseOrderReq> for PurchaseOrderInput {
    fn from(r: PurchaseOrderReq) -> Self {
        Self {
            id: r.id,
            supplier_id: r.supplier_id,
            order_no: r.order_no,
            order_date: r.order_date,
            total_amount: r.total_amount,
            discount_rate: r.discount_rate,
            amount_reduction: r.amount_reduction,
            final_amount: r.final_amount,
            warehouse_id: r.warehouse_id,
            warehouse_name: r.warehouse_name,
            user_id: r.user_id,
            handler_phone: r.handler_phone,
            items: r.items.into_iter().map(Into::into).collect(),
            remark: r.remark,
            version: r.version,
            is_settled: r.is_settled,
        }
    }
}

impl From<OrderSupplementItemReq> for OrderSupplementInput {
    fn from(r: OrderSupplementItemReq) -> Self {
        Self {
            target_order_id: r.target_order_id,
            source_order_id: r.source_order_id,
            source_remark: r.source_remark,
            product_id: r.product_id,
            product_name: r.product_name,
            alias1: r.alias1,
            alias2: r.alias2,
            spec: r.spec,
            unit: r.unit,
            unit_price: r.unit_price,
            quantity: r.quantity,
            amount: r.amount,
            allocate_date: r.allocate_date,
            operation_type: r.operation_type,
            target_order_item_id: r.target_order_item_id,
        }
    }
}
