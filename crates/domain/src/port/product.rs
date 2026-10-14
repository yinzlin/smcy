//! 商品域出站端口：列表/详情/搜索/CRUD/状态切换
//!
//! SQL 与旧 api_product_* handler 逐字一致，行为保持注释见各方法。

use crate::error::DomainError;

/// 商品列表行（api_product_list 的 select_cols 全列）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductRow {
    pub id: i64,
    pub name: String,
    pub spec: Option<String>,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub unit: String,
    pub base_unit: String,
    pub base_price: f64,
    pub purchase_price: f64,
    pub max_purchase_price: f64,
    pub min_purchase_price: f64,
    pub markup_rate: f64,
    pub auto_update_price: i64,
    pub image_url: Option<String>,
    pub category_id: Option<i64>,
    pub status: i64,
    pub audit_status: Option<String>,
    pub shelf_life: Option<String>,
    pub gov_status: i64,
    pub category_name: Option<String>,
}

/// 商品列表过滤条件
#[derive(Debug, Clone, Default)]
pub struct ProductListFilter {
    pub product_id: Option<i64>,
    pub category_id: Option<i64>,
    pub keyword_pattern: String,
    pub gov_status: Option<i64>,
    pub include_thumbs: bool,
    pub page: i64,
    pub page_size: i64,
    pub offset: i64,
}

/// 商品单位行（product_unit）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductUnitRow {
    pub id: i64,
    pub unit_name: String,
    pub ratio: f64,
    pub unit_price: f64,
    pub purchase_price: f64,
    pub sort_order: i32,
}

/// 商品价格行（product_price）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductPriceRow {
    pub price_type: String,
    pub price: f64,
}

/// 同名商品检查行（api_product_check_name）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductSummaryRow {
    pub id: i64,
    pub name: String,
    pub spec: Option<String>,
    pub unit: String,
    pub base_unit: String,
    pub base_price: f64,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
}

/// 商品详情行（api_product_by_id，selling_price 为 SQL 计算列）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductDetailRow {
    pub id: i64,
    pub name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub base_unit: Option<String>,
    pub base_price: f64,
    pub purchase_price: f64,
    pub selling_price: f64,
    pub shelf_life: Option<String>,
    pub category_name: Option<String>,
}

/// 商品搜索行（api_product_search，selling_price 为 SQL 计算列）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductSearchRow {
    pub id: i64,
    pub name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: String,
    pub base_unit: String,
    pub base_price: f64,
    pub purchase_price: f64,
    pub selling_price: f64,
    pub category_name: Option<String>,
}

/// 新建商品入参
#[derive(Debug, Clone, Default)]
pub struct NewProduct {
    pub name: String,
    pub spec: Option<String>,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub unit: String,
    pub base_unit: String,
    pub base_price: f64,
    pub purchase_price: f64,
    pub category_id: Option<i64>,
    pub shelf_life: Option<String>,
    pub gov_status: i64,
}

/// 更新商品入参
#[derive(Debug, Clone, Default)]
pub struct UpdateProduct {
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
    pub shelf_life: Option<String>,
    pub gov_status: i64,
}

/// 编辑前价格快照（旧值，用于进价/售价变更日志与重算触发判断）
#[derive(Debug, Clone, Copy, Default)]
pub struct PriceSnapshot {
    pub base_price: f64,
    pub purchase_price: f64,
    pub markup_rate: f64,
    pub auto_update_price: i64,
}

/// 删除前关联数据计数
#[derive(Debug, Clone, Copy, Default)]
pub struct RefCounts {
    pub inventory: i64,
    pub purchase_item: i64,
    pub sales_item: i64,
    pub supplement_item: i64,
}

/// 删除前关联数据计数查询错误（携带表名供上层文案）
#[derive(Debug)]
pub struct RefCountError {
    pub table: &'static str,
    pub message: String,
}

/// 级联删除四阶段错误分类（开启事务/清附属/删主/提交）
#[derive(Debug)]
pub enum DeleteCascadeError {
    Begin(String),
    Cleanup(String),
    Delete(String),
    Commit(String),
}

impl std::fmt::Display for DeleteCascadeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Begin(m) => write!(f, "begin: {}", m),
            Self::Cleanup(m) => write!(f, "cleanup: {}", m),
            Self::Delete(m) => write!(f, "delete: {}", m),
            Self::Commit(m) => write!(f, "commit: {}", m),
        }
    }
}

/// 商品仓储端口
pub trait ProductRepo: Send + Sync {
    /// 分页列表，返回 (行, 总数)；总数查询失败按旧行为上抛（旧 unwrap → 500）
    async fn list(&self, f: &ProductListFilter) -> Result<(Vec<ProductRow>, i64), DomainError>;
    /// 商品单位列表（失败按旧行为吞错返回空，由用例层处理）
    async fn units_of(&self, product_id: i64) -> Result<Vec<ProductUnitRow>, DomainError>;
    /// 商品价格列表（失败按旧行为吞错返回空，由用例层处理）
    async fn prices_of(&self, product_id: i64) -> Result<Vec<ProductPriceRow>, DomainError>;
    /// 同名商品检查
    async fn find_by_name(&self, name: &str) -> Result<Vec<ProductSummaryRow>, DomainError>;
    /// 按 id 取详情（含 selling_price 计算列）
    async fn get_detail(&self, id: i64) -> Result<Option<ProductDetailRow>, DomainError>;
    /// 关键字搜索（含 selling_price 计算列；only_audited 仅已审核）
    async fn search(&self, pattern: &str, only_audited: bool) -> Result<Vec<ProductSearchRow>, DomainError>;
    /// 商品状态（toggle_status 用；None=不存在）
    async fn get_status(&self, id: i64) -> Result<Option<i64>, DomainError>;
    /// 更新商品启用/停用状态
    async fn set_status(&self, id: i64, status: i64) -> Result<(), DomainError>;
    /// 审核状态（锁定守卫用）
    async fn get_audit_status(&self, id: i64) -> Result<Option<String>, DomainError>;
    /// 新建商品
    async fn create(&self, p: &NewProduct) -> Result<(), DomainError>;
    /// 编辑前价格快照
    async fn get_price_snapshot(&self, id: i64) -> Result<Option<PriceSnapshot>, DomainError>;
    /// 更新商品（SQL 内置 audit_status='pending'）
    async fn update(&self, id: i64, p: &UpdateProduct) -> Result<(), DomainError>;
    /// 删除前关联数据计数（四表计数，任一查询失败携带表名上抛）
    async fn count_refs(&self, id: i64) -> Result<RefCounts, RefCountError>;
    /// 事务内清附属数据并删除商品，返回 rows_affected（0=不存在）
    async fn delete_cascade(&self, id: i64) -> Result<u64, DeleteCascadeError>;
}
