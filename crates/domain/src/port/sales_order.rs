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

/// 导出行（LEFT JOIN 明细；明细列可空）
#[derive(Debug, Clone)]
pub struct SalesOrderExportRow {
    pub id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub final_amount: f64,
    pub status: String,
    pub remark: Option<String>,
    pub purchaser_name: String,
    pub product_name: Option<String>,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub pre_sale_quantity: Option<f64>,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub base_quantity: Option<f64>,
    pub amount: Option<f64>,
    pub item_remark: Option<String>,
}

/// 批量导入主表行
#[derive(Debug, Clone)]
pub struct SalesOrderImportMain {
    pub order_no: String,
    pub purchaser_id: i64,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub final_amount: f64,
    pub remark: String,
}

/// 批量导入明细行
#[derive(Debug, Clone)]
pub struct SalesOrderImportItem {
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

/// 验收单聚合主表行（JOIN purchaser）
#[derive(Debug, Clone)]
pub struct SalesOrderAcceptOrderRow {
    pub id: i64,
    pub purchaser_id: i64,
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub discount_rate: f64,
    pub final_amount: f64,
    pub remark: Option<String>,
    pub purchaser_name: String,
    pub purchaser_address: Option<String>,
}

/// 验收单真实明细行（LEFT JOIN 分类，用于排序键计算）
#[derive(Debug, Clone)]
pub struct SalesOrderAcceptItemRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub category_name: Option<String>,
    pub parent_name: Option<String>,
}

/// 验收单增项行（order_supplement_item）
#[derive(Debug, Clone)]
pub struct SalesOrderAcceptSupplementRow {
    pub id: i64,
    pub operation_type: String,
    pub target_order_item_id: Option<i64>,
    pub product_id: i64,
    pub product_name: String,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: String,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
}

/// 验收单导出（Excel）主表行：含供应商名称/车牌号快照
#[derive(Debug, Clone)]
pub struct AcceptExcelOrderRow {
    pub order_no: String,
    pub order_date: String,
    pub discount_rate: f64,
    pub purchaser_name: String,
    pub supplier_company: Option<String>,
    pub truck_plate: Option<String>,
}

/// 验收单导出真实明细行（含商品规格快照、生产日期/批号、政采状态与分类名）
#[derive(Debug, Clone)]
pub struct AcceptExcelItemRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub product_spec: Option<String>,
    pub unit: Option<String>,
    pub product_base_unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub production_date: Option<String>,
    pub batch_no: Option<String>,
    pub shelf_life: Option<String>,
    pub gov_status: i64,
    pub category_name: Option<String>,
    pub parent_name: Option<String>,
}

/// 验收单导出增项行（order_supplement_item，含来源备注、商品分类与政采状态）
#[derive(Debug, Clone)]
pub struct AcceptExcelSupplementRow {
    pub id: i64,
    pub operation_type: String,
    pub target_order_item_id: Option<i64>,
    pub product_id: i64,
    pub product_name: String,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub product_spec: Option<String>,
    pub unit: Option<String>,
    pub product_base_unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub source_remark: Option<String>,
    pub shelf_life: Option<String>,
    pub gov_status: i64,
    pub category_name: Option<String>,
    pub parent_name: Option<String>,
}

/// 生成采购单：销售单主表行
#[derive(Debug, Clone)]
pub struct SalesOrderGenerateRow {
    pub order_date: String,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub purchaser_id: i64,
}

/// 生成采购单：已存在关联/兜底命中 PO 行
#[derive(Debug, Clone)]
pub struct GenerateExistedPo {
    pub id: i64,
    pub order_no: String,
    pub status: String,
    pub supplier_name: String,
}

/// 生成采购单：销售明细行（purchase_price = COALESCE(psp.price, p.purchase_price)）
#[derive(Debug, Clone)]
pub struct GenerateItemRow {
    pub product_id: i64,
    pub product_name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub quantity: f64,
    pub pre_sale_quantity: Option<f64>,
    pub supplier_id: i64,
    pub remark: Option<String>,
    pub purchase_price: f64,
    pub base_unit: Option<String>,
    pub base_price: f64,
}

