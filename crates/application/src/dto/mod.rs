//! 应用层响应 DTO 与入站命令：JSON 字段名与顺序 = 旧 handler json! 输出
//!
//! `dto/*` 存放 HTTP 契约（请求体 `*Req` 与响应体）与到 domain 端口输入类型的映射；
//! domain 不再持有 HTTP 契约。

pub mod auth;
pub mod basic;
pub mod food_accept;
pub mod order;
pub mod price_schedule;
pub mod product;
pub mod user;

pub use auth::*;
pub use basic::*;
pub use food_accept::*;
pub use order::*;
pub use price_schedule::*;
pub use product::*;
pub use user::*;
