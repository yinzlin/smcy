//! 单号生成：SOyyyyMMddNNN / POyyyyMMddNNN（按当日最大序号递增）

use database::pool;

pub async fn generate_order_no(order_type: &str, order_date: &str) -> String {
    let prefix = if order_type == "sales" { "SO" } else { "PO" };

    let date_str: Vec<&str> = order_date.split('-').collect();
    let date_part = format!("{}{}{}", date_str[0], date_str[1], date_str[2]);

    let max_seq: i64 = if order_type == "sales" {
        sqlx::query_scalar(
            "SELECT COALESCE(MAX(CAST(SUBSTR(order_no, 11, 3) AS INTEGER)), 0) FROM sales_order WHERE order_date = ?",
        )
        .bind(order_date)
        .fetch_one(pool())
        .await
        .unwrap_or(0)
    } else {
        sqlx::query_scalar(
            "SELECT COALESCE(MAX(CAST(SUBSTR(order_no, 11, 3) AS INTEGER)), 0) FROM purchase_order WHERE order_date = ?",
        )
        .bind(order_date)
        .fetch_one(pool())
        .await
        .unwrap_or(0)
    };

    format!("{}{}{:03}", prefix, date_part, max_seq + 1)
}
