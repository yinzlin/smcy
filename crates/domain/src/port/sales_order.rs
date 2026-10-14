//! 销售订单聚合出站端口
//!
//! SQL 与旧 api_sales_order_* handler 逐字一致；乐观锁更新 / 状态流转（含 FIFO
//! 批次快照与库存流水）/ 反审核冲销的多步事务序列建模为聚合级仓储方法，实现内部
//! begin/commit（Unit-of-Work 属于基础设施层）。错误文案按旧 handler 逐字预置在
//! DomainError 中。

use crate::error::DomainError;
use crate::{SalesOrderItemReq, SalesOrderReq};

/// 详情主表行（LEFT JOIN purchaser）
#[derive(Debug, Clone)]
pub struct SalesOrderDetailRow {
    pub id: i64,
    pub purchaser_id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub amount_reduction: f64,
    pub final_amount: f64,
    pub status: String,
    pub version: i64,
    pub remark: Option<String>,
    pub warehouse_id: Option<i64>,
    pub warehouse_name: Option<String>,
    pub customer_order_image: Option<String>,
    pub signed_order_image: Option<String>,
    pub supplier_company: Option<String>,
    pub truck_plate: Option<String>,
    pub is_settled: i64,
    pub purchaser_name: String,
}

/// 详情明细行
#[derive(Debug, Clone)]
pub struct SalesOrderItemRow {
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
    pub pre_sale_quantity: Option<f64>,
    pub supplier_id: Option<i64>,
    pub supplier_name: Option<String>,
    pub production_date: Option<String>,
    pub batch_no: Option<String>,
    pub remark: Option<String>,
}

/// 列表行（JOIN purchaser + 增项数子查询）
#[derive(Debug, Clone)]
pub struct SalesOrderListRow {
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
    pub purchaser_name: String,
    pub supplement_count: i64,
}

/// by_purchaser 行（LEFT JOIN purchaser；discount_total 取 final_amount 别名）
#[derive(Debug, Clone)]
pub struct SalesOrderByPurchaserRow {
    pub id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub discount_total: f64,
    pub status: String,
    pub purchaser_name: Option<String>,
    pub purchaser_id: i64,
}

/// 列表筛选条件（purchaser_id 为 HTTP 原始值，角色强制在用例层完成）
#[derive(Debug, Clone, Default)]
pub struct SalesOrderListFilter {
    pub keyword_pattern: String,
    pub purchaser_id: Option<i64>,
    pub is_settled: Option<String>,
    pub sort_field: String,
    pub sort_order: String,
    pub page: i64,
    pub page_size: i64,
}

/// 乐观锁更新事务结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SalesOrderUpdateTx {
    /// 事务已提交
    Committed,
    /// 主表 UPDATE rows_affected = 0：版本冲突（409）
    VersionConflict,
}

/// 状态流转事务结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SalesOrderTransitionTx {
    /// 事务已提交
    Committed,
    /// WHERE 当前状态不匹配，rows_affected = 0（409）
    StateConflict,
}

/// 反审核事务结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SalesOrderUnapproveTx {
    /// 事务已提交
    Committed,
    /// WHERE 状态不匹配，rows_affected = 0（409）
    StateConflict,
}

