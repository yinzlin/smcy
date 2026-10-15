//! 采购查询用例：采购订单/价格/汇总查询与导出、商品价格趋势、采购比价
//!
//! 行为保持：count 查询失败 unwrap()（panic 语义）；明细/汇总查询失败 unwrap_or_default()
//! 返回空；商品信息查询失败 unwrap_or(None) 吞错 → 404；workbook 构建 .unwrap() 原样保留。

use std::collections::HashMap;

use domain::UserCtx;
use domain::port::query_purchase::QueryPurchaseRepo;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};

use crate::error::UseCaseError;
use crate::port::query_purchase::QueryPurchaseUseCase;

#[derive(Clone)]
pub struct QueryPurchaseService<R: QueryPurchaseRepo> {
    repo: R,
}

impl<R: QueryPurchaseRepo> QueryPurchaseService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: QueryPurchaseRepo> QueryPurchaseUseCase for QueryPurchaseService<R> {
    async fn query_orders(&self, ctx: &UserCtx, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        // 行级数据权限：supplier 角色强制只看自己绑定的供应商
        let supplier_id = if ctx.role == "supplier" {
            if ctx.supplier_id > 0 { ctx.supplier_id.to_string() } else { "-1".to_string() }
        } else {
            params.get("supplier_id").map(|s| s.as_str()).unwrap_or("").to_string()
        };
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let status = params.get("status").map(|s| s.as_str()).unwrap_or("");

        let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
        let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20);
        let offset = (page - 1) * page_size;

