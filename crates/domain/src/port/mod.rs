//! 出站端口（Repository Trait），按业务域拆分。
//!
//! 注入方式为泛型单态化（Rust 2024 AFIT 不可 dyn）；SQLite 为唯一实现，
//! 具体实现位于 database crate。方法 future 的 auto trait（如 Send）在
//! 单态化后按实际实现内容推导，故此处显式关闭 AFIT 公共 trait 警告。

#![allow(async_fn_in_trait)]

pub mod audit;
pub mod category;
pub mod food_accept;
pub mod inventory;
pub mod oplog;
pub mod order_allocation;
pub mod order_supplement;
pub mod price_schedule;
pub mod product;
pub mod purchase_document;
pub mod purchase_order;
pub mod purchaser;
pub mod query_finance;
pub mod query_purchase;
pub mod query_sales;
pub mod query_stats;
pub mod query_stock;
pub mod sales_order;
pub mod supplier;
pub mod system_mgmt;
pub mod today_price;
pub mod user;
pub mod warehouse;

pub use audit::*;
pub use category::*;
pub use food_accept::*;
pub use inventory::*;
pub use oplog::*;
pub use order_allocation::*;
pub use order_supplement::*;
pub use price_schedule::*;
pub use product::*;
pub use purchase_document::*;
pub use purchase_order::*;
pub use purchaser::*;
pub use query_finance::*;
pub use query_purchase::*;
pub use query_sales::*;
pub use query_stats::*;
pub use query_stock::*;
pub use sales_order::*;
pub use supplier::*;
pub use system_mgmt::*;
pub use today_price::*;
pub use user::*;
pub use warehouse::*;
