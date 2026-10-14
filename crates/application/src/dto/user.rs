//! 用户域响应 DTO 与入站命令
//!
//! 带 skip 的 Option 字段用于复刻旧 json! 中「失败分支不输出 user 键、
//! 成功分支不输出 message 键」的形态。

use serde::Serialize;

// ---- 响应 ----

/// 通用操作结果（旧 json!({"success":...,"message":...})）
#[derive(Debug, Serialize)]
pub struct ApiMessage {
    pub success: bool,
    pub message: String,
}

/// 登录/会话中的用户信息
#[derive(Debug, Serialize)]
pub struct LoginUser {
    pub id: i64,
    pub username: String,
    pub nickname: String,
    pub role: String,
}

/// 登录成功响应体（Set-Cookie 由 handler 附加；失败分支 user 不输出）
#[derive(Debug, Serialize)]
pub struct LoginResp {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<LoginUser>,
}

/// api_user_get：失败只有 success+message，成功只有 success+user
#[derive(Debug, Serialize)]
pub struct UserDetailResp {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserDetail>,
}

#[derive(Debug, Serialize)]
pub struct UserDetail {
    pub id: i64,
    pub username: String,
    pub nickname: String,
    pub role: String,
    pub status: i32,
    pub supplier_id: i64,
    pub purchaser_id: i64,
}

/// 联系方式回填（GET/SET 共用）
#[derive(Debug, Serialize)]
pub struct ContactPhoneResp {
    pub success: bool,
    pub contact_phone: String,
}

/// api_login_check：失败只有 logged_in，成功带 user
#[derive(Debug, Serialize)]
pub struct LoginCheckResp {
    pub logged_in: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<LoginUser>,
}

/// 登录结果：响应体 + 会话 token（handler 拼 Set-Cookie 头）
pub struct LoginOutcome {
    pub body: LoginResp,
    pub session_token: String,
}

// ---- 入站命令 ----

#[derive(Debug, Clone)]
pub struct CreateUserCmd {
    pub username: String,
    pub password: String,
    pub nickname: String,
    pub role: String,
    pub supplier_id: i64,
    pub purchaser_id: i64,
}

#[derive(Debug, Clone)]
pub struct UpdateUserCmd {
    pub id: i64,
    pub username: String,
    pub password: String,
    pub nickname: String,
    pub role: String,
    pub supplier_id: i64,
    pub purchaser_id: i64,
}
