//! 业务服务集合（由原 src/main.rs 业务函数切片迁移而来）
pub mod basic_audit;
pub mod category;
pub mod excel;
pub mod legacy_user;
pub mod order_no;
pub mod price;
pub mod product;
pub mod purchase;
pub mod purchaser;
pub mod stock;
pub mod supplier;
pub mod today_price;
pub mod user;
pub mod warehouse;

pub use basic_audit::*;
pub use category::*;
pub use excel::*;
pub use legacy_user::*;
pub use order_no::*;
pub use price::*;
pub use product::*;
pub use purchase::*;
pub use purchaser::*;
pub use stock::*;
pub use supplier::*;
pub use today_price::*;
pub use user::*;
pub use warehouse::*;


pub fn get_category_sort_key(category_name: &str, parent_name: &str) -> i64 {
    let name = category_name.trim();
    let parent = parent_name.trim();
    if parent == "荤鲜类" || name == "荤鲜类" {
        if name == "家禽" { return 101; }
        if name == "家畜" { return 102; }
        if name == "水产" { return 103; }
        return 100;
    }
    if name == "鲜蔬类" { return 200; }
    if name == "粮油干调" { return 300; }
    if name == "豆制品" { return 400; }
    if name == "粉面制品" { return 500; }
    if name == "水果类" { return 600; }
    if name == "其它" { return 700; }
    if name == "耗材类" { return 800; }
    999
}















// 导出报销单（报销口径）：合并分摊增项后的明细

// 导出验收单（真实口径）：真实账套明细，不合并分摊增项
