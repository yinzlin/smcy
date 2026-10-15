//! 销售查询用例：销售订单/价格/汇总查询与导出、报销口径汇总与导出、商品排行与导出
//!
//! 行为保持：count 查询失败 unwrap()（panic 语义）；明细/汇总/聚合查询失败
//! unwrap_or_default() 返回空；workbook 构建 .unwrap() 原样保留。

use std::collections::HashMap;

use domain::UserCtx;
use domain::port::query_sales::QuerySalesRepo;
use rust_xlsxwriter::Workbook;

use crate::error::UseCaseError;
use crate::port::query_sales::QuerySalesUseCase;

#[derive(Clone)]
pub struct QuerySalesService<R: QuerySalesRepo> {
    repo: R,
}

impl<R: QuerySalesRepo> QuerySalesService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: QuerySalesRepo> QuerySalesUseCase for QuerySalesService<R> {
    async fn query_orders(&self, ctx: &UserCtx, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        // 行级数据权限：purchaser 角色强制只看自己绑定的采购单位
        let purchaser_id = if ctx.role == "purchaser" {
            if ctx.purchaser_id > 0 { ctx.purchaser_id.to_string() } else { "-1".to_string() }
        } else {
            params.get("purchaser_id").map(|s| s.as_str()).unwrap_or("").to_string()
        };
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let status = params.get("status").map(|s| s.as_str()).unwrap_or("");

        let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
        let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20);
        let offset = (page - 1) * page_size;

        // 排序处理：支持订单号、采购单位、日期、金额、状态列点击排序
        let sort_field = params.get("sort_field").map(|s| s.as_str()).unwrap_or("id");
        let sort_order_raw = params.get("sort_order").map(|s| s.as_str()).unwrap_or("desc");
        // 仅允许 asc/desc，防止任意 SQL 片段注入
        let sort_order = if sort_order_raw.eq_ignore_ascii_case("asc") { "asc" } else { "desc" };
        let order_clause = match sort_field {
            "order_no" => format!("so.order_no {}", sort_order),
            "unit_name" => format!("p.name {}", sort_order),
            "order_date" => format!("so.order_date {}", sort_order),
            "total_amount" => format!("so.total_amount {}", sort_order),
            "status" => format!("so.status {}", sort_order),
            _ => format!("so.id {}", sort_order),
        };

        // 行为保持：count 查询失败 unwrap() panic
        let total = self
            .repo
            .count_orders(&purchaser_id, start_date, end_date, status)
            .await
            .unwrap();
        // 行为保持：数据查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_orders(&purchaser_id, start_date, end_date, status, &order_clause, page_size, offset)
            .await
            .unwrap_or_default();

