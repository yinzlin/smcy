//! 业务服务集合（由原 src/main.rs 业务函数切片迁移而来）
pub mod basic_audit;
pub mod category;
pub mod excel;
pub mod food_accept;
pub mod inventory;
pub mod order_allocation;
pub mod order_no;
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
pub mod stock;
pub mod supplier;
pub mod system_mgmt;
pub mod today_price;
pub mod user;
pub mod warehouse;

pub use basic_audit::*;
pub use category::*;
pub use excel::*;
pub use food_accept::*;
pub use inventory::*;
pub use order_allocation::*;
pub use order_no::*;
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
pub use stock::*;
pub use supplier::*;
pub use system_mgmt::*;
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
