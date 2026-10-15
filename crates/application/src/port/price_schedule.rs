//! 价格策略时段入站端口

use crate::dto::PriceScheduleReq;
use domain::UserCtx;
use domain::port::price_schedule::ScheduleFilter;

use crate::error::UseCaseError;

/// 写操作用例返回体：预序列化 JSON（逐字节兼容）+ 可选操作日志参数
/// log = Some((target_id, detail)) 时由 HTTP 层补记 log_operation（action/target_type  handler 侧已知）
#[derive(Debug, Clone)]
pub struct PriceScheduleOutcome {
    pub json: String,
    pub log: Option<(String, String)>,
}

impl PriceScheduleOutcome {
    pub fn with_log(json: String, target_id: String, detail: String) -> Self {
        Self { json, log: Some((target_id, detail)) }
    }

    pub fn without_log(json: String) -> Self {
        Self { json, log: None }
    }
}

/// 价格策略时段用例端口（权限检查与操作日志写库在 HTTP 层）
pub trait PriceScheduleUseCase: Send + Sync {
    /// 新增时段（成功后联动 rebuild/快照/政采标记；UNIQUE 冲突 409）
    async fn create(&self, req: PriceScheduleReq) -> Result<PriceScheduleOutcome, UseCaseError>;
    /// 更新时段（旧组合+新组合双端重建）
    async fn update(&self, req: PriceScheduleReq) -> Result<PriceScheduleOutcome, UseCaseError>;
    /// 删除时段（成功后联动 rebuild/快照/政采标记）
    async fn delete(&self, id: i64) -> Result<PriceScheduleOutcome, UseCaseError>;
    /// 列表分页查询（预序列化 JSON；查询失败吞为空/0）
    async fn list(&self, filter: ScheduleFilter, page: i64, page_size: i64) -> String;
    /// 批量导入（upsert/ignore 两策略；导入后联动 rebuild/快照/政采标记/base_price 同步）
    async fn batch_import(&self, items: Vec<PriceScheduleReq>, conflict_policy: Option<String>) -> PriceScheduleOutcome;
    /// 历史时段（product_id<=0 或 price_type 空时返回空 data）
    async fn history(&self, product_id: i64, price_type: String) -> String;
    /// 按 id 查单条（id<=0 → 400；找不到 → 200 success:false）
    async fn get(&self, id: i64) -> Result<String, UseCaseError>;
    /// 时段诊断（product_id<=0 → 400）
    async fn diagnose(&self, product_id: i64) -> Result<String, UseCaseError>;
    /// 批量查 5 类价（date 空 → 400；product_ids 空 → 空 prices）
    async fn lookup_batch(&self, date: String, product_ids: Vec<i64>) -> Result<String, UseCaseError>;
    /// 单商品生效价查询（query_date=None 取当天；约定价缺 party_id 返回 matched:false+message）
    async fn lookup(&self, product_id: i64, price_type: String, query_date: Option<String>, party_id: i64) -> String;
    /// 导入模板 xlsx 字节
    async fn export_template(&self) -> Result<Vec<u8>, UseCaseError>;
    /// 按筛选导出列表 xlsx 字节
    async fn export(&self, filter: ScheduleFilter) -> Result<Vec<u8>, UseCaseError>;
    /// 销售单按时段价补价（行级权限在用例内按 UserCtx 校验；force=1 全量重算否则仅 unit_price<=0）
    async fn backfill_sales_order(&self, order_id: i64, force: bool, price_type: String, ctx: &UserCtx) -> Result<PriceScheduleOutcome, UseCaseError>;
    /// Excel 批量导入（解析+四步商品匹配+upsert 语义写入）
    async fn batch_import_excel(&self, bytes: Vec<u8>) -> Result<PriceScheduleOutcome, UseCaseError>;
}
