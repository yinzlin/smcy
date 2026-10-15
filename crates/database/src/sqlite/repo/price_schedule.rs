//! 价格策略时段仓储实现：SQL 与旧 api_price_schedule_* handler 及 service::price 过渡态函数逐字一致

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::price_schedule::{
    BackfillItemRow, DiagnoseRow, HistoryRow, PriceScheduleInput, PriceScheduleRepo,
    PurchasePriceInput, ScheduleExportRow, ScheduleFilter, ScheduleListRow,
};

#[derive(Clone)]
pub struct SqlitePriceScheduleRepo {
    pool: SqlitePool,
}

impl SqlitePriceScheduleRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

/// 售价保留两位小数（四舍五入），等价 common::utils::round2（database 不依赖 common，内联保持）
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// 列表/导出共用的动态 WHERE 构造（行为保持：所有条件统一按字符串绑定）
fn build_where(filter: &ScheduleFilter) -> (String, Vec<String>) {
    let mut where_sql = String::from(" WHERE 1=1");
    let mut simple_binds: Vec<String> = Vec::new();
    if filter.product_id > 0 {
        where_sql.push_str(" AND pps.product_id = ?");
        simple_binds.push(filter.product_id.to_string());
    }
    if !filter.price_type.is_empty() {
        where_sql.push_str(" AND pps.price_type = ?");
        simple_binds.push(filter.price_type.clone());
    }
    if !filter.effective_from.is_empty() {
        where_sql.push_str(" AND pps.effective_date >= ?");
        simple_binds.push(filter.effective_from.clone());
    }
    if !filter.effective_to.is_empty() {
        where_sql.push_str(" AND pps.effective_date <= ?");
        simple_binds.push(filter.effective_to.clone());
    }
    if !filter.keyword.is_empty() {
        where_sql.push_str(" AND (p.name LIKE ? OR p.alias1 LIKE ? OR p.alias2 LIKE ?)");
        let k = format!("%{}%", filter.keyword);
        simple_binds.push(k.clone());
        simple_binds.push(k.clone());
        simple_binds.push(k);
    }
    (where_sql, simple_binds)
}

fn list_row(r: &sqlx::sqlite::SqliteRow) -> ScheduleListRow {
    ScheduleListRow {
        id: r.get::<i64, _>("id"),
        product_id: r.get::<i64, _>("product_id"),
        product_name: r.get::<Option<String>, _>("product_name"),
        spec: r.get::<Option<String>, _>("spec"),
        unit: r.get::<Option<String>, _>("unit"),
        alias1: r.get::<Option<String>, _>("alias1"),
        alias2: r.get::<Option<String>, _>("alias2"),
        price_type: r.get::<String, _>("price_type"),
        price: r.get::<f64, _>("price"),
        effective_date: r.get::<String, _>("effective_date"),
        end_date: r.get::<Option<String>, _>("end_date"),
        source: r.get::<Option<String>, _>("source"),
        remark: r.get::<Option<String>, _>("remark"),
        create_at: r.get::<Option<String>, _>("create_at"),
        supplier_id: r.get::<Option<i64>, _>("supplier_id"),
        purchaser_id: r.get::<Option<i64>, _>("purchaser_id"),
        supplier_name: r.get::<Option<String>, _>("supplier_name"),
        purchaser_name: r.get::<Option<String>, _>("purchaser_name"),
    }
}

impl PriceScheduleRepo for SqlitePriceScheduleRepo {
    async fn create(&self, req: &PriceScheduleInput, price: f64) -> Result<i64, DomainError> {
        // 录入时 end_date 强制 NULL；由 rebuild_end_dates 维护
        let res = sqlx::query(
            "INSERT INTO product_price_schedule(product_id, price_type, price, effective_date, end_date, source, remark, supplier_id, purchaser_id)
             VALUES (?, ?, ?, ?, NULL, ?, ?, ?, ?)"
        )
        .bind(req.product_id)
        .bind(&req.price_type)
        .bind(price)
        .bind(&req.effective_date)
        .bind(&req.source)
        .bind(&req.remark)
        .bind(req.supplier_id)
        .bind(req.purchaser_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.last_insert_rowid())
    }

