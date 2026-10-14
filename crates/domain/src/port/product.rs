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

/// 商品导出行（api_product_export 宽表：政采/商超1/2/3/AI 左联展开）
#[derive(Debug, Clone)]
pub struct ProductExportRow {
    pub id: i64,
    pub name: String,
    pub alias1: Option<String>,
    pub alias2: Option<String>,
    pub spec: Option<String>,
    pub unit: String,
    pub base_unit: Option<String>,
    pub base_price: f64,
    pub purchase_price: f64,
    pub category_name: Option<String>,
    pub gov_price: f64,
    pub sm1_price: f64,
    pub sm2_price: f64,
    pub sm3_price: f64,
    pub ai_price: f64,
}

/// 价格快照完整行（api_product_price_list）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PriceSnapshotRow {
    pub id: i64,
    pub product_id: i64,
    pub price_type: String,
    pub price: f64,
    pub collected_at: Option<String>,
    pub source: Option<String>,
}

/// 价格变更日志行（api_product_price_log_list）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PriceLogRow {
    pub id: i64,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub price_type: String,
    pub old_price: f64,
    pub new_price: f64,
    pub source: Option<String>,
    pub ref_id: Option<i64>,
    pub remark: Option<String>,
    pub changed_at: Option<String>,
}

/// 商品进价信息（api_product_last_purchase_price 第一段查询）
#[derive(Debug, Clone)]
pub struct ProductPurchaseInfo {
    pub purchase_price: f64,
    pub base_unit: String,
}

/// 最近一次采购明细行（api_product_last_purchase_price 第二段查询）
#[derive(Debug, Clone)]
pub struct LastPurchaseItemRow {
    pub unit_price: f64,
    pub unit: String,
}

/// 导入行解析后的商品字段（价格已四舍五入两位）
#[derive(Debug, Clone, Default)]
pub struct ImportProduct {
    pub name: String,
    pub alias1: String,
    pub alias2: String,
    pub spec: String,
    pub unit: String,
    pub base_unit: String,
    pub base_price: f64,
    pub purchase_price: f64,
    pub category_id: Option<i64>,
}

/// 导入追加结果：Inserted(新行id) / Ignored（INSERT OR IGNORE 被唯一约束忽略）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImportInsertOutcome {
    Inserted(i64),
    Ignored,
}

/// 商品单位写入入参（unit_create / unit_update 共用）
#[derive(Debug, Clone, Default)]
pub struct NewProductUnit {
    pub product_id: i64,
    pub unit_name: String,
    pub ratio: f64,
    pub unit_price: f64,
    pub purchase_price: f64,
    pub sort_order: i32,
}

/// 远程图片转存出站端口：下载网络图片到本地 uploads 目录（导入/图片上传等场景）
pub trait RemoteImageStore: Send + Sync {
    async fn save_remote(&self, url: &str, name_prefix: &str) -> Result<String, String>;
}

/// 本地图片文件存储出站端口（uploads 目录的写/删/读）
pub trait ProductImageStore: Send + Sync {
    /// 保存上传图片到 uploads/products/，返回本地访问 URL（/api/uploads/products/xxx）
    async fn save_upload(&self, name_prefix: &str, ext: &str, bytes: &[u8]) -> Result<String, String>;
    /// 按访问 URL 删除本地文件（image_url_to_path 解析失败/删除失败均吞错）
    async fn remove_by_url(&self, url: &str);
    /// 读取 uploads 下文件（path 为完整相对路径，由调用方做白名单/防穿越校验）
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, String>;
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

    // —— 子步c：图片读写 ——

    /// 按 id 取商品名称（不存在或查询失败均返回 None）
    async fn name_of(&self, id: i64) -> Result<Option<String>, DomainError>;
    /// 按 id 取 image_url（行缺失/列 NULL 均返回 None）
    async fn image_url_of(&self, id: i64) -> Result<Option<String>, DomainError>;
    /// 更新 image_url；url=None 时绑定 NULL（替代旧 SET image_url = NULL 字面量）
    async fn set_image_url_opt(&self, id: i64, url: Option<&str>) -> Result<(), DomainError>;

