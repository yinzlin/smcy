//! product 仓储实现：SQL 与旧 api_product_* handler 逐字一致

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::product::{
    DeleteCascadeError, ImportInsertOutcome, ImportProduct, LastPurchaseItemRow, NewProduct, NewProductUnit,
    PriceLogRow, PriceSnapshot, PriceSnapshotRow, ProductDetailRow, ProductExportRow, ProductListFilter,
    ProductPriceRow, ProductPurchaseInfo, ProductRepo, ProductRow, ProductSearchRow, ProductSummaryRow,
    ProductUnitRow, RefCountError, RefCounts, UpdateProduct,
};

#[derive(Clone)]
pub struct SqliteProductRepo {
    pool: SqlitePool,
}

impl SqliteProductRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl ProductRepo for SqliteProductRepo {
    async fn list(&self, f: &ProductListFilter) -> Result<(Vec<ProductRow>, i64), DomainError> {
        // 构造 WHERE 条件、绑定参数和 COUNT 基础 SQL（与旧 api_product_list 逐字一致）
        struct QueryParts {
            where_sql: String,
            binds: Vec<String>,
            category_bind: Option<i64>,
            id_bind: Option<i64>,
        }

        let keyword_pattern = &f.keyword_pattern;
        let mut query_parts = if let Some(pid) = f.product_id {
            // 按 id 精确查询(用于前端按需加载单条商品图片)
            QueryParts {
                where_sql: "WHERE p.id = ?".to_string(),
                binds: vec![],
                category_bind: None,
                id_bind: Some(pid),
            }
        } else if let Some(cid) = f.category_id {
            QueryParts {
                where_sql: format!(
                    "WHERE p.category_id IN (\n                    WITH RECURSIVE cat_tree(id) AS (\n                        SELECT id FROM category WHERE id = ?\n                        UNION ALL\n                        SELECT c.id FROM category c\n                        JOIN cat_tree ct ON c.parent_id = ct.id\n                    )\n                    SELECT id FROM cat_tree\n                )\n                AND (p.name LIKE ? OR p.alias1 LIKE ? OR p.alias2 LIKE ?)"
                ),
                binds: vec![
                    keyword_pattern.clone(),
                    keyword_pattern.clone(),
                    keyword_pattern.clone(),
                ],
                category_bind: Some(cid),
                id_bind: None,
            }
        } else {
            QueryParts {
                where_sql: "WHERE (p.name LIKE ? OR p.alias1 LIKE ? OR p.alias2 LIKE ?)".to_string(),
                binds: vec![
                    keyword_pattern.clone(),
                    keyword_pattern.clone(),
                    keyword_pattern.clone(),
                ],
                category_bind: None,
                id_bind: None,
            }
        };

        // 政采状态筛选：0=非政采 1=政采有指导价 2=政采无指导价
        if let Some(gs) = f.gov_status {
            if (0..=2).contains(&gs) {
                query_parts.where_sql.push_str(" AND p.gov_status = ?");
                query_parts.binds.push(gs.to_string());
            }
        }

        // 总数查询
        let count_sql = format!(
            "SELECT COUNT(*) as count FROM product p LEFT JOIN category c ON p.category_id = c.id {}",
            query_parts.where_sql
        );
        let mut count_q = sqlx::query(AssertSqlSafe(count_sql.as_str()));
        if let Some(pid) = query_parts.id_bind {
            count_q = count_q.bind(pid);
        }
        if let Some(cid) = query_parts.category_bind {
            count_q = count_q.bind(cid);
        }
        for b in &query_parts.binds {
            count_q = count_q.bind(b);
        }
        let total_row = count_q.fetch_one(&self.pool).await.map_err(storage)?;
        let total: i64 = total_row.get("count");

        // 分页数据查询
        let select_cols = if f.include_thumbs {
            "p.id, p.name, p.spec, p.alias1, p.alias2, p.unit, p.base_unit, p.base_price, p.purchase_price, p.max_purchase_price, p.min_purchase_price, p.markup_rate, p.auto_update_price, p.image_url, p.category_id, p.status, p.audit_status, p.shelf_life, p.gov_status, c.name as category_name"
        } else {
            // thumbs=0 模式: 省 image_url 列,流量更小;前端按需单独请求 ?thumbs=1
            "p.id, p.name, p.spec, p.alias1, p.alias2, p.unit, p.base_unit, p.base_price, p.purchase_price, p.max_purchase_price, p.min_purchase_price, p.markup_rate, p.auto_update_price, p.category_id, p.status, p.audit_status, p.shelf_life, p.gov_status, c.name as category_name"
        };
        let data_sql = format!(
            "SELECT {} \n         FROM product p LEFT JOIN category c ON p.category_id = c.id\n         {}\n         ORDER BY p.id DESC LIMIT ? OFFSET ?",
            select_cols, query_parts.where_sql
        );
        let mut data_q = sqlx::query(AssertSqlSafe(data_sql.as_str()));
        if let Some(pid) = query_parts.id_bind {
            data_q = data_q.bind(pid);
        }
        if let Some(cid) = query_parts.category_bind {
            data_q = data_q.bind(cid);
        }
        for b in &query_parts.binds {
            data_q = data_q.bind(b);
        }
        data_q = data_q.bind(f.page_size).bind(f.offset);
        let rows = data_q.fetch_all(&self.pool).await.map_err(storage)?;

        Ok((rows.iter().map(row_to_product).collect(), total))
    }

