//! 入站端口（用例 trait）：backend 适配器面向此契约调用。
//!
//! 注入方式为泛型单态化（Rust 2024 AFIT 不可 dyn）；具体实现位于
//! application::service。方法 future 的 auto trait（如 Send）在单态化后
//! 按实际实现内容推导，故此处显式关闭 AFIT 公共 trait 警告。

#![allow(async_fn_in_trait)]

pub mod audit;
pub mod category;
pub mod product;
pub mod purchaser;
pub mod supplier;
pub mod user;
pub mod warehouse;

pub use audit::*;
pub use category::*;
pub use product::*;
pub use purchaser::*;
pub use supplier::*;
pub use user::*;
pub use warehouse::*;
