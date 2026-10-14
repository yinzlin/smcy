//! 用户域出站端口：user_account 表的读写契约
//!
//! 行结构即读模型，serde 字段名与旧 handler json! 输出一致（序列化
//! 字段顺序 = 声明顺序，保证响应逐字节兼容）。

use serde::Serialize;

use crate::error::DomainError;

/// 活跃用户列表行（api_user_list）
#[derive(Debug, Clone, Serialize)]
pub struct ActiveUserRow {
    pub id: i64,
    pub username: String,
    pub nickname: String,
    pub phone: String,
    pub role: String,
    pub supplier_id: i64,
    pub purchaser_id: i64,
}

/// 用户详情行（api_user_get；status 与列表行不同，为 i32）
#[derive(Debug, Clone, Serialize)]
pub struct UserDetailRow {
    pub id: i64,
    pub username: String,
    pub nickname: String,
    pub role: String,
    pub status: i32,
    pub supplier_id: i64,
    pub purchaser_id: i64,
}

/// 登录校验行（api_login）
#[derive(Debug, Clone)]
pub struct LoginRow {
    pub id: i64,
    pub password: String,
    pub status: i32,
    pub nickname: String,
    pub role: String,
}

/// 会话校验行（api_login_check）
#[derive(Debug, Clone)]
pub struct SessionUserRow {
    pub username: String,
    pub nickname: String,
    pub role: String,
}

/// 新建用户命令（密码为已哈希值，明文密码不进入领域层）
#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub password_hash: String,
    pub nickname: String,
    pub role: String,
    pub supplier_id: i64,
    pub purchaser_id: i64,
}

/// 更新用户资料命令（role 为空表示不更新角色字段）
#[derive(Debug, Clone)]
pub struct UpdateUser {
    pub id: i64,
    pub username: String,
    pub nickname: String,
    pub role: String,
    pub supplier_id: i64,
    pub purchaser_id: i64,
}

/// 用户域仓储端口（SQLite 单实现，泛型单态化注入）
pub trait UserRepo: Send + Sync {
    async fn list_active(&self) -> Result<Vec<ActiveUserRow>, DomainError>;
    async fn get_detail(&self, id: i64) -> Result<Option<UserDetailRow>, DomainError>;
    async fn username_exists(&self, username: &str) -> Result<bool, DomainError>;
    async fn create(&self, user: &NewUser) -> Result<(), DomainError>;
    async fn update_password(&self, id: i64, password_hash: &str) -> Result<(), DomainError>;
    async fn update_profile(&self, user: &UpdateUser) -> Result<(), DomainError>;
    async fn delete(&self, id: i64) -> Result<(), DomainError>;
    async fn set_status(&self, id: i64, status: i64) -> Result<(), DomainError>;
    async fn get_contact_phone(&self, id: i64) -> Result<String, DomainError>;
    async fn set_contact_phone(&self, id: i64, phone: &str) -> Result<(), DomainError>;
    async fn find_login(&self, username: &str) -> Result<Option<LoginRow>, DomainError>;
    async fn touch_last_login(&self, id: i64) -> Result<(), DomainError>;
    async fn find_active_by_id(&self, id: i64) -> Result<Option<SessionUserRow>, DomainError>;
}
