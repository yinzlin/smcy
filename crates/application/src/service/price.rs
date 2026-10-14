use sqlx::{AssertSqlSafe, Row};

use common::utils::*;
use database::*;
use domain::*;

/// 维护 product_price_schedule 时段连续性：
/// 在 (product_id, price_type) 分组下，重新计算 end_date 字段：
///   - 按 effective_date 升序排列
///   - 每条记录的 end_date = 下一条 effective_date（不含）；最后一条 end_date = NULL（至今有效）
/// 新增/修改/删除时调用，确保查询时无需再做范围扫描判断。
pub async fn rebuild_price_schedule_end_dates(
    product_id: i64,
    price_type: &str,
) -> Result<(), String> {
    // 拉取该 (product, price_type) 下所有时段，含 supplier_id / purchaser_id 用于分组
    let rows = sqlx::query(
        "SELECT id, effective_date, supplier_id, purchaser_id FROM product_price_schedule
         WHERE product_id = ? AND price_type = ?
         ORDER BY effective_date ASC, id ASC"
    )
    .bind(product_id)
    .bind(price_type)
    .fetch_all(database::pool())
    .await
    .map_err(|e| format!("查询时段失败: {}", e))?;

    // 按 (supplier_id, purchaser_id) 分组，每组独立维护时段连续性
    let mut groups: std::collections::HashMap<(Option<i64>, Option<i64>), Vec<(i64, String)>> = std::collections::HashMap::new();
    for r in rows {
        let sid: Option<i64> = r.get("supplier_id");
        let pid: Option<i64> = r.get("purchaser_id");
        let id: i64 = r.get("id");
        let date: String = r.get("effective_date");
        groups.entry((sid, pid)).or_default().push((id, date));
    }

    // 逐条重写 end_date
    for (_key, dates) in &groups {
        for (i, (id, _)) in dates.iter().enumerate() {
            let next_date: Option<String> = if i + 1 < dates.len() {
                Some(dates[i + 1].1.clone())
            } else {
                None
            };
            let _ = sqlx::query("UPDATE product_price_schedule SET end_date = ? WHERE id = ?")
                .bind(&next_date)
                .bind(id)
                .execute(database::pool())
                .await;
        }
    }
    Ok(())
}

/// 根据 (product_id, price_type, query_date) 查找当日生效的价格。
/// - 若查询日期 < 最早 effective_date：返回 None（该日还没有任何价）
/// - 若查询日期 >= 最早 effective_date 且 < 下一条 effective_date：返回前一条 price
/// - 若查询日期 >= 最后一条 effective_date：返回最后一条 price（end_date IS NULL）
///
/// 返回 (price, matched_schedule_id, matched_effective_date)：
///   - price = 0.0 表示该商品尚未录入任何价（前端应回退到 base_price）
///   - matched_schedule_id 用于审计追溯
pub async fn lookup_effective_price(
    product_id: i64,
    price_type: &str,
    query_date: &str,
) -> Option<(f64, i64, String)> {
    // query_date 形如 'YYYY-MM-DD'，按字符串比较（YYYY-MM-DD 字典序 == 时间顺序）即可
    let row = sqlx::query(
        "SELECT id, price, effective_date FROM product_price_schedule
         WHERE product_id = ? AND price_type = ?
           AND effective_date <= ?
           AND (end_date IS NULL OR end_date > ?)
         ORDER BY effective_date DESC, id DESC
         LIMIT 1"
    )
    .bind(product_id)
    .bind(price_type)
    .bind(query_date)
    .bind(query_date)
    .fetch_optional(database::pool())
    .await
    .ok()
    .flatten()?;

    Some((
        row.get::<f64, _>("price"),
        row.get::<i64, _>("id"),
        row.get::<String, _>("effective_date"),
    ))
}