    async fn get_product_type(&self, id: i64) -> Result<Option<(i64, String)>, DomainError> {
        let row = sqlx::query(
            "SELECT product_id, price_type FROM product_price_schedule WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.map(|r| (r.get::<i64, _>("product_id"), r.get::<String, _>("price_type"))))
    }

    async fn update(&self, id: i64, req: &PriceScheduleInput) -> Result<u64, DomainError> {
        let res = sqlx::query(
            "UPDATE product_price_schedule
             SET price = ?, remark = ?, source = ?, price_type = ?, effective_date = ?, supplier_id = ?, purchaser_id = ?
             WHERE id = ?"
        )
        .bind(req.price)
        .bind(&req.remark)
        .bind(&req.source)
        .bind(&req.price_type)
        .bind(&req.effective_date)
        .bind(req.supplier_id)
        .bind(req.purchaser_id)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.rows_affected())
    }

    async fn delete(&self, id: i64) -> Result<u64, DomainError> {
        let res = sqlx::query("DELETE FROM product_price_schedule WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(res.rows_affected())
    }

    async fn count_list(&self, filter: &ScheduleFilter) -> Result<i64, DomainError> {
        let (where_sql, simple_binds) = build_where(filter);
        let count_sql = format!("SELECT COUNT(*) FROM product_price_schedule pps LEFT JOIN product p ON pps.product_id = p.id{}", where_sql);
        let mut count_q = sqlx::query_scalar::<_, i64>(AssertSqlSafe(count_sql.as_str()));
        for b in &simple_binds {
            count_q = count_q.bind(b);
        }
        count_q.fetch_one(&self.pool).await.map_err(storage)
    }

    async fn list(&self, filter: &ScheduleFilter, page_size: i64, offset: i64) -> Result<Vec<ScheduleListRow>, DomainError> {
        let (where_sql, simple_binds) = build_where(filter);
        // 列表：与 product / supplier / purchaser JOIN 一次性带回名称（前端展示需要）
        let mut list_sql = format!(
            "SELECT pps.id, pps.product_id, p.name as product_name, p.spec, p.unit, p.alias1, p.alias2,
                    pps.price_type, pps.price, pps.effective_date, pps.end_date, pps.source, pps.remark, pps.create_at,
                    pps.supplier_id, pps.purchaser_id,
                    s.name as supplier_name, pu.name as purchaser_name
             FROM product_price_schedule pps
             LEFT JOIN product p ON pps.product_id = p.id
             LEFT JOIN supplier s ON pps.supplier_id = s.id
             LEFT JOIN purchaser pu ON pps.purchaser_id = pu.id{}",
            where_sql
        );
        list_sql.push_str(" ORDER BY pps.effective_date DESC, pps.id DESC LIMIT ? OFFSET ?");
        let mut list_q = sqlx::query(AssertSqlSafe(list_sql.as_str()));
        for b in &simple_binds {
            list_q = list_q.bind(b);
        }
        list_q = list_q.bind(page_size).bind(offset);
        let rows = list_q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(list_row).collect())
    }

