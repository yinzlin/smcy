//! database：数据访问层（当前仅 SQLite 实现，postgres/redis 为同构占位）

pub mod sqlite;

pub use sqlite::*;
