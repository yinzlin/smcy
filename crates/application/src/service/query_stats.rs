//! 统计概览用例：月度进销存概览、品类进销存统计、单据汇总（含导出）
//!
//! 行为保持：全部查询失败 unwrap_or(0.0)/unwrap_or_default() 吞错；
//! workbook 构建 .unwrap() 原样保留；Excel 文件名/响应头由 HTTP 薄适配层拼接。

use std::collections::HashMap;

use domain::port::query_stats::{DocumentSummaryRow, QueryStatsRepo};
use rust_xlsxwriter::Workbook;

use crate::error::UseCaseError;
use crate::port::query_stats::QueryStatsUseCase;

#[derive(Clone)]
pub struct QueryStatsService<R: QueryStatsRepo> {
    repo: R,
}

impl<R: QueryStatsRepo> QueryStatsService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

/// 单据汇总聚合：UNION ALL 采购段/销售段对同一月份可能各出一行，
/// 同月多行按 max 合并（订单数取较大者、金额取较大者），最后按月份升序排序。
fn aggregate_document_rows(rows: &[DocumentSummaryRow]) -> Vec<serde_json::Value> {
    let mut month_map: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();

    for row in rows {
        let m = row.month.clone();
        let purchase_count: i64 = row.purchase_count;
        let purchase_amount: f64 = row.purchase_amount;
        let sales_count: i64 = row.sales_count;
        let sales_amount: f64 = row.sales_amount;

        if let Some(existing) = month_map.get_mut(&m) {
            let current_purchase_count = existing["purchase_count"].as_i64().unwrap_or(0);
            let current_purchase_amount = existing["purchase_amount"].as_f64().unwrap_or(0.0);
            let current_sales_count = existing["sales_count"].as_i64().unwrap_or(0);
            let current_sales_amount = existing["sales_amount"].as_f64().unwrap_or(0.0);

            existing["purchase_count"] = serde_json::json!(std::cmp::max(current_purchase_count, purchase_count));
            existing["purchase_amount"] = serde_json::json!(current_purchase_amount.max(purchase_amount));
            existing["sales_count"] = serde_json::json!(std::cmp::max(current_sales_count, sales_count));
            existing["sales_amount"] = serde_json::json!(current_sales_amount.max(sales_amount));
        } else {
            month_map.insert(m.clone(), serde_json::json!({
                "month": m,
                "purchase_count": purchase_count,
                "purchase_amount": purchase_amount,
                "sales_count": sales_count,
                "sales_amount": sales_amount,
            }));
        }
    }

    let mut result: Vec<serde_json::Value> = month_map.values().cloned().collect();
    result.sort_by(|a, b| a["month"].as_str().unwrap_or("").cmp(b["month"].as_str().unwrap_or("")));
    result
}