        // 行为保持：count 查询失败 unwrap() panic
        let total = self
            .repo
            .count_orders(&supplier_id, start_date, end_date, status)
            .await
            .unwrap();
        // 行为保持：数据查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_orders(&supplier_id, start_date, end_date, status, page_size, offset)
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
                    "final_amount": r.final_amount,
                    "status": r.status,
                    "remark": r.remark,
                    "supplier_name": r.supplier_name,
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
        // 行级数据权限：supplier 角色只能导出自己供应商的数据
        let supplier_id = if ctx.role == "supplier" {
            if ctx.supplier_id > 0 { ctx.supplier_id.to_string() } else { "-1".to_string() }
        } else {
            params.get("supplier_id").map(|s| s.as_str()).unwrap_or("").to_string()
        };
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let status = params.get("status").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_orders_for_export(&supplier_id, start_date, end_date, status)
            .await
            .unwrap_or_default();

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("采购订单查询").unwrap();

        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White)
            .set_align(FormatAlign::Center)
            .set_border(FormatBorder::Thin);

        let headers = ["订单号", "供应商", "日期", "订单金额", "实付金额", "状态", "备注"];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_with_format(0, col as u16, *header, &header_format).unwrap();
        }

        for (row_idx, row) in rows.iter().enumerate() {
            worksheet.write(row_idx as u32 + 1, 0, &row.order_no).unwrap();
            worksheet.write(row_idx as u32 + 1, 1, &row.supplier_name).unwrap();
            worksheet.write(row_idx as u32 + 1, 2, &row.order_date).unwrap();
            worksheet.write(row_idx as u32 + 1, 3, row.total_amount).unwrap();
            worksheet.write(row_idx as u32 + 1, 4, row.final_amount).unwrap();
            worksheet.write(row_idx as u32 + 1, 5, &row.status).unwrap();
            worksheet.write(row_idx as u32 + 1, 6, row.remark.clone().unwrap_or_default()).unwrap();
        }

        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn price_items(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let product_name = params.get("product_name").map(|s| s.as_str()).unwrap_or("");
        let supplier_id = params.get("supplier_id").map(|s| s.as_str()).unwrap_or("");

        let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
        let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20);
        let offset = (page - 1) * page_size;

        // 行为保持：count 查询失败 unwrap() panic
        let total = self.repo.count_price_items(product_name, supplier_id).await.unwrap();
        // 行为保持：数据查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_price_items(product_name, supplier_id, page_size, offset)
            .await
            .unwrap_or_default();

        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "order_date": r.order_date,
                    "product_name": r.product_name,
                    "unit": r.unit,
                    "unit_price": r.unit_price,
                    "quantity": r.quantity,
                    "amount": r.amount,
                    "production_date": r.production_date,
                    "shelf_life": r.shelf_life,
                    "supplier_name": r.supplier_name,
                    "contact_phone": r.contact_phone,
                    "remark": r.remark,
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

    async fn purchase_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：两路查询失败均 unwrap_or_default 返回空
        let supplier_rows = self.repo.summary_by_supplier(start_date, end_date).await.unwrap_or_default();
        let product_rows = self.repo.summary_by_product(start_date, end_date).await.unwrap_or_default();

        let by_supplier: Vec<serde_json::Value> = supplier_rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.name,
                    "quantity": r.quantity,
                    "amount": r.amount,
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
                    "amount": r.amount,
                })
            })
            .collect();

        let result = serde_json::json!({
            "by_supplier": by_supplier,
            "by_product": by_product,
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn price_trend(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let product_id: i64 = match params.get("product_id").and_then(|s| s.parse().ok()) {
            Some(v) => v,
            None => return Err(UseCaseError::BadRequest(serde_json::json!({"error": "missing product_id"}).to_string())),
        };
        let start_date = params.get("start_date").cloned().unwrap_or_default();
        let end_date = params.get("end_date").cloned().unwrap_or_default();

        // 1) 取商品信息（基础单位）——行为保持：查询失败吞错视为不存在 → 404
        let product = self.repo.fetch_product_info(product_id).await.unwrap_or(None);
        if product.is_none() {
            return Err(UseCaseError::NotFound(serde_json::json!({"error": "product not found"}).to_string()));
        }
        let product = product.unwrap();
        let product_name: String = product.name;
        let base_unit: String = product.base_unit;

        // 2) 同一商品可能因别名/规格存在多个 product_id，需要一并查询（仅当同 base_unit 时聚合）
        // 简化处理：只查主 product_id；如 spec 不同则按名称+基础单位匹配
        let same_base_products: Vec<i64> = self
            .repo
            .list_same_base_product_ids(&product_name, &base_unit)
            .await
            .unwrap_or_default();
        if same_base_products.is_empty() {
            return Ok(serde_json::to_string(&serde_json::json!({
                "product_id": product_id,
                "product_name": product_name,
                "base_unit": base_unit,
                "purchase_points": [],
                "selling_points": [],
            })).unwrap());
        }

        // 3) 采购明细：JOIN supplier 取供应商名称，按日聚合加权均价
        // 行为保持：查询失败 unwrap_or_default 返回空
        let purchase_rows = self
            .repo
            .list_trend_purchase(&same_base_products, &start_date, &end_date)
            .await
            .unwrap_or_default();

        // 按日期聚合：加权均价 = sum(base_unit_price * weight) / sum(weight)
        // weight = base_quantity（>0 时），否则 unit==base_unit 时用 quantity
        let mut purchase_by_date: std::collections::HashMap<String, Vec<serde_json::Value>> = std::collections::HashMap::new();
        for r in &purchase_rows {
            let date: String = r.date.clone();
            let unit: String = r.unit.clone();
            let unit_price: f64 = r.price;
            let bqv: Option<f64> = r.bq;
            let qty: Option<f64> = r.qty;
            let amount: Option<f64> = r.amount;
            let supplier_name: String = r.supplier_name.clone();
            let bq_val = bqv.unwrap_or(0.0);
            let qty_val = qty.unwrap_or(0.0);
            let amt_val = amount.unwrap_or(0.0);
            let base_unit_price = if unit == base_unit {
                unit_price
            } else if bq_val > 0.0 {
                if amt_val > 0.0 { amt_val / bq_val } else { unit_price }
            } else {
                continue;
            };
            purchase_by_date.entry(date).or_default().push(serde_json::json!({
                "supplier_name": supplier_name,
                "unit": unit,
                "unit_price": unit_price,
                "base_unit_price": base_unit_price,
                "quantity": qty_val,
                "base_quantity": bq_val,
            }));
        }

        let mut purchase_points: Vec<serde_json::Value> = Vec::new();
        let mut purchase_dates: Vec<String> = purchase_by_date.keys().cloned().collect();
        purchase_dates.sort();
        for date in &purchase_dates {
            let details = purchase_by_date.get(date).unwrap();
            let count = details.len();
            let mut sum_weighted = 0.0;
            let mut sum_weight = 0.0;
            let mut min_price = f64::MAX;
            let mut max_price = f64::MIN;
            for d in details {
                let bp = d["base_unit_price"].as_f64().unwrap_or(0.0);
                let bq = d["base_quantity"].as_f64().unwrap_or(0.0);
                let qty = d["quantity"].as_f64().unwrap_or(0.0);
                let u = d["unit"].as_str().unwrap_or("");
                let weight = if bq > 0.0 { bq } else if u == base_unit { qty } else { 0.0 };
                sum_weighted += bp * weight;
                sum_weight += weight;
                if bp < min_price { min_price = bp; }
                if bp > max_price { max_price = bp; }
            }
            let avg_price = if sum_weight > 0.0 { sum_weighted / sum_weight } else { 0.0 };
            let details_json: Vec<serde_json::Value> = details.iter().map(|d| serde_json::json!({
                "supplier_name": d["supplier_name"],
                "unit": d["unit"],
                "unit_price": d["unit_price"],
                "base_unit_price": d["base_unit_price"],
                "quantity": d["quantity"],
            })).collect();
            purchase_points.push(serde_json::json!({
                "date": date,
                "avg_price": (avg_price * 100.0).round() / 100.0,
                "min_price": min_price,
                "max_price": max_price,
                "count": count,
                "details": details_json,
            }));
        }

        // 4) 售价明细：JOIN purchaser 取采购单位名称，按日聚合加权均价
        // 行为保持：查询失败 unwrap_or_default 返回空
        let selling_rows = self
            .repo
            .list_trend_selling(&same_base_products, &start_date, &end_date)
            .await
            .unwrap_or_default();

        let mut selling_by_date: std::collections::HashMap<String, Vec<serde_json::Value>> = std::collections::HashMap::new();
        for r in &selling_rows {
            let date: String = r.date.clone();
            let unit: String = r.unit.clone();
            let unit_price: f64 = r.price;
            let bqv: Option<f64> = r.bq;
            let qty: Option<f64> = r.qty;
            let amount: Option<f64> = r.amount;
            let purchaser_name: String = r.purchaser_name.clone();
            let bq_val = bqv.unwrap_or(0.0);
            let qty_val = qty.unwrap_or(0.0);
            let amt_val = amount.unwrap_or(0.0);
            let base_unit_price = if unit == base_unit {
                unit_price
            } else if bq_val > 0.0 {
                if amt_val > 0.0 { amt_val / bq_val } else { unit_price }
            } else {
                continue;
            };
            selling_by_date.entry(date).or_default().push(serde_json::json!({
                "purchaser_name": purchaser_name,
                "unit": unit,
                "unit_price": unit_price,
                "base_unit_price": base_unit_price,
                "quantity": qty_val,
                "base_quantity": bq_val,
            }));
        }

        let mut selling_points: Vec<serde_json::Value> = Vec::new();
        let mut selling_dates: Vec<String> = selling_by_date.keys().cloned().collect();
        selling_dates.sort();
        for date in &selling_dates {
            let details = selling_by_date.get(date).unwrap();
            let count = details.len();
            let mut sum_weighted = 0.0;
            let mut sum_weight = 0.0;
            let mut min_price = f64::MAX;
            let mut max_price = f64::MIN;
            for d in details {
                let bp = d["base_unit_price"].as_f64().unwrap_or(0.0);
                let bq = d["base_quantity"].as_f64().unwrap_or(0.0);
                let qty = d["quantity"].as_f64().unwrap_or(0.0);
                let u = d["unit"].as_str().unwrap_or("");
                let weight = if bq > 0.0 { bq } else if u == base_unit { qty } else { 0.0 };
                sum_weighted += bp * weight;
                sum_weight += weight;
                if bp < min_price { min_price = bp; }
                if bp > max_price { max_price = bp; }
            }
            let avg_price = if sum_weight > 0.0 { sum_weighted / sum_weight } else { 0.0 };
            let details_json: Vec<serde_json::Value> = details.iter().map(|d| serde_json::json!({
                "purchaser_name": d["purchaser_name"],
                "unit": d["unit"],
                "unit_price": d["unit_price"],
                "base_unit_price": d["base_unit_price"],
                "quantity": d["quantity"],
            })).collect();
            selling_points.push(serde_json::json!({
                "date": date,
                "avg_price": (avg_price * 100.0).round() / 100.0,
                "min_price": min_price,
                "max_price": max_price,
                "count": count,
                "details": details_json,
            }));
        }

        Ok(serde_json::to_string(&serde_json::json!({
            "product_id": product_id,
            "product_name": product_name,
            "base_unit": base_unit,
            "purchase_points": purchase_points,
            "selling_points": selling_points,
        })).unwrap())
    }

    async fn price_compare(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let product_id: i64 = match params.get("product_id").and_then(|s| s.parse().ok()) {
            Some(v) => v,
            None => return Err(UseCaseError::BadRequest(r#"{"error":"missing product_id"}"#.to_string())),
        };

        // 行为保持：查询失败吞错视为不存在 → 404
        let product_row = self.repo.fetch_product_base(product_id).await.unwrap_or(None);
        let (product_name, base_unit): (String, String) = match product_row {
            Some(r) => (r.name, r.base_unit),
            None => return Err(UseCaseError::NotFound(r#"{"error":"product not found"}"#.to_string())),
        };

        // 1) 历史采购订单明细（行为保持：查询失败 unwrap_or_default 返回空）
        let rows = self.repo.list_price_history(product_id).await.unwrap_or_default();

        // 2) 今日采集价（分供应商）（行为保持：查询失败 unwrap_or_default 返回空）
        let today_rows = self.repo.list_today_supplier_prices(product_id).await.unwrap_or_default();

        // 按供应商聚合：首条=最近采购（DESC），加权均价=sum(base_unit_price*weight)/sum(weight)
        // Vec<(supplier_name, date, unit_price, unit, base_unit_price, weight)>
        let mut by_supplier: std::collections::HashMap<i64, Vec<(String, String, f64, String, f64, f64)>> = std::collections::HashMap::new();
        for r in &rows {
            let sid: i64 = r.supplier_id;
            let sname: String = r.supplier_name.clone();
            let date: String = r.date.clone();
            let unit: String = r.unit.clone();
            let unit_price: f64 = r.price;
            let bqv: Option<f64> = r.bq;
            let qty: Option<f64> = r.qty;
            let amount: Option<f64> = r.amount;
            let bq_val = bqv.unwrap_or(0.0);
            let qty_val = qty.unwrap_or(0.0);
            let amt_val = amount.unwrap_or(0.0);
            let base_unit_price = if unit == base_unit {
                unit_price
            } else if bq_val > 0.0 {
                if amt_val > 0.0 { amt_val / bq_val } else { unit_price }
            } else {
                continue;
            };
            let weight = if bq_val > 0.0 { bq_val } else if unit == base_unit { qty_val } else { 0.0 };
            by_supplier.entry(sid).or_default().push((sname, date, unit_price, unit, base_unit_price, weight));
        }

        // 今日采集价：supplier_id -> (supplier_name, price, updated_at)
        let mut today_map: std::collections::HashMap<i64, (String, f64, String)> = std::collections::HashMap::new();
        for r in &today_rows {
            let sid: i64 = r.supplier_id;
            let sname: String = r.supplier_name.clone();
            let price: f64 = r.price;
            let updated_at: String = r.updated_at.clone();
            today_map.insert(sid, (sname, price, updated_at));
        }

        let mut suppliers: Vec<serde_json::Value> = Vec::new();
        // 合并：有历史订单的供应商 + 仅有今日采集价的供应商
        let mut all_sids: std::collections::HashSet<i64> = by_supplier.keys().cloned().collect();
        for sid in today_map.keys() { all_sids.insert(*sid); }

        for sid in all_sids {
            let (sname, today_price, today_updated) = today_map.get(&sid)
                .map(|(n, p, d)| (n.clone(), *p, d.clone()))
                .unwrap_or_else(|| (String::new(), 0.0, String::new()));
            let sname = if !sname.is_empty() { sname } else {
                by_supplier.get(&sid).and_then(|v| v.first()).map(|e| e.0.clone()).unwrap_or_else(|| format!("供应商{}", sid))
            };

            if let Some(entries) = by_supplier.get(&sid) {
                let first = &entries[0];
                let mut sum_weighted = 0.0;
                let mut sum_weight = 0.0;
                let mut min_price = f64::MAX;
                let mut max_price = f64::MIN;
                for e in entries {
                    sum_weighted += e.4 * e.5;
                    sum_weight += e.5;
                    if e.4 < min_price { min_price = e.4; }
                    if e.4 > max_price { max_price = e.4; }
                }
                let avg = if sum_weight > 0.0 { sum_weighted / sum_weight } else { 0.0 };
                suppliers.push(serde_json::json!({
                    "supplier_id": sid,
                    "supplier_name": sname,
                    "last_date": first.1,
                    "last_unit_price": first.2,
                    "last_unit": first.3,
                    "last_base_unit_price": first.4,
                    "avg_price": (avg * 100.0).round() / 100.0,
                    "min_price": if min_price == f64::MAX { 0.0 } else { min_price },
                    "max_price": if max_price == f64::MIN { 0.0 } else { max_price },
                    "order_count": entries.len(),
                    "today_price": today_price,
                    "today_updated": today_updated,
                }));
            } else {
                // 仅有今日采集价，无历史订单
                suppliers.push(serde_json::json!({
                    "supplier_id": sid,
                    "supplier_name": sname,
                    "last_date": "",
                    "last_unit_price": 0.0,
                    "last_unit": "",
                    "last_base_unit_price": 0.0,
                    "avg_price": 0.0,
                    "min_price": 0.0,
                    "max_price": 0.0,
                    "order_count": 0,
                    "today_price": today_price,
                    "today_updated": today_updated,
                }));
            }
        }

        // 排序：有今日价的优先按今日价升序，否则按加权均价升序
        suppliers.sort_by(|a, b| {
            let pa = if a["today_price"].as_f64().unwrap_or(0.0) > 0.0 {
                a["today_price"].as_f64().unwrap()
            } else {
                a["avg_price"].as_f64().unwrap_or(0.0)
            };
            let pb = if b["today_price"].as_f64().unwrap_or(0.0) > 0.0 {
                b["today_price"].as_f64().unwrap()
            } else {
                b["avg_price"].as_f64().unwrap_or(0.0)
            };
            pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(serde_json::to_string(&serde_json::json!({
            "product_id": product_id,
            "product_name": product_name,
            "base_unit": base_unit,
            "suppliers": suppliers,
        })).unwrap())
    }

    async fn export_price_items(&self, ctx: &UserCtx, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        // 单价/金额为进价信息，仅超级管理员可见：非 super_admin 导出时这两列置空
        let is_super_admin = ctx.role == "super_admin";
        let product_name = params.get("product_name").map(|s| s.as_str()).unwrap_or(""); let supplier_id = params.get("supplier_id").map(|s| s.as_str()).unwrap_or("");
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_price_items_for_export(product_name, supplier_id)
            .await
            .unwrap_or_default();
        let mut workbook = Workbook::new(); let ws = workbook.add_worksheet(); ws.set_name("采购台账").unwrap();
        // 列头：浅灰底、黑色加粗字
        let hf = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0xD9D9D9))
            .set_font_color(Color::Black)
            .set_align(FormatAlign::Center)
            .set_border(FormatBorder::Thin);
        // 打印布局：A4 横版，上下边距为 0，左右预留 2cm 装订区，水平居中（便于双面打印装订）
        ws.set_landscape();
        ws.set_paper_size(9); // 9 = A4
        let margin_cm = 2.0f64 / 2.54; // 2cm 转英寸
        ws.set_margins(margin_cm, margin_cm, 0.0, 0.0, 0.0, 0.0);
        ws.set_print_center_horizontally(true);
        // 页脚居中输出页码：第n页
        ws.set_footer("&C第&[Page]页");
        // 每页重复输出表头大标题行（第1行）与列头行（第3行）
        ws.set_repeat_rows(0, 2).unwrap();
        // 记录行网格边框：居中格式 / 默认左对齐格式
        let grid_center = Format::new().set_border(FormatBorder::Thin).set_align(FormatAlign::Center);
        let grid_left = Format::new().set_border(FormatBorder::Thin);
        // 长文本自动缩小填充：品名规格（左对齐）、供应商（居中）
        let grid_shrink_left = Format::new().set_border(FormatBorder::Thin).set_shrink();
        let grid_shrink_center = Format::new().set_border(FormatBorder::Thin).set_align(FormatAlign::Center).set_shrink();
        // 表头大标题：商品进货台账
        let title_fmt = Format::new().set_bold().set_font_size(20.0).set_align(FormatAlign::Center);
        ws.merge_range(0, 0, 0, 10, "商 品 进 货 台 账", &title_fmt).unwrap();
        // 第2行为空白间隔行（表头与表格间隔一行）
        // 列头（第3行）：进货日期 序号 品名规格 单位 单价 数量 金额 生产日期 保质期 供应商 备注
        // 供应商地址/电话等高度重复信息已移至独立档案维护页，台账不再逐行重复
        let headers_arr = ["进货日期", "序号", "品名规格", "单位", "单价", "数量", "金额", "生产日期", "保质期", "供应商", "备注"];
        for (col, h) in headers_arr.iter().enumerate() {
            let blank = !is_super_admin && (*h == "单价" || *h == "金额");
            ws.write_with_format(2, col as u16, if blank { "" } else { *h }, &hf).unwrap();
        }
        let widths = [11u32, 5, 20, 5, 8, 8, 12, 11, 6, 18, 14];
        for (col, w) in widths.iter().enumerate() { ws.set_column_width(col as u16, *w).unwrap(); }
        // 序号以日期重新计数（每行均为明细记录，逐行编号）
        let mut prev_date = String::new();
        let mut seq: u32 = 0;
        for (i, row) in rows.iter().enumerate() {
            let r = (i + 3) as u32;
            let date = row.order_date.clone();
            if date != prev_date { seq = 1; prev_date = date.clone(); } else { seq += 1; }
            ws.write_with_format(r, 0, date, &grid_center).unwrap();
            ws.write_with_format(r, 1, seq, &grid_center).unwrap();
            ws.write_with_format(r, 2, row.product_name.clone(), &grid_shrink_left).unwrap();
            ws.write_with_format(r, 3, row.unit.clone().unwrap_or_default(), &grid_center).unwrap();
            // 单价/数量/金额：为 0 或空时留白不填 0
            let unit_price = row.unit_price.unwrap_or(0.0);
            if is_super_admin && unit_price != 0.0 {
                ws.write_with_format(r, 4, unit_price, &grid_left).unwrap();
            } else { ws.write_with_format(r, 4, "", &grid_left).unwrap(); }
            let qty = row.quantity.unwrap_or(0.0);
            if qty != 0.0 {
                ws.write_with_format(r, 5, qty, &grid_left).unwrap();
            } else { ws.write_with_format(r, 5, "", &grid_left).unwrap(); }
            let amount = row.amount.unwrap_or(0.0);
            if is_super_admin && amount != 0.0 {
                ws.write_with_format(r, 6, amount, &grid_left).unwrap();
            } else { ws.write_with_format(r, 6, "", &grid_left).unwrap(); }
            ws.write_with_format(r, 7, row.production_date.clone().unwrap_or_default(), &grid_center).unwrap();
            ws.write_with_format(r, 8, row.shelf_life.clone().unwrap_or_default(), &grid_center).unwrap();
            ws.write_with_format(r, 9, row.supplier_name.clone(), &grid_shrink_center).unwrap();
            ws.write_with_format(r, 10, row.remark.clone().unwrap_or_default(), &grid_left).unwrap();
        }
        // 数据不足 25 条时补空行，保证每页固定 25 条明细记录（空行带网格边框）
        const PAGE_SIZE: usize = 25;
        let total_data = rows.len();
        let fill = (PAGE_SIZE - (total_data % PAGE_SIZE)) % PAGE_SIZE;
        for i in 0..fill {
            let r = (total_data + i + 3) as u32;
            for col in 0..11u16 {
                ws.write_with_format(r, col, "", &grid_left).unwrap();
            }
        }
        // 行高：表头行 30，其余（间隔行、列头、数据行、补齐空行）均为 20
        let last_row = (rows.len() + fill) as u32 + 2;
        for r in 0..=last_row {
            ws.set_row_height(r, 20.0).unwrap();
        }
        ws.set_row_height(0, 30.0).unwrap();
        // 每页固定 25 条明细记录（不含列头）：在每满 25 条数据后插入分页符（0 基行号，数据从第 3 行开始）
        // set_page_breaks(x) 表示在 x 行之后分页；第 c 条数据行号 = c+2，故分界 = c+3
        let total = rows.len();
        let breaks: Vec<u32> = (1..)
            .map(|k| k * 25)
            .take_while(|&c| c < total)
            .map(|c| c as u32 + 3)
            .collect();
        if !breaks.is_empty() {
            ws.set_page_breaks(&breaks).unwrap();
        }
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn export_purchase_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or(""); let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        // 行为保持：两路查询失败均 unwrap_or_default 返回空
        let supplier_rows = self.repo.summary_export_by_supplier(start_date, end_date).await.unwrap_or_default();
        let product_rows = self.repo.summary_export_by_product(start_date, end_date).await.unwrap_or_default();
        let mut workbook = Workbook::new(); let hf = common::utils::xlsx_header_format(0x4472C4);
        let ws1 = workbook.add_worksheet(); ws1.set_name("按供应商汇总").unwrap();
        for (col, h) in ["供应商", "采购数量", "采购金额", "平均成本"].iter().enumerate() { ws1.write_with_format(0, col as u16, *h, &hf).unwrap(); }
        ws1.set_column_width(0, 18).unwrap(); ws1.set_column_width(1, 14).unwrap(); ws1.set_column_width(2, 14).unwrap(); ws1.set_column_width(3, 14).unwrap();
        for (i, row) in supplier_rows.iter().enumerate() { let r = (i + 1) as u32; let qty: f64 = row.quantity; let amt: f64 = row.amount; ws1.write(r, 0, row.name.clone()).unwrap(); ws1.write(r, 1, qty).unwrap(); ws1.write(r, 2, amt).unwrap(); ws1.write(r, 3, if qty > 0.0 { amt / qty } else { 0.0 }).unwrap(); }
        let ws2 = workbook.add_worksheet(); ws2.set_name("按商品汇总").unwrap();
        for (col, h) in ["商品名称", "规格", "采购数量", "采购金额", "平均单价"].iter().enumerate() { ws2.write_with_format(0, col as u16, *h, &hf).unwrap(); }
        ws2.set_column_width(0, 20).unwrap(); ws2.set_column_width(1, 14).unwrap(); ws2.set_column_width(2, 14).unwrap(); ws2.set_column_width(3, 14).unwrap(); ws2.set_column_width(4, 14).unwrap();
        for (i, row) in product_rows.iter().enumerate() { let r = (i + 1) as u32; let qty: f64 = row.quantity; let amt: f64 = row.amount; ws2.write(r, 0, row.product_name.clone()).unwrap(); ws2.write(r, 1, row.spec.clone().unwrap_or_default()).unwrap(); ws2.write(r, 2, qty).unwrap(); ws2.write(r, 3, amt).unwrap(); ws2.write(r, 4, if qty > 0.0 { amt / qty } else { 0.0 }).unwrap(); }
        Ok(workbook.save_to_buffer().unwrap())
    }
}
