//! SQLite 仓储实现：实现 domain::port 出站端口
//!
//! SQL 与旧 handler 逐字一致；多步事务序列建模为聚合级仓储方法，
//! 实现内部 begin/commit（Unit-of-Work 属于基础设施层）。

pub mod audit;
pub mod category;
pub mod oplog;
pub mod price_schedule;
pub mod product;
pub mod purchaser;
pub mod supplier;
pub mod today_price;
pub mod user;
pub mod warehouse;

pub use audit::*;
pub use category::*;
pub use oplog::*;
pub use price_schedule::*;
pub use product::*;
pub use purchaser::*;
pub use supplier::*;
pub use today_price::*;
pub use user::*;
pub use warehouse::*;
