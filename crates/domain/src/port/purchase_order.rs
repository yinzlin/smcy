//! 采购订单聚合出站端口
//!
//! SQL 与旧 api_purchase_order_* handler 逐字一致；多步事务序列（乐观锁更新 /
//! 审核写库存流水 / 反审核冲销）建模为聚合级仓储方法，实现内部 begin/commit
//! （Unit-of-Work 属于基础设施层）。错误文案按旧 handler 逐字预置在 DomainError 中。

use crate::error::DomainError;

/// 列表行（JOIN supplier + 明细仓库聚合子查询）
#[derive(Debug, Clone)]
pub struct PurchaseOrderListRow {
    pub id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub amount_reduction: f64,
    pub final_amount: f64,
    pub status: String,
    pub remark: Option<String>,
    pub warehouse_id: i64,
    pub warehouse_name: Option<String>,
    pub is_settled: i64,
    pub supplier_name: String,
    pub item_warehouse_names: Option<String>,
    pub item_warehouse_count: i64,
}

/// 详情主表行（JOIN supplier）
#[derive(Debug, Clone)]
pub struct PurchaseOrderDetailRow {
    pub id: i64,
    pub supplier_id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub amount_reduction: f64,
    pub final_amount: f64,
    pub status: String,
    pub remark: Option<String>,
    pub warehouse_id: i64,
    pub warehouse_name: Option<String>,
    #[allow(dead_code)]
    pub user_id: i64,
    pub version: i64,
    pub is_settled: i64,
    pub supplier_name: String,
}

/// 详情明细行
#[derive(Debug, Clone)]
pub struct PurchaseOrderItemRow {
    pub id: i64,
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
    pub warehouse_id: i64,
    pub warehouse_name: Option<String>,
    pub production_date: Option<String>,
    pub batch_no: Option<String>,
}

/// 导出行（LEFT JOIN 明细；明细列可空）
#[derive(Debug, Clone)]
pub struct PurchaseOrderExportRow {
    pub id: i64,
    pub order_no: String,
    pub order_date: String,
    pub supplier_name: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub final_amount: f64,
    pub status: String,
    pub remark: Option<String>,
    pub product_name: Option<String>,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub ordered_quantity: Option<f64>,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub base_quantity: Option<f64>,
    pub amount: Option<f64>,
    pub item_remark: Option<String>,
    pub item_warehouse_name: Option<String>,
}

/// 打印模板主表行（JOIN supplier + LEFT JOIN user_account）
#[derive(Debug, Clone)]
pub struct PurchaseOrderPrintRow {
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub amount_reduction: f64,
    pub final_amount: f64,
    pub remark: Option<String>,
    pub supplier_name: Option<String>,
    pub supplier_phone: Option<String>,
    pub supplier_address: Option<String>,
    pub user_name: Option<String>,
    pub handler_phone: Option<String>,
}

/// 打印模板明细行（warehouse_name 已 trim 归一）
#[derive(Debug, Clone)]
pub struct PurchaseOrderPrintItemRow {
    pub warehouse_name: String,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub quantity: f64,
    pub unit_price: f64,
    pub amount: f64,
    pub remark: Option<String>,
}

/// 列表/导出筛选条件（supplier_id 为 HTTP 原始值，角色强制在用例层完成）
#[derive(Debug, Clone, Default)]
pub struct PurchaseOrderListFilter {
    pub keyword_pattern: String,
    pub supplier_id: Option<i64>,
    pub is_settled: Option<String>,
    pub sort_field: String,
    pub sort_order: String,
    pub page: i64,
    pub page_size: i64,
}

/// 批量导入主表行
#[derive(Debug, Clone)]
pub struct PurchaseOrderImportMain {
    pub order_no: String,
    pub supplier_id: i64,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub final_amount: f64,
    pub remark: String,
}

/// 批量导入明细行
#[derive(Debug, Clone)]
pub struct PurchaseOrderImportItem {
    pub product_id: i64,
    pub product_name: String,
    pub alias1: String,
    pub alias2: String,
    pub spec: String,
    pub unit: String,
    pub unit_price: f64,
    pub quantity: f64,
    pub base_quantity: f64,
    pub amount: f64,
    pub remark: String,
}

/// 采购单明细写入输入（domain 自有类型，不含 serde）
#[derive(Debug, Clone)]
pub struct PurchaseOrderItemInput {
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
    pub warehouse_id: Option<i64>,
    pub warehouse_name: Option<String>,
    pub production_date: Option<String>,
    pub batch_no: Option<String>,
}

/// 采购单主表写入输入（domain 自有类型，不含 serde）
#[derive(Debug, Clone)]
pub struct PurchaseOrderInput {
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
    pub items: Vec<PurchaseOrderItemInput>,
    pub remark: Option<String>,
    pub version: Option<i64>,
    pub is_settled: Option<i64>,
}

