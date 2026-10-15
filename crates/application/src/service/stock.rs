//! 出入库汇总（真实账套 / 报销口径）：SQL 已下沉 QueryStockRepo，本层仅做按日按仓聚合与行组装。

use domain::port::query_stock::QueryStockRepo;

pub struct StockSummaryRow {
    pub day: String,
    pub warehouse_id: i64,
    pub warehouse_name: String,
    pub is_summary: bool,
    pub in_amount: f64,
    pub in_item_count: i64,
    pub in_order_count: i64,
    pub out_amount: f64,
    pub out_item_count: i64,
    pub out_order_count: i64,
    pub discounted_out_amount: f64, // 下浮后出库金额 = 出库金额 × (1 - 下浮率/100)
    pub gross_profit: f64,          // 毛利 = 下浮后出库金额 - 入库金额
}

pub async fn compute_stock_summary<R: QueryStockRepo>(repo: &R, start_date: &str, end_date: &str) -> (Vec<StockSummaryRow>, f64, f64, f64) {
    // 采购入库按日+仓库汇总
    let purchase_rows = repo.stock_summary_purchase_rows(start_date, end_date).await;

    // 销售出库按明细行返回，含订单下浮率，在 Rust 端按明细计算下浮后金额
    let sales_rows = repo.stock_summary_sales_rows(start_date, end_date).await;

    use std::collections::{BTreeMap, HashMap};

    // Key: (day, warehouse_id) -> (in_amount, in_items, in_orders, out_amount, out_items, out_orders, warehouse_name, discounted_out)
    let mut day_wh_map: HashMap<String, BTreeMap<i64, (f64, i64, i64, f64, i64, i64, String, f64)>> = HashMap::new();

    // 先收集所有仓库名称用于排序
    let mut warehouse_names: BTreeMap<i64, String> = BTreeMap::new();

    for row in &purchase_rows {
        let day: String = row.day.clone();
        let wh_id: i64 = row.warehouse_id;
        let wh_name: String = row.warehouse_name.clone();
        let amount: f64 = row.amount;
        let items: i64 = row.item_count;
        let orders: i64 = row.order_count;

        warehouse_names.insert(wh_id, wh_name.clone());

        let wh_map = day_wh_map.entry(day).or_insert_with(BTreeMap::new);
        let e = wh_map.entry(wh_id).or_insert((0.0, 0, 0, 0.0, 0, 0, wh_name, 0.0));
        e.0 += amount;
        e.1 += items;
        e.2 += orders;
    }

    // 按明细行汇总：out_amount 累加原值，discounted_out 累加下浮后金额
    // 下浮后金额 = amount × (1 - discount_rate/100)
    // 订单数与条数通过 Set 去重统计
    let mut seen_orders: std::collections::HashSet<(String, i64, i64)> = std::collections::HashSet::new();
    for row in &sales_rows {
        let day: String = row.day.clone();
        let wh_id: i64 = row.warehouse_id;
        let wh_name: String = row.warehouse_name.clone();
        let amount: f64 = row.amount;
        let order_id: i64 = row.order_id;
        let discount_rate: f64 = row.discount_rate;
        let discounted = amount * (1.0 - discount_rate / 100.0);

        warehouse_names.insert(wh_id, wh_name.clone());

        let wh_map = day_wh_map.entry(day.clone()).or_insert_with(BTreeMap::new);
        let e = wh_map.entry(wh_id).or_insert((0.0, 0, 0, 0.0, 0, 0, wh_name, 0.0));
        e.3 += amount;
        e.4 += 1; // 条数
        e.7 += discounted;
        if seen_orders.insert((day, wh_id, order_id)) {
            e.5 += 1; // 单数（去重）
        }
    }

    // 按日期倒序，每天输出仓库明细行 + 汇总行
    let mut days: Vec<String> = day_wh_map.keys().cloned().collect();
    days.sort_by(|a, b| b.cmp(a)); // 倒序

    let mut rows: Vec<StockSummaryRow> = Vec::new();
    let mut total_in = 0.0;
    let mut total_out = 0.0;
    let mut total_discounted_out = 0.0;

    for day in &days {
        if let Some(wh_map) = day_wh_map.get(day) {
            let mut day_in = 0.0;
            let mut day_out = 0.0;
            let mut day_discounted_out = 0.0;
            let mut day_in_items = 0;
            let mut day_out_items = 0;
            let mut day_in_orders = 0;
            let mut day_out_orders = 0;

            // 输出每个仓库行
            for (wh_id, v) in wh_map {
                day_in += v.0;
                day_out += v.3;
                day_discounted_out += v.7;
                day_in_items += v.1;
                day_out_items += v.4;
                day_in_orders += v.2;
                day_out_orders += v.5;

                rows.push(StockSummaryRow {
                    day: day.clone(),
                    warehouse_id: *wh_id,
                    warehouse_name: v.6.clone(),
                    is_summary: false,
                    in_amount: v.0,
                    in_item_count: v.1,
                    in_order_count: v.2,
                    out_amount: v.3,
                    out_item_count: v.4,
                    out_order_count: v.5,
                    discounted_out_amount: v.7,
                    gross_profit: v.7 - v.0,
                });
            }

            // 输出当天汇总行
            rows.push(StockSummaryRow {
                day: day.clone(),
                warehouse_id: -1,
                warehouse_name: "当日汇总".to_string(),
                is_summary: true,
                in_amount: day_in,
                in_item_count: day_in_items,
                in_order_count: day_in_orders,
                out_amount: day_out,
                out_item_count: day_out_items,
                out_order_count: day_out_orders,
                discounted_out_amount: day_discounted_out,
                gross_profit: day_discounted_out - day_in,
            });

            total_in += day_in;
            total_out += day_out;
            total_discounted_out += day_discounted_out;
        }
    }

    (rows, total_in, total_out, total_discounted_out)
}