/// 生成采购单：force 快照明细行（仅 source=本单 的快照 id 翻译结果）
#[derive(Debug, Clone)]
pub struct GenerateSnapshotRow {
    pub id: i64,
    pub product_id: i64,
    pub unit_key: String,
    pub warehouse_id: i64,
}

/// 生成采购单：force 旧明细行（PO 中 source=本单，待 diff）
#[derive(Debug, Clone)]
pub struct GenerateOldItemRow {
    pub id: i64,
    pub product_id: i64,
    pub unit_key: String,
    pub quantity: f64,
    pub amount: f64,
}

/// 生成采购单：PO 明细写入参数（快照 UPDATE / 孤儿复用 UPDATE / INSERT 共用字段集）
#[derive(Debug, Clone)]
pub struct GeneratePoItemWrite {
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
    pub ordered_quantity: f64,
    pub remark: String,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub sales_unit: String,
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
    /// 导出行查询（LEFT JOIN 明细，按 so.id, soi.id；失败由用例层吞为空）
    async fn export_rows(&self, keyword_pattern: &str, purchaser_id: Option<i64>) -> Result<Vec<SalesOrderExportRow>, DomainError>;
    /// 导入：按名称查采购单位 id（吞为 None）
    async fn find_purchaser_id_by_name(&self, name: &str) -> Result<Option<i64>, DomainError>;
    /// 导入：INSERT OR IGNORE 主表（返回 (last_insert_rowid, rows_affected)）
    async fn insert_import_order(&self, main: &SalesOrderImportMain) -> Result<(i64, u64), DomainError>;
    /// 导入：按 name+spec 查商品 id（吞为 None）
    async fn find_product_id_by_name_spec(&self, name: &str, spec: &str) -> Result<Option<i64>, DomainError>;
    /// 导入：查辅助单位换算比（吞为 None）
    async fn unit_ratio(&self, product_id: i64, unit: &str) -> Result<Option<f64>, DomainError>;
    /// 导入：插入明细（旧行为吞错，由用例层 let _ 忽略）
    async fn insert_import_item(&self, order_id: i64, item: &SalesOrderImportItem) -> Result<(), DomainError>;
    /// 验收单聚合主表行（JOIN purchaser；错误吞为 None 由用例层 404）
    async fn accept_order_row(&self, id: i64) -> Result<Option<SalesOrderAcceptOrderRow>, DomainError>;
    /// 验收单真实明细行（LEFT JOIN 分类；错误吞为空）
    async fn accept_item_rows(&self, id: i64) -> Result<Vec<SalesOrderAcceptItemRow>, DomainError>;
    /// 验收单增项行（错误吞为空）
    async fn accept_supplement_rows(&self, id: i64) -> Result<Vec<SalesOrderAcceptSupplementRow>, DomainError>;
    /// 验收单导出（Excel）主表行：含供应商名称/车牌号快照（错误吞为 None 由用例层 404）
    async fn accept_excel_order_row(&self, id: i64) -> Result<Option<AcceptExcelOrderRow>, DomainError>;
    /// 验收单导出（Excel）真实明细行（错误吞为空）
    async fn accept_excel_item_rows(&self, id: i64) -> Result<Vec<AcceptExcelItemRow>, DomainError>;
    /// 验收单导出（Excel）增项行（错误吞为空）
    async fn accept_excel_supplement_rows(&self, id: i64) -> Result<Vec<AcceptExcelSupplementRow>, DomainError>;
    /// 当日 sales_order 最大单号序号（失败吞为 0）
    async fn max_seq_for_date(&self, order_date: &str) -> Result<i64, DomainError>;
    /// 按 product_unit.ratio 批量换算明细基础单位数量（失败回退前端基础数量/quantity）
    async fn compute_base_quantities(&self, items: &[(i64, String, f64, f64)]) -> Vec<f64>;
    /// 增项排序分类反查：(category_name, parent_name)（错误吞为 None，用例层兜底空串）
    async fn product_category_names(&self, product_id: i64) -> Result<Option<(Option<String>, Option<String>)>, DomainError>;
    /// 生成采购单前置读：销售单主表行（错误吞为 None 由用例层 404）
    async fn generate_order_row(&self, id: i64) -> Result<Option<SalesOrderGenerateRow>, DomainError>;
    /// 生成采购单：已存在关联 PO（主表 source OR 明细 source；错误吞为空）
    async fn existing_po_rows(&self, id: i64) -> Result<Vec<GenerateExistedPo>, DomainError>;
    /// 生成采购单兜底：本单涉及的供应商 id（DISTINCT，supplier_id > 0；错误吞为空）
    async fn involved_supplier_ids(&self, id: i64) -> Result<Vec<i64>, DomainError>;
    /// 生成采购单兜底：当天同供应商 pending PO（supplier_ids 动态 IN；错误吞为空）
    async fn pending_po_pool(&self, order_date: &str, supplier_ids: &[i64]) -> Result<Vec<GenerateExistedPo>, DomainError>;
    /// force 路径：当天全部 pending PO（id, supplier_id；错误吞为空）
    async fn po_pool_for_date(&self, order_date: &str) -> Result<Vec<(i64, i64)>, DomainError>;
    /// force 路径：本单涉及的全部明细 supplier_id（不做过滤，用例层筛 >0；错误吞为空）
    async fn supplier_ids_of_order(&self, id: i64) -> Result<Vec<i64>, DomainError>;
    /// force 路径：PO 中 source=本单 的旧明细（错误吞为空）
    async fn old_items_by_po(&self, po_id: i64, so_id: i64) -> Result<Vec<GenerateOldItemRow>, DomainError>;
    /// force 路径：读取本单快照列原文（错误吞为 None）
    async fn snapshot_str(&self, id: i64) -> Option<String>;
    /// force 路径：快照 id 翻译为 (P,U,wh) 索引行（错误吞为空）
    async fn snapshot_rows(&self, so_id: i64, ids: &[i64]) -> Result<Vec<GenerateSnapshotRow>, DomainError>;
    /// 生成采购单：销售明细行（含分供应商进价 JOIN，按品名排序；错误吞为空）
    async fn generate_item_rows(&self, id: i64) -> Result<Vec<GenerateItemRow>, DomainError>;
    /// 辅助单位 (ratio, purchase_price COALESCE 0)（错误吞为 None）
    async fn unit_ratio_price(&self, product_id: i64, unit: &str) -> Result<Option<(f64, f64)>, DomainError>;
    /// 当天同供应商 pending PO id（ORDER BY id LIMIT 1；错误吞为 None）
    async fn find_pending_po_of_supplier(&self, supplier_id: i64, order_date: &str) -> Result<Option<i64>, DomainError>;
    /// 供应商名称（错误吞为 None，用例层兜底"未知供应商"）
    async fn supplier_name(&self, supplier_id: i64) -> Result<Option<String>, DomainError>;
    /// 新建采购单主表（金额 0 初始化，source=本销售单），返回 last_insert_rowid
    async fn insert_po_main(&self, supplier_id: i64, order_no: &str, order_date: &str, main_wh_id: i64, main_wh_name: &str, so_id: i64) -> Result<i64, DomainError>;
    /// PO 中 source=本单 的既有明细 (product_id, quantity, sales_unit)（错误吞为空）
    async fn existing_po_items(&self, po_id: i64, so_id: i64) -> Result<Vec<(i64, f64, String)>, DomainError>;
    /// PO 明细旧金额（COALESCE 0；错误吞为 0.0）
    async fn po_item_amount(&self, po_item_id: i64, po_id: i64) -> f64;
    /// 快照命中按主键 UPDATE（15 列，不动 source），返回 rows_affected（错误吞为 0）
    async fn update_po_item_via_snapshot(&self, po_id: i64, po_item_id: i64, w: &GeneratePoItemWrite) -> u64;
    /// 孤儿匹配：同 (P,U,warehouse) 任意 source，排除本轮已写入 id（动态 SQL），错误吞为 None
    async fn find_orphan_item(&self, po_id: i64, product_id: i64, unit_trim: &str, main_wh_id: i64, exclude_ids: &[i64], so_id: i64) -> Option<i64>;
    /// 孤儿复用 UPDATE（16 列含 source=本单），返回 rows_affected（错误吞为 0）
    async fn update_po_item_orphan(&self, po_id: i64, orphan_po_id: i64, so_id: i64, w: &GeneratePoItemWrite) -> u64;
    /// 插入 PO 明细（17 列含 source=本单），返回 last_insert_rowid
    async fn insert_po_item(&self, po_id: i64, so_id: i64, w: &GeneratePoItemWrite) -> Result<i64, DomainError>;
    /// 差量调整 PO 主表金额（错误吞掉）
    async fn adjust_po_totals(&self, po_id: i64, delta: f64);
    /// 回写本单快照列（错误吞掉）
    async fn write_snapshot(&self, so_id: i64, json: &str);
    /// 图片上传前置读：采购方名称与订单日期（fetch_optional 错误吞为 None，用例层兜底 "purchaser"/"nodate"）
    async fn image_purchaser_info(&self, order_id: i64) -> Option<(Option<String>, Option<String>)>;
    /// 图片上传写库：动态列名 UPDATE（旧行为 let _ 吞错）
    async fn update_image_column(&self, order_id: i64, column: &str, value: &str);
    /// 图片删除前置读：动态列名读图片 URL（错误吞为 None）
    async fn image_url_of(&self, order_id: i64, column: &str) -> Option<Option<String>>;
    /// 图片删除清列：动态列名 UPDATE 置 NULL（旧行为 let _ 吞错）
    async fn clear_image_column(&self, order_id: i64, column: &str);

