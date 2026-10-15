//! 价格策略时段出站端口：时段 CRUD / 生效价查询 / 快照与政采状态联动 / 销售单补价
//!
//! SQL 与旧 api_price_schedule_* handler 及 application::service::price 过渡态函数逐字一致，
//! 行为保持注释见各方法。

use crate::error::DomainError;

/// 列表/单条查询行（JOIN product/supplier/purchaser 带回名称）
#[derive(Debug, Clone)]
pub struct ScheduleListRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub price_type: String,
    pub price: f64,
    pub effective_date: String,
    pub end_date: Option<String>,
    pub source: Option<String>,
    pub remark: Option<String>,
    pub create_at: Option<String>,
    pub supplier_id: Option<i64>,
    pub purchaser_id: Option<i64>,
    pub supplier_name: Option<String>,
    pub purchaser_name: Option<String>,
}

/// 导出行（仅 JOIN product，无 create_at/约定方列）
#[derive(Debug, Clone)]
pub struct ScheduleExportRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub price_type: String,
    pub price: f64,
    pub effective_date: String,
    pub end_date: Option<String>,
    pub source: Option<String>,
    pub remark: Option<String>,
}

/// 历史时段行
#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub id: i64,
    pub price: f64,
    pub effective_date: String,
    pub end_date: Option<String>,
    pub source: Option<String>,
    pub remark: Option<String>,
    pub create_at: Option<String>,
}

/// 时段诊断行
#[derive(Debug, Clone)]
pub struct DiagnoseRow {
    pub id: i64,
    pub price_type: String,
    pub price: f64,
    pub effective_date: String,
    pub end_date: Option<String>,
    pub source: Option<String>,
    pub remark: Option<String>,
    pub supplier_id: Option<i64>,
    pub purchaser_id: Option<i64>,
}

/// 销售单补价明细行
#[derive(Debug, Clone)]
pub struct BackfillItemRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub unit: Option<String>,
    pub quantity: f64,
    pub unit_price: f64,
    #[allow(dead_code)]
    pub amount: f64,
}

/// 采购明细进价更新输入（domain 自有类型，不含 serde）
///
/// 供 `PriceScheduleRepo::update_product_purchase_prices` 使用：由 application 层把
/// HTTP 采购明细契约映射为本结构体后送入端口。
#[derive(Debug, Clone)]
pub struct PurchasePriceInput {
    pub product_id: i64,
    pub unit_price: f64,
    pub quantity: f64,
    pub base_quantity: Option<f64>,
    pub amount: f64,
}

/// 列表/导出筛选条件（空串/0 表示不过滤）
#[derive(Debug, Clone, Default)]
pub struct ScheduleFilter {
    pub product_id: i64,
    pub price_type: String,
    pub effective_from: String,
    pub effective_to: String,
    pub keyword: String,
}

/// 价格策略时段写入输入（domain 自有类型，不含 serde）
///
/// 由 application 层把 HTTP 价格时段契约映射为本结构体后送入端口。
#[derive(Debug, Clone)]
pub struct PriceScheduleInput {
    pub id: Option<i64>,
    pub product_id: i64,
    pub price_type: String,
    pub price: f64,
    pub effective_date: String,
    pub end_date: Option<String>,
    pub source: Option<String>,
    pub remark: Option<String>,
    pub supplier_id: Option<i64>,
    pub purchaser_id: Option<i64>,
}

