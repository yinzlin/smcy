//! 采购订单入站端口（UseCase 契约）

use domain::port::purchase_order::PurchaseOrderListFilter;
use domain::{PurchaseOrderReq, UserCtx};

use crate::error::UseCaseError;

/// 写操作用例返回体：文本响应体 + 可选操作日志参数
/// log = Some((target_id, detail)) 时由 HTTP 层补记 log_operation（action/target_type handler 侧已知）
#[derive(Debug, Clone)]
pub struct PurchaseOrderOutcome {
    pub body: String,
    pub log: Option<(String, String)>,
}

impl PurchaseOrderOutcome {
    pub fn with_log(body: String, target_id: String, detail: String) -> Self {
        Self { body, log: Some((target_id, detail)) }
    }

    #[allow(dead_code)]
    pub fn without_log(body: String) -> Self {
        Self { body, log: None }
    }
}

/// 采购订单用例端口（权限点校验与操作日志写库在 HTTP 层；行级数据权限在用例层按 UserCtx 校验）
pub trait PurchaseOrderUseCase: Send + Sync {
    /// 创建采购单（供应商审核校验 + supplier 行级权限 + 主表仓库汇总 + base_quantity 服务端重算 + 进价联动）
    async fn create(&self, req: PurchaseOrderReq, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError>;
    /// 列表分页（supplier 角色强制只看自己；预序列化 JSON 逐字节保持）
    async fn list(&self, ctx: &UserCtx, filter: PurchaseOrderListFilter) -> Result<String, UseCaseError>;
    /// 详情（行级权限校验；预序列化 JSON）
    async fn detail(&self, id: i64, ctx: &UserCtx) -> Result<String, UseCaseError>;
    /// 乐观锁更新（version 必传且 >0；明细非空护栏；409 版本冲突）
    async fn update(&self, req: PurchaseOrderReq, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError>;
    /// 删除（仅 pending；删除后从剩余采购历史重算商品进价）
    async fn delete(&self, id: i64, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError>;
    /// 设置结算状态（仅 pending；is_settled 仅 0/1；参数校验在状态校验之后）
    async fn settle(&self, id: i64, is_settled: Option<i64>, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError>;
    /// 审核（仅 pending；事务内写库存流水）
    async fn approve(&self, id: i64, reason: String, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError>;
    /// 反审核（reason 必填；super_admin 任意时刻可反审核，其他角色仅 confirmed）
    async fn unapprove(&self, id: i64, reason: String, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError>;
    /// 列表导出 xlsx 字节（供应商角色强制只看自己；Content-Disposition 旧格式由 handler 组装）
    async fn export(&self, ctx: &UserCtx, keyword_pattern: String, supplier_id: Option<i64>) -> Result<Vec<u8>, UseCaseError>;
    /// 打印模板导出：返回 (xlsx 字节, 文件名)；user_id 覆盖经手人
    async fn print_excel(&self, id: i64, user_id_override: Option<i64>) -> Result<(Vec<u8>, String), UseCaseError>;
    /// 批量导入（xlsx/csv 解析 + 按 order_no 分组 + INSERT OR IGNORE）
    async fn import(&self, bytes: Vec<u8>, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError>;
}