    // ===== 分拣清单（sort*）：旧 handler 对 SQL 错误 unwrap_or_default 吞为空结果，用例层照旧吞错 =====
    /// 分拣清单·按商品聚合行（date 空查 pending/sorting 待分拣，非空查该日期）
    async fn sort_item_rows(&self, date: &str) -> Result<Vec<SortItemRow>, DomainError>;
    /// 分拣清单·按商品导出行（含 spec，display_name 规格优先在用例层）
    async fn sort_item_excel_rows(&self, date: &str) -> Result<Vec<SortItemExcelRow>, DomainError>;
    /// 分拣清单·按分类聚合行
    async fn sort_category_rows(&self, date: &str) -> Result<Vec<SortCategoryRow>, DomainError>;
    /// 分拣清单·按分类导出行（含 spec/pre_sale_quantity，amount 可空）
    async fn sort_category_excel_rows(&self, date: &str) -> Result<Vec<SortCategoryExcelRow>, DomainError>;
    /// 分拣清单·按供应商聚合行
    async fn sort_supplier_rows(&self, date: &str) -> Result<Vec<SortSupplierRow>, DomainError>;
    /// 分拣清单·按供应商导出行
    async fn sort_supplier_excel_rows(&self, date: &str) -> Result<Vec<SortSupplierExcelRow>, DomainError>;
    /// 分拣清单·按采购单位聚合行
    async fn sort_purchaser_rows(&self, date: &str) -> Result<Vec<SortPurchaserRow>, DomainError>;
    /// 分拣清单·按采购单位导出行（含分类排序键来源 category/parent 名）
    async fn sort_purchaser_excel_rows(&self, date: &str) -> Result<Vec<SortPurchaserExcelRow>, DomainError>;
    /// 采购历史价（MAX/MIN/最新一条）——by_purchaser_excel 第二查询
    async fn sort_purchaser_price_rows(&self) -> Result<Vec<SortPurchaserPriceRow>, DomainError>;
    /// 综合分拣聚合行
    async fn sort_comprehensive_rows(&self, date: &str) -> Result<Vec<SortComprehensiveRow>, DomainError>;
    /// 综合分拣导出行
    async fn sort_comprehensive_excel_rows(&self, date: &str) -> Result<Vec<SortComprehensiveExcelRow>, DomainError>;