/// 销售订单仓储出站端口
pub trait SalesOrderRepo: Send + Sync {
    /// 守卫查询：订单的 (purchaser_id, status)；不存在返回 None
    async fn get_purchaser_status(&self, id: i64) -> Result<Option<(i64, String)>, DomainError>;
    /// 详情守卫：仅取 purchaser_id（fetch_optional，错误吞为 None 由用例层兜底）
    async fn get_detail_purchaser_id(&self, id: i64) -> Option<i64>;
    /// 采购方审核状态（id<=0 的通过语义在用例层）
    async fn purchaser_audit_status(&self, id: i64) -> Result<Option<String>, DomainError>;
    /// 商品审核状态
    async fn product_audit_status(&self, id: i64) -> Result<Option<String>, DomainError>;
    /// 详情主表（LEFT JOIN purchaser）
    async fn get_order_detail(&self, id: i64) -> Result<Option<SalesOrderDetailRow>, DomainError>;
    /// 详情明细
    async fn list_items(&self, order_id: i64) -> Result<Vec<SalesOrderItemRow>, DomainError>;
    /// 列表总数
    async fn count_list(&self, filter: &SalesOrderListFilter) -> Result<i64, DomainError>;
    /// 列表分页（order_clause 由用例层按 sort_field 白名单生成）
    async fn list(&self, filter: &SalesOrderListFilter, order_clause: &str) -> Result<Vec<SalesOrderListRow>, DomainError>;
    /// 按采购单位列出订单（ORDER BY order_date DESC）
    async fn list_by_purchaser(&self, purchaser_id: i64) -> Result<Vec<SalesOrderByPurchaserRow>, DomainError>;
    /// 批量查耗材分配状态：返回 (source_order_id, status) 序列，按 id ASC（用例层取每单最早一条）
    async fn allocation_status_rows(&self, order_ids: &[i64]) -> Result<Vec<(i64, i64)>, DomainError>;
    /// 创建主表，返回 order_id
    async fn insert_order(&self, req: &SalesOrderReq) -> Result<i64, DomainError>;
    /// 批量插入明细（base_qtys 与 items 同序；数量兜底已在入参组装完成）
    async fn insert_items_batch(&self, order_id: i64, items: &[SalesOrderItemReq], base_qtys: &[f64]) -> Result<(), DomainError>;
    /// 乐观锁更新：BEGIN IMMEDIATE + 主表 UPDATE(version+1) + 明细全量重写 + commit
    async fn update_order_with_items(&self, req: &SalesOrderReq, ver: i64, base_qtys: &[f64]) -> Result<SalesOrderUpdateTx, DomainError>;
    /// 删除明细（外键冲突文案由用例层识别）
    async fn delete_items(&self, order_id: i64) -> Result<(), DomainError>;
    /// 删除主表
    async fn delete_order(&self, id: i64) -> Result<(), DomainError>;
    /// 设置结算状态，返回 rows_affected
    async fn set_settled(&self, id: i64, is_settled: i64) -> Result<u64, DomainError>;
    /// 审核：UPDATE status='confirmed' WHERE id AND status='pending'，返回 rows_affected
    /// （错误与 0 行同样落入用例层 409 分支，逐字保持旧行为）
    async fn approve(&self, id: i64) -> Result<u64, DomainError>;
    /// 反审核：BEGIN IMMEDIATE + 主表 UPDATE（超管无状态条件）+ 原状态 accepted 时写冲销流水 + commit
    async fn unapprove(&self, id: i64, is_super_admin: bool, was_accepted: bool) -> Result<SalesOrderUnapproveTx, DomainError>;
    /// 状态流转：BEGIN IMMEDIATE + 原子 UPDATE(WHERE 当前状态) + accepted 进/出库存流水
    /// + FIFO 批次快照写入/清空 + commit
    async fn transition_status(&self, id: i64, current_status: &str, new_status: &str) -> Result<SalesOrderTransitionTx, DomainError>;
    /// update_status 前置读：fetch_one 语义，任何错误吞为 "pending"（逐字保持）
    async fn fetch_status_or_pending(&self, id: i64) -> String;
    /// update_status 前置读：fetch_one 语义，任何错误吞为 -1（逐字保持）
    async fn fetch_purchaser_id_or_neg1(&self, id: i64) -> i64;
    /// 调价读取：明细 (id, product_id, quantity)
    async fn list_item_price_inputs(&self, order_id: i64) -> Vec<(i64, i64, f64)>;
    /// 调价读取：商品基础售价（fetch_optional，错误吞为 None）
    async fn product_base_price(&self, product_id: i64) -> Option<f64>;
    /// 调价读取：(discount_rate, amount_reduction)，各自 fetch_one 错误吞为 0.0
    async fn fetch_discount_and_reduction(&self, id: i64) -> (f64, f64);
}
