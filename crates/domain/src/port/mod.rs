//! 出站端口（Repository Trait），按业务域拆分。
//!
//! 注入方式为泛型单态化（Rust 2024 AFIT 不可 dyn）；SQLite 为唯一实现，
//! 具体实现位于 database crate。方法 future 的 auto trait（如 Send）在
//! 单态化后按实际实现内容推导，故此处显式关闭 AFIT 公共 trait 警告。

#![allow(async_fn_in_trait)]

pub mod audit;
pub mod category;
pub mod inventory;
pub mod oplog;
pub mod price_schedule;
pub mod product;
pub mod purchase_order;
pub mod purchaser;
pub mod supplier;
pub mod today_price;
pub mod user;
pub mod warehouse;

pub use audit::*;
pub use category::*;
pub use inventory::*;
pub use oplog::*;
pub use price_schedule::*;
pub use product::*;
pub use purchase_order::*;
pub use purchaser::*;
pub use supplier::*;
pub use today_price::*;
pub use user::*;
pub use warehouse::*;
