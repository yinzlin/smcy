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
    /// 导出销售单列表 xlsx（purchaser 角色强制只看自己；行级权限在用例层）
    async fn export(&self, ctx: &UserCtx, keyword_pattern: String, purchaser_id: Option<i64>) -> Result<Vec<u8>, UseCaseError>;
    /// 批量导入销售单（xlsx/csv 解析 + INSERT OR IGNORE 幂等 + base_quantity 服务端重算）
    async fn import(&self, bytes: Vec<u8>, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 验收单聚合数据（真实明细 + 分摊增项合并 + 分类排序 + 重算验收金额；JSON 预序列化）
    async fn accept(&self, id: i64, ctx: &UserCtx) -> Result<String, UseCaseError>;
    /// 由销售单生成采购订单（warning/force 两遍法 + 快照驱动 CRUD；成功时含待补记日志）
    async fn generate_purchase(&self, id: i64, force: bool, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 上传订单图片（客户订单/签字验收单；行级权限 + 白名单校验 + 写盘 + 更新订单列）
    /// file 为 None 表示 multipart 中无 file 字段（旧行为 400 "请选择要上传的图片"）
    async fn upload_image(&self, order_id: i64, image_type: &str, file: Option<(String, Vec<u8>)>, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
    /// 删除订单图片（移除磁盘文件并清空订单列）
    async fn delete_image(&self, order_id: i64, image_type: &str, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;

    // ===== 分拣清单（GET 公开接口 + xlsx 导出；SQL 错误旧行为吞为空结果）=====
    /// 分拣清单·按商品聚合（date 空=当前 pending/sorting 待分拣；JSON 预序列化）
    async fn sort_items(&self, date: String) -> Result<String, UseCaseError>;
    /// 分拣清单导出 xlsx（7 列含合计；返回文件名与字节）
    async fn sort_items_excel(&self, date: String) -> Result<(String, Vec<u8>), UseCaseError>;
    /// 分拣清单·按分类聚合
    async fn sort_items_by_category(&self, date: String) -> Result<String, UseCaseError>;
    /// 分拣清单·按分类导出 xlsx（分类彩色分组；空数据时返回空缓冲由 HTTP 层出 JSON 提示）
    async fn sort_items_by_category_excel(&self, date: String) -> Result<(String, Vec<u8>), UseCaseError>;
    /// 分拣清单·按供应商聚合
    async fn sort_items_by_supplier(&self, date: String) -> Result<String, UseCaseError>;
    /// 分拣清单·按供应商导出 xlsx（每供应商一 sheet；print_values 决定实量/单价/金额列）
    async fn sort_items_by_supplier_excel(&self, date: String, print_values: bool) -> Result<(String, Vec<u8>), UseCaseError>;
    /// 分拣清单·按采购单位聚合
    async fn sort_items_by_purchaser(&self, date: String) -> Result<String, UseCaseError>;
    /// 分拣清单·按采购单位导出 xlsx（含历史最高/最低/最近与售价四列）
    async fn sort_items_by_purchaser_excel(&self, date: String) -> Result<(String, Vec<u8>), UseCaseError>;
    /// 综合分拣聚合
    async fn sort_comprehensive(&self, date: String) -> Result<String, UseCaseError>;
    /// 综合分拣导出 xlsx
    async fn sort_comprehensive_excel(&self, date: String) -> Result<(String, Vec<u8>), UseCaseError>;
    /// 分拣修正（数量/售价/进价；数量与单价服务端 round 两位；purchase_price 回写商品并联动自动售价）
    async fn correction(&self, corrections: Vec<serde_json::Value>, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError>;
}
