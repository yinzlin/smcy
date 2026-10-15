//! 财务查询仓储实现：SQL 与旧 api_query_supplier_balance* / api_query_purchaser_balance* /
//! api_query_allocation_source* / api_query_finance_settlement / api_query_income_expense* /
//! api_query_profit_detail* handler 逐字一致
//!
//! 行为保持：全部查询的错误由用例层 unwrap_or_default()/unwrap_or(0) 吞错（含 count）；
//! 收支流水/毛利明细的数值列 try_get + unwrap_or 吞错（与旧 handler 一致）。

use sqlx::sqlite::SqlitePool;
use sqlx::{AssertSqlSafe, Row};

use domain::error::DomainError;
use domain::port::query_finance::{
    AllocationSourceRow, AllocationTargetRow, FinanceSettlementRow, IncomeExpenseRow,
    ProfitDetailRow, PurchaserBalanceRow, QueryFinanceRepo, SupplierBalanceRow,
};

#[derive(Clone)]
pub struct SqliteQueryFinanceRepo {
    pool: SqlitePool,
}

impl SqliteQueryFinanceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl QueryFinanceRepo for SqliteQueryFinanceRepo {
    async fn list_supplier_balance(&self, restricted: bool, supplier_id: i64) -> Result<Vec<SupplierBalanceRow>, DomainError> {
        let mut sql = String::from(
            "SELECT s.id, s.name, 
                    COALESCE(SUM(po.final_amount), 0.0) as purchase_total,
                    COALESCE(SUM(po.final_amount), 0.0) as unpaid
             FROM supplier s 
             LEFT JOIN purchase_order po ON po.supplier_id = s.id"
        );
        let mut binds: Vec<i64> = Vec::new();
        if restricted {
            sql.push_str(" WHERE s.id = ?");
            binds.push(supplier_id);
        }
        sql.push_str(" GROUP BY s.id, s.name ORDER BY purchase_total DESC, s.id ASC");

        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        for b in &binds {
            q = q.bind(b);
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| SupplierBalanceRow {
                id: row.get::<i64, _>("id"),
                name: row.get::<String, _>("name"),
                purchase_total: row.get("purchase_total"),
                unpaid: row.get("unpaid"),
            })
            .collect())
    }

    async fn list_purchaser_balance(&self, restricted: bool, purchaser_id: i64) -> Result<Vec<PurchaserBalanceRow>, DomainError> {
        let mut sql = String::from(
            "SELECT p.id, p.name, 
                    COALESCE(SUM(so.final_amount), 0.0) as sales_total,
                    COALESCE(SUM(so.final_amount), 0.0) as unreceived
             FROM purchaser p 
             LEFT JOIN sales_order so ON so.purchaser_id = p.id"
        );
        let mut binds: Vec<i64> = Vec::new();
        if restricted {
            sql.push_str(" WHERE p.id = ?");
            binds.push(purchaser_id);
        }
        sql.push_str(" GROUP BY p.id, p.name ORDER BY sales_total DESC, p.id ASC");

        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        for b in &binds {
            q = q.bind(b);
        }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| PurchaserBalanceRow {
                id: row.get::<i64, _>("id"),
                name: row.get::<String, _>("name"),
                sales_total: row.get("sales_total"),
                unreceived: row.get("unreceived"),
            })
            .collect())
    }

    async fn list_allocation_sources(&self, start_date: &str, end_date: &str) -> Result<Vec<AllocationSourceRow>, DomainError> {
        let mut sql = String::from(
            "SELECT ca.source_order_id, ca.total_amount, ca.allocated_amount, ca.remaining_balance, ca.status,
                    so.order_no, so.order_date
             FROM consumable_allocation ca
             JOIN sales_order so ON ca.source_order_id = so.id
             WHERE 1=1"
        );
        let mut binds: Vec<String> = Vec::new();
        if !start_date.is_empty() {
            sql.push_str(" AND so.order_date >= ?");
            binds.push(start_date.to_string());
        }
        if !end_date.is_empty() {
            sql.push_str(" AND so.order_date <= ?");
            binds.push(end_date.to_string());
        }
        sql.push_str(" ORDER BY so.order_date DESC");

        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        for b in &binds { q = q.bind(b); }
        let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| AllocationSourceRow {
                source_order_id: r.get::<i64, _>("source_order_id"),
                total_amount: r.get::<f64, _>("total_amount"),
                allocated_amount: r.get::<f64, _>("allocated_amount"),
                remaining_balance: r.get::<f64, _>("remaining_balance"),
                status: r.get::<i64, _>("status"),
                order_no: r.get::<String, _>("order_no"),
                order_date: r.get::<String, _>("order_date"),
            })
            .collect())
    }

    async fn list_allocation_targets(&self, source_order_id: i64) -> Result<Vec<AllocationTargetRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT so.order_no, SUM(osi.amount) as amount
             FROM order_supplement_item osi
             JOIN sales_order so ON osi.target_order_id = so.id
             WHERE osi.source_order_id = ?
             GROUP BY osi.target_order_id
             HAVING SUM(osi.amount) <> 0
             ORDER BY amount DESC"
        )
        .bind(source_order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|t| AllocationTargetRow {
                order_no: t.get::<String, _>("order_no"),
                amount: t.get::<f64, _>("amount"),
            })
            .collect())
    }

    async fn list_allocation_sources_export(&self, start_date: &str, end_date: &str) -> Result<Vec<AllocationSourceRow>, DomainError> {
        let mut sql = String::from("SELECT ca.source_order_id, ca.total_amount, ca.allocated_amount, ca.remaining_balance, ca.status, so.order_no, so.order_date FROM consumable_allocation ca JOIN sales_order so ON ca.source_order_id = so.id WHERE 1=1");
        let mut binds: Vec<String> = Vec::new(); if !start_date.is_empty() { sql.push_str(" AND so.order_date >= ?"); binds.push(start_date.to_string()); } if !end_date.is_empty() { sql.push_str(" AND so.order_date <= ?"); binds.push(end_date.to_string()); }
        sql.push_str(" ORDER BY so.order_date DESC, so.id DESC"); let mut q = sqlx::query(AssertSqlSafe(sql.as_str())); for b in &binds { q = q.bind(b); } let rows = q.fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| AllocationSourceRow {
                source_order_id: r.get::<i64, _>("source_order_id"),
                total_amount: r.get::<f64, _>("total_amount"),
                allocated_amount: r.get::<f64, _>("allocated_amount"),
                remaining_balance: r.get::<f64, _>("remaining_balance"),
                status: r.get::<i64, _>("status"),
                order_no: r.get::<String, _>("order_no"),
                order_date: r.get::<String, _>("order_date"),
            })
            .collect())
    }

    async fn list_allocation_targets_export(&self, source_order_id: i64) -> Result<Vec<AllocationTargetRow>, DomainError> {
        let rows = sqlx::query("SELECT so.order_no, SUM(osi.amount) as amount FROM order_supplement_item osi JOIN sales_order so ON osi.target_order_id=so.id WHERE osi.source_order_id=? GROUP BY osi.target_order_id HAVING SUM(osi.amount)<>0 ORDER BY amount DESC, osi.target_order_id ASC")
            .bind(source_order_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|t| AllocationTargetRow {
                order_no: t.get::<String, _>("order_no"),
                amount: t.get::<f64, _>("amount"),
            })
            .collect())
    }

    async fn count_finance_settlement(&self, where_sql: &str, type_filter: &str) -> Result<i64, DomainError> {
        let mut count_query = String::from(
            "SELECT COUNT(*) as cnt FROM (
                SELECT 'purchase' as order_type, po.order_no, po.order_date, po.total_amount, po.discount_rate, po.amount_reduction, po.final_amount, po.is_settled, s.name as party_name
                FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                UNION ALL
                SELECT 'sales' as order_type, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.amount_reduction, so.final_amount, so.is_settled, p.name as party_name
                FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
            ) combined"
        );
        if !where_sql.is_empty() {
            count_query.push(' ');
            count_query.push_str(where_sql);
        }
        if !type_filter.is_empty() {
            count_query.push(' ');
            count_query.push_str(type_filter);
        }
        let row: i64 = sqlx::query_scalar(AssertSqlSafe(count_query.as_str()))
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row)
    }

    async fn list_finance_settlement(&self, where_sql: &str, type_filter: &str, page_size: i64, offset: i64) -> Result<Vec<FinanceSettlementRow>, DomainError> {
        let mut data_query = String::from(
            "SELECT * FROM (
                SELECT 'purchase' as order_type, po.id as source_id, po.order_no, po.order_date, po.total_amount, po.discount_rate, po.amount_reduction, po.final_amount, po.is_settled, s.name as party_name, s.id as party_id
                FROM purchase_order po JOIN supplier s ON po.supplier_id = s.id
                UNION ALL
                SELECT 'sales' as order_type, so.id as source_id, so.order_no, so.order_date, so.total_amount, so.discount_rate, so.amount_reduction, so.final_amount, so.is_settled, p.name as party_name, p.id as party_id
                FROM sales_order so JOIN purchaser p ON so.purchaser_id = p.id
            ) combined"
        );
        if !where_sql.is_empty() {
            data_query.push(' ');
            data_query.push_str(where_sql);
        }
        if !type_filter.is_empty() {
            data_query.push(' ');
            data_query.push_str(type_filter);
        }
        data_query.push_str(&format!(" ORDER BY order_date DESC, order_no DESC LIMIT {} OFFSET {}", page_size, offset));
        let rows = sqlx::query(AssertSqlSafe(data_query.as_str()))
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|row| FinanceSettlementRow {
                order_type: row.get::<String, _>("order_type"),
                source_id: row.get::<i64, _>("source_id"),
                order_no: row.get::<String, _>("order_no"),
                order_date: row.get::<String, _>("order_date"),
                party_name: row.get::<String, _>("party_name"),
                total_amount: row.get("total_amount"),
                discount_rate: row.get("discount_rate"),
                amount_reduction: row.get("amount_reduction"),
                final_amount: row.get("final_amount"),
                is_settled: row.get::<i64, _>("is_settled"),
            })
            .collect())
    }

    async fn list_income_expense(&self, start_date: &str, end_date: &str) -> Result<Vec<IncomeExpenseRow>, DomainError> {
        let mut date_filter = String::new();
        if !start_date.is_empty() {
            date_filter.push_str(&format!(" AND order_date >= '{}'", start_date));
        }
        if !end_date.is_empty() {
            date_filter.push_str(&format!(" AND order_date <= '{}'", end_date));
        }

        let sql = format!(
            "SELECT order_date, '销售订单' as type, CAST(total_amount AS REAL) as total_amount, CAST(final_amount AS REAL) as final_amount, '收入' as direction
             FROM sales_order WHERE status != 'cancelled' {}
             UNION ALL
             SELECT order_date, '采购订单' as type, CAST(total_amount AS REAL) as total_amount, CAST(final_amount AS REAL) as final_amount, '支出' as direction
             FROM purchase_order WHERE status != 'cancelled' {}
             ORDER BY order_date",
            date_filter, date_filter
        );

        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows.iter().map(|row| IncomeExpenseRow {
            order_date: row.get::<String, _>("order_date"),
            type_name: row.get::<String, _>("type"),
            // 行为保持：旧 handler 对这两列 try_get + unwrap_or 吞错
            total_amount: row.try_get::<f64, _>("total_amount").unwrap_or(0.0),
            final_amount: row.try_get::<f64, _>("final_amount").unwrap_or(0.0),
            direction: row.get::<String, _>("direction"),
        }).collect())
    }

    async fn list_income_expense_export(&self, start_date: &str, end_date: &str) -> Result<Vec<IncomeExpenseRow>, DomainError> {
        let mut df=String::new();if!start_date.is_empty(){df.push_str(&format!(" AND order_date>='{}'",start_date));}if!end_date.is_empty(){df.push_str(&format!(" AND order_date<='{}'",end_date));}
        let sql=format!("SELECT order_date,'销售订单' as type,CAST(total_amount AS REAL) as total_amount,CAST(final_amount AS REAL) as final_amount,'收入' as direction,id as oid FROM sales_order WHERE status!='cancelled'{} UNION ALL SELECT order_date,'采购订单' as type,CAST(total_amount AS REAL) as total_amount,CAST(final_amount AS REAL) as final_amount,'支出' as direction,id as oid FROM purchase_order WHERE status!='cancelled'{} ORDER BY order_date, direction, oid",df,df);
        let rows=sqlx::query(AssertSqlSafe(sql.as_str())).fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(|row| IncomeExpenseRow {
            order_date: row.get::<String, _>("order_date"),
            type_name: row.get::<String, _>("type"),
            // 行为保持：旧 handler 对这两列 try_get + unwrap_or 吞错
            total_amount: row.try_get::<f64, _>("total_amount").unwrap_or(0.0),
            final_amount: row.try_get::<f64, _>("final_amount").unwrap_or(0.0),
            direction: row.get::<String, _>("direction"),
        }).collect())
    }

    async fn list_profit_detail(&self, start_date: &str, end_date: &str) -> Result<Vec<ProfitDetailRow>, DomainError> {
        let mut date_filter = String::new();
        if !start_date.is_empty() {
            date_filter.push_str(&format!(" AND so.order_date >= '{}'", start_date));
        }
        if !end_date.is_empty() {
            date_filter.push_str(&format!(" AND so.order_date <= '{}'", end_date));
        }

        let sql = format!(
            "SELECT so.order_no, so.order_date, soi.product_name, CAST(soi.quantity AS REAL) as quantity, CAST(soi.unit_price AS REAL) as sale_price,
                    COALESCE(CAST(p.purchase_price AS REAL), 0) as purchase_price,
                    (CAST(soi.unit_price AS REAL) - COALESCE(CAST(p.purchase_price AS REAL), 0)) * CAST(soi.quantity AS REAL) as profit,
                    CAST(soi.amount AS REAL) as sale_amount,
                    COALESCE(CAST(p.purchase_price AS REAL), 0) * CAST(soi.quantity AS REAL) as cost_amount
             FROM sales_order_item soi
             JOIN sales_order so ON soi.order_id = so.id
             LEFT JOIN product p ON soi.product_id = p.id
             WHERE so.status != 'cancelled' {}
             ORDER BY so.order_date, so.order_no",
            date_filter
        );

        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows.iter().map(|row| ProfitDetailRow {
            order_no: row.get::<String, _>("order_no"),
            order_date: row.get::<String, _>("order_date"),
            product_name: row.get::<String, _>("product_name"),
            // 行为保持：旧 handler 对这些数值列 try_get + unwrap_or 吞错
            quantity: row.try_get::<f64, _>("quantity").unwrap_or(0.0),
            sale_price: row.try_get::<f64, _>("sale_price").unwrap_or(0.0),
            purchase_price: row.try_get::<f64, _>("purchase_price").unwrap_or(0.0),
            profit: row.try_get::<f64, _>("profit").unwrap_or(0.0),
            sale_amount: row.try_get::<f64, _>("sale_amount").unwrap_or(0.0),
            cost_amount: row.try_get::<f64, _>("cost_amount").unwrap_or(0.0),
        }).collect())
    }

    async fn list_profit_detail_export(&self, start_date: &str, end_date: &str) -> Result<Vec<ProfitDetailRow>, DomainError> {
        let mut df=String::new();if!start_date.is_empty(){df.push_str(&format!(" AND so.order_date>='{}'",start_date));}if!end_date.is_empty(){df.push_str(&format!(" AND so.order_date<='{}'",end_date));}
        let sql=format!("SELECT so.order_no,so.order_date,soi.product_name,CAST(soi.quantity AS REAL) as quantity,CAST(soi.unit_price AS REAL) as sale_price,COALESCE(CAST(p.purchase_price AS REAL),0) as purchase_price,(CAST(soi.unit_price AS REAL)-COALESCE(CAST(p.purchase_price AS REAL),0))*CAST(soi.quantity AS REAL) as profit,CAST(soi.amount AS REAL) as sale_amount,COALESCE(CAST(p.purchase_price AS REAL),0)*CAST(soi.quantity AS REAL) as cost_amount FROM sales_order_item soi JOIN sales_order so ON soi.order_id=so.id LEFT JOIN product p ON soi.product_id=p.id WHERE so.status!='cancelled'{} ORDER BY so.order_date, so.id, soi.id",df);
        let rows=sqlx::query(AssertSqlSafe(sql.as_str())).fetch_all(&self.pool).await.map_err(storage)?;
        Ok(rows.iter().map(|row| ProfitDetailRow {
            order_no: row.get::<String, _>("order_no"),
            order_date: row.get::<String, _>("order_date"),
            product_name: row.get::<String, _>("product_name"),
            // 行为保持：旧 handler 对这些数值列 try_get + unwrap_or 吞错
            quantity: row.try_get::<f64, _>("quantity").unwrap_or(0.0),
            sale_price: row.try_get::<f64, _>("sale_price").unwrap_or(0.0),
            purchase_price: row.try_get::<f64, _>("purchase_price").unwrap_or(0.0),
            profit: row.try_get::<f64, _>("profit").unwrap_or(0.0),
            sale_amount: row.try_get::<f64, _>("sale_amount").unwrap_or(0.0),
            cost_amount: row.try_get::<f64, _>("cost_amount").unwrap_or(0.0),
        }).collect())
    }
}