/// 价格策略仓储端口
pub trait PriceScheduleRepo: Send + Sync {
    /// 新增时段（end_date 强制 NULL；返回 last_insert_rowid；UNIQUE 冲突以 Err 返回且消息含 "UNIQUE"）
    async fn create(&self, req: &PriceScheduleInput, price: f64) -> Result<i64, DomainError>;
    /// 按 id 查 (product_id, price_type)（update/delete 前取原组合；查询失败按旧行为吞为 None）
    async fn get_product_type(&self, id: i64) -> Result<Option<(i64, String)>, DomainError>;
    /// 更新时段（返回 rows_affected）
    async fn update(&self, id: i64, req: &PriceScheduleInput) -> Result<u64, DomainError>;
    /// 删除时段（返回 rows_affected）
    async fn delete(&self, id: i64) -> Result<u64, DomainError>;
    /// 列表总数（动态 WHERE；失败由用例层吞为 0）
    async fn count_list(&self, filter: &ScheduleFilter) -> Result<i64, DomainError>;
    /// 列表分页查询（失败由用例层吞为空）
    async fn list(&self, filter: &ScheduleFilter, page_size: i64, offset: i64) -> Result<Vec<ScheduleListRow>, DomainError>;
    /// 导出查询（无分页；失败由用例层吞为空）
    async fn export_list(&self, filter: &ScheduleFilter) -> Result<Vec<ScheduleExportRow>, DomainError>;
    /// 历史时段（按生效日期升序；失败由用例层吞为空）
    async fn history(&self, product_id: i64, price_type: &str) -> Result<Vec<HistoryRow>, DomainError>;
    /// 按 id 查单条（含名称 JOIN；查询失败按旧行为吞为 None）
    async fn get_by_id(&self, id: i64) -> Result<Option<ScheduleListRow>, DomainError>;
    /// 商品名（时段诊断用；查询失败吞为 None）
    async fn product_name(&self, product_id: i64) -> Result<Option<String>, DomainError>;
    /// 时段诊断：某商品全部价格类型时段链（失败由用例层吞为空）
    async fn diagnose(&self, product_id: i64) -> Result<Vec<DiagnoseRow>, DomainError>;
    /// 通用价存在性查询（批量导入 upsert 用，带 supplier_id/purchaser_id IS NULL 条件；失败吞为 None）
    async fn find_existing_general(&self, product_id: i64, price_type: &str, effective_date: &str) -> Result<Option<i64>, DomainError>;
    /// 通用价计数（批量导入 ignore 用，带 NULL 条件；失败吞为 0）
    async fn count_general(&self, product_id: i64, price_type: &str, effective_date: &str) -> Result<i64, DomainError>;
    /// 按键计数（Excel 导入用，不带 NULL 条件；失败吞为 0）
    async fn count_by_key(&self, product_id: i64, price_type: &str, effective_date: &str) -> Result<i64, DomainError>;
    /// 按 id 更新价格/来源/备注（批量导入 upsert 分支；失败返回 Err 由用例层收集）
    async fn update_price_by_id(&self, id: i64, price: f64, source: &Option<String>, remark: &Option<String>) -> Result<(), DomainError>;
    /// 按键更新价格/来源/备注（Excel 导入分支，返回 rows_affected；失败返回 Err 由用例层收集）
    async fn update_price_by_key(&self, product_id: i64, price_type: &str, effective_date: &str, price: f64, source: &Option<String>, remark: &Option<String>) -> Result<u64, DomainError>;
    /// 插入通用价（supplier_id/purchaser_id 写 NULL；失败返回 Err 由用例层收集）
    async fn insert_general(&self, req: &PriceScheduleInput, price: f64) -> Result<(), DomainError>;
    /// 插入 Excel 导入行（无 supplier/purchaser 列；失败返回 Err 由用例层收集）
    async fn insert_excel(&self, req: &PriceScheduleInput, price: f64) -> Result<(), DomainError>;
    /// 维护 (product, price_type) 时段连续性：按 (supplier_id, purchaser_id) 分组重写 end_date
    /// （首查失败返回 Err；逐条 UPDATE 失败吞错，与旧 rebuild_price_schedule_end_dates 一致）
    async fn rebuild_end_dates(&self, product_id: i64, price_type: &str) -> Result<(), DomainError>;
    /// 同步 product_price 快照：取该组合最后一条 effective 价（失败由用例层吞错）
    async fn sync_price_snapshot(&self, product_id: i64, price_type: &str) -> Result<(), DomainError>;
    /// 政采状态双向联动：快照有价→1；无价→仅回退自动标记的 1（两条 UPDATE 均吞错）
    async fn auto_mark_gov_status(&self, product_id: i64) -> Result<(), DomainError>;
    /// 价格变更日志：仅 |old-new|>=0.001 时写入（吞错）
    async fn log_price_change(&self, product_id: i64, price_type: &str, old_price: f64, new_price: f64, source: &str, ref_id: Option<i64>, remark: Option<&str>) -> Result<(), DomainError>;
    /// 按加成率重算 base_price（内部读商品/跳过判断/写回/记日志，吞错；eprintln 逐字保持）
    async fn recalc_base_price_by_markup(&self, product_id: i64, source: &str, ref_id: Option<i64>) -> Result<(), DomainError>;
    /// 按快照同步商品 base_price（政采优先/商超均价；内部调 auto_mark/log/recalc，吞错）
    async fn sync_base_price(&self, product_id: i64) -> Result<(), DomainError>;
    /// 单商品生效价查询：(price, schedule_id, effective_date)；查询失败吞为 None
    async fn lookup_effective(&self, product_id: i64, price_type: &str, query_date: &str) -> Result<Option<(f64, i64, String)>, DomainError>;
    /// 批量生效价查询：返回 price>0 且四舍五入的 map；失败吞为空 map
    async fn batch_lookup_effective(&self, product_ids: &[i64], price_type: &str, query_date: &str) -> Result<std::collections::HashMap<i64, f64>, DomainError>;
    /// 约定价查询（price_type 决定 party 列）；查询失败吞为 None
    async fn lookup_contract(&self, product_id: i64, price_type: &str, party_id: i64, query_date: &str) -> Result<Option<(f64, i64, String)>, DomainError>;
    /// 销售单 purchaser_id（行级权限用；查询失败吞为 None）
    async fn sales_order_purchaser_id(&self, order_id: i64) -> Result<Option<i64>, DomainError>;
    /// 销售单 order_date（失败吞为空串）
    async fn sales_order_order_date(&self, order_id: i64) -> Result<String, DomainError>;
    /// 销售单明细（补价用；失败吞为空）
    async fn sales_order_items(&self, order_id: i64) -> Result<Vec<BackfillItemRow>, DomainError>;
    /// 更新明细单价与金额（失败吞错，与旧行为一致）
    async fn update_sales_item_price(&self, item_id: i64, price: f64, amount: f64) -> Result<(), DomainError>;
    /// 销售单折扣率（失败吞为 0）
    async fn sales_order_discount_rate(&self, order_id: i64) -> Result<f64, DomainError>;
    /// 销售单明细金额合计（失败吞为 0）
    async fn sales_order_total_amount(&self, order_id: i64) -> Result<f64, DomainError>;
    /// 回写销售单金额三字段（失败吞错，与旧行为一致）
    async fn update_sales_order_amounts(&self, order_id: i64, total: f64, reduction: f64, final_amount: f64) -> Result<(), DomainError>;
    /// Excel 导入商品匹配四步回退：name+spec → name+alias1 → name+alias2 → name only（查询失败吞为 None）
    async fn find_product_id(&self, name: &str, spec: &str, alias1: &str, alias2: &str) -> Result<Option<i64>, DomainError>;
    /// 采购明细进价更新：换算基础单位价后回写 product 当前/最高/最低进价，
    /// 记价格日志并在开启自动更新售价时按加成率重算（逐条吞错，与旧 free fn 一致）
    async fn update_product_purchase_prices(&self, items: &[PurchasePriceInput]) -> Result<(), DomainError>;
    /// 采购单删除后从剩余采购明细历史重算商品最近/最高/最低进价；无明细则三项置 0
    async fn recalc_product_purchase_prices_from_history(&self, product_id: i64) -> Result<(), DomainError>;
    /// 单商品开启/关闭自动更新售价，并立即按加成率重算 base_price（吞错）
    async fn set_auto_update_price(&self, product_id: i64, auto: i64) -> Result<(), DomainError>;
    /// 批量开启/关闭自动更新售价；开启时对所有进价>0 的商品按加成率重算（吞错）
    async fn batch_set_auto_update_price(&self, auto: i64) -> Result<(), DomainError>;
}