    async fn units_of(&self, product_id: i64) -> Result<Vec<ProductUnitRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, unit_name, ratio, unit_price, purchase_price, sort_order FROM product_unit \n             WHERE product_id = ? ORDER BY sort_order, id",
        )
        .bind(product_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|ur| ProductUnitRow {
                id: ur.get::<i64, _>("id"),
                unit_name: ur.get::<String, _>("unit_name"),
                ratio: ur.get::<f64, _>("ratio"),
                unit_price: ur.get::<f64, _>("unit_price"),
                purchase_price: ur.get::<f64, _>("purchase_price"),
                sort_order: ur.get::<i32, _>("sort_order"),
            })
            .collect())
    }

    async fn prices_of(&self, product_id: i64) -> Result<Vec<ProductPriceRow>, DomainError> {
        let rows = sqlx::query("SELECT price_type, price FROM product_price WHERE product_id = ?")
            .bind(product_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|pr| ProductPriceRow {
                price_type: pr.get::<String, _>("price_type"),
                price: pr.get::<f64, _>("price"),
            })
            .collect())
    }

    async fn find_by_name(&self, name: &str) -> Result<Vec<ProductSummaryRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT p.id, p.name, p.spec, p.unit, p.base_unit, p.base_price, p.category_id, c.name as category_name \n         FROM product p LEFT JOIN category c ON p.category_id = c.id\n         WHERE p.name = ?",
        )
        .bind(name)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| ProductSummaryRow {
                id: row.get::<i64, _>("id"),
                name: row.get::<String, _>("name"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<String, _>("unit"),
                base_unit: row.get::<String, _>("base_unit"),
                base_price: row.get::<f64, _>("base_price"),
                category_id: row.get::<Option<i64>, _>("category_id"),
                category_name: row.get::<Option<String>, _>("category_name"),
            })
            .collect())
    }

    async fn get_detail(&self, id: i64) -> Result<Option<ProductDetailRow>, DomainError> {
        let r = sqlx::query(
            "SELECT p.id, p.name, p.alias1, p.alias2, p.spec, p.unit, p.base_unit, p.base_price, p.purchase_price, p.shelf_life,\n                COALESCE(NULLIF((SELECT price FROM product_price WHERE product_id = p.id AND price_type = 'gov_procurement'), 0),\n                         (SELECT AVG(price) FROM product_price WHERE product_id = p.id AND price_type IN ('supermarket_1','supermarket_2','supermarket_3','ai_realtime') AND price > 0),\n                         p.base_price) as selling_price,\n                c.name as category_name\n         FROM product p LEFT JOIN category c ON p.category_id = c.id\n         WHERE p.id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await;
        match r {
            Ok(r) => Ok(Some(ProductDetailRow {
                id: r.get::<i64, _>("id"),
                name: r.get::<String, _>("name"),
                alias1: r.get::<Option<String>, _>("alias1"),
                alias2: r.get::<Option<String>, _>("alias2"),
                spec: r.get::<Option<String>, _>("spec"),
                unit: r.get::<Option<String>, _>("unit"),
                base_unit: r.get::<Option<String>, _>("base_unit"),
                base_price: r.get::<f64, _>("base_price"),
                purchase_price: r.get::<f64, _>("purchase_price"),
                selling_price: r.get::<f64, _>("selling_price"),
                shelf_life: r.get::<Option<String>, _>("shelf_life"),
                category_name: r.get::<Option<String>, _>("category_name"),
            })),
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(e) => Err(storage(e)),
        }
    }

    async fn search(&self, pattern: &str, only_audited: bool) -> Result<Vec<ProductSearchRow>, DomainError> {
        let where_clause = if only_audited {
            "WHERE p.status = 1 AND p.audit_status = 'confirmed' AND (p.name LIKE ? OR p.alias1 LIKE ? OR p.alias2 LIKE ?)"
        } else {
            "WHERE p.status = 1 AND (p.name LIKE ? OR p.alias1 LIKE ? OR p.alias2 LIKE ?)"
        };
        let sql = format!(
            "SELECT p.id, p.name, p.alias1, p.alias2, p.spec, p.unit, p.base_unit, p.base_price, p.purchase_price,\n                COALESCE(NULLIF((SELECT price FROM product_price WHERE product_id = p.id AND price_type = 'gov_procurement'), 0),\n                         (SELECT AVG(price) FROM product_price WHERE product_id = p.id AND price_type IN ('supermarket_1','supermarket_2','supermarket_3','ai_realtime') AND price > 0),\n                         p.base_price) as selling_price,\n                c.name as category_name\n         FROM product p LEFT JOIN category c ON p.category_id = c.id\n         {}\n         ORDER BY p.name",
            where_clause
        );
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .bind(pattern)
            .bind(pattern)
            .bind(pattern)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| ProductSearchRow {
                id: row.get::<i64, _>("id"),
                name: row.get::<String, _>("name"),
                alias1: row.get::<Option<String>, _>("alias1"),
                alias2: row.get::<Option<String>, _>("alias2"),
                spec: row.get::<Option<String>, _>("spec"),
                unit: row.get::<String, _>("unit"),
                base_unit: row.get::<String, _>("base_unit"),
                base_price: row.get::<f64, _>("base_price"),
                purchase_price: row.get::<f64, _>("purchase_price"),
                selling_price: row.get::<f64, _>("selling_price"),
                category_name: row.get::<Option<String>, _>("category_name"),
            })
            .collect())
    }

    async fn get_status(&self, id: i64) -> Result<Option<i64>, DomainError> {
        sqlx::query("SELECT status FROM product WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| Ok(r.get::<i64, _>("status")))
            .transpose()
    }

    async fn set_status(&self, id: i64, status: i64) -> Result<(), DomainError> {
        sqlx::query("UPDATE product SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn get_audit_status(&self, id: i64) -> Result<Option<String>, DomainError> {
        let row = sqlx::query("SELECT audit_status FROM product WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(match row {
            Some(r) => r.get::<Option<String>, _>("audit_status"),
            None => None,
        })
    }

    async fn create(&self, p: &NewProduct) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO product(name, spec, alias1, alias2, unit, base_unit, base_price, purchase_price, category_id, shelf_life, gov_status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&p.name)
        .bind(&p.spec)
        .bind(&p.alias1)
        .bind(&p.alias2)
        .bind(&p.unit)
        .bind(&p.base_unit)
        .bind(p.base_price)
        .bind(p.purchase_price)
        .bind(&p.category_id)
        .bind(&p.shelf_life)
        .bind(p.gov_status)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn get_price_snapshot(&self, id: i64) -> Result<Option<PriceSnapshot>, DomainError> {
        let row = sqlx::query("SELECT base_price, purchase_price, markup_rate, auto_update_price FROM product WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| PriceSnapshot {
            base_price: r.get::<f64, _>("base_price"),
            purchase_price: r.get::<f64, _>("purchase_price"),
            markup_rate: r.get::<f64, _>("markup_rate"),
            auto_update_price: r.get::<i64, _>("auto_update_price"),
        }))
    }

    async fn update(&self, id: i64, p: &UpdateProduct) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE product SET name = ?, spec = ?, alias1 = ?, alias2 = ?, unit = ?, base_unit = ?, base_price = ?, purchase_price = ?, image_url = ?, category_id = ?, markup_rate = ?, auto_update_price = ?, shelf_life = ?, gov_status = ?, audit_status='pending' WHERE id = ?",
        )
        .bind(&p.name)
        .bind(&p.spec)
        .bind(&p.alias1)
        .bind(&p.alias2)
        .bind(&p.unit)
        .bind(&p.base_unit)
        .bind(&p.base_price)
        .bind(&p.purchase_price)
        .bind(&p.image_url)
        .bind(&p.category_id)
        .bind(&p.markup_rate)
        .bind(&p.auto_update_price)
        .bind(&p.shelf_life)
        .bind(p.gov_status)
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn count_refs(&self, id: i64) -> Result<RefCounts, RefCountError> {
        let check_tables: [(&'static str, &str); 4] = [
            ("库存", "SELECT COUNT(*) FROM inventory WHERE product_id = ?"),
            ("采购订单明细", "SELECT COUNT(*) FROM purchase_order_item WHERE product_id = ?"),
            ("销售订单明细", "SELECT COUNT(*) FROM sales_order_item WHERE product_id = ?"),
            ("补货/订单调整明细", "SELECT COUNT(*) FROM order_supplement_item WHERE product_id = ?"),
        ];
        let mut counts = RefCounts::default();
        for (name, sql) in check_tables {
            // 查询本身出错（如表结构变更导致列不存在）必须显式报错，不能静默当作 0 放行
            let count: i64 = sqlx::query(sql)
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RefCountError { table: name, message: e.to_string() })?
                .get(0);
            match name {
                "库存" => counts.inventory = count,
                "采购订单明细" => counts.purchase_item = count,
                "销售订单明细" => counts.sales_item = count,
                _ => counts.supplement_item = count,
            }
        }
        Ok(counts)
    }

    async fn delete_cascade(&self, id: i64) -> Result<u64, DeleteCascadeError> {
        // product 被多张表以外键引用，且数据库开启了 foreign_keys 约束，
        // 必须先清理附属数据再删商品，否则 DELETE 会因 FOREIGN KEY constraint failed 而失败
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DeleteCascadeError::Begin(e.to_string()))?;

        for sql in [
            "DELETE FROM product_unit WHERE product_id = ?",
            "DELETE FROM product_price WHERE product_id = ?",
            "DELETE FROM product_price_schedule WHERE product_id = ?",
            "DELETE FROM product_price_log WHERE product_id = ?",
        ] {
            if let Err(e) = sqlx::query(sql).bind(id).execute(&mut *tx).await {
                return Err(DeleteCascadeError::Cleanup(e.to_string()));
            }
        }

        let result = sqlx::query("DELETE FROM product WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DeleteCascadeError::Delete(e.to_string()))?;
        let rows = result.rows_affected();
        if rows > 0 {
            tx.commit()
                .await
                .map_err(|e| DeleteCascadeError::Commit(e.to_string()))?;
        }
        Ok(rows)
    }

    // —— 子步b：导出 / 导入 ——

    async fn list_for_export(&self) -> Result<Vec<ProductExportRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT p.id, p.name, p.alias1, p.alias2, p.spec, p.unit, p.base_unit, p.base_price, p.purchase_price, c.name as category_name,\n                COALESCE(gp.price, 0.0) as gov_price,\n                COALESCE(sp1.price, 0.0) as sm1_price,\n                COALESCE(sp2.price, 0.0) as sm2_price,\n                COALESCE(sp3.price, 0.0) as sm3_price,\n                COALESCE(ai.price, 0.0) as ai_price\n         FROM product p\n         LEFT JOIN category c ON p.category_id = c.id\n         LEFT JOIN product_price gp ON gp.product_id = p.id AND gp.price_type = 'gov_procurement'\n         LEFT JOIN product_price sp1 ON sp1.product_id = p.id AND sp1.price_type = 'supermarket_1'\n         LEFT JOIN product_price sp2 ON sp2.product_id = p.id AND sp2.price_type = 'supermarket_2'\n         LEFT JOIN product_price sp3 ON sp3.product_id = p.id AND sp3.price_type = 'supermarket_3'\n         LEFT JOIN product_price ai ON ai.product_id = p.id AND ai.price_type = 'ai_realtime'\n         ORDER BY p.id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| ProductExportRow {
                id: r.get::<i64, _>("id"),
                name: r.get::<String, _>("name"),
                alias1: r.get::<Option<String>, _>("alias1"),
                alias2: r.get::<Option<String>, _>("alias2"),
                spec: r.get::<Option<String>, _>("spec"),
                unit: r.get::<String, _>("unit"),
                base_unit: r.get::<Option<String>, _>("base_unit"),
                base_price: r.get::<f64, _>("base_price"),
                purchase_price: r.get::<f64, _>("purchase_price"),
                category_name: r.get::<Option<String>, _>("category_name"),
                gov_price: r.get::<f64, _>("gov_price"),
                sm1_price: r.get::<f64, _>("sm1_price"),
                sm2_price: r.get::<f64, _>("sm2_price"),
                sm3_price: r.get::<f64, _>("sm3_price"),
                ai_price: r.get::<f64, _>("ai_price"),
            })
            .collect())
    }

    async fn find_category_id(&self, name: &str) -> Result<Option<i64>, DomainError> {
        sqlx::query("SELECT id FROM category WHERE name = ? AND entity_type = 'product'")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| Ok(r.get::<i64, _>("id")))
            .transpose()
    }

    async fn count_by_id(&self, id: i64) -> Result<i64, DomainError> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        Ok(c)
    }

    async fn import_update(&self, id: i64, p: &ImportProduct) -> Result<u64, DomainError> {
        sqlx::query(
            "UPDATE product SET name = ?, alias1 = ?, alias2 = ?, spec = ?, unit = ?, base_unit = ?, base_price = ?, purchase_price = ?, category_id = ?, audit_status = 'pending' WHERE id = ?",
        )
        .bind(&p.name)
        .bind(&p.alias1)
        .bind(&p.alias2)
        .bind(&p.spec)
        .bind(&p.unit)
        .bind(&p.base_unit)
        .bind(p.base_price)
        .bind(p.purchase_price)
        .bind(&p.category_id)
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected())
        .map_err(storage)
    }

    async fn find_id_by_name_spec(&self, name: &str, spec: &str) -> Result<Option<i64>, DomainError> {
        sqlx::query("SELECT id FROM product WHERE name = ? AND spec = ? LIMIT 1")
            .bind(name)
            .bind(spec)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| Ok(r.get::<i64, _>("id")))
            .transpose()
    }

    async fn import_insert(&self, id: Option<i64>, p: &ImportProduct) -> Result<ImportInsertOutcome, DomainError> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO product(id, name, alias1, alias2, spec, unit, base_unit, base_price, purchase_price, category_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&p.name)
        .bind(&p.alias1)
        .bind(&p.alias2)
        .bind(&p.spec)
        .bind(&p.unit)
        .bind(&p.base_unit)
        .bind(p.base_price)
        .bind(p.purchase_price)
        .bind(&p.category_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        if result.rows_affected() > 0 {
            Ok(ImportInsertOutcome::Inserted(result.last_insert_rowid()))
        } else {
            Ok(ImportInsertOutcome::Ignored)
        }
    }

    async fn set_image_url(&self, id: i64, image_url: &str) -> Result<(), DomainError> {
        sqlx::query("UPDATE product SET image_url = ? WHERE id = ?")
            .bind(image_url)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    async fn import_schedule_price(&self, product_id: i64, price_type: &str, price: f64, effective_date: &str, source: &str) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT INTO product_price_schedule(product_id, price_type, price, effective_date, end_date, source, remark)\n                             VALUES (?, ?, ?, ?, NULL, ?, NULL)\n                             ON CONFLICT(product_id, price_type, effective_date) DO UPDATE SET price = excluded.price",
        )
        .bind(product_id)
        .bind(price_type)
        .bind(price)
        .bind(effective_date)
        .bind(source)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn import_price_snapshot(&self, product_id: i64, price_type: &str, price: f64, collected_at: &str, source: &str) -> Result<(), DomainError> {
        sqlx::query("INSERT OR REPLACE INTO product_price(product_id, price_type, price, collected_at, source) VALUES (?, ?, ?, ?, ?)")
            .bind(product_id)
            .bind(price_type)
            .bind(price)
            .bind(collected_at)
            .bind(source)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    // —— 子步b：商品单位 ——

    async fn unit_create(&self, u: &NewProductUnit) -> Result<(), DomainError> {
        // 行为保持：旧 500 响应体携带 sqlx 错误的 Debug 形态，此处以 Debug 字符串存入 Storage
        sqlx::query(
            "INSERT INTO product_unit(product_id, unit_name, ratio, unit_price, purchase_price, sort_order) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(u.product_id)
        .bind(&u.unit_name)
        .bind(u.ratio)
        .bind(u.unit_price)
        .bind(u.purchase_price)
        .bind(u.sort_order)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| DomainError::Storage(format!("{:?}", e)))
    }

    async fn unit_update(&self, id: i64, u: &NewProductUnit) -> Result<(), DomainError> {
        sqlx::query(
            "UPDATE product_unit SET unit_name = ?, ratio = ?, unit_price = ?, purchase_price = ?, sort_order = ? WHERE id = ?",
        )
        .bind(&u.unit_name)
        .bind(u.ratio)
        .bind(u.unit_price)
        .bind(u.purchase_price)
        .bind(u.sort_order)
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn unit_delete(&self, id: i64) -> Result<u64, DomainError> {
        sqlx::query("DELETE FROM product_unit WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected())
            .map_err(storage)
    }

    async fn unit_delete_by_product(&self, product_id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM product_unit WHERE product_id = ?")
            .bind(product_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    // —— 子步b：价格快照 ——

    async fn price_upsert(&self, product_id: i64, price_type: &str, price: f64, collected_at: Option<&str>, source: Option<&str>) -> Result<(), DomainError> {
        sqlx::query(
            "INSERT OR REPLACE INTO product_price(product_id, price_type, price, collected_at, source) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(product_id)
        .bind(price_type)
        .bind(price)
        .bind(collected_at)
        .bind(source)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(storage)
    }

    async fn price_rows(&self, product_id: i64) -> Result<Vec<PriceSnapshotRow>, DomainError> {
        let rows = sqlx::query("SELECT id, product_id, price_type, price, collected_at, source FROM product_price WHERE product_id = ?")
            .bind(product_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| PriceSnapshotRow {
                id: r.get::<i64, _>("id"),
                product_id: r.get::<i64, _>("product_id"),
                price_type: r.get::<String, _>("price_type"),
                price: r.get::<f64, _>("price"),
                collected_at: r.get::<Option<String>, _>("collected_at"),
                source: r.get::<Option<String>, _>("source"),
            })
            .collect())
    }

    async fn price_delete(&self, id: i64) -> Result<u64, DomainError> {
        sqlx::query("DELETE FROM product_price WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected())
            .map_err(storage)
    }

    async fn price_delete_by_product(&self, product_id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM product_price WHERE product_id = ?")
            .bind(product_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    // —— 子步b：最近进价 / 价格日志 ——

    async fn get_purchase_info(&self, product_id: i64) -> Result<Option<ProductPurchaseInfo>, DomainError> {
        sqlx::query("SELECT purchase_price, base_unit FROM product WHERE id = ?")
            .bind(product_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| {
                Ok(ProductPurchaseInfo {
                    purchase_price: r.get::<f64, _>("purchase_price"),
                    base_unit: r.get::<String, _>("base_unit"),
                })
            })
            .transpose()
    }

    async fn get_last_purchase_item(&self, product_id: i64) -> Result<Option<LastPurchaseItemRow>, DomainError> {
        sqlx::query(
            "SELECT poi.unit_price, poi.unit, p.base_unit\n             FROM purchase_order_item poi\n             JOIN purchase_order po ON poi.order_id = po.id\n             JOIN product p ON poi.product_id = p.id\n             WHERE poi.product_id = ?\n             ORDER BY po.order_date DESC, po.id DESC\n             LIMIT 1",
        )
        .bind(product_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .map(|lr| {
            Ok(LastPurchaseItemRow {
                unit_price: lr.get::<f64, _>("unit_price"),
                unit: lr.get::<String, _>("unit"),
            })
        })
        .transpose()
    }

    async fn price_logs(&self, product_id: Option<i64>, limit: i64) -> Result<Vec<PriceLogRow>, DomainError> {
        let rows = if let Some(pid) = product_id {
            sqlx::query(
                "SELECT ppl.id, ppl.product_id, p.name as product_name, ppl.price_type, ppl.old_price, ppl.new_price, ppl.source, ppl.ref_id, ppl.remark, ppl.changed_at\n             FROM product_price_log ppl LEFT JOIN product p ON ppl.product_id = p.id\n             WHERE ppl.product_id = ? ORDER BY ppl.changed_at DESC LIMIT ?",
            )
            .bind(pid)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
        } else {
            sqlx::query(
                "SELECT ppl.id, ppl.product_id, p.name as product_name, ppl.price_type, ppl.old_price, ppl.new_price, ppl.source, ppl.ref_id, ppl.remark, ppl.changed_at\n             FROM product_price_log ppl LEFT JOIN product p ON ppl.product_id = p.id\n             ORDER BY ppl.changed_at DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
        };
        Ok(rows
            .iter()
            .map(|r| PriceLogRow {
                id: r.get::<i64, _>("id"),
                product_id: r.get::<i64, _>("product_id"),
                product_name: r.get::<Option<String>, _>("product_name"),
                price_type: r.get::<String, _>("price_type"),
                old_price: r.get::<f64, _>("old_price"),
                new_price: r.get::<f64, _>("new_price"),
                source: r.get::<Option<String>, _>("source"),
                ref_id: r.get::<Option<i64>, _>("ref_id"),
                remark: r.get::<Option<String>, _>("remark"),
                changed_at: r.get::<Option<String>, _>("changed_at"),
            })
            .collect())
    }
}

fn row_to_product(row: &sqlx::sqlite::SqliteRow) -> ProductRow {
    ProductRow {
        id: row.get("id"),
        name: row.get("name"),
        spec: row.get("spec"),
        alias1: row.get("alias1"),
        alias2: row.get("alias2"),
        unit: row.get("unit"),
        base_unit: row.get("base_unit"),
        base_price: row.get("base_price"),
        purchase_price: row.get("purchase_price"),
        max_purchase_price: row.get("max_purchase_price"),
        min_purchase_price: row.get("min_purchase_price"),
        markup_rate: row.get("markup_rate"),
        auto_update_price: row.get("auto_update_price"),
        image_url: row.get("image_url"),
        category_id: row.get("category_id"),
        status: row.get("status"),
        audit_status: row.get("audit_status"),
        shelf_life: row.get("shelf_life"),
        gov_status: row.try_get::<i64, _>("gov_status").unwrap_or(0),
        category_name: row.get("category_name"),
    }
}
