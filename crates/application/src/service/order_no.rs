//! 单号生成：SOyyyyMMddNNN / POyyyyMMddNNN（按当日最大序号递增）
//!
//! 单表序号查询已下沉领域端口（SalesOrderRepo / PurchaseOrderRepo::max_seq_for_date），
//! 本层仅负责板式拼装与查询失败吞 0 的行为保持。

use domain::port::purchase_order::PurchaseOrderRepo;
use domain::port::sales_order::SalesOrderRepo;

/// 按 (单据类型, 日期, 当日最大序号) 板式拼装单号（纯函数，无 IO）
pub fn format_order_no(order_type: &str, order_date: &str, max_seq: i64) -> String {
    let prefix = if order_type == "sales" { "SO" } else { "PO" };

    let date_str: Vec<&str> = order_date.split('-').collect();
    let date_part = format!("{}{}{}", date_str[0], date_str[1], date_str[2]);

    format!("{}{}{:03}", prefix, date_part, max_seq + 1)
}

/// 单号生成：按日从对应单据表取当日最大序号（查询失败吞为 0，与旧行为一致）
pub async fn generate_order_no<S: SalesOrderRepo, P: PurchaseOrderRepo>(
    order_type: &str,
    order_date: &str,
    sales: &S,
    purchase: &P,
) -> String {
    let max_seq: i64 = if order_type == "sales" {
        sales.max_seq_for_date(order_date).await.unwrap_or(0)
    } else {
        purchase.max_seq_for_date(order_date).await.unwrap_or(0)
    };

    format_order_no(order_type, order_date, max_seq)
}
