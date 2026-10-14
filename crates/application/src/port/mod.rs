//! 入站端口（UseCase Trait），按业务域拆分。
//!
//! 与出站端口同样采用 AFIT + 泛型单态化；backend handler 面向 App 中
//! 的具体用例类型调用（单态化后 future auto trait 按实际内容推导），
//! 故此处显式关闭 AFIT 公共 trait 警告。

#![allow(async_fn_in_trait)]

pub mod user;

pub use user::*;
