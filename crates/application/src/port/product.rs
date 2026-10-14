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
}
