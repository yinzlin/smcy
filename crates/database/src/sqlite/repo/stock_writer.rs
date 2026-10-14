//! 库存流水写入与基础数量换算：purchase/sales 审核链路共享的基础设施函数
//! （自 backend api/handlers.rs 平移，SQL 与行为逐字一致；pool 改为显式入参）

use sqlx::{AssertSqlSafe, Row};

/// 按 product_unit.ratio 在服务端统一计算明细的基础单位数量，不依赖前端传入的 base_quantity。
/// 入参每项：(product_id, unit, quantity, 前端传入 base_quantity)，返回同序的基础数量。
/// - 命中辅助单位比例：quantity × ratio（保留 2 位小数）
/// - 未命中（基础单位/无单位配置）：前端值 > 0 用前端值，否则用 quantity 兜底
pub async fn compute_base_quantities(
    pool: &sqlx::SqlitePool,
    items: &[(i64, String, f64, f64)],
) -> Vec<f64> {
    let mut unique_pids: Vec<i64> = items.iter().map(|x| x.0).filter(|id| *id > 0).collect();
    unique_pids.sort_unstable();
    unique_pids.dedup();

    let mut ratio_map: std::collections::HashMap<(i64, String), f64> = std::collections::HashMap::new();
    if !unique_pids.is_empty() {
        let placeholders = unique_pids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT product_id, unit_name, ratio FROM product_unit WHERE product_id IN ({})",
            placeholders
        );
        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        for pid in &unique_pids {
            q = q.bind(pid);
        }
        if let Ok(rows) = q.fetch_all(pool).await {
            for r in rows {
                let pid: i64 = r.get("product_id");
                let uname: String = r.get("unit_name");
                let ratio: f64 = r.get("ratio");
                ratio_map.insert((pid, uname), ratio);
            }
        }
    }

    items
        .iter()
        .map(|(pid, unit, qty, client_bq)| {
            if let Some(ratio) = ratio_map.get(&(*pid, unit.clone())) {
                (*qty * ratio * 100.0).round() / 100.0
            } else if *client_bq > 0.0 {
                *client_bq
            } else {
                *qty
            }
        })
        .collect()
}

/// 审核订单时为每个明细写一条 stock_movement + 同步 inventory.quantity。
/// - direction: "in"（采购入库）/"out"（销售出库），决定 base_quantity 符号
/// - movement_type/ref_type: 例如 "purchase"/"purchase" 或 "sales"/"sales"
/// - remark_prefix: 备注前缀，例如 "采购入库"
/// - action_label: 动作标签，拼在 prefix 后，例如 "审核" → "采购入库-审核"、"确认验收" → "销售出库-确认验收"
/// 必须在已开始的事务内调用。
pub async fn write_stock_movements_for_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    order_id: i64,
    item_table: &str,
    direction: &str,
    movement_type: &str,
    ref_type: &str,
    remark_prefix: &str,
    action_label: &str,
) -> Result<(), sqlx::Error> {
    // base_quantity 缺失（老数据/部分开单入口未计算）时，用 quantity × product_unit.ratio 现算
    // 同时取明细 id 与批次快照（采购=到货批次；销售=确认验收时写入的 FIFO 批次）
    let sql = format!(
        "SELECT items.id, items.product_id,
                CASE WHEN items.base_quantity > 0 THEN items.base_quantity
                     ELSE items.quantity * COALESCE(
                         (SELECT pu.ratio FROM product_unit pu
                          WHERE pu.product_id = items.product_id AND pu.unit_name = items.unit), 1)
                END AS eff_base,
                items.quantity, items.unit, items.production_date, items.batch_no
         FROM {} items WHERE items.order_id = ?",
        item_table
    );
    let items: Vec<(i64, i64, f64, f64, String, Option<String>, Option<String>)> = sqlx::query_as(AssertSqlSafe(sql))
        .bind(order_id)
        .fetch_all(&mut **tx)
        .await?;
    // 业务归属日期取单据日期（定量），与实际审核时间分离；同时快照单据号与往来单位名称
    let (order_table, party_table, party_fk): (&str, &str, &str) = if movement_type == "purchase" {
        ("purchase_order", "supplier", "supplier_id")
    } else {
        ("sales_order", "purchaser", "purchaser_id")
    };
    let order_info: (String, String, Option<String>) = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT o.order_date, o.order_no, pt.name
         FROM {} o JOIN {} pt ON o.{} = pt.id WHERE o.id = ?",
        order_table, party_table, party_fk
    )))
    .bind(order_id)
    .fetch_one(&mut **tx)
    .await?;
    let (order_date, order_no, party_name) = order_info;
    let sign: f64 = if direction == "in" { 1.0 } else { -1.0 };
    for (item_id, product_id, base_qty, orig_qty, unit, production_date, batch_no) in items {
        if base_qty == 0.0 { continue; }
        // 当前余额（事务内）
        let cur: f64 = sqlx::query_scalar(
            "SELECT quantity FROM inventory WHERE product_id=? AND warehouse_id=1"
        )
        .bind(product_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(0.0);
        let new_balance = cur + sign * base_qty;
        // UPSERT inventory
        let _ = sqlx::query(
            "INSERT INTO inventory (product_id, warehouse_id, quantity, last_update)
             VALUES (?, 1, ?, datetime('now','localtime'))
             ON CONFLICT(product_id, warehouse_id) DO UPDATE SET quantity = excluded.quantity, last_update = datetime('now','localtime')"
        )
        .bind(product_id).bind(new_balance)
        .execute(&mut **tx).await?;
        // 写流水（含账本快照：明细id/批次/单据号/往来单位）
        let _ = sqlx::query(
            "INSERT INTO stock_movement (product_id, warehouse_id, direction, movement_type, base_quantity, orig_quantity, orig_unit, balance_after, ref_type, ref_id, remark, order_date,
                                         ref_item_id, production_date, batch_no, ref_no, party_name, snapshot_version)
             VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)"
        )
        .bind(product_id).bind(direction).bind(movement_type)
        .bind(base_qty).bind(orig_qty).bind(&unit).bind(new_balance)
        .bind(ref_type).bind(order_id)
        .bind(format!("{}-{}", remark_prefix, action_label))
        .bind(&order_date)
        .bind(item_id).bind(production_date).bind(batch_no).bind(&order_no).bind(&party_name)
        .execute(&mut **tx).await?;
    }
    Ok(())
}

