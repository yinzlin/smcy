//! 销售订单入站端口（UseCase 契约）

use domain::port::sales_order::SalesOrderListFilter;
use domain::{SalesOrderReq, UserCtx};

use crate::error::UseCaseError;

/// 一条待补记的操作日志（log_operation 写库在 HTTP 层）
#[derive(Debug, Clone)]
pub struct SalesOrderLog {
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub detail: String,
}

impl SalesOrderLog {
    pub fn new(action: &str, target_id: String, detail: String) -> Self {
        Self {
            action: action.to_string(),
            target_type: "sales_order".to_string(),
            target_id,
            detail,
        }
    }
}

/// 写操作用例返回体：文本/JSON 响应体 + 待补记的操作日志序列
/// （销售单创建/更新可能先产生「价格策略自动补价」日志，再产生主日志，故为 Vec）
#[derive(Debug, Clone)]
pub struct SalesOrderOutcome {
    pub body: String,
    pub logs: Vec<SalesOrderLog>,
}

impl SalesOrderOutcome {
    pub fn with_logs(body: String, logs: Vec<SalesOrderLog>) -> Self {
        Self { body, logs }
    }
}

/// 销售订单用例端口（权限点校验与操作日志写库在 HTTP 层；行级数据权限在用例层按 UserCtx 校验）
pub trait SalesOrderUseCase: Send + Sync {
    /// 创建销售单（采购方/商品审核校验 + purchaser 行级权限 + 价格策略时段补价 + base_quantity 服务端重算）
    async fn create(&self, req: SalesOrderReq, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 列表分页（purchaser 角色强制只看自己；预序列化 JSON 逐字节保持）
    async fn list(&self, ctx: &UserCtx, filter: SalesOrderListFilter) -> Result<String, UseCaseError>;
    /// 按采购单位列出订单（含耗材分配状态；裸 JSON 数组）
    async fn by_purchaser(&self, purchaser_id: i64, ctx: &UserCtx) -> Result<String, UseCaseError>;
    /// 详情（两段式存在性校验 + 行级权限；预序列化 JSON）
    async fn detail(&self, id: i64, ctx: &UserCtx) -> Result<String, UseCaseError>;
    /// 乐观锁更新（version 必传且 >0；明细非空护栏；409 版本冲突；价格策略时段补价）
    async fn update(&self, req: SalesOrderReq, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 一键获取最新售价（仅 pending；返回 items/total/final/errors JSON）
    async fn update_prices(&self, id: i64, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 删除（仅 pending；外键冲突文案识别）
    async fn delete(&self, id: i64, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 设置结算状态（仅 pending；is_settled 仅 0/1；409 两分支逐字保持）
    async fn settle(&self, id: i64, is_settled: Option<i64>, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 状态流转（状态机白名单 + 原子 UPDATE + accepted 进/出库存流水 + FIFO 批次快照）
    async fn update_status(&self, id: Option<i64>, new_status: Option<String>, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 审核（仅 pending；只转状态不动库存）
    async fn approve(&self, id: i64, reason: String, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 反审核（reason 必填；super_admin 任意时刻可反审核；原状态 accepted 时写冲销流水）
    async fn unapprove(&self, id: i64, reason: String, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
}