    // —— 子步b：导出 / 导入 ——

    /// 导出宽表（政采/商超1/2/3/AI 左联展开；失败按旧行为吞错返回空，由用例层处理）
    async fn list_for_export(&self) -> Result<Vec<ProductExportRow>, DomainError>;
    /// 按分类名查 id（entity_type='product'）
    async fn find_category_id(&self, name: &str) -> Result<Option<i64>, DomainError>;
    /// 按 id 计数（导入按 ID 更新分支；失败按旧行为吞为 0）
    async fn count_by_id(&self, id: i64) -> Result<i64, DomainError>;
    /// 导入按 ID 更新（SQL 内置 audit_status='pending'），返回 rows_affected
    async fn import_update(&self, id: i64, p: &ImportProduct) -> Result<u64, DomainError>;
    /// 名称+规格定位占用商品 id（UNIQUE 冲突提示用）
    async fn find_id_by_name_spec(&self, name: &str, spec: &str) -> Result<Option<i64>, DomainError>;
    /// 导入追加（INSERT OR IGNORE；被忽略=Ignored，成功=Inserted(rowid)）
    async fn import_insert(&self, id: Option<i64>, p: &ImportProduct) -> Result<ImportInsertOutcome, DomainError>;
    /// 回填导入转存的图片 URL
    async fn set_image_url(&self, id: i64, image_url: &str) -> Result<(), DomainError>;
    /// 导入价格列：时段价写入（今天生效，同日覆盖）
    async fn import_schedule_price(&self, product_id: i64, price_type: &str, price: f64, effective_date: &str, source: &str) -> Result<(), DomainError>;
    /// 导入价格列：快照写入
    async fn import_price_snapshot(&self, product_id: i64, price_type: &str, price: f64, collected_at: &str, source: &str) -> Result<(), DomainError>;

    // —— 子步b：商品单位 ——

    /// 新建商品单位
    async fn unit_create(&self, u: &NewProductUnit) -> Result<(), DomainError>;
    /// 更新商品单位（旧行为 WHERE id 直接取请求的 product_id 字段）
    async fn unit_update(&self, id: i64, u: &NewProductUnit) -> Result<(), DomainError>;
    /// 删除商品单位，返回 rows_affected（0=不存在）
    async fn unit_delete(&self, id: i64) -> Result<u64, DomainError>;
    /// 按商品清空单位（吞错由用例层处理）
    async fn unit_delete_by_product(&self, product_id: i64) -> Result<(), DomainError>;

    // —— 子步b：价格快照 ——

    /// 价格快照 UPSERT（INSERT OR REPLACE）
    async fn price_upsert(&self, product_id: i64, price_type: &str, price: f64, collected_at: Option<&str>, source: Option<&str>) -> Result<(), DomainError>;
    /// 价格快照全列行（失败按旧行为吞错返回空，由用例层处理）
    async fn price_rows(&self, product_id: i64) -> Result<Vec<PriceSnapshotRow>, DomainError>;
    /// 删除价格快照，返回 rows_affected（0=不存在）
    async fn price_delete(&self, id: i64) -> Result<u64, DomainError>;
    /// 按商品清空价格快照（旧行为吞错恒成功）
    async fn price_delete_by_product(&self, product_id: i64) -> Result<(), DomainError>;

    // —— 子步b：最近进价 / 价格日志 ——

    /// 商品进价信息（purchase_price/base_unit；None=不存在或查询失败）
    async fn get_purchase_info(&self, product_id: i64) -> Result<Option<ProductPurchaseInfo>, DomainError>;
    /// 最近一次采购明细（按 order_date DESC, id DESC 取一）
    async fn get_last_purchase_item(&self, product_id: i64) -> Result<Option<LastPurchaseItemRow>, DomainError>;
    /// 价格变更日志（可按商品过滤；失败按旧行为吞错返回空，由用例层处理）
    async fn price_logs(&self, product_id: Option<i64>, limit: i64) -> Result<Vec<PriceLogRow>, DomainError>;
}
