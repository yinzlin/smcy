//! product 仓储实现：SQL 与旧 api_product_* handler 逐字一致

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::product::{
    DeleteCascadeError, NewProduct, PriceSnapshot, ProductDetailRow, ProductListFilter, ProductPriceRow,
    ProductRepo, ProductRow, ProductSearchRow, ProductSummaryRow, ProductUnitRow, RefCountError, RefCounts,
    UpdateProduct,
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