        let orders: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "order_no": r.order_no,
                    "order_date": r.order_date,
                    "total_amount": r.total_amount,
                    "discount_rate": r.discount_rate,
                    "final_amount": r.final_amount,
                    "status": r.status,
                    "remark": r.remark,
                    "purchaser_name": r.purchaser_name,
                })
            })
            .collect();

        let result = serde_json::json!({
            "data": orders,
            "page": page,
            "page_size": page_size,
            "total": total,
            "total_pages": (total + page_size - 1) / page_size
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_orders(&self, ctx: &UserCtx, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        // 行级数据权限：purchaser 角色只能导出自己采购单位的数据
        let purchaser_id = if ctx.role == "purchaser" {
            if ctx.purchaser_id > 0 { ctx.purchaser_id.to_string() } else { "-1".to_string() }
        } else {
            params.get("purchaser_id").map(|s| s.as_str()).unwrap_or("").to_string()
        };
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let status = params.get("status").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_orders_for_export(&purchaser_id, start_date, end_date, status)
            .await
            .unwrap_or_default();

        let mut workbook = rust_xlsxwriter::Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("销售订单查询").unwrap();

        let header_format = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0x70AD47))
            .set_font_color(rust_xlsxwriter::Color::White)
            .set_align(rust_xlsxwriter::FormatAlign::Center)
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        let headers = ["订单号", "采购单位", "日期", "订单金额", "下浮比例", "下浮后金额", "状态", "备注"];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_with_format(0, col as u16, *header, &header_format).unwrap();
        }

        for (row_idx, row) in rows.iter().enumerate() {
            worksheet.write(row_idx as u32 + 1, 0, &row.order_no).unwrap();
            worksheet.write(row_idx as u32 + 1, 1, &row.purchaser_name).unwrap();
            worksheet.write(row_idx as u32 + 1, 2, &row.order_date).unwrap();
            worksheet.write(row_idx as u32 + 1, 3, row.total_amount).unwrap();
            worksheet.write(row_idx as u32 + 1, 4, row.discount_rate).unwrap();
            worksheet.write(row_idx as u32 + 1, 5, row.final_amount).unwrap();
            worksheet.write(row_idx as u32 + 1, 6, &row.status).unwrap();
            worksheet.write(row_idx as u32 + 1, 7, row.remark.clone().unwrap_or_default()).unwrap();
        }

        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn price_items(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let product_name = params.get("product_name").map(|s| s.as_str()).unwrap_or("");
        let product_id = params.get("product_id").map(|s| s.as_str()).unwrap_or("");
        let purchaser_id = params.get("purchaser_id").map(|s| s.as_str()).unwrap_or("");

        let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
        let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20);
        let offset = (page - 1) * page_size;

        // 行为保持：count 查询失败 unwrap() panic
        let total = self
            .repo
            .count_price_items(product_name, product_id, purchaser_id)
            .await
            .unwrap();
        // 行为保持：数据查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_price_items(product_name, product_id, purchaser_id, page_size, offset)
            .await
            .unwrap_or_default();

        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "product_name": r.product_name,
                    "spec": r.spec,
                    "unit": r.unit,
                    "purchaser_name": r.purchaser_name,
                    "unit_price": r.unit_price,
                    "order_date": r.order_date,
                    "quantity": r.quantity,
                })
            })
            .collect();

        let result = serde_json::json!({
            "data": items,
            "page": page,
            "page_size": page_size,
            "total": total,
            "total_pages": (total + page_size - 1) / page_size
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn sales_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：两路查询失败均 unwrap_or_default 返回空
        let purchaser_rows = self.repo.summary_by_purchaser(start_date, end_date).await.unwrap_or_default();
        let product_rows = self.repo.summary_by_product(start_date, end_date).await.unwrap_or_default();

        let by_purchaser: Vec<serde_json::Value> = purchaser_rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.name,
                    "quantity": r.quantity,
                    "sales_amount": r.sales_amount,
                    "cost_amount": 0.0,
                })
            })
            .collect();

        let by_product: Vec<serde_json::Value> = product_rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "product_name": r.product_name,
                    "spec": r.spec,
                    "quantity": r.quantity,
                    "sales_amount": r.sales_amount,
                    "cost_amount": 0.0,
                })
            })
            .collect();

        let result = serde_json::json!({
            "by_purchaser": by_purchaser,
            "by_product": by_product,
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn reimburse_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");

        // 作为分摊来源的订单 id（排除，不计入报销口径）
        // 行为保持：查询失败 unwrap_or_default 返回空
        let source_ids = self.repo.list_allocation_source_ids().await.unwrap_or_default();
        let source_set: std::collections::HashSet<i64> = source_ids.into_iter().collect();

        // 1) 真实明细：按采购单位聚合（真实账套口径，含全部订单，不排除来源耗材单）
        // 行为保持：查询失败 unwrap_or_default 返回空
        let real_rows = self
            .repo
            .reimburse_real_by_purchaser(start_date, end_date)
            .await
            .unwrap_or_default();

        // 按采购单位累加：real_amount
        let mut purchaser_map: std::collections::HashMap<i64, (String, f64, f64)> = std::collections::HashMap::new(); // id -> (name, real, supp)
        for r in &real_rows {
            let entry = purchaser_map.entry(r.purchaser_id).or_insert((r.name.clone(), 0.0, 0.0));
            entry.1 += r.amount;
        }

        // 2a) 分摊增项：作为「目标单」收到的增项（正），按目标单采购单位聚合
        // 行为保持：查询失败 unwrap_or_default 返回空
        let supp_target_rows = self
            .repo
            .reimburse_supp_by_purchaser(start_date, end_date)
            .await
            .unwrap_or_default();
        for r in &supp_target_rows {
            let entry = purchaser_map.entry(r.purchaser_id).or_insert((r.name.clone(), 0.0, 0.0));
            entry.2 += r.amount;
        }

        // 2b) 分摊减项：来源耗材单在报销口径里不报销，扣除其「真实明细金额」（非已分摊金额）
        // 净额 = 目标单收到的分摊增项(+) − 来源单真实金额(−)，正好等于分摊尾差（超分为正、少分为负）
        // 仅针对实际作为分摊来源的订单
        if !source_set.is_empty() {
            // 行为保持：查询失败 unwrap_or_default 返回空
            let source_real_rows = self
                .repo
                .reimburse_real_by_order(start_date, end_date)
                .await
                .unwrap_or_default();
            for r in &source_real_rows {
                if !source_set.contains(&r.order_id) { continue; } // 只扣除来源单
                let entry = purchaser_map.entry(r.purchaser_id).or_insert((r.name.clone(), 0.0, 0.0));
                entry.2 -= r.amount;
            }
        }

        let mut by_purchaser: Vec<serde_json::Value> = purchaser_map.values().map(|(name, real, supp)| {
            serde_json::json!({
                "name": name,
                "real_amount": real,
                "supplement_amount": supp,
                "reimburse_amount": real + supp,
            })
        }).collect();
        by_purchaser.sort_by(|a, b| b["reimburse_amount"].as_f64().unwrap_or(0.0).partial_cmp(&a["reimburse_amount"].as_f64().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));

        // 3) 按商品汇总（报销口径）：真实明细（排除来源单）+ 分摊增项
        let mut product_map: std::collections::HashMap<(String, String), (f64, f64)> = std::collections::HashMap::new(); // (name,spec) -> (qty, amount)
        // 行为保持：查询失败 unwrap_or_default 返回空
        let prod_real_rows = self
            .repo
            .reimburse_product_real(start_date, end_date)
            .await
            .unwrap_or_default();
        for r in &prod_real_rows {
            if source_set.contains(&r.order_id) { continue; }
            let entry = product_map.entry((r.product_name.clone(), r.spec.clone())).or_insert((0.0, 0.0));
            entry.0 += r.quantity;
            entry.1 += r.amount;
        }
        // 行为保持：查询失败 unwrap_or_default 返回空
        let prod_supp_rows = self
            .repo
            .reimburse_product_supp(start_date, end_date)
            .await
            .unwrap_or_default();
        for r in &prod_supp_rows {
            let entry = product_map.entry((r.product_name.clone(), r.spec.clone())).or_insert((0.0, 0.0));
            entry.0 += r.quantity;
            entry.1 += r.amount;
        }
        let mut by_product: Vec<serde_json::Value> = product_map.iter().map(|((name, spec), (qty, amount))| {
            serde_json::json!({
                "product_name": name,
                "spec": spec,
                "quantity": qty,
                "reimburse_amount": amount,
            })
        }).collect();
        by_product.sort_by(|a, b| b["reimburse_amount"].as_f64().unwrap_or(0.0).partial_cmp(&a["reimburse_amount"].as_f64().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));

        let result = serde_json::json!({
            "by_purchaser": by_purchaser,
            "by_product": by_product,
        });
        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn product_rank(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：两路查询失败均 unwrap_or_default 返回空
        let top_rows = self.repo.rank_top(start_date, end_date).await.unwrap_or_default();
        let slow_rows = self.repo.rank_slow(start_date, end_date).await.unwrap_or_default();

        let top_selling: Vec<serde_json::Value> = top_rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "product_name": r.product_name,
                    "spec": r.spec,
                    "quantity": r.quantity,
                    "amount": r.amount,
                })
            })
            .collect();

        let slow_moving: Vec<serde_json::Value> = slow_rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "product_name": r.product_name,
                    "spec": r.spec,
                    "stock_quantity": r.stock_quantity,
                    "last_sale_date": r.last_sale_date,
                })
            })
            .collect();

        let result = serde_json::json!({
            "top_selling": top_selling,
            "slow_moving": slow_moving,
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_price_items(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let product_name = params.get("product_name").map(|s| s.as_str()).unwrap_or(""); let purchaser_id = params.get("purchaser_id").map(|s| s.as_str()).unwrap_or("");
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_price_for_export(product_name, purchaser_id)
            .await
            .unwrap_or_default();
        let mut workbook = Workbook::new(); let ws = workbook.add_worksheet(); ws.set_name("销售价格").unwrap(); let hf = common::utils::xlsx_header_format(0x70AD47);
        for (col, h) in ["商品名称", "规格", "采购单位", "销售单价", "销售日期", "销售数量"].iter().enumerate() { ws.write_with_format(0, col as u16, *h, &hf).unwrap(); }
        ws.set_column_width(0, 20).unwrap(); ws.set_column_width(1, 14).unwrap(); ws.set_column_width(2, 16).unwrap(); ws.set_column_width(3, 14).unwrap(); ws.set_column_width(4, 14).unwrap(); ws.set_column_width(5, 14).unwrap();
        for (i, row) in rows.iter().enumerate() { let r = (i + 1) as u32; ws.write(r, 0, row.product_name.clone()).unwrap(); ws.write(r, 1, row.spec.clone().unwrap_or_default()).unwrap(); ws.write(r, 2, row.purchaser_name.clone()).unwrap(); ws.write(r, 3, row.unit_price).unwrap(); ws.write(r, 4, row.order_date.clone()).unwrap(); ws.write(r, 5, row.quantity).unwrap(); }
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn export_sales_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or(""); let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        // 行为保持：两路查询失败均 unwrap_or_default 返回空
        let purchaser_rows = self.repo.summary_export_by_purchaser(start_date, end_date).await.unwrap_or_default();
        let product_rows = self.repo.summary_export_by_product(start_date, end_date).await.unwrap_or_default();
        let mut workbook = Workbook::new(); let hf = common::utils::xlsx_header_format(0x70AD47);
        let ws1 = workbook.add_worksheet(); ws1.set_name("按采购单位汇总").unwrap();
        for (col, h) in ["采购单位", "销售数量", "销售金额", "成本", "毛利", "毛利率"].iter().enumerate() { ws1.write_with_format(0, col as u16, *h, &hf).unwrap(); }
        ws1.set_column_width(0, 18).unwrap(); ws1.set_column_width(1, 14).unwrap(); ws1.set_column_width(2, 14).unwrap(); ws1.set_column_width(3, 14).unwrap(); ws1.set_column_width(4, 14).unwrap(); ws1.set_column_width(5, 14).unwrap();
        for (i, row) in purchaser_rows.iter().enumerate() { let r = (i + 1) as u32; let qty: f64 = row.quantity; let sales: f64 = row.sales_amount; let cost = 0.0; let profit = sales - cost; let margin = if sales > 0.0 { profit / sales * 100.0 } else { 0.0 }; ws1.write(r, 0, row.name.clone()).unwrap(); ws1.write(r, 1, qty).unwrap(); ws1.write(r, 2, sales).unwrap(); ws1.write(r, 3, cost).unwrap(); ws1.write(r, 4, profit).unwrap(); ws1.write(r, 5, margin).unwrap(); }
        let ws2 = workbook.add_worksheet(); ws2.set_name("按商品汇总").unwrap();
        for (col, h) in ["商品名称", "规格", "销售数量", "销售金额", "成本", "毛利"].iter().enumerate() { ws2.write_with_format(0, col as u16, *h, &hf).unwrap(); }
        ws2.set_column_width(0, 20).unwrap(); ws2.set_column_width(1, 14).unwrap(); ws2.set_column_width(2, 14).unwrap(); ws2.set_column_width(3, 14).unwrap(); ws2.set_column_width(4, 14).unwrap(); ws2.set_column_width(5, 14).unwrap();
        for (i, row) in product_rows.iter().enumerate() { let r = (i + 1) as u32; let qty: f64 = row.quantity; let sales: f64 = row.sales_amount; let cost = 0.0; let profit = sales - cost; ws2.write(r, 0, row.product_name.clone()).unwrap(); ws2.write(r, 1, row.spec.clone().unwrap_or_default()).unwrap(); ws2.write(r, 2, qty).unwrap(); ws2.write(r, 3, sales).unwrap(); ws2.write(r, 4, cost).unwrap(); ws2.write(r, 5, profit).unwrap(); }
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn export_product_rank(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or(""); let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self.repo.rank_top(start_date, end_date).await.unwrap_or_default();
        let mut workbook = Workbook::new(); let ws = workbook.add_worksheet(); ws.set_name("畅销商品TOP10").unwrap(); let hf = common::utils::xlsx_header_format(0x70AD47);
        for (col, h) in ["排名", "商品名称", "规格", "销售数量", "销售金额"].iter().enumerate() { ws.write_with_format(0, col as u16, *h, &hf).unwrap(); }
        ws.set_column_width(0, 8).unwrap(); ws.set_column_width(1, 20).unwrap(); ws.set_column_width(2, 14).unwrap(); ws.set_column_width(3, 14).unwrap(); ws.set_column_width(4, 14).unwrap();
        for (i, row) in rows.iter().enumerate() { let r = (i + 1) as u32; ws.write(r, 0, (i + 1) as i64).unwrap(); ws.write(r, 1, row.product_name.clone()).unwrap(); ws.write(r, 2, row.spec.clone().unwrap_or_default()).unwrap(); ws.write(r, 3, row.quantity).unwrap(); ws.write(r, 4, row.amount).unwrap(); }
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn export_reimburse_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or(""); let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        // 行为保持：查询失败 unwrap_or_default 返回空
        let source_ids = self.repo.list_allocation_source_ids().await.unwrap_or_default();
        let source_set: std::collections::HashSet<i64> = source_ids.into_iter().collect();
        // 行为保持：查询失败 unwrap_or_default 返回空
        let real_rows = self.repo.reimburse_real_by_purchaser(start_date, end_date).await.unwrap_or_default();
        let mut pmap: std::collections::HashMap<i64, (String, f64, f64)> = std::collections::HashMap::new();
        for r in &real_rows { let e = pmap.entry(r.purchaser_id).or_insert((r.name.clone(), 0.0, 0.0)); e.1 += r.amount; }
        // 行为保持：查询失败 unwrap_or_default 返回空
        for r in self.repo.reimburse_supp_by_purchaser(start_date, end_date).await.unwrap_or_default() {
            let e = pmap.entry(r.purchaser_id).or_insert((r.name.clone(), 0.0, 0.0));
            e.2 += r.amount;
        }
        if !source_set.is_empty() {
            // 行为保持：查询失败 unwrap_or_default 返回空
            for r in self.repo.reimburse_real_by_order(start_date, end_date).await.unwrap_or_default() {
                if !source_set.contains(&r.order_id) { continue; }
                let e = pmap.entry(r.purchaser_id).or_insert((r.name.clone(), 0.0, 0.0));
                e.2 -= r.amount;
            }
        }
        let mut by_purchaser: Vec<serde_json::Value> = pmap.values().map(|(n,r,s)| serde_json::json!({"name":n,"real_amount":r,"supplement_amount":s,"reimburse_amount":r+s})).collect();
        by_purchaser.sort_by(|a,b| b["reimburse_amount"].as_f64().unwrap_or(0.0).partial_cmp(&a["reimburse_amount"].as_f64().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))));
        let mut prod_map: std::collections::HashMap<(String, String), (f64, f64)> = std::collections::HashMap::new();
        // 行为保持：查询失败 unwrap_or_default 返回空
        for r in self.repo.reimburse_product_real(start_date, end_date).await.unwrap_or_default() {
            if source_set.contains(&r.order_id) { continue; }
            let e = prod_map.entry((r.product_name.clone(), r.spec.clone())).or_insert((0.0, 0.0));
            e.0 += r.quantity; e.1 += r.amount;
        }
        // 行为保持：查询失败 unwrap_or_default 返回空
        for r in self.repo.reimburse_product_supp(start_date, end_date).await.unwrap_or_default() {
            let e = prod_map.entry((r.product_name.clone(), r.spec.clone())).or_insert((0.0, 0.0));
            e.0 += r.quantity; e.1 += r.amount;
        }
        let mut by_product: Vec<serde_json::Value> = prod_map.iter().map(|((n,s),(q,a))| serde_json::json!({"product_name":n,"spec":s,"quantity":q,"reimburse_amount":a})).collect();
        by_product.sort_by(|a,b| b["reimburse_amount"].as_f64().unwrap_or(0.0).partial_cmp(&a["reimburse_amount"].as_f64().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a["product_name"].as_str().unwrap_or("").cmp(b["product_name"].as_str().unwrap_or("")))
            .then_with(|| a["spec"].as_str().unwrap_or("").cmp(b["spec"].as_str().unwrap_or(""))));
        let mut workbook = Workbook::new(); let hf = common::utils::xlsx_header_format(0x70AD47);
        let ws1 = workbook.add_worksheet(); ws1.set_name("按采购单位汇总").unwrap();
        for (c,h) in ["采购单位","真实金额","分摊增项净额","报销金额"].iter().enumerate() { ws1.write_with_format(0,c as u16,*h,&hf).unwrap(); }
        ws1.set_column_width(0,18).unwrap(); ws1.set_column_width(1,14).unwrap(); ws1.set_column_width(2,16).unwrap(); ws1.set_column_width(3,14).unwrap();
        for (i,item) in by_purchaser.iter().enumerate() { let r=(i+1)as u32; ws1.write(r,0,item.get("name").and_then(|v|v.as_str()).unwrap_or("")).unwrap(); ws1.write(r,1,item.get("real_amount").and_then(|v|v.as_f64()).unwrap_or(0.0)).unwrap(); ws1.write(r,2,item.get("supplement_amount").and_then(|v|v.as_f64()).unwrap_or(0.0)).unwrap(); ws1.write(r,3,item.get("reimburse_amount").and_then(|v|v.as_f64()).unwrap_or(0.0)).unwrap(); }
        let ws2 = workbook.add_worksheet(); ws2.set_name("按商品汇总").unwrap();
        for (c,h) in ["商品名称","规格","数量","报销金额"].iter().enumerate() { ws2.write_with_format(0,c as u16,*h,&hf).unwrap(); }
        ws2.set_column_width(0,20).unwrap(); ws2.set_column_width(1,14).unwrap(); ws2.set_column_width(2,14).unwrap(); ws2.set_column_width(3,14).unwrap();
        for (i,item) in by_product.iter().enumerate() { let r=(i+1)as u32; ws2.write(r,0,item.get("product_name").and_then(|v|v.as_str()).unwrap_or("")).unwrap(); ws2.write(r,1,item.get("spec").and_then(|v|v.as_str()).unwrap_or_default()).unwrap(); ws2.write(r,2,item.get("quantity").and_then(|v|v.as_f64()).unwrap_or(0.0)).unwrap(); ws2.write(r,3,item.get("reimburse_amount").and_then(|v|v.as_f64()).unwrap_or(0.0)).unwrap(); }
        Ok(workbook.save_to_buffer().unwrap())
    }
}
