use axum::{
    http::StatusCode,
    response::IntoResponse,
};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError};
use sqlx::Row;

use database::*;

pub fn build_purchase_order_export_workbook(rows: Vec<sqlx::sqlite::SqliteRow>) -> axum::response::Response {
    let result: Result<Vec<u8>, XlsxError> = (|| {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        let header_format = Format::new()
            .set_bold()
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter);

        // 分组标题（仓库）格式：参照供应商分拣页的采购单位分组行
        let section_format = Format::new()
            .set_bold()
            .set_font_size(11)
            .set_align(FormatAlign::Left)
            .set_align(FormatAlign::VerticalCenter)
            .set_background_color("#E5E7EB")
            .set_font_color("#374151");

        // 分组小计/总计格式：参照图示样式（小计：包装数量 N；总计：灰底加粗）
        let summary_format = Format::new()
            .set_bold()
            .set_font_size(10)
            .set_align(FormatAlign::Left)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin);

        let summary_right_format = Format::new()
            .set_bold()
            .set_font_size(10)
            .set_align(FormatAlign::Right)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin);

        let grand_total_format = Format::new()
            .set_bold()
            .set_font_size(11)
            .set_align(FormatAlign::Left)
            .set_align(FormatAlign::VerticalCenter)
            .set_background_color("#E5E7EB")
            .set_font_color("#374151");

        let grand_total_right_format = Format::new()
            .set_bold()
            .set_font_size(11)
            .set_align(FormatAlign::Right)
            .set_align(FormatAlign::VerticalCenter)
            .set_background_color("#E5E7EB")
            .set_font_color("#374151");

        // 明细按仓库分组：主表不再显示"入库仓库"列，仓库通过分组标题行展示
        let headers = ["订单ID", "订单号", "订单日期", "供应商", "总金额", "下浮率(%)", "下浮后合计", "状态", "备注", "商品名称", "下订名称(别称1)", "配单名称(别称2)", "规格", "单位", "订购数量", "数量", "单价", "基本数量", "金额", "商品备注"];
        for (i, &header) in headers.iter().enumerate() {
            worksheet.write_with_format(0, i as u16, header, &header_format)?;
        }

        let mut row_idx = 1;
        let mut last_item_wh: Option<String> = None;
        let mut group_item_count = 0i64;
        let mut group_amount = 0.0;
        let mut grand_item_count = 0i64;
        let mut grand_amount = 0.0;

        // 每个仓库分组的末尾写一行小计（参照图示样式：仓库名 + 小计: 包装数量 N + 金额）
        let write_group_subtotal = |worksheet: &mut Worksheet, row: &mut u32, wh: &str, item_count: i64, amount: f64| -> Result<(), XlsxError> {
            let wh_label = if wh.is_empty() { "未指定".to_string() } else { wh.to_string() };
            let subtotal = format!("├── {}   小计: 包装数量 {}", wh_label, item_count);
            worksheet.merge_range(*row, 0, *row, 17, subtotal.as_str(), &summary_format)?;
            worksheet.write_with_format(*row, 18, amount, &summary_right_format)?;
            worksheet.write_with_format(*row, 19, "", &summary_format)?;
            worksheet.set_row_height(*row, 18)?;
            *row += 1;
            Ok(())
        };

        for row in rows {
            // 明细仓库作为分组键：仓库变化时插入分组标题行，并给上一分组写小计
            let item_wh: String = row.try_get::<Option<String>, _>("item_warehouse_name").unwrap_or(None).unwrap_or_default().trim().to_string();
            if last_item_wh.as_deref() != Some(item_wh.as_str()) {
                if let Some(prev_wh) = last_item_wh.take() {
                    write_group_subtotal(&mut *worksheet, &mut row_idx, &prev_wh, group_item_count, group_amount)?;
                    grand_item_count += group_item_count;
                    grand_amount += group_amount;
                }
                last_item_wh = Some(item_wh.clone());
                group_item_count = 0;
                group_amount = 0.0;
                let wh_display = if item_wh.is_empty() { "未指定".to_string() } else { item_wh.clone() };
                let section_title = format!("├── {}", wh_display);
                worksheet.merge_range(row_idx, 0, row_idx, 19, section_title.as_str(), &section_format)?;
                worksheet.set_row_height(row_idx, 20)?;
                row_idx += 1;
            }
            group_item_count += 1;
            group_amount += row.get::<Option<f64>, _>("amount").unwrap_or(0.0);

            worksheet.write(row_idx, 0, row.get::<i64, _>("id"))?;
            worksheet.write(row_idx, 1, row.get::<String, _>("order_no"))?;
            worksheet.write(row_idx, 2, row.get::<String, _>("order_date"))?;
            worksheet.write(row_idx, 3, row.get::<String, _>("supplier_name"))?;
            worksheet.write(row_idx, 4, row.get::<f64, _>("total_amount"))?;
            worksheet.write(row_idx, 5, row.get::<f64, _>("discount_rate"))?;
            worksheet.write(row_idx, 6, row.get::<f64, _>("final_amount"))?;
            worksheet.write(row_idx, 7, row.get::<String, _>("status"))?;
            worksheet.write(row_idx, 8, row.get::<Option<String>, _>("remark").unwrap_or_default())?;
            worksheet.write(row_idx, 9, row.get::<Option<String>, _>("product_name").unwrap_or_default())?;
            worksheet.write(row_idx, 10, row.get::<Option<String>, _>("alias1").unwrap_or_default())?;
            worksheet.write(row_idx, 11, row.get::<Option<String>, _>("alias2").unwrap_or_default())?;
            worksheet.write(row_idx, 12, row.get::<Option<String>, _>("spec").unwrap_or_default())?;
            worksheet.write(row_idx, 13, row.get::<Option<String>, _>("unit").unwrap_or_default())?;
            worksheet.write(row_idx, 14, row.get::<Option<f64>, _>("ordered_quantity").unwrap_or(0.0))?;
            worksheet.write(row_idx, 15, row.get::<Option<f64>, _>("quantity").unwrap_or(0.0))?;
            worksheet.write(row_idx, 16, row.get::<Option<f64>, _>("unit_price").unwrap_or(0.0))?;
            worksheet.write(row_idx, 17, row.get::<Option<f64>, _>("base_quantity").unwrap_or(0.0))?;
            worksheet.write(row_idx, 18, row.get::<Option<f64>, _>("amount").unwrap_or(0.0))?;
            worksheet.write(row_idx, 19, row.get::<Option<String>, _>("item_remark").unwrap_or_default())?;
            row_idx += 1;
        }

        // 最后一组的小计 + 全部总计
        if let Some(prev_wh) = last_item_wh.take() {
            write_group_subtotal(&mut *worksheet, &mut row_idx, &prev_wh, group_item_count, group_amount)?;
            grand_item_count += group_item_count;
            grand_amount += group_amount;
        }
        if grand_item_count > 0 {
            let grand_total = format!("总计: 包装数量 {}", grand_item_count);
            worksheet.merge_range(row_idx, 0, row_idx, 17, grand_total.as_str(), &grand_total_format)?;
            worksheet.write_with_format(row_idx, 18, grand_amount, &grand_total_right_format)?;
            worksheet.write_with_format(row_idx, 19, "", &grand_total_format)?;
            worksheet.set_row_height(row_idx, 22)?;
        }

        worksheet.set_column_width(0, 8)?;
        worksheet.set_column_width(1, 18)?;
        worksheet.set_column_width(2, 12)?;
        worksheet.set_column_width(3, 15)?;
        worksheet.set_column_width(4, 10)?;
        worksheet.set_column_width(5, 12)?;
        worksheet.set_column_width(6, 12)?;
        worksheet.set_column_width(7, 10)?;
        worksheet.set_column_width(8, 12)?;
        worksheet.set_column_width(9, 15)?;
        worksheet.set_column_width(10, 15)?;
        worksheet.set_column_width(11, 15)?;
        worksheet.set_column_width(12, 10)?;
        worksheet.set_column_width(13, 8)?;
        worksheet.set_column_width(14, 10)?;
        worksheet.set_column_width(15, 8)?;
        worksheet.set_column_width(16, 8)?;
        worksheet.set_column_width(17, 10)?;
        worksheet.set_column_width(18, 10)?;
        worksheet.set_column_width(19, 15)?;

        workbook.save_to_buffer()
    })();

    match result {
        Ok(data) => (
            StatusCode::OK,
            [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", "attachment; filename=\"purchase_orders.xlsx\""),
            ],
            data,
        ).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("导出失败: {}", e)).into_response(),
    }
}