/// 反审核/撤销时为每个明细写一条冲销流水（direction 与原方向相反）+ 同步 inventory.quantity。
/// 必须在已开始的事务内调用。
pub async fn write_stock_movements_for_unaudit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    order_id: i64,
    item_table: &str,
    original_direction: &str,  // "in" 或 "out" — 原审核时的方向
    movement_type: &str,      // 保持原 movement_type，台账按 direction 区分正/冲销
    ref_type: &str,
    remark_prefix: &str,
    action_label: &str,       // 例如 "反审核冲销"/"撤销验收冲销"
) -> Result<(), sqlx::Error> {
    // base_quantity 缺失时同样用 quantity × product_unit.ratio 现算；批次取原明细当前快照
    let sql = format!(
        "SELECT items.id, items.product_id,
                CASE WHEN items.base_quantity > 0 THEN items.base_quantity
                     ELSE items.quantity * COALESCE(
                         (SELECT pu.ratio FROM product_unit pu
                          WHERE pu.product_id = items.product_id AND pu.unit_name = items.unit), 1)
                END AS eff_base,
                items.quantity, items.unit, items.production_date, items.batch_no
         FROM {} items WHERE items.order_id = ?",
        item_table
    );
    let items: Vec<(i64, i64, f64, f64, String, Option<String>, Option<String>)> = sqlx::query_as(AssertSqlSafe(sql))
        .bind(order_id)
        .fetch_all(&mut **tx)
        .await?;
    // 冲销归属原单据日期：同月审核又反审核时净影响为 0；单据号/往来单位随原单据快照
    let (order_table, party_table, party_fk): (&str, &str, &str) = if movement_type == "purchase" {
        ("purchase_order", "supplier", "supplier_id")
    } else {
        ("sales_order", "purchaser", "purchaser_id")
    };
    let order_info: (String, String, Option<String>) = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT o.order_date, o.order_no, pt.name
         FROM {} o JOIN {} pt ON o.{} = pt.id WHERE o.id = ?",
        order_table, party_table, party_fk
    )))
    .bind(order_id)
    .fetch_one(&mut **tx)
    .await?;
    let (order_date, order_no, party_name) = order_info;
    // 冲销方向：原 in → out，原 out → in
    let reversal_direction = if original_direction == "in" { "out" } else { "in" };
    let sign: f64 = if reversal_direction == "in" { 1.0 } else { -1.0 };
    for (item_id, product_id, base_qty, orig_qty, unit, production_date, batch_no) in items {
        if base_qty == 0.0 { continue; }
        let cur: f64 = sqlx::query_scalar(
            "SELECT quantity FROM inventory WHERE product_id=? AND warehouse_id=1"
        )
        .bind(product_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(0.0);
        let new_balance = cur + sign * base_qty;
        let _ = sqlx::query(
            "INSERT INTO inventory (product_id, warehouse_id, quantity, last_update)
             VALUES (?, 1, ?, datetime('now','localtime'))
             ON CONFLICT(product_id, warehouse_id) DO UPDATE SET quantity = excluded.quantity, last_update = datetime('now','localtime')"
        )
        .bind(product_id).bind(new_balance)
        .execute(&mut **tx).await?;
        let _ = sqlx::query(
            "INSERT INTO stock_movement (product_id, warehouse_id, direction, movement_type, base_quantity, orig_quantity, orig_unit, balance_after, ref_type, ref_id, remark, order_date,
                                         ref_item_id, production_date, batch_no, ref_no, party_name, snapshot_version)
             VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)"
        )
        .bind(product_id).bind(reversal_direction).bind(movement_type)
        .bind(base_qty).bind(orig_qty).bind(&unit).bind(new_balance)
        .bind(ref_type).bind(order_id)
        .bind(format!("{}-{}", remark_prefix, action_label))
        .bind(&order_date)
        .bind(item_id).bind(production_date).bind(batch_no).bind(&order_no).bind(&party_name)
        .execute(&mut **tx).await?;
    }
    Ok(())
}