    // ===== 分拣修正（correction）：旧代码逐条吞错（.ok() / if let Ok）=====
    /// 由明细 id 反查 product_id（错误吞为 None）
    async fn correction_product_id_by_item(&self, item_id: i64) -> Option<i64>;
    /// 回写商品进价，返回 rows_affected（错误吞为 None）
    async fn correction_update_product_price(&self, purchase_price: f64, product_id: i64) -> Option<u64>;
    /// 按 item_id 修正明细数量/单价（amount=ROUND(price*qty,2)），错误吞为 None
    async fn correction_update_item_by_id(&self, quantity: Option<f64>, unit_price: Option<f64>, item_id: i64) -> Option<u64>;
    /// 按 product_id 修正明细（同上 SQL），错误吞为 None
    async fn correction_update_item_by_product(&self, quantity: Option<f64>, unit_price: Option<f64>, product_id: i64) -> Option<u64>;
}

// ===== 分拣清单（sort*）行 DTO：字段可空性与旧 handler decode 方式逐字对应 =====

/// 分拣清单·按商品聚合行（GET sort_items）
#[derive(Debug, Clone)]
pub struct SortItemRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub purchaser_name: Option<String>,
    pub order_no: Option<String>,
}

/// 分拣清单·按商品导出行（sort_items_excel）
#[derive(Debug, Clone)]
pub struct SortItemExcelRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub purchaser_name: Option<String>,
    pub order_no: Option<String>,
}