// 导出采购单（打印模板样式）：单张采购单按打印模板格式导出

#[derive(Debug)]
pub struct PurchaseOrderPrint {
    pub order_no: String,
    pub order_date: String,
    pub total_amount: f64,
    pub amount_reduction: f64,
    pub final_amount: f64,
    pub remark: Option<String>,
    pub supplier_name: Option<String>,
    pub supplier_phone: Option<String>,
    pub supplier_address: Option<String>,
    pub user_name: Option<String>,
    pub handler_phone: Option<String>,
}

#[derive(Debug)]
pub struct PurchaseOrderPrintItem {
    pub warehouse_name: String,
    pub product_name: String,
    pub spec: Option<String>,
    pub unit: Option<String>,
    pub quantity: f64,
    pub unit_price: f64,
    pub amount: f64,
    pub remark: Option<String>,
}

pub struct UserSimple { pub nickname: String, pub phone: String }

pub async fn get_purchase_order_with_items(id: i64) -> Option<(PurchaseOrderPrint, Vec<PurchaseOrderPrintItem>)> {
    let order = sqlx::query_as::<_, (
        String, String, f64, f64, f64, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>, Option<String>,
    )>(
        "SELECT po.order_no, po.order_date, po.total_amount, po.amount_reduction, po.final_amount,
                po.remark,
                s.name, s.phone, s.address,
                u.nickname, po.handler_phone
         FROM purchase_order po
         JOIN supplier s ON po.supplier_id = s.id
         LEFT JOIN user_account u ON po.user_id = u.id
         WHERE po.id = ?"
    )
        .bind(id)
        .fetch_optional(pool())
        .await
        .ok()
        .flatten()?;

    let (order_no, order_date, total_amount, amount_reduction, final_amount, remark,
         supplier_name, supplier_phone, supplier_address, user_name, handler_phone) = order;

    let item_rows = sqlx::query_as::<_, (
        Option<String>, String, Option<String>, Option<String>, f64, f64, f64, Option<String>
    )>(
        "SELECT warehouse_name, product_name, spec, unit, quantity, unit_price, amount, remark FROM purchase_order_item WHERE order_id = ? ORDER BY warehouse_name, id"
    )
        .bind(id)
        .fetch_all(pool())
        .await
        .unwrap_or_default();

    let items: Vec<PurchaseOrderPrintItem> = item_rows
        .into_iter()
        .map(|(warehouse_name, product_name, spec, unit, quantity, unit_price, amount, remark)| {
            PurchaseOrderPrintItem {
                warehouse_name: warehouse_name.unwrap_or_default().trim().to_string(),
                product_name, spec, unit, quantity, unit_price, amount, remark
            }
        })
        .collect();

    Some((
        PurchaseOrderPrint {
            order_no, order_date, total_amount, amount_reduction, final_amount,
            remark, supplier_name, supplier_phone, supplier_address,
            user_name, handler_phone,
        },
        items,
    ))
}
