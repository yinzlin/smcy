//! 财务查询用例：供应商/采购单位往来对账、分摊来源、财务结算、收支流水、毛利明细（含导出）
//!
//! 行为保持：全部查询失败 unwrap_or_default() 返回空（财务结算 count 为 unwrap_or(0)）；
//! workbook 构建 .unwrap() 原样保留。

use std::collections::HashMap;

use domain::UserCtx;
use domain::port::query_finance::QueryFinanceRepo;
use rust_xlsxwriter::Workbook;

use crate::error::UseCaseError;
use crate::port::query_finance::QueryFinanceUseCase;

#[derive(Clone)]
pub struct QueryFinanceService<R: QueryFinanceRepo> {
    repo: R,
}

impl<R: QueryFinanceRepo> QueryFinanceService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: QueryFinanceRepo> QueryFinanceUseCase for QueryFinanceService<R> {
    async fn supplier_balance(&self, ctx: &UserCtx) -> Result<String, UseCaseError> {
        // 行级数据权限：supplier 角色只能看自己绑定的供应商往来
        let (restricted, supplier_id) = if ctx.role == "supplier" { (true, ctx.supplier_id) } else { (false, 0) };
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_supplier_balance(restricted, supplier_id)
            .await
            .unwrap_or_default();

        let balances: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "id": row.id,
                    "name": row.name,
                    "purchase_total": row.purchase_total,
                    "paid_total": 0.0,
                    "unpaid": row.unpaid,
                    "prepay_balance": 0.0,
                })
            })
            .collect();

        Ok(serde_json::to_string(&balances).unwrap())
    }

    async fn export_supplier_balance(&self, ctx: &UserCtx) -> Result<Vec<u8>, UseCaseError> {
        // 行级数据权限：supplier 角色只能导出自己绑定的供应商往来
        let (restricted, supplier_id) = if ctx.role == "supplier" { (true, ctx.supplier_id) } else { (false, 0) };
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_supplier_balance(restricted, supplier_id)
            .await
            .unwrap_or_default();

        let mut workbook = rust_xlsxwriter::Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("供应商往来对账").unwrap();

        let header_format = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0x4472C4))
            .set_font_color(rust_xlsxwriter::Color::White)
            .set_align(rust_xlsxwriter::FormatAlign::Center)
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        let headers = ["供应商名称", "本期进货总额", "已付款", "未付款", "预付款余额"];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_with_format(0, col as u16, *header, &header_format).unwrap();
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let name = &row.name;
            let purchase_total = row.purchase_total;

            worksheet.write(row_idx as u32 + 1, 0, name).unwrap();
            worksheet.write(row_idx as u32 + 1, 1, purchase_total).unwrap();
            worksheet.write(row_idx as u32 + 1, 2, 0.0).unwrap();
            worksheet.write(row_idx as u32 + 1, 3, purchase_total).unwrap();
            worksheet.write(row_idx as u32 + 1, 4, 0.0).unwrap();
        }

        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn purchaser_balance(&self, ctx: &UserCtx) -> Result<String, UseCaseError> {
        // 行级数据权限：purchaser 角色只能看自己绑定的采购单位往来
        let (restricted, purchaser_id) = if ctx.role == "purchaser" { (true, ctx.purchaser_id) } else { (false, 0) };
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_purchaser_balance(restricted, purchaser_id)
            .await
            .unwrap_or_default();

        let balances: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "id": row.id,
                    "name": row.name,
                    "sales_total": row.sales_total,
                    "received_total": 0.0,
                    "unreceived": row.unreceived,
                    "prepay_balance": 0.0,
                })
            })
            .collect();

        Ok(serde_json::to_string(&balances).unwrap())
    }

    async fn export_purchaser_balance(&self, ctx: &UserCtx) -> Result<Vec<u8>, UseCaseError> {
        // 行级数据权限：purchaser 角色只能导出自己绑定的采购单位往来
        let (restricted, purchaser_id) = if ctx.role == "purchaser" { (true, ctx.purchaser_id) } else { (false, 0) };
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_purchaser_balance(restricted, purchaser_id)
            .await
            .unwrap_or_default();

        let mut workbook = rust_xlsxwriter::Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("采购方应收对账").unwrap();

        let header_format = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0x70AD47))
            .set_font_color(rust_xlsxwriter::Color::White)
            .set_align(rust_xlsxwriter::FormatAlign::Center)
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        let headers = ["采购单位名称", "累计销售", "已收款", "未收款", "预收款余额"];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_with_format(0, col as u16, *header, &header_format).unwrap();
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let name = &row.name;
            let sales_total = row.sales_total;

            worksheet.write(row_idx as u32 + 1, 0, name).unwrap();
            worksheet.write(row_idx as u32 + 1, 1, sales_total).unwrap();
            worksheet.write(row_idx as u32 + 1, 2, 0.0).unwrap();
            worksheet.write(row_idx as u32 + 1, 3, sales_total).unwrap();
            worksheet.write(row_idx as u32 + 1, 4, 0.0).unwrap();
        }

        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn allocation_source(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：主查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_allocation_sources(start_date, end_date)
            .await
            .unwrap_or_default();

        let mut result: Vec<serde_json::Value> = Vec::new();
        for r in &rows {
            let source_order_id = r.source_order_id;
            // 查询该来源单分摊到的各目标单及金额
            // 行为保持：嵌套查询失败 unwrap_or_default 返回空
            let targets = self
                .repo
                .list_allocation_targets(source_order_id)
                .await
                .unwrap_or_default();

            let target_list: Vec<serde_json::Value> = targets.iter().map(|t| {
                serde_json::json!({
                    "order_no": t.order_no,
                    "amount": t.amount,
                })
            }).collect();

            result.push(serde_json::json!({
                "source_order_id": source_order_id,
                "order_no": r.order_no,
                "order_date": r.order_date,
                "total_amount": r.total_amount,
                "allocated_amount": r.allocated_amount,
                "remaining_balance": r.remaining_balance,
                "status": r.status,
                "targets": target_list,
            }));
        }

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_allocation_source(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or(""); let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        // 行为保持：主查询失败 unwrap_or_default 返回空
        let rows = self.repo.list_allocation_sources_export(start_date, end_date).await.unwrap_or_default();
        let mut workbook = Workbook::new(); let ws = workbook.add_worksheet(); ws.set_name("分摊来源").unwrap(); let hf = common::utils::xlsx_header_format(0x70AD47);
        for (c,h) in ["来源订单","日期","来源金额","已分摊","剩余","状态","分摊去向"].iter().enumerate() { ws.write_with_format(0,c as u16,*h,&hf).unwrap(); }
        ws.set_column_width(0,18).unwrap(); ws.set_column_width(1,14).unwrap(); ws.set_column_width(2,14).unwrap(); ws.set_column_width(3,14).unwrap(); ws.set_column_width(4,14).unwrap(); ws.set_column_width(5,10).unwrap(); ws.set_column_width(6,30).unwrap();
        let sm = ["未分摊","分摊中","已完成","已终止"];
        for (i,row) in rows.iter().enumerate() { let r=(i+1)as u32; let src_id=row.source_order_id; let st:i64=row.status;
            // 行为保持：嵌套查询失败 unwrap_or_default 返回空
            let tgts=self.repo.list_allocation_targets_export(src_id).await.unwrap_or_default();
            let tstr:String=tgts.iter().map(|t| format!("{}(¥{:.2})",t.order_no,t.amount)).collect::<Vec<_>>().join("、");
            let ss=if(st as usize)<sm.len(){sm[st as usize]}else{"未知"}; ws.write(r,0,&row.order_no).unwrap(); ws.write(r,1,&row.order_date).unwrap(); ws.write(r,2,row.total_amount).unwrap(); ws.write(r,3,row.allocated_amount).unwrap(); ws.write(r,4,row.remaining_balance).unwrap(); ws.write(r,5,ss).unwrap(); ws.write(r,6,&tstr).unwrap(); }
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn finance_settlement(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let keyword = params.get("keyword").map(|s| s.as_str()).unwrap_or("");
        let order_type = params.get("type").map(|s| s.as_str()).unwrap_or(""); // ""=全部, purchase=采购, sales=销售
        let is_settled = params.get("is_settled").map(|s| s.as_str()).unwrap_or(""); // ""=全部, 0=未结, 1=已结
        let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
        let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20);
        let offset = (page - 1) * page_size;
        let kw = format!("%{}%", keyword);

        // 用 UNION ALL 合并采购/销售订单，统一字段名
        // 应付 = total_amount, 下浮 = total_amount * discount_rate / 100, 扣减 = amount_reduction
        let mut where_clauses = Vec::new();

        // 日期筛选
        if !start_date.is_empty() {
            where_clauses.push(format!("order_date >= '{}'", start_date.replace('\'', "''")));
        }
        if !end_date.is_empty() {
            where_clauses.push(format!("order_date <= '{}'", end_date.replace('\'', "''")));
        }
        // 关键字筛选
        if !keyword.is_empty() {
            where_clauses.push(format!("(order_no LIKE '{}' OR party_name LIKE '{}')", kw.replace('\'', "''"), kw.replace('\'', "''")));
        }
        // 结算状态筛选
        if is_settled == "0" || is_settled == "1" {
            where_clauses.push(format!("is_settled = {}", is_settled));
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        // 类型筛选：当 where_sql 为空时加 WHERE，否则用 AND
        let type_filter = if order_type == "purchase" {
            if where_sql.is_empty() { "WHERE order_type = 'purchase'".to_string() } else { "AND order_type = 'purchase'".to_string() }
        } else if order_type == "sales" {
            if where_sql.is_empty() { "WHERE order_type = 'sales'".to_string() } else { "AND order_type = 'sales'".to_string() }
        } else {
            String::new()
        };

        // 统计总数
        // 行为保持：count 查询失败 unwrap_or(0)
        let total_rows = self
            .repo
            .count_finance_settlement(&where_sql, &type_filter)
            .await
            .unwrap_or(0);

        // 查询数据
        // 行为保持：数据查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_finance_settlement(&where_sql, &type_filter, page_size, offset)
            .await
            .unwrap_or_default();

        let mut orders: Vec<serde_json::Value> = Vec::new();
        let mut sum_total = 0.0f64;
        let mut sum_discount = 0.0f64;
        let mut sum_reduction = 0.0f64;
        let mut sum_paid = 0.0f64;
        let mut sum_unpaid = 0.0f64;

        for row in &rows {
            let total_amount = row.total_amount;
            let discount_rate = row.discount_rate;
            let amount_reduction = row.amount_reduction;
            let final_amount = row.final_amount;
            let settled = row.is_settled;
            let discount_amount = total_amount * discount_rate / 100.0;

            let paid = if settled == 1 { final_amount } else { 0.0 };
            let unpaid = if settled == 0 { final_amount } else { 0.0 };

            sum_total += total_amount;
            sum_discount += discount_amount;
            sum_reduction += amount_reduction;
            sum_paid += paid;
            sum_unpaid += unpaid;

            orders.push(serde_json::json!({
                "order_type": row.order_type,
                "source_id": row.source_id,
                "order_no": row.order_no,
                "order_date": row.order_date,
                "party_name": row.party_name,
                "total_amount": total_amount,
                "discount_rate": discount_rate,
                "discount_amount": (discount_amount * 100.0).round() / 100.0,
                "amount_reduction": amount_reduction,
                "final_amount": final_amount,
                "is_settled": settled,
                "paid_amount": (paid * 100.0).round() / 100.0,
                "unpaid_amount": (unpaid * 100.0).round() / 100.0,
            }));
        }

        let result = serde_json::json!({
            "data": orders,
            "page": page,
            "page_size": page_size,
            "total": total_rows,
            "total_pages": (total_rows + page_size - 1) / page_size,
            "summary": {
                "total_amount": (sum_total * 100.0).round() / 100.0,
                "discount_amount": (sum_discount * 100.0).round() / 100.0,
                "reduction_amount": (sum_reduction * 100.0).round() / 100.0,
                "paid_amount": (sum_paid * 100.0).round() / 100.0,
                "unpaid_amount": (sum_unpaid * 100.0).round() / 100.0,
            }
        });
        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn income_expense(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_income_expense(start_date, end_date)
            .await
            .unwrap_or_default();

        let items: Vec<serde_json::Value> = rows.iter().map(|row| {
            serde_json::json!({
                "order_date": row.order_date,
                "type": row.type_name,
                "total_amount": row.total_amount,
                "final_amount": row.final_amount,
                "direction": row.direction,
            })
        }).collect();

        Ok(serde_json::to_string(&items).unwrap())
    }

    async fn export_income_expense(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date=params.get("start_date").map(|s|s.as_str()).unwrap_or("");let end_date=params.get("end_date").map(|s|s.as_str()).unwrap_or("");
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows=self.repo.list_income_expense_export(start_date, end_date).await.unwrap_or_default();
        let mut workbook=Workbook::new();let ws=workbook.add_worksheet();ws.set_name("收支流水").unwrap();let hf=common::utils::xlsx_header_format(0x2E75B6);
        for(c,h)in["日期","类型","方向","订单金额","实付金额"].iter().enumerate(){ws.write_with_format(0,c as u16,*h,&hf).unwrap();}
        ws.set_column_width(0,14).unwrap();ws.set_column_width(1,12).unwrap();ws.set_column_width(2,10).unwrap();ws.set_column_width(3,14).unwrap();ws.set_column_width(4,14).unwrap();
        for(i,row)in rows.iter().enumerate(){let r=(i+1)as u32;ws.write(r,0,&row.order_date).unwrap();ws.write(r,1,&row.type_name).unwrap();ws.write(r,2,&row.direction).unwrap();ws.write(r,3,row.total_amount).unwrap();ws.write(r,4,row.final_amount).unwrap();}
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn profit_detail(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self
            .repo
            .list_profit_detail(start_date, end_date)
            .await
            .unwrap_or_default();

        let items: Vec<serde_json::Value> = rows.iter().map(|row| {
            serde_json::json!({
                "order_no": row.order_no,
                "order_date": row.order_date,
                "product_name": row.product_name,
                "quantity": row.quantity,
                "sale_price": row.sale_price,
                "purchase_price": row.purchase_price,
                "profit": row.profit,
                "sale_amount": row.sale_amount,
                "cost_amount": row.cost_amount,
            })
        }).collect();

        Ok(serde_json::to_string(&items).unwrap())
    }

    async fn export_profit_detail(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date=params.get("start_date").map(|s|s.as_str()).unwrap_or("");let end_date=params.get("end_date").map(|s|s.as_str()).unwrap_or("");
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows=self.repo.list_profit_detail_export(start_date, end_date).await.unwrap_or_default();
        let mut workbook=Workbook::new();let ws=workbook.add_worksheet();ws.set_name("毛利明细").unwrap();let hf=common::utils::xlsx_header_format(0x2E75B6);
        for(c,h)in["订单号","日期","商品名称","数量","销售单价","进货价","销售金额","成本金额","毛利","毛利率"].iter().enumerate(){ws.write_with_format(0,c as u16,*h,&hf).unwrap();}
        ws.set_column_width(0,20).unwrap();ws.set_column_width(1,14).unwrap();ws.set_column_width(2,20).unwrap();ws.set_column_width(3,10).unwrap();ws.set_column_width(4,12).unwrap();ws.set_column_width(5,12).unwrap();ws.set_column_width(6,14).unwrap();ws.set_column_width(7,14).unwrap();ws.set_column_width(8,14).unwrap();ws.set_column_width(9,10).unwrap();
        for(i,row)in rows.iter().enumerate(){let r=(i+1)as u32;let qty=row.quantity;let sp=row.sale_price;let pp=row.purchase_price;let sa=row.sale_amount;let ca=row.cost_amount;let pf=row.profit;let mg=if sa>0.0{pf/sa*100.0}else{0.0};ws.write(r,0,&row.order_no).unwrap();ws.write(r,1,&row.order_date).unwrap();ws.write(r,2,&row.product_name).unwrap();ws.write(r,3,qty).unwrap();ws.write(r,4,sp).unwrap();ws.write(r,5,pp).unwrap();ws.write(r,6,sa).unwrap();ws.write(r,7,ca).unwrap();ws.write(r,8,pf).unwrap();ws.write(r,9,mg).unwrap();}
        Ok(workbook.save_to_buffer().unwrap())
    }
}