/// 分拣清单·按分类聚合行（purchaser_name decode 为 String：NULL 令整批查询失败 → 旧行为空结果）
#[derive(Debug, Clone)]
pub struct SortCategoryRow {
    pub item_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub purchaser_name: String,
    pub purchaser_id: i64,
    pub order_no: Option<String>,
    pub category_name: Option<String>,
    pub category_id: i64,
}

/// 分拣清单·按分类导出行
#[derive(Debug, Clone)]
pub struct SortCategoryExcelRow {
    pub product_id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub quantity: f64,
    pub pre_sale_quantity: Option<f64>,
    pub amount: Option<f64>,
    pub remark: Option<String>,
    pub purchaser_name: String,
    pub purchaser_id: i64,
    pub order_no: Option<String>,
    pub category_name: Option<String>,
    pub category_id: i64,
}

/// 分拣清单·按供应商聚合行
#[derive(Debug, Clone)]
pub struct SortSupplierRow {
    pub item_id: i64,
    pub order_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub supplier_id: i64,
    pub supplier_name: Option<String>,
    pub purchaser_name: String,
    pub purchaser_id: i64,
    pub order_no: Option<String>,
    pub purchase_price: f64,
}

/// 分拣清单·按供应商导出行
#[derive(Debug, Clone)]
pub struct SortSupplierExcelRow {
    pub product_id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub quantity: f64,
    pub pre_sale_quantity: Option<f64>,
    pub amount: Option<f64>,
    pub remark: Option<String>,
    pub supplier_id: i64,
    pub supplier_name: Option<String>,
    pub purchaser_name: String,
    pub purchaser_id: i64,
    pub order_no: Option<String>,
    pub purchase_price: f64,
}

/// 分拣清单·按采购单位聚合行
#[derive(Debug, Clone)]
pub struct SortPurchaserRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub purchaser_id: i64,
    pub purchaser_name: Option<String>,
    pub order_no: Option<String>,
}

/// 分拣清单·按采购单位导出行
#[derive(Debug, Clone)]
pub struct SortPurchaserExcelRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub purchaser_id: i64,
    pub purchaser_name: Option<String>,
    pub order_no: Option<String>,
    pub category_name: Option<String>,
    pub parent_name: Option<String>,
}

/// 采购历史价行（MAX/MIN/最新一条）
#[derive(Debug, Clone)]
pub struct SortPurchaserPriceRow {
    pub product_id: i64,
    pub max_price: Option<f64>,
    pub min_price: Option<f64>,
    pub latest_price: Option<f64>,
}

/// 综合分拣聚合行
#[derive(Debug, Clone)]
pub struct SortComprehensiveRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub unit: Option<String>,
    pub unit_price: f64,
    pub quantity: f64,
    pub amount: f64,
    pub remark: Option<String>,
    pub purchaser_id: i64,
    pub purchaser_name: Option<String>,
    pub order_no: Option<String>,
    pub category_name: Option<String>,
    pub category_id: i64,
}

/// 综合分拣导出行
#[derive(Debug, Clone)]
pub struct SortComprehensiveExcelRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub quantity: f64,
    pub remark: Option<String>,
    pub purchaser_id: i64,
    pub purchaser_name: Option<String>,
    pub order_no: Option<String>,
    pub category_name: Option<String>,
    pub category_id: i64,
}