    async fn export_list(&self, filter: &ScheduleFilter) -> Result<Vec<ScheduleExportRow>, DomainError> {
        let (where_sql, simple_binds) = build_where(filter);
        let mut list_sql = format!(
            "SELECT pps.id, pps.product_id, p.name as product_name, p.spec, p.unit, p.alias1, p.alias2,
                    pps.price_type, pps.price, pps.effective_date, pps.end_date, pps.source, pps.remark
             FROM product_price_schedule pps LEFT JOIN product p ON pps.product_id = p.id{}",
            where_sql
        );
        list_sql.push_str(" ORDER BY pps.effective_date DESC, pps.id DESC");
        let mut list_q = sqlx::query(AssertSqlSafe(list_sql.as_str()));
        for b in &simple_binds {
            list_q = list_q.bind(b);
        }
        let rows = list_q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| ScheduleExportRow {
                id: r.get::<i64, _>("id"),
                product_id: r.get::<i64, _>("product_id"),
                product_name: r.get::<Option<String>, _>("product_name"),
                spec: r.get::<Option<String>, _>("spec"),
                unit: r.get::<Option<String>, _>("unit"),
                alias1: r.get::<Option<String>, _>("alias1"),
                alias2: r.get::<Option<String>, _>("alias2"),
                price_type: r.get::<String, _>("price_type"),
                price: r.get::<f64, _>("price"),
                effective_date: r.get::<String, _>("effective_date"),
                end_date: r.try_get("end_date").ok().flatten(),
                source: r.try_get("source").ok().flatten(),
                remark: r.try_get("remark").ok().flatten(),
            })
            .collect())
    }

    async fn history(&self, product_id: i64, price_type: &str) -> Result<Vec<HistoryRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, price, effective_date, end_date, source, remark, create_at
             FROM product_price_schedule
             WHERE product_id = ? AND price_type = ?
             ORDER BY effective_date ASC, id ASC"
        )
        .bind(product_id)
        .bind(price_type)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| HistoryRow {
                id: r.get::<i64, _>("id"),
                price: r.get::<f64, _>("price"),
                effective_date: r.get::<String, _>("effective_date"),
                end_date: r.get::<Option<String>, _>("end_date"),
                source: r.get::<Option<String>, _>("source"),
                remark: r.get::<Option<String>, _>("remark"),
                create_at: r.get::<Option<String>, _>("create_at"),
            })
            .collect())
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<ScheduleListRow>, DomainError> {
        let row = sqlx::query(
            "SELECT pps.id, pps.product_id, p.name as product_name, p.spec, p.unit, p.alias1, p.alias2,
                    pps.price_type, pps.price, pps.effective_date, pps.end_date, pps.source, pps.remark, pps.create_at,
                    pps.supplier_id, pps.purchaser_id,
                    s.name as supplier_name, pu.name as purchaser_name
             FROM product_price_schedule pps
             LEFT JOIN product p ON pps.product_id = p.id
             LEFT JOIN supplier s ON pps.supplier_id = s.id
             LEFT JOIN purchaser pu ON pps.purchaser_id = pu.id
             WHERE pps.id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(row.as_ref().map(list_row))
    }

    async fn product_name(&self, product_id: i64) -> Result<Option<String>, DomainError> {
        sqlx::query_scalar("SELECT name FROM product WHERE id = ?")
            .bind(product_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }

    async fn diagnose(&self, product_id: i64) -> Result<Vec<DiagnoseRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, price_type, price, effective_date, end_date, source, remark, supplier_id, purchaser_id
             FROM product_price_schedule
             WHERE product_id = ?
             ORDER BY price_type ASC, effective_date ASC, id ASC"
        )
        .bind(product_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| DiagnoseRow {
                id: r.get::<i64, _>("id"),
                price_type: r.get::<String, _>("price_type"),
                price: r.get::<f64, _>("price"),
                effective_date: r.get::<String, _>("effective_date"),
                end_date: r.get::<Option<String>, _>("end_date"),
                source: r.get::<Option<String>, _>("source"),
                remark: r.get::<Option<String>, _>("remark"),
                supplier_id: r.get::<Option<i64>, _>("supplier_id"),
                purchaser_id: r.get::<Option<i64>, _>("purchaser_id"),
            })
            .collect())
    }

    async fn find_existing_general(&self, product_id: i64, price_type: &str, effective_date: &str) -> Result<Option<i64>, DomainError> {
        // 手动 upsert：先查同维度记录是否存在（批量导入仅处理通用价格类型，supplier_id/purchaser_id 均为 NULL）
        sqlx::query_scalar(
            "SELECT id FROM product_price_schedule
             WHERE product_id = ? AND price_type = ? AND effective_date = ?
               AND supplier_id IS NULL AND purchaser_id IS NULL"
        )
        .bind(product_id)
        .bind(price_type)
        .bind(effective_date)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)
    }

    async fn count_general(&self, product_id: i64, price_type: &str, effective_date: &str) -> Result<i64, DomainError> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM product_price_schedule WHERE product_id = ? AND price_type = ? AND effective_date = ?
             AND supplier_id IS NULL AND purchaser_id IS NULL"
        )
        .bind(product_id)
        .bind(price_type)
        .bind(effective_date)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)
    }

    async fn count_by_key(&self, product_id: i64, price_type: &str, effective_date: &str) -> Result<i64, DomainError> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM product_price_schedule WHERE product_id = ? AND price_type = ? AND effective_date = ?"
        )
        .bind(product_id)
        .bind(price_type)
        .bind(effective_date)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)
    }

    async fn update_price_by_id(&self, id: i64, price: f64, source: &Option<String>, remark: &Option<String>) -> Result<(), DomainError> {
        sqlx::query("UPDATE product_price_schedule SET price = ?, source = ?, remark = ? WHERE id = ?")
            .bind(price)
            .bind(source)
            .bind(remark)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn update_price_by_key(&self, product_id: i64, price_type: &str, effective_date: &str, price: f64, source: &Option<String>, remark: &Option<String>) -> Result<u64, DomainError> {
        // 存在则更新 price / source / remark（避免与手工录入冲突，采用 upsert 语义）
        let res = sqlx::query(
            "UPDATE product_price_schedule SET price = ?, source = ?, remark = ? WHERE product_id = ? AND price_type = ? AND effective_date = ?"
        )
        .bind(price)
        .bind(source)
        .bind(remark)
        .bind(product_id)
        .bind(price_type)
        .bind(effective_date)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(res.rows_affected())
    }

    async fn insert_general(&self, req: &PriceScheduleInput, price: f64) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO product_price_schedule(product_id, price_type, price, effective_date, end_date, source, remark, supplier_id, purchaser_id)
             VALUES (?, ?, ?, ?, NULL, ?, ?, NULL, NULL)"
        )
        .bind(req.product_id)
        .bind(&req.price_type)
        .bind(price)
        .bind(&req.effective_date)
        .bind(&req.source)
        .bind(&req.remark)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn insert_excel(&self, req: &PriceScheduleInput, price: f64) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO product_price_schedule(product_id, price_type, price, effective_date, end_date, source, remark)
             VALUES (?, ?, ?, ?, NULL, ?, ?)"
        )
        .bind(req.product_id)
        .bind(&req.price_type)
        .bind(price)
        .bind(&req.effective_date)
        .bind(&req.source)
        .bind(&req.remark)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn rebuild_end_dates(&self, product_id: i64, price_type: &str) -> Result<(), DomainError> {
        // 拉取该 (product, price_type) 下所有时段，含 supplier_id / purchaser_id 用于分组
        let rows = sqlx::query(
            "SELECT id, effective_date, supplier_id, purchaser_id FROM product_price_schedule
             WHERE product_id = ? AND price_type = ?
             ORDER BY effective_date ASC, id ASC"
        )
        .bind(product_id)
        .bind(price_type)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Storage(format!("查询时段失败: {}", e)))?;

        // 按 (supplier_id, purchaser_id) 分组，每组独立维护时段连续性
        let mut groups: std::collections::HashMap<(Option<i64>, Option<i64>), Vec<(i64, String)>> = std::collections::HashMap::new();
        for r in rows {
            let sid: Option<i64> = r.get("supplier_id");
            let pid: Option<i64> = r.get("purchaser_id");
            let id: i64 = r.get("id");
            let date: String = r.get("effective_date");
            groups.entry((sid, pid)).or_default().push((id, date));
        }

        // 逐条重写 end_date（行为保持：单条失败吞错）
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
                    .execute(&self.pool)
                    .await;
            }
        }
        Ok(())
    }

    async fn sync_price_snapshot(&self, product_id: i64, price_type: &str) -> Result<(), DomainError> {
        // 同步更新 product_price 快照（兼容旧逻辑：取当前最后一条 effective 的价）
        sqlx::query(
            "INSERT OR REPLACE INTO product_price(product_id, price_type, price, collected_at, source)
             SELECT product_id, price_type, price, effective_date, source
             FROM product_price_schedule
             WHERE product_id = ? AND price_type = ?
             ORDER BY effective_date DESC, id DESC LIMIT 1"
        )
        .bind(product_id)
        .bind(price_type)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn auto_mark_gov_status(&self, product_id: i64) -> Result<(), DomainError> {
        // 有价 → 1
        let _ = sqlx::query(
            "UPDATE product SET gov_status = 1
             WHERE id = ? AND gov_status != 1
             AND EXISTS (
                 SELECT 1 FROM product_price
                 WHERE product_id = product.id AND price_type = 'gov_procurement' AND price > 0
             )",
        )
        .bind(product_id)
        .execute(&self.pool)
        .await;
        // 无价/零 → 仅回退自动标记的 1 到 0（2 为手动选择，保留）
        let _ = sqlx::query(
            "UPDATE product SET gov_status = 0
             WHERE id = ? AND gov_status = 1
             AND NOT EXISTS (
                 SELECT 1 FROM product_price
                 WHERE product_id = product.id AND price_type = 'gov_procurement' AND price > 0
             )",
        )
        .bind(product_id)
        .execute(&self.pool)
        .await;
        Ok(())
    }

    async fn log_price_change(&self, product_id: i64, price_type: &str, old_price: f64, new_price: f64, source: &str, ref_id: Option<i64>, remark: Option<&str>) -> Result<(), DomainError> {
        // 仅当价格实际变化时记录
        if (old_price - new_price).abs() < 0.001 {
            return Ok(());
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
        .execute(&self.pool)
        .await;
        Ok(())
    }

    async fn recalc_base_price_by_markup(&self, product_id: i64, source: &str, ref_id: Option<i64>) -> Result<(), DomainError> {
        let row = sqlx::query(
            "SELECT purchase_price, base_price, markup_rate, auto_update_price FROM product WHERE id = ?"
        )
        .bind(product_id)
        .fetch_optional(&self.pool)
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
                return Ok(());
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
                return Ok(());
            }

            let _ = sqlx::query("UPDATE product SET base_price = ? WHERE id = ?")
                .bind(new_base_price)
                .bind(product_id)
                .execute(&self.pool)
                .await;

            self.log_price_change(
                product_id,
                "base_price",
                old_base_price,
                new_base_price,
                source,
                ref_id,
                Some("按加成率自动重算"),
            ).await?;
        }
        Ok(())
    }

    async fn sync_base_price(&self, product_id: i64) -> Result<(), DomainError> {
        let gov_row = sqlx::query("SELECT price FROM product_price WHERE product_id = ? AND price_type = 'gov_procurement'")
            .bind(product_id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();
        let gov_price: Option<f64> = gov_row.map(|r| r.get("price"));

        // 政采平台状态联动（有价→1，零/空→回退0；手动选的2保留）
        let _ = self.auto_mark_gov_status(product_id).await;

        let avg_row = || async {
            sqlx::query("SELECT COALESCE(AVG(price), 0.0) as avg_price FROM product_price WHERE product_id = ? AND price_type IN ('supermarket_1','supermarket_2','supermarket_3','ai_realtime') AND price > 0")
                .bind(product_id)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten()
                .map(|r| r.get::<f64, _>("avg_price"))
                .unwrap_or(0.0)
        };

        let selling_price: f64 = match gov_price {
            Some(gp) if gp > 0.0 => gp,
            _ => avg_row().await,
        };

        if selling_price > 0.0 {
            // 读取旧值用于日志
            let old_row = sqlx::query("SELECT base_price FROM product WHERE id = ?")
                .bind(product_id)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();
            let old_base_price: f64 = old_row.as_ref().map(|r| r.get::<f64, _>("base_price")).unwrap_or(0.0);

            // 售价保留两位小数（四舍五入）后同步，不做末位数字限制
            let normalized = round2(selling_price);
            eprintln!(
                "[售价同步] 商品ID={} 同步售价={:.4} 旧售价={:.4}",
                product_id, normalized, old_base_price
            );

            let _ = sqlx::query("UPDATE product SET base_price = ? WHERE id = ?")
                .bind(normalized)
                .bind(product_id)
                .execute(&self.pool)
                .await;

            // 若开启了自动售价更新，则该同步值会立即被加成率重算覆盖，这里仅记录日志
            self.log_price_change(
                product_id,
                "base_price",
                old_base_price,
                normalized,
                "sync_base_price",
                None,
                Some("按政府指导价/超市价同步"),
            ).await?;

            // 若开启自动更新售价，则按加成率重算 base_price
            self.recalc_base_price_by_markup(product_id, "sync_base_price", None).await?;
        }
        Ok(())
    }

    async fn lookup_effective(&self, product_id: i64, price_type: &str, query_date: &str) -> Result<Option<(f64, i64, String)>, DomainError> {
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
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;

        Ok(row.map(|r| (
            r.get::<f64, _>("price"),
            r.get::<i64, _>("id"),
            r.get::<String, _>("effective_date"),
        )))
    }

    async fn batch_lookup_effective(&self, product_ids: &[i64], price_type: &str, query_date: &str) -> Result<std::collections::HashMap<i64, f64>, DomainError> {
        let mut map = std::collections::HashMap::new();
        if product_ids.is_empty() {
            return Ok(map);
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
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        for r in rows {
            let pid = r.get::<i64, _>("product_id");
            let price = r.get::<f64, _>("price");
            if price > 0.0 {
                // 价格统一四舍五入保留两位小数
                map.insert(pid, (price * 100.0).round() / 100.0);
            }
        }
        Ok(map)
    }

    async fn lookup_contract(&self, product_id: i64, price_type: &str, party_id: i64, query_date: &str) -> Result<Option<(f64, i64, String)>, DomainError> {
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
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| (
            r.get::<f64, _>("price"),
            r.get::<i64, _>("id"),
            r.get::<String, _>("effective_date"),
        )))
    }

    async fn sales_order_purchaser_id(&self, order_id: i64) -> Result<Option<i64>, DomainError> {
        sqlx::query_scalar("SELECT purchaser_id FROM sales_order WHERE id = ?")
            .bind(order_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }

    async fn sales_order_order_date(&self, order_id: i64) -> Result<String, DomainError> {
        sqlx::query_scalar("SELECT order_date FROM sales_order WHERE id = ?")
            .bind(order_id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    async fn sales_order_items(&self, order_id: i64) -> Result<Vec<BackfillItemRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, product_id, product_name, unit, quantity, unit_price, amount
             FROM sales_order_item WHERE order_id = ?"
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| BackfillItemRow {
                id: r.get::<i64, _>("id"),
                product_id: r.get::<i64, _>("product_id"),
                product_name: r.get("product_name"),
                unit: r.get::<Option<String>, _>("unit"),
                quantity: r.get::<f64, _>("quantity"),
                unit_price: r.get::<f64, _>("unit_price"),
                amount: r.get::<f64, _>("amount"),
            })
            .collect())
    }

    async fn update_sales_item_price(&self, item_id: i64, price: f64, amount: f64) -> Result<(), DomainError> {
        let _ = sqlx::query(
            "UPDATE sales_order_item SET unit_price = ?, amount = ? WHERE id = ?"
        )
        .bind(price)
        .bind(amount)
        .bind(item_id)
        .execute(&self.pool)
        .await;
        Ok(())
    }

    async fn sales_order_discount_rate(&self, order_id: i64) -> Result<f64, DomainError> {
        sqlx::query_scalar("SELECT discount_rate FROM sales_order WHERE id = ?")
            .bind(order_id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    async fn sales_order_total_amount(&self, order_id: i64) -> Result<f64, DomainError> {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount), 0.0) FROM sales_order_item WHERE order_id = ?"
        )
        .bind(order_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)
    }

    async fn update_sales_order_amounts(&self, order_id: i64, total: f64, reduction: f64, final_amount: f64) -> Result<(), DomainError> {
        let _ = sqlx::query(
            "UPDATE sales_order SET total_amount = ?, amount_reduction = ?, final_amount = ? WHERE id = ?"
        )
        .bind(total)
        .bind(reduction)
        .bind(final_amount)
        .bind(order_id)
        .execute(&self.pool)
        .await;
        Ok(())
    }

    async fn find_product_id(&self, name: &str, spec: &str, alias1: &str, alias2: &str) -> Result<Option<i64>, DomainError> {
        // 用 product_name + spec + alias1 + alias2 匹配商品
        // 优先 (name, spec)，无 spec 时回退 name + alias
        // 1) name + spec
        if !spec.is_empty() {
            let row = sqlx::query("SELECT id FROM product WHERE name = ? AND COALESCE(spec,'') = ? LIMIT 1")
                .bind(name).bind(spec)
                .fetch_optional(&self.pool).await.map_err(storage)?;
            if let Some(r) = row {
                return Ok(Some(r.get("id")));
            }
        }
        // 2) name + alias1
        if !alias1.is_empty() {
            let row = sqlx::query("SELECT id FROM product WHERE name = ? AND COALESCE(alias1,'') = ? LIMIT 1")
                .bind(name).bind(alias1)
                .fetch_optional(&self.pool).await.map_err(storage)?;
            if let Some(r) = row {
                return Ok(Some(r.get("id")));
            }
        }
        // 3) name + alias2
        if !alias2.is_empty() {
            let row = sqlx::query("SELECT id FROM product WHERE name = ? AND COALESCE(alias2,'') = ? LIMIT 1")
                .bind(name).bind(alias2)
                .fetch_optional(&self.pool).await.map_err(storage)?;
            if let Some(r) = row {
                return Ok(Some(r.get("id")));
            }
        }
        // 4) name only
        let row = sqlx::query("SELECT id FROM product WHERE name = ? LIMIT 1")
            .bind(name)
            .fetch_optional(&self.pool).await.map_err(storage)?;
        Ok(row.map(|r| r.get("id")))
    }

    async fn update_product_purchase_prices(&self, items: &[PurchasePriceInput]) -> Result<(), DomainError> {
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
            .fetch_optional(&self.pool)
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
                .execute(&self.pool)
                .await;

                // 记录进价变更日志
                let _ = self.log_price_change(
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
                let _ = self.recalc_base_price_by_markup(item.product_id, "purchase_order", None).await;
            }
        }
        Ok(())
    }

    async fn recalc_product_purchase_prices_from_history(&self, product_id: i64) -> Result<(), DomainError> {
        // 现存所有采购明细（含已审核与待审核订单），换算为基础单位单价
        let rows = sqlx::query(
            "SELECT poi.unit_price, poi.quantity, poi.base_quantity, poi.amount, po.order_date
             FROM purchase_order_item poi
             JOIN purchase_order po ON poi.order_id = po.id
             WHERE poi.product_id = ?
             ORDER BY po.order_date DESC, po.id DESC, poi.id DESC"
        )
        .bind(product_id)
        .fetch_all(&self.pool)
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
        .fetch_optional(&self.pool)
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
            .execute(&self.pool)
            .await;

            if (old_purchase - new_purchase).abs() >= 0.001 {
                let _ = self.log_price_change(
                    product_id,
                    "purchase_price",
                    old_purchase,
                    new_purchase,
                    "purchase_order_delete",
                    None,
                    Some("采购单删除后从历史重算"),
                ).await;
                // 进价变化后，若开启自动售价更新则按加成率重算 base_price
                let _ = self.recalc_base_price_by_markup(product_id, "purchase_order_delete", None).await;
            }
            eprintln!(
                "[采购单删除回滚] 商品ID={} 最近进价={:.4} 最高={:.4} 最低={:.4}",
                product_id, new_purchase, new_max, new_min
            );
        }
        Ok(())
    }

    async fn set_auto_update_price(&self, product_id: i64, auto: i64) -> Result<(), DomainError> {
        let _ = sqlx::query("UPDATE product SET auto_update_price = ? WHERE id = ?")
            .bind(auto)
            .bind(product_id)
            .execute(&self.pool)
            .await;

        let _ = self.recalc_base_price_by_markup(product_id, "set_auto_update_price", None).await;
        Ok(())
    }

    async fn batch_set_auto_update_price(&self, auto: i64) -> Result<(), DomainError> {
        let _ = sqlx::query("UPDATE product SET auto_update_price = ?")
            .bind(auto)
            .execute(&self.pool)
            .await;

        if auto == 1 {
            eprintln!("[批量售价自动更新] 开始处理所有商品（auto=开启）");
            // 开启时，对所有进价>0 的商品按加成率重算售价
            let rows = sqlx::query("SELECT id, purchase_price, base_price, markup_rate FROM product WHERE purchase_price > 0")
                .fetch_all(&self.pool)
                .await
                .unwrap_or_default();
            eprintln!("[批量售价自动更新] 共 {} 个商品需要重算", rows.len());
            for r in rows {
                let pid: i64 = r.get("id");
                let old_base: f64 = r.get("base_price");
                let purchase: f64 = r.get("purchase_price");
                let markup: f64 = r.get("markup_rate");
                let raw = purchase * (1.0 + markup);
                // 保留两位小数（四舍五入），不做末位数字限制
                let new_base = round2(raw);
                eprintln!(
                    "[批量售价自动更新] 商品ID={} 进价={:.4} 加成率={:.4} 计算售价={:.6} 旧售价={:.4}",
                    pid, purchase, markup, new_base, old_base
                );
                if (old_base - new_base).abs() >= 0.001 {
                    let _ = sqlx::query("UPDATE product SET base_price = ? WHERE id = ?")
                        .bind(new_base)
                        .bind(pid)
                        .execute(&self.pool)
                        .await;
                    let _ = self.log_price_change(
                        pid,
                        "base_price",
                        old_base,
                        new_base,
                        "batch_set_auto_update_price",
                        None,
                        Some("批量开启自动更新售价"),
                    ).await;
                }
            }
        }
        Ok(())
    }
}