pub async fn compute_stock_summary_reimburse<R: QueryStockRepo>(repo: &R, start_date: &str, end_date: &str) -> (Vec<StockSummaryRow>, f64, f64, f64) {
    // 入库：与真实账套一致
    let purchase_rows = repo.stock_summary_purchase_rows(start_date, end_date).await;

    // 真实出库（按明细行返回，含订单下浮率）
    let sales_rows = repo.stock_summary_sales_rows(start_date, end_date).await;

    // 分摊调整1：目标单收到的增项金额（+出库），按明细行返回，含目标单下浮率
    let adj1_rows = repo.stock_summary_adj_target_rows(start_date, end_date).await;

    // 分摊调整2：来源耗材单真实金额（−出库），按明细行返回，含来源单下浮率
    let adj2_rows = repo.stock_summary_adj_source_rows(start_date, end_date).await;

    use std::collections::{BTreeMap, HashMap};
    // 元组: (in_amount, in_items, in_orders, out_amount, out_items, out_orders, warehouse_name, discounted_out)
    let mut day_wh_map: HashMap<String, BTreeMap<i64, (f64, i64, i64, f64, i64, i64, String, f64)>> = HashMap::new();

    for row in &purchase_rows {
        let day: String = row.day.clone();
        let wh_id: i64 = row.warehouse_id;
        let wh_name: String = row.warehouse_name.clone();
        let amount: f64 = row.amount;
        let items: i64 = row.item_count;
        let orders: i64 = row.order_count;
        let wh_map = day_wh_map.entry(day).or_insert_with(BTreeMap::new);
        let e = wh_map.entry(wh_id).or_insert((0.0, 0, 0, 0.0, 0, 0, wh_name, 0.0));
        e.0 += amount; e.1 += items; e.2 += orders;
    }

    // 真实出库：累加原值与下浮后金额，订单数去重
    let mut seen_orders: std::collections::HashSet<(String, i64, i64)> = std::collections::HashSet::new();
    for row in &sales_rows {
        let day: String = row.day.clone();
        let wh_id: i64 = row.warehouse_id;
        let wh_name: String = row.warehouse_name.clone();
        let amount: f64 = row.amount;
        let order_id: i64 = row.order_id;
        let discount_rate: f64 = row.discount_rate;
        let discounted = amount * (1.0 - discount_rate / 100.0);
        let wh_map = day_wh_map.entry(day.clone()).or_insert_with(BTreeMap::new);
        let e = wh_map.entry(wh_id).or_insert((0.0, 0, 0, 0.0, 0, 0, wh_name, 0.0));
        e.3 += amount; e.4 += 1; e.7 += discounted;
        if seen_orders.insert((day, wh_id, order_id)) { e.5 += 1; }
    }

    // 加目标单分摊增项（含下浮）
    for row in &adj1_rows {
        let day: String = row.day.clone();
        let wh_id: i64 = row.warehouse_id;
        let wh_name: String = row.warehouse_name.clone();
        let amount: f64 = row.amount;
        let discount_rate: f64 = row.discount_rate;
        let discounted = amount * (1.0 - discount_rate / 100.0);
        let wh_map = day_wh_map.entry(day).or_insert_with(BTreeMap::new);
        let e = wh_map.entry(wh_id).or_insert((0.0, 0, 0, 0.0, 0, 0, wh_name, 0.0));
        e.3 += amount; e.7 += discounted;
    }

    // 减来源单真实金额（含下浮）
    for row in &adj2_rows {
        let day: String = row.day.clone();
        let wh_id: i64 = row.warehouse_id;
        let wh_name: String = row.warehouse_name.clone();
        let amount: f64 = row.amount;
        let discount_rate: f64 = row.discount_rate;
        let discounted = amount * (1.0 - discount_rate / 100.0);
        let wh_map = day_wh_map.entry(day).or_insert_with(BTreeMap::new);
        let e = wh_map.entry(wh_id).or_insert((0.0, 0, 0, 0.0, 0, 0, wh_name, 0.0));
        e.3 -= amount; e.7 -= discounted;
    }

    let mut days: Vec<String> = day_wh_map.keys().cloned().collect();
    days.sort_by(|a, b| b.cmp(a));

    let mut rows: Vec<StockSummaryRow> = Vec::new();
    let mut total_in = 0.0;
    let mut total_out = 0.0;
    let mut total_discounted_out = 0.0;

    for day in &days {
        if let Some(wh_map) = day_wh_map.get(day) {
            let mut day_in = 0.0;
            let mut day_out = 0.0;
            let mut day_discounted_out = 0.0;
            let mut day_in_items = 0;
            let mut day_out_items = 0;
            let mut day_in_orders = 0;
            let mut day_out_orders = 0;

            for (wh_id, v) in wh_map {
                day_in += v.0; day_out += v.3; day_discounted_out += v.7;
                day_in_items += v.1; day_out_items += v.4;
                day_in_orders += v.2; day_out_orders += v.5;

                rows.push(StockSummaryRow {
                    day: day.clone(),
                    warehouse_id: *wh_id,
                    warehouse_name: v.6.clone(),
                    is_summary: false,
                    in_amount: v.0,
                    in_item_count: v.1,
                    in_order_count: v.2,
                    out_amount: v.3,
                    out_item_count: v.4,
                    out_order_count: v.5,
                    discounted_out_amount: v.7,
                    gross_profit: v.7 - v.0,
                });
            }

            rows.push(StockSummaryRow {
                day: day.clone(), warehouse_id: -1, warehouse_name: "当日汇总".to_string(),
                is_summary: true,
                in_amount: day_in, in_item_count: day_in_items, in_order_count: day_in_orders,
                out_amount: day_out, out_item_count: day_out_items, out_order_count: day_out_orders,
                discounted_out_amount: day_discounted_out,
                gross_profit: day_discounted_out - day_in,
            });
            total_in += day_in; total_out += day_out; total_discounted_out += day_discounted_out;
        }
    }
    (rows, total_in, total_out, total_discounted_out)
}