/// 采购订单仓储端口
pub trait PurchaseOrderRepo: Send + Sync {
    /// 供应商审核状态（下单前置校验用；查询失败吞为 None）
    async fn supplier_audit_status(&self, supplier_id: i64) -> Result<Option<String>, DomainError>;
    /// 新增主表（返回 last_insert_rowid）
    async fn insert_order(&self, req: &PurchaseOrderInput, main_wh_id: i64, main_wh_name: &str) -> Result<i64, DomainError>;
    /// 批量插入明细（17 列无 sales_unit/source；旧行为失败吞错，由用例层 let _ 忽略）
    async fn insert_items_batch(&self, order_id: i64, items: &[PurchaseOrderItemInput], base_qtys: &[f64]) -> Result<(), DomainError>;
    /// 列表总数（动态 WHERE 四分支；bind 全字符串，与旧一致）
    async fn count_list(&self, filter: &PurchaseOrderListFilter) -> Result<i64, DomainError>;
    /// 列表分页（ORDER BY 白名单子句由用例层构造；失败由用例层吞为空）
    async fn list(&self, filter: &PurchaseOrderListFilter, order_clause: &str) -> Result<Vec<PurchaseOrderListRow>, DomainError>;
    /// 详情主表（查询失败吞为 None）
    async fn get_order_detail(&self, id: i64) -> Result<Option<PurchaseOrderDetailRow>, DomainError>;
    /// 详情明细（查询失败吞为空）
    async fn list_items(&self, order_id: i64) -> Result<Vec<PurchaseOrderItemRow>, DomainError>;
    /// 行级权限/状态护栏：查 (supplier_id, status)（查询失败吞为 None）
    async fn get_supplier_status(&self, id: i64) -> Result<Option<(i64, String)>, DomainError>;
    /// 乐观锁更新：BEGIN IMMEDIATE 事务内 主表 UPDATE(version 自增+校验) → 明细按 id 同步（删/改/插）
    /// Err(Conflict)=version 冲突；Err(Storage)=带旧文案的完整消息
    async fn update_order_with_items(
        &self,
        req: &PurchaseOrderInput,
        main_wh_id: i64,
        main_wh_name: &str,
        ver: i64,
        base_qtys: &[f64],
    ) -> Result<(), DomainError>;
    /// 删除前收集本单涉及的商品 id（去重；查询失败吞为空）
    async fn distinct_product_ids(&self, order_id: i64) -> Result<Vec<i64>, DomainError>;
    /// 删除明细（旧行为吞错，由用例层 let _ 忽略）
    async fn delete_items_of_order(&self, order_id: i64) -> Result<(), DomainError>;
    /// 删除主表
    async fn delete_order(&self, id: i64) -> Result<(), DomainError>;
    /// 设置结算状态（返回 rows_affected）
    async fn set_settled(&self, id: i64, is_settled: i64) -> Result<u64, DomainError>;
    /// 审核：BEGIN IMMEDIATE 事务内 状态流转 + 写采购入库流水 + 同步 inventory
    /// Err(Conflict)=状态已变化；Err(Storage)=带旧文案的完整消息
    async fn approve_tx(&self, id: i64) -> Result<(), DomainError>;
    /// 反审核：super_admin 无状态约束，其他角色仅 confirmed；事务内写冲销流水
    async fn unapprove_tx(&self, id: i64, is_super_admin: bool) -> Result<(), DomainError>;
    /// 导出行查询（LEFT JOIN 明细，按 po.id, poi.id；失败吞为空）
    async fn export_rows(&self, keyword_pattern: &str, supplier_id: Option<i64>) -> Result<Vec<PurchaseOrderExportRow>, DomainError>;
    /// 打印模板主表（查询失败吞为 None）
    async fn get_print_order(&self, id: i64) -> Result<Option<PurchaseOrderPrintRow>, DomainError>;
    /// 打印模板明细（按 warehouse_name, id；warehouse_name trim 归一；失败吞为空）
    async fn get_print_items(&self, id: i64) -> Result<Vec<PurchaseOrderPrintItemRow>, DomainError>;
    /// 经手人覆盖：查用户 (nickname, phone)（查询失败吞为 None）
    async fn user_nickname_phone(&self, user_id: i64) -> Result<Option<(String, String)>, DomainError>;
    /// 导入：按名称查供应商 id（吞为 None）
    async fn find_supplier_id_by_name(&self, name: &str) -> Result<Option<i64>, DomainError>;
    /// 导入：INSERT OR IGNORE 主表（返回 (last_insert_rowid, rows_affected)）
    async fn insert_import_order(&self, main: &PurchaseOrderImportMain) -> Result<(i64, u64), DomainError>;
    /// 导入：按 name+spec 查商品 id（吞为 None）
    async fn find_product_id_by_name_spec(&self, name: &str, spec: &str) -> Result<Option<i64>, DomainError>;
    /// 导入：查辅助单位换算比（吞为 None）
    async fn unit_ratio(&self, product_id: i64, unit: &str) -> Result<Option<f64>, DomainError>;
    /// 导入：插入明细（旧行为吞错，实现内部忽略错误）
    async fn insert_import_item(&self, order_id: i64, item: &PurchaseOrderImportItem) -> Result<(), DomainError>;
    /// 当日 purchase_order 最大单号序号（失败吞为 0）
    async fn max_seq_for_date(&self, order_date: &str) -> Result<i64, DomainError>;
    /// 按 product_unit.ratio 批量换算明细基础单位数量（失败回退前端基础数量/quantity）
    async fn compute_base_quantities(&self, items: &[(i64, String, f64, f64)]) -> Vec<f64>;
}
