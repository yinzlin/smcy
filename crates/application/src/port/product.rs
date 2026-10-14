//! 商品域入站端口

use crate::error::UseCaseError;
use domain::port::product::ProductListFilter;

/// 商品用例端口（列表组装与脱敏、CRUD、状态切换、锁定守卫）
pub trait ProductUseCase: Send + Sync {
    /// 分页列表：N+1 组装 units/prices，selling_price 计算，非超管进价脱敏
    async fn list(&self, is_super_admin: bool, f: ProductListFilter) -> Result<serde_json::Value, UseCaseError>;
    /// 同名商品检查（吞错返回空数组；返回预序列化 JSON 文本保证逐字节兼容）
    async fn check_name(&self, name: Option<String>) -> String;
    /// 商品详情（吞错返回 {}；返回预序列化 JSON 文本保证逐字节兼容）
    async fn by_id(&self, is_super_admin: bool, id: Option<i64>) -> String;
    /// 开单商品搜索（吞错返回空数组；返回预序列化 JSON 文本保证逐字节兼容）
    async fn search(&self, is_super_admin: bool, keyword: Option<String>, only_audited: bool) -> String;
    /// 新建商品（默认单位/价格舍入/进价脱敏在用例层；失败 500 空响应体）
    async fn create(&self, is_super_admin: bool, req: domain::ProductReq) -> Result<(), UseCaseError>;
    /// 启用/停用切换
    async fn toggle_status(&self, id: i64) -> Result<String, UseCaseError>;
    /// 编辑商品（非超管进价保持原值；价格变更记日志、加成率变更触发重算）
    async fn update(&self, is_super_admin: bool, req: domain::ProductUpdateReq) -> Result<String, UseCaseError>;
    /// 删除商品（关联数据守卫 + 事务级联清理）
    async fn delete(&self, id: i64) -> Result<String, UseCaseError>;

    // —— 子步b：导出 / 导入 ——

    /// 导出商品 Excel（宽表 + 计算售价；失败 500 携带"导出失败: …"）
    async fn export(&self) -> Result<Vec<u8>, UseCaseError>;
    /// 导入商品（Excel/CSV 解析在 HTTP 层，行级写入/价格列/图片转存在用例层；
    /// 返回汇总文本含失败明细，恒 200）
    async fn import(&self, rows: Vec<Vec<String>>) -> String;

    // —— 子步b：商品单位 ——

    /// 新建商品单位（价格四舍五入两位；失败 500 携带"创建单位失败: …"）
    async fn unit_create(&self, req: domain::ProductUnitReq) -> Result<String, UseCaseError>;
    /// 更新商品单位（旧行为 WHERE id 取请求的 product_id 字段）
    async fn unit_update(&self, req: domain::ProductUnitReq) -> Result<String, UseCaseError>;
    /// 删除商品单位（0 行 → 404"单位不存在"）
    async fn unit_delete(&self, id: i64) -> Result<String, UseCaseError>;
    /// 按商品清空单位
    async fn unit_delete_by_product(&self, product_id: i64) -> Result<String, UseCaseError>;
    /// 商品单位列表（product_id 缺失返回 "[]"；非超管进价脱敏；返回预序列化 JSON 文本）
    async fn unit_list(&self, is_super_admin: bool, product_id: Option<i64>) -> String;

    // —— 子步b：价格快照 ——

    /// 价格快照 UPSERT，成功后联动政采状态（失败 500 空响应体）
    async fn price_upsert(&self, req: domain::ProductPriceReq) -> Result<(), UseCaseError>;
    /// 价格快照列表（预序列化 JSON 文本；吞错返回空数组）
    async fn price_list(&self, product_id: i64) -> String;
    /// 删除价格快照（0 行 → 404"价格记录不存在"）
    async fn price_delete(&self, id: i64) -> Result<String, UseCaseError>;
    /// 按商品清空价格快照（旧行为吞错恒 200）
    async fn price_delete_by_product(&self, product_id: i64);

    // —— 子步b：售价同步 / 自动更新 / 最近进价 / 价格日志 ——

    /// 按价格策略快照同步商品基础单价
    async fn sync_base_price(&self, product_id: i64);
    /// 开关自动更新售价并按加成率重算
    async fn set_auto_update_price(&self, product_id: i64, auto: i64);
    /// 批量开关自动更新售价（开启时全量重算）
    async fn batch_set_auto_update_price(&self, auto: i64);
    /// 最近进价与最近一次采购明细（None=商品不存在/查询失败 → 404）
    async fn last_purchase_price(&self, product_id: i64) -> Option<String>;
    /// 价格变更日志（预序列化 JSON 文本；吞错返回空数组）
    async fn price_log_list(&self, product_id: Option<i64>, limit: i64) -> String;
}
