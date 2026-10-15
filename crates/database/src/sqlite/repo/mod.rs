//! SQLite 仓储实现：实现 domain::port 出站端口
//!
//! SQL 与旧 handler 逐字一致；多步事务序列建模为聚合级仓储方法，
//! 实现内部 begin/commit（Unit-of-Work 属于基础设施层）。

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
pub mod stock_writer;
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