impl<R: QueryStatsRepo> QueryStatsUseCase for QueryStatsService<R> {
    async fn overview(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let month = params.get("month").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：查询失败 unwrap_or(0.0) / unwrap_or_default 吞错
        let purchase_total = self.repo.sum_purchase_total(month).await.unwrap_or(0.0);

        let sales_total = self.repo.sum_sales_total(month).await.unwrap_or(0.0);

        let stock_total = self.repo.sum_stock_total().await.unwrap_or(0.0);

        let profit_total = sales_total - purchase_total;

        let purchase_by_supplier_rows = self.repo.list_purchase_by_supplier(month).await.unwrap_or_default();

        let purchase_by_supplier: Vec<serde_json::Value> = purchase_by_supplier_rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "name": row.name,
                    "amount": row.amount,
                    "quantity": row.quantity,
                })
            })
            .collect();

        let sales_by_purchaser_rows = self.repo.list_sales_by_purchaser(month).await.unwrap_or_default();

        let sales_by_purchaser: Vec<serde_json::Value> = sales_by_purchaser_rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "name": row.name,
                    "amount": row.amount,
                    "quantity": row.quantity,
                })
            })
            .collect();

        let result = serde_json::json!({
            "purchase_total": purchase_total,
            "sales_total": sales_total,
            "stock_total": stock_total,
            "profit_total": profit_total,
            "purchase_by_supplier": purchase_by_supplier,
            "sales_by_purchaser": sales_by_purchaser,
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_overview(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let month = params.get("month").map(|s| s.as_str()).unwrap_or("");
        // 行为保持：查询失败 unwrap_or(0.0) / unwrap_or_default 吞错
        let purchase_total = self.repo.sum_purchase_total_export(month).await.unwrap_or(0.0);
        let sales_total = self.repo.sum_sales_total_export(month).await.unwrap_or(0.0);
        let stock_total = self.repo.sum_stock_total_export().await.unwrap_or(0.0);
        let profit_total = sales_total - purchase_total;
        let pur_rows = self.repo.list_purchase_by_supplier_export(month).await.unwrap_or_default();
        let sal_rows = self.repo.list_sales_by_purchaser_export(month).await.unwrap_or_default();
        let mut workbook=Workbook::new(); let hf=common::utils::xlsx_header_format(0x2E75B6);
        let ws1=workbook.add_worksheet(); ws1.set_name("汇总").unwrap();
        for (c,h) in ["指标","金额"].iter().enumerate(){ws1.write_with_format(0,c as u16,*h,&hf).unwrap();} ws1.set_column_width(0,16).unwrap(); ws1.set_column_width(1,16).unwrap();
        ws1.write(1,0,"总进货").unwrap(); ws1.write(1,1,purchase_total).unwrap(); ws1.write(2,0,"总销售").unwrap(); ws1.write(2,1,sales_total).unwrap(); ws1.write(3,0,"库存").unwrap(); ws1.write(3,1,stock_total).unwrap(); ws1.write(4,0,"毛利").unwrap(); ws1.write(4,1,profit_total).unwrap();
        let ws2=workbook.add_worksheet(); ws2.set_name("采购汇总").unwrap();
        for (c,h) in ["供应商","采购金额","数量"].iter().enumerate(){ws2.write_with_format(0,c as u16,*h,&hf).unwrap();} ws2.set_column_width(0,18).unwrap(); ws2.set_column_width(1,14).unwrap(); ws2.set_column_width(2,14).unwrap();
        for (i,row) in pur_rows.iter().enumerate(){let r=(i+1)as u32;ws2.write(r,0,row.name.as_str()).unwrap();ws2.write(r,1,row.amount).unwrap();ws2.write(r,2,row.quantity).unwrap();}
        let ws3=workbook.add_worksheet(); ws3.set_name("销售汇总").unwrap();
        for (c,h) in ["采购单位","销售金额","数量"].iter().enumerate(){ws3.write_with_format(0,c as u16,*h,&hf).unwrap();} ws3.set_column_width(0,18).unwrap(); ws3.set_column_width(1,14).unwrap(); ws3.set_column_width(2,14).unwrap();
        for (i,row) in sal_rows.iter().enumerate(){let r=(i+1)as u32;ws3.write(r,0,row.name.as_str()).unwrap();ws3.write(r,1,row.amount).unwrap();ws3.write(r,2,row.quantity).unwrap();}
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn category_stats(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：查询失败 unwrap_or_default 吞错
        let category_rows = self.repo.list_category_stats(start_date, end_date).await.unwrap_or_default();

        let result: Vec<serde_json::Value> = category_rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "category_name": row.category_name,
                    "purchase_quantity": row.purchase_quantity,
                    "purchase_amount": row.purchase_amount,
                    "sales_quantity": row.sales_quantity,
                    "sales_amount": row.sales_amount,
                    "stock_quantity": row.stock_quantity,
                    "stock_amount": row.stock_amount,
                })
            })
            .collect();

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_category_stats(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date=params.get("start_date").map(|s|s.as_str()).unwrap_or("");let end_date=params.get("end_date").map(|s|s.as_str()).unwrap_or("");
        // 行为保持：查询失败 unwrap_or_default 吞错
        let rows = self.repo.list_category_stats_export(start_date, end_date).await.unwrap_or_default();
        let mut workbook=Workbook::new();let ws=workbook.add_worksheet();ws.set_name("品类统计").unwrap();let hf=common::utils::xlsx_header_format(0x2E75B6);
        for(c,h)in["品类名称","采购数量","采购金额","销售数量","销售金额","库存数量","库存金额","毛利"].iter().enumerate(){ws.write_with_format(0,c as u16,*h,&hf).unwrap();}
        ws.set_column_width(0,16).unwrap();ws.set_column_width(1,12).unwrap();ws.set_column_width(2,12).unwrap();ws.set_column_width(3,12).unwrap();ws.set_column_width(4,12).unwrap();ws.set_column_width(5,12).unwrap();ws.set_column_width(6,12).unwrap();ws.set_column_width(7,12).unwrap();
        for(i,row)in rows.iter().enumerate(){let r=(i+1)as u32;let pa:f64=row.purchase_amount;let sa:f64=row.sales_amount;let mg=sa-pa;ws.write(r,0,row.category_name.as_str()).unwrap();ws.write(r,1,row.purchase_quantity).unwrap();ws.write(r,2,pa).unwrap();ws.write(r,3,row.sales_quantity).unwrap();ws.write(r,4,sa).unwrap();ws.write(r,5,row.stock_quantity).unwrap();ws.write(r,6,row.stock_amount).unwrap();ws.write(r,7,mg).unwrap();}
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn document_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let month = params.get("month").map(|s| s.as_str()).unwrap_or("");

        // 行为保持：查询失败 unwrap_or_default 吞错
        let document_rows = self.repo.list_document_summary(month).await.unwrap_or_default();

        let result = aggregate_document_rows(&document_rows);

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_document_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let month=params.get("month").map(|s|s.as_str()).unwrap_or("");
        // 行为保持：查询失败 unwrap_or_default 吞错
        let rows = self.repo.list_document_summary_export(month).await.unwrap_or_default();
        let result = aggregate_document_rows(&rows);
        let mut workbook=Workbook::new();let ws=workbook.add_worksheet();ws.set_name("单据汇总").unwrap();let hf=common::utils::xlsx_header_format(0x2E75B6);
        for(c,h)in["月份","采购订单数","销售订单数","采购金额","销售金额"].iter().enumerate(){ws.write_with_format(0,c as u16,*h,&hf).unwrap();}
        ws.set_column_width(0,12).unwrap();ws.set_column_width(1,14).unwrap();ws.set_column_width(2,14).unwrap();ws.set_column_width(3,14).unwrap();ws.set_column_width(4,14).unwrap();
        for(i,item)in result.iter().enumerate(){let r=(i+1)as u32;ws.write(r,0,item.get("month").and_then(|v|v.as_str()).unwrap_or("")).unwrap();ws.write(r,1,item.get("purchase_count").and_then(|v|v.as_i64()).unwrap_or(0)).unwrap();ws.write(r,2,item.get("sales_count").and_then(|v|v.as_i64()).unwrap_or(0)).unwrap();ws.write(r,3,item.get("purchase_amount").and_then(|v|v.as_f64()).unwrap_or(0.0)).unwrap();ws.write(r,4,item.get("sales_amount").and_then(|v|v.as_f64()).unwrap_or(0.0)).unwrap();}
        Ok(workbook.save_to_buffer().unwrap())
    }
}