/// 批量查询指定 (order_date) 多个商品在某 price_type 下的生效价。
/// 返回 HashMap<product_id, price>（含 price > 0 的记录；缺失的 key 表示该商品无价）。
pub async fn batch_lookup_effective_prices(
    product_ids: &[i64],
    price_type: &str,
    query_date: &str,
) -> std::collections::HashMap<i64, f64> {
    if product_ids.is_empty() {
        return std::collections::HashMap::new();
    }
    // IN (?) 动态占位
    let placeholders = vec!["?"; product_ids.len()].join(",");
    // 取每个 product_id 在 query_date 当日生效的那一条（effective_date 最大且 <= query_date）。
    // 用窗口函数 ROW_NUMBER 而非 GROUP BY + HAVING MAX()，避免依赖 SQLite bare column 行为。
    let sql = format!(
        "SELECT product_id, price FROM (
             SELECT product_id, price, effective_date,
                    ROW_NUMBER() OVER (PARTITION BY product_id ORDER BY effective_date DESC, id DESC) AS rn
             FROM product_price_schedule
             WHERE price_type = ?
               AND product_id IN ({})
               AND effective_date <= ?
               AND (end_date IS NULL OR end_date > ?)
         ) WHERE rn = 1",
        placeholders
    );
    // 绑定顺序必须与 SQL 中占位符出现顺序一致：
    // price_type -> product_ids... -> query_date(effective_date <=) -> query_date(end_date >)
    let mut q = sqlx::query(AssertSqlSafe(sql.as_str())).bind(price_type);
    for pid in product_ids {
        q = q.bind(pid);
    }
    q = q.bind(query_date).bind(query_date);
    let rows = q.fetch_all(database::pool()).await.unwrap_or_default();
    let mut map = std::collections::HashMap::new();
    for r in rows {
        let pid = r.get::<i64, _>("product_id");
        let price = r.get::<f64, _>("price");
        if price > 0.0 {
            // 价格统一四舍五入保留两位小数
            map.insert(pid, (price * 100.0).round() / 100.0);
        }
    }
    map
}

/// 查询供应商约定采购价或采购单位约定售价。
/// price_type = "supplier_contract" 时 party_id 为 supplier_id；
/// price_type = "purchaser_contract" 时 party_id 为 purchaser_id。
pub async fn lookup_contract_price(
    product_id: i64,
    price_type: &str,
    party_id: i64,
    query_date: &str,
) -> Option<(f64, i64, String)> {
    let party_col = if price_type == "supplier_contract" { "supplier_id" } else { "purchaser_id" };
    let sql = format!(
        "SELECT id, price, effective_date FROM product_price_schedule
         WHERE product_id = ? AND price_type = ? AND {col} = ?
           AND effective_date <= ?
           AND (end_date IS NULL OR end_date > ?)
         ORDER BY effective_date DESC, id DESC
         LIMIT 1",
        col = party_col
    );
    let row = sqlx::query(AssertSqlSafe(sql.as_str()))
        .bind(product_id)
        .bind(price_type)
        .bind(party_id)
        .bind(query_date)
        .bind(query_date)
        .fetch_optional(database::pool())
        .await
        .ok()
        .flatten()?;
    Some((
        row.get::<f64, _>("price"),
        row.get::<i64, _>("id"),
        row.get::<String, _>("effective_date"),
    ))
}

