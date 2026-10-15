pub mod error;
pub mod port;

/// 当前登录用户的上下文（角色 + 用户ID + 行级数据权限关联）
#[derive(Debug, Clone)]
pub struct UserCtx {
    pub role: String,
    pub user_id: i64,
    pub supplier_id: i64,
    pub purchaser_id: i64,
}