// 记录价格变更日志（price_type: purchase_price / base_price）
pub async fn log_price_change(
    product_id: i64,
    price_type: &str,
    old_price: f64,
    new_price: f64,
    source: &str,
    ref_id: Option<i64>,
    remark: Option<&str>,
) {
    // 仅当价格实际变化时记录
    if (old_price - new_price).abs() < 0.001 {
        return;
    }
    let _ = sqlx::query(
        "INSERT INTO product_price_log(product_id, price_type, old_price, new_price, source, ref_id, remark) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(product_id)
    .bind(price_type)
    .bind(old_price)
    .bind(new_price)
    .bind(source)
    .bind(ref_id)
    .bind(remark)
    .execute(pool())
    .await;
}

// 根据加成率自动重算 base_price；返回是否实际更新了售价及旧/新值
// 当商品开启 auto_update_price 且 purchase_price > 0 时生效
pub async fn recalc_base_price_by_markup(
    product_id: i64,
    source: &str,
    ref_id: Option<i64>,
) {
    let row = sqlx::query(
        "SELECT purchase_price, base_price, markup_rate, auto_update_price FROM product WHERE id = ?"
    )
    .bind(product_id)
    .fetch_optional(pool())
    .await
    .unwrap_or(None);

    if let Some(r) = row {
        let purchase_price: f64 = r.get("purchase_price");
        let old_base_price: f64 = r.get("base_price");
        let markup_rate: f64 = r.get("markup_rate");
        let auto_update: i64 = r.get("auto_update_price");

        if auto_update == 0 || purchase_price <= 0.0 {
            eprintln!(
                "[售价自动更新] 商品ID={} 跳过：auto_update_price={}, purchase_price={:.2}",
                product_id, auto_update, purchase_price
            );
            return;
        }

        let raw_price = purchase_price * (1.0 + markup_rate);
        // 售价保留两位小数（四舍五入），不做末位数字限制
        let new_base_price = round2(raw_price);
        eprintln!(
            "[售价自动更新] 商品ID={} source={} 进价={:.4} 加成率={:.4} 计算售价={:.6} 旧售价={:.4}",
            product_id, source, purchase_price, markup_rate, new_base_price, old_base_price
        );
        if (old_base_price - new_base_price).abs() < 0.001 {
            eprintln!("[售价自动更新] 商品ID={} 售价未变化，跳过写库", product_id);
            return;
        }

        let _ = sqlx::query("UPDATE product SET base_price = ? WHERE id = ?")
            .bind(new_base_price)
            .bind(product_id)
            .execute(pool())
            .await;

        log_price_change(
            product_id,
            "base_price",
            old_base_price,
            new_base_price,
            source,
            ref_id,
            Some("按加成率自动重算"),
        ).await;
    }
}

// 单个商品开启/关闭自动更新售价，并立即按加成率重算 base_price

// 批量：对所有商品开启/关闭自动更新售价，并对开启的商品立即按加成率重算 base_price

// 获取某商品最近一次采购的基础单位单价（按 purchase_order_item 的 base_unit 维度的同基础单位）
// 通过该商品最近的采购单明细反推：unit_price 为下单单位价，乘以 ratio 得基础单位价

// 规则：当前进价 = 最近一次采购价；最高进价 = 历史最高；最低进价 = 历史最低（新品或价格为0时初始化）。
pub async fn update_product_purchase_prices(items: &[PurchaseOrderItemReq]) {
    for item in items {
        if item.product_id == 0 {
            continue;
        }
        // 换算为基础单位单价（四舍五入保留两位小数，避免 amount/base_quantity 除法产生浮点长尾）
        let base_unit_price = ((if let Some(bq) = item.base_quantity {
            if bq > 0.0 && item.quantity > 0.0 {
                // amount / base_quantity 得到基础单位单价
                if item.amount > 0.0 { item.amount / bq } else { item.unit_price }
            } else {
                item.unit_price
            }
        } else {
            item.unit_price
        }) * 100.0).round() / 100.0;
        if base_unit_price <= 0.0 {
            continue;
        }

        // 读取商品当前的最高/最低进价
        let row = sqlx::query(
            "SELECT purchase_price, max_purchase_price, min_purchase_price FROM product WHERE id = ?"
        )
        .bind(item.product_id)
        .fetch_optional(pool())
        .await
        .unwrap_or(None);

        if let Some(r) = row {
            let old_purchase: f64 = r.get::<f64, _>("purchase_price");
            let old_max: f64 = r.get::<f64, _>("max_purchase_price");
            let old_min: f64 = r.get::<f64, _>("min_purchase_price");

            // 若历史最高/最低为0（新品或首次采购），则以本次价格初始化（四舍五入保留两位小数）
            let new_max = ((if old_max <= 0.0 { base_unit_price } else { old_max.max(base_unit_price) }) * 100.0).round() / 100.0;
            let new_min = ((if old_min <= 0.0 { base_unit_price } else { old_min.min(base_unit_price) }) * 100.0).round() / 100.0;

            let _ = sqlx::query(
                "UPDATE product SET purchase_price = ?, max_purchase_price = ?, min_purchase_price = ? WHERE id = ?"
            )
            .bind(base_unit_price)
            .bind(new_max)
            .bind(new_min)
            .bind(item.product_id)
            .execute(pool())
            .await;

            // 记录进价变更日志
            log_price_change(
                item.product_id,
                "purchase_price",
                old_purchase,
                base_unit_price,
                "purchase_order",
                None,
                Some("采购单更新进价"),
            ).await;

            // 进价变化后，若开启自动售价更新则按加成率重算 base_price
            eprintln!(
                "[采购单进价更新] 商品ID={} 基础单位进价={:.4} 触发售价重算",
                item.product_id, base_unit_price
            );
            recalc_base_price_by_markup(item.product_id, "purchase_order", None).await;
        }
    }
}

// 采购单删除后，从剩余采购明细历史重新计算商品的最近/最高/最低进价并回写
// 规则：purchase_price = 现存明细中最近一次的采购价；max/min = 现存明细历史最高/最低
//       若该商品已无任何采购明细，则三项均置 0
pub async fn recalc_product_purchase_prices_from_history(product_id: i64) {
    // 现存所有采购明细（含已审核与待审核订单），换算为基础单位单价
    let rows = sqlx::query(
        "SELECT poi.unit_price, poi.quantity, poi.base_quantity, poi.amount, po.order_date
         FROM purchase_order_item poi
         JOIN purchase_order po ON poi.order_id = po.id
         WHERE poi.product_id = ?
         ORDER BY po.order_date DESC, po.id DESC, poi.id DESC"
    )
    .bind(product_id)
    .fetch_all(pool())
    .await
    .unwrap_or_default();

    let mut prices: Vec<f64> = Vec::new();
    let mut latest_price: f64 = 0.0;
    for r in &rows {
        let unit_price: f64 = r.get("unit_price");
        let quantity: f64 = r.get("quantity");
        let base_quantity: f64 = r.try_get("base_quantity").unwrap_or(0.0);
        let amount: f64 = r.get("amount");
        // 与 update_product_purchase_prices 保持一致的换算逻辑
        let base_unit_price = if base_quantity > 0.0 && quantity > 0.0 && amount > 0.0 {
            amount / base_quantity
        } else {
            unit_price
        };
        if base_unit_price <= 0.0 {
            continue;
        }
        if latest_price <= 0.0 {
            latest_price = base_unit_price;
        }
        prices.push(base_unit_price);
    }

    let new_purchase = ((latest_price) * 100.0).round() / 100.0;
    let new_max = ((prices.iter().cloned().fold(0.0, f64::max)) * 100.0).round() / 100.0;
    let new_min = ((prices.iter().cloned().fold(f64::INFINITY, f64::min)) * 100.0).round() / 100.0;
    let new_min = if new_min.is_infinite() { 0.0 } else { new_min };

    let row = sqlx::query(
        "SELECT purchase_price, max_purchase_price, min_purchase_price FROM product WHERE id = ?"
    )
    .bind(product_id)
    .fetch_optional(pool())
    .await
    .unwrap_or(None);

    if let Some(r) = row {
        let old_purchase: f64 = r.get("purchase_price");

        let _ = sqlx::query(
            "UPDATE product SET purchase_price = ?, max_purchase_price = ?, min_purchase_price = ? WHERE id = ?"
        )
        .bind(new_purchase)
        .bind(new_max)
        .bind(new_min)
        .bind(product_id)
        .execute(pool())
        .await;

        if (old_purchase - new_purchase).abs() >= 0.001 {
            log_price_change(
                product_id,
                "purchase_price",
                old_purchase,
                new_purchase,
                "purchase_order_delete",
                None,
                Some("采购单删除后从历史重算"),
            ).await;
            // 进价变化后，若开启自动售价更新则按加成率重算 base_price
            recalc_base_price_by_markup(product_id, "purchase_order_delete", None).await;
        }
        eprintln!(
            "[采购单删除回滚] 商品ID={} 最近进价={:.4} 最高={:.4} 最低={:.4}",
            product_id, new_purchase, new_max, new_min
        );
    }
}
