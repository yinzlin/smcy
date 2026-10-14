//! 今日进价采集用例：聚合查询 / 保存联动 / 三种导出（按供应商、A4 三栏、按分类）

use chrono::Local;
use domain::port::today_price::{TodayPriceRepo, TodayPriceSaveItem};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook, XlsxError};

use crate::error::UseCaseError;
use crate::port::today_price::TodayPriceUseCase;

#[derive(Clone)]
pub struct TodayPriceService<R: TodayPriceRepo> {
    repo: R,
}

impl<R: TodayPriceRepo> TodayPriceService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

/// 数字格式化：去尾零（2.00→"2"，2.50→"2.5"）
fn fmt_qty(v: f64) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl<R: TodayPriceRepo> TodayPriceUseCase for TodayPriceService<R> {
    async fn items(&self, date: Option<String>) -> String {
        // 行为保持：查询失败旧 unwrap_or_default → 空数组
        let rows = self.repo.fetch_items(date.as_deref()).await.unwrap_or_default();

        // 供应商 → 商品聚合；product_index 记录 (supplier, product) 在 items vec 中的位置
        let mut supplier_map: std::collections::HashMap<i64, (String, Vec<serde_json::Value>)> = std::collections::HashMap::new();
        let mut product_index: std::collections::HashMap<(i64, i64), usize> = std::collections::HashMap::new();
        // (supplier, product) → (total_base_qty, 单位明细 "数量+单位", 是否存在非基础单位)
        let mut qty_acc: std::collections::HashMap<(i64, i64), (f64, Vec<String>, bool)> = std::collections::HashMap::new();

        for r in &rows {
            let supplier_id = r.supplier_id;
            let supplier_name = r.supplier_name.clone().unwrap_or_else(|| {
                if supplier_id == 0 { "未分配供应商".to_string() } else { format!("供应商{}", supplier_id) }
            });
            let product_id = r.product_id;
            let base_unit = r.base_unit.clone().unwrap_or_default();
            let unit = r.sale_unit.clone();
            let unit_qty: f64 = r.unit_qty.unwrap_or(0.0);
            let base_qty: f64 = r.base_qty.unwrap_or(0.0);

            let entry = supplier_map.entry(supplier_id).or_insert_with(|| (supplier_name.clone(), Vec::new()));
            if !product_index.contains_key(&(supplier_id, product_id)) {
                product_index.insert((supplier_id, product_id), entry.1.len());
                entry.1.push(serde_json::json!({
                    "product_id": product_id,
                    "product_name": r.product_name.clone().unwrap_or_default(),
                    "spec": r.spec.clone().unwrap_or_default(),
                    "base_unit": base_unit,
                    "total_qty": 0.0,
                    "purchase_price": r.purchase_price.unwrap_or(0.0),
                    "max_purchase_price": r.max_purchase_price.unwrap_or(0.0),
                    "min_purchase_price": r.min_purchase_price.unwrap_or(0.0),
                    "supplier_price": r.supplier_price.unwrap_or(0.0),
                }));
            }

            // 单位为空视为基础单位
            let display_unit = if unit.is_empty() { base_unit.clone() } else { unit.clone() };
            let acc = qty_acc.entry((supplier_id, product_id)).or_insert_with(|| (0.0, Vec::new(), false));
            acc.0 += base_qty;
            acc.1.push(format!("{}{}", fmt_qty(unit_qty), display_unit));
            if !unit.is_empty() && unit != base_unit {
                acc.2 = true;
            }
        }

        // 回填 total_qty（基础单位换算合计）与 qty_detail（多单位明细）
        for ((supplier_id, product_id), (total, details, has_non_base)) in &qty_acc {
            if let Some((_, items)) = supplier_map.get_mut(supplier_id) {
                if let Some(&idx) = product_index.get(&(*supplier_id, *product_id)) {
                    if let Some(item) = items.get_mut(idx) {
                        item["total_qty"] = serde_json::json!(((*total) * 100.0).round() / 100.0);
                        if *has_non_base || details.len() > 1 {
                            item["qty_detail"] = serde_json::json!(details.join(" + "));
                        }
                    }
                }
            }
        }

        let mut result: Vec<serde_json::Value> = Vec::new();
        for (supplier_id, (supplier_name, items)) in supplier_map {
            let total_qty: f64 = items.iter().map(|i| i["total_qty"].as_f64().unwrap_or(0.0)).sum();
            result.push(serde_json::json!({
                "supplier_id": supplier_id,
                "supplier_name": supplier_name,
                "items": items,
                "total_quantity": total_qty,
            }));
        }
        result.sort_by(|a, b| a["supplier_name"].as_str().unwrap_or("").cmp(b["supplier_name"].as_str().unwrap_or(""))
            .then_with(|| a["supplier_id"].as_i64().unwrap_or(0).cmp(&b["supplier_id"].as_i64().unwrap_or(0))));

        serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
    }

    async fn save(&self, items: Vec<TodayPriceSaveItem>) -> String {
        let mut updated = 0i64;
        for item in items {
            let pid = item.product_id;
            let sid = item.supplier_id;
            let price = item.price;
            if pid <= 0 || sid <= 0 || price <= 0.0 {
                continue;
            }
            // 进价统一四舍五入保留两位小数
            let price = (price * 100.0).round() / 100.0;

            // 1) 写入分供应商价表（UPSERT，同一商品同一供应商唯一）
            // 行为保持：UPSERT 失败 continue 跳过该条
            if self.repo.upsert_supplier_price(pid, sid, price).await.is_err() {
                continue;
            }

            // 2) 同步更新 product.purchase_price（全局回退价）及 max/min 区间
            // 行为保持：区间快照查询失败吞为 None
            let range = self.repo.get_price_range(pid).await.ok().flatten();
            if let Some((old_purchase, old_max, old_min)) = range {
                let new_max = ((if old_max > 0.0 { old_max.max(price) } else { price }) * 100.0).round() / 100.0;
                let new_min = ((if old_min > 0.0 { old_min.min(price) } else { price }) * 100.0).round() / 100.0;
                // 行为保持：UPDATE 成功才计数 + 记日志 + 触发加成率重算
                if self.repo.update_price_range(pid, price, new_max, new_min).await.is_ok() {
                    updated += 1;
                    crate::service::price::log_price_change(pid, "purchase_price", old_purchase, price, "today_price_collect", None, Some("今日进价采集录入")).await;
                    crate::service::price::recalc_base_price_by_markup(pid, "today_price_collect", None).await;
                }
            }
        }
        serde_json::json!({ "success": true, "message": format!("已保存 {} 个商品的今日进价", updated) }).to_string()
    }

    async fn export_excel(&self, date: Option<String>, print_values: bool) -> Result<(String, Vec<u8>), UseCaseError> {
        // 行为保持：查询失败旧 unwrap_or_default → 空
        let rows = self.repo.fetch_export_rows(date.as_deref()).await.unwrap_or_default();

        let mut supplier_map: std::collections::HashMap<i64, (String, Vec<serde_json::Value>)> = std::collections::HashMap::new();
        for r in &rows {
            let supplier_id = r.supplier_id;
            let supplier_name = r.supplier_name.clone().unwrap_or_else(|| {
                if supplier_id == 0 { "未分配供应商".to_string() } else { format!("供应商{}", supplier_id) }
            });
            let item = serde_json::json!({
                "product_id": r.product_id,
                "product_name": r.product_name.clone().unwrap_or_default(),
                "spec": r.spec.clone().unwrap_or_default(),
                "base_unit": r.base_unit.clone().unwrap_or_default(),
                "total_qty": r.total_qty.unwrap_or(0.0),
                "purchase_price": r.purchase_price.unwrap_or(0.0),
                "max_purchase_price": r.max_purchase_price.unwrap_or(0.0),
                "min_purchase_price": r.min_purchase_price.unwrap_or(0.0),
            });
            let entry = supplier_map.entry(supplier_id).or_insert_with(|| (supplier_name.clone(), Vec::new()));
            entry.1.push(item);
        }

        let has_date = date.is_some();
        let excel_result: Result<Vec<u8>, XlsxError> = (|| {
            let mut workbook = Workbook::new();

            let title_format = Format::new()
                .set_bold()
                .set_font_size(14)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter);

            let header_format = Format::new()
                .set_bold()
                .set_font_size(10)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin)
                .set_text_wrap();

            let cell_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            let cell_left_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            let date_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Right)
                .set_align(FormatAlign::VerticalCenter);

            let grand_total_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#E5E7EB")
                .set_font_color("#374151");

            let col_widths = [4, 16, 6, 8, 8, 8, 8, 8];
            let headers = ["序号", "品名规格", "基本单位", "合计数量", "最高价", "最低价", "最近价", "今日价"];
            let display_date = if has_date {
                date.clone().unwrap_or_default()
            } else {
                Local::now().format("%Y-%m-%d").to_string()
            };

            let max_col = 7u16;

            if supplier_map.is_empty() {
                let worksheet = workbook.add_worksheet();
                worksheet.set_name("无数据")?;
                worksheet.merge_range(0, 0, 0, max_col, "暂无采购数据", &title_format)?;
            } else {
                // 固定排序：供应商按名称排序，保证每次导出 sheet 顺序一致
                let mut supplier_list: Vec<(i64, String, Vec<serde_json::Value>)> = supplier_map
                    .into_iter()
                    .map(|(sid, (sname, sitems))| (sid, sname, sitems))
                    .collect();
                supplier_list.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

                for (supplier_id, supplier_name, items) in supplier_list {
                    let sheet_name: String = supplier_name
                        .chars()
                        .filter(|c| !matches!(c, '\\' | '/' | '?' | '*' | '[' | ']' | ':'))
                        .take(31)
                        .collect();
                    let worksheet = workbook.add_worksheet();
                    worksheet.set_name(sheet_name.as_str())?;

                    worksheet.set_landscape();
                    worksheet.set_margins(0.0, 0.0, 0.4, 0.5, 0.0, 0.2);
                    worksheet.set_print_center_vertically(false);
                    worksheet.set_print_center_horizontally(true);
                    worksheet.set_header("");
                    worksheet.set_footer("&C第 &P 页，共 &N 页");
                    worksheet.set_repeat_rows(0, 2)?;

                    for (i, w) in col_widths.iter().enumerate() {
                        worksheet.set_column_width(i as u16, *w)?;
                    }

                    let mut current_row = 0;
                    let title = format!("{} - 今日进价采集清单", supplier_name);
                    worksheet.merge_range(current_row, 0, current_row, max_col, title.as_str(), &title_format)?;
                    worksheet.set_row_height(current_row, 28)?;
                    current_row += 1;

                    worksheet.merge_range(current_row, 4, current_row, max_col, display_date.as_str(), &date_format)?;
                    worksheet.set_row_height(current_row, 14)?;
                    current_row += 1;

                    for (i, header) in headers.iter().enumerate() {
                        worksheet.write_with_format(current_row, i as u16, *header, &header_format)?;
                    }
                    current_row += 1;

                    let mut grand_total_qty = 0.0;
                    let mut seq = 1i64;
                    for item in &items {
                        // 导出名称与采购单规则对齐：规格 -> 名称（独立取值不拼接）
                        let product_name = item["product_name"].as_str().unwrap_or("");
                        let spec = item["spec"].as_str().unwrap_or("").trim();
                        let name_with_spec = if spec.is_empty() {
                            product_name.to_string()
                        } else {
                            spec.to_string()
                        };
                        let base_unit = item["base_unit"].as_str().unwrap_or("");
                        let total_qty = item["total_qty"].as_f64().unwrap_or(0.0);
                        let max_p = item["max_purchase_price"].as_f64().unwrap_or(0.0);
                        let min_p = item["min_purchase_price"].as_f64().unwrap_or(0.0);
                        let purchase_price = item["purchase_price"].as_f64().unwrap_or(0.0);

                        worksheet.write_with_format(current_row, 0, seq as f64, &cell_format)?;
                        worksheet.write_with_format(current_row, 1, name_with_spec.as_str(), &cell_left_format)?;
                        worksheet.write_with_format(current_row, 2, base_unit, &cell_format)?;
                        worksheet.write_with_format(current_row, 3, total_qty, &cell_format)?;
                        worksheet.write_with_format(current_row, 4, max_p, &cell_format)?;
                        worksheet.write_with_format(current_row, 5, min_p, &cell_format)?;
                        worksheet.write_with_format(current_row, 6, purchase_price, &cell_format)?;
                        if print_values {
                            worksheet.write_with_format(current_row, 7, purchase_price, &cell_format)?;
                        } else {
                            worksheet.write_with_format(current_row, 7, "", &cell_format)?;
                        }
                        worksheet.set_row_height(current_row, 20.0)?;
                        current_row += 1;
                        seq += 1;
                        grand_total_qty += total_qty;
                    }

                    let grand_total = format!("合计数量 {}", grand_total_qty);
                    worksheet.merge_range(current_row, 0, current_row, 6, grand_total.as_str(), &grand_total_format)?;
                    worksheet.set_row_height(current_row, 22)?;
                    let _ = supplier_id;
                }
            }

            let buf = workbook.save_to_buffer()?;
            Ok(buf)
        })();

        match excel_result {
            Ok(buf) => {
                let filename = if has_date {
                    format!("今日进价采集清单_{}.xlsx", date.unwrap_or_default())
                } else {
                    "今日进价采集清单.xlsx".to_string()
                };
                Ok((filename, buf))
            }
            Err(e) => {
                eprintln!("今日进价采集清单导出失败: {}", e);
                Err(UseCaseError::Storage("导出失败".to_string()))
            }
        }
    }

    async fn export_a4(&self, date: Option<String>, print_values: bool) -> Result<(String, Vec<u8>), UseCaseError> {
        // 行为保持：查询失败旧 unwrap_or_default → 空
        let rows = self.repo.fetch_export_rows(date.as_deref()).await.unwrap_or_default();

        // 供应商 → 商品去重结果（与采集页数据一致）
        let mut supplier_map: std::collections::HashMap<i64, (String, Vec<serde_json::Value>)> = std::collections::HashMap::new();
        for r in &rows {
            let supplier_id = r.supplier_id;
            let supplier_name = r.supplier_name.clone().unwrap_or_else(|| {
                if supplier_id == 0 { "未分配供应商".to_string() } else { format!("供应商{}", supplier_id) }
            });
            let item = serde_json::json!({
                "product_id": r.product_id,
                "product_name": r.product_name.clone().unwrap_or_default(),
                "spec": r.spec.clone().unwrap_or_default(),
                "base_unit": r.base_unit.clone().unwrap_or_default(),
                "total_qty": r.total_qty.unwrap_or(0.0),
                "purchase_price": r.purchase_price.unwrap_or(0.0),
                "max_purchase_price": r.max_purchase_price.unwrap_or(0.0),
                "min_purchase_price": r.min_purchase_price.unwrap_or(0.0),
            });
            let entry = supplier_map.entry(supplier_id).or_insert_with(|| (supplier_name.clone(), Vec::new()));
            entry.1.push(item);
        }

        // 供应商按名称排序
        let mut supplier_list: Vec<(i64, String, Vec<serde_json::Value>)> = supplier_map
            .into_iter()
            .map(|(sid, (sname, sitems))| (sid, sname, sitems))
            .collect();
        supplier_list.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

        let has_date = date.is_some();
        let display_date = if has_date { date.clone().unwrap_or_default() } else { Local::now().format("%Y-%m-%d").to_string() };

        // A4 三栏排版：14 列 = 3 个大栏（每栏 4 列：序号/品名规格/单位/今日价）+ 2 个间隔列。
        // 供应商分组排列，每栏可容纳多家供应商（按名称排序依次填充），序号按栏竖向从 1 递增。
        // 每栏有固定行容量，放不下时切到下一栏；超过 3 栏则分 sheet（每 sheet 一页 A4）。
        // 每栏行容量：A4 纵向一页（标题28+日期16+表头22≈66pt，内容行18pt）
        // 可打印高度 ≈ 799pt，40 行内容可完整落在一页内
        const MAX_ROWS_PER_COL: usize = 40;
        const COL_BASE: [u16; 3] = [0, 5, 10];

        let excel_result: Result<Vec<u8>, XlsxError> = (|| {
            let mut workbook = Workbook::new();

            let title_format = Format::new()
                .set_bold()
                .set_font_size(14)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter);

            let date_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter);

            let header_format = Format::new()
                .set_bold()
                .set_font_size(10)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            let supplier_format = Format::new()
                .set_bold()
                .set_font_size(10)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin)
                .set_background_color("#FDE68A");

            let cell_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            let cell_left_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            let cell_right_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Right)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            // 展平商品（保留供应商分组，供应商已按名称排序）
            struct FlatItem {
                supplier: String,
                name_spec: String,
                base_unit: String,
                /// print_values=1 时为价格，否则为 None（今日价留空手写）
                price: Option<f64>,
            }
            let mut flat: Vec<FlatItem> = Vec::new();
            for (_sid, sname, items) in &supplier_list {
                for it in items {
                    let name = it["product_name"].as_str().unwrap_or("");
                    // 导出名称与采购单规则对齐：规格 -> 名称（独立取值不拼接）
                    let spec = it["spec"].as_str().unwrap_or("").trim();
                    let name_spec = if spec.is_empty() { name.to_string() } else { spec.to_string() };
                    let price = if print_values {
                        let raw = it["purchase_price"].as_f64().unwrap_or(0.0);
                        Some((raw * 100.0).round() / 100.0)
                    } else {
                        None
                    };
                    flat.push(FlatItem {
                        supplier: sname.clone(),
                        name_spec,
                        base_unit: it["base_unit"].as_str().unwrap_or("").to_string(),
                        price,
                    });
                }
            }

            if flat.is_empty() {
                let worksheet = workbook.add_worksheet();
                worksheet.set_name("无数据")?;
                worksheet.merge_range(0, 0, 0, 13, "暂无采购数据", &title_format)?;
                let buf = workbook.save_to_buffer()?;
                return Ok(buf);
            }

            // 按供应商分组切块（保持供应商名称排序）；单供应商超过栏容量时拆成多块，
            // 每块最多 MAX_ROWS_PER_COL-1 个商品（留 1 行给供应商标题），避免单栏溢出分页
            let mut blocks: Vec<(String, Vec<&FlatItem>)> = Vec::new();
            for it in &flat {
                if let Some(last) = blocks.last_mut() {
                    if last.0 == it.supplier {
                        last.1.push(it);
                        continue;
                    }
                }
                blocks.push((it.supplier.clone(), vec![it]));
            }
            let mut split_blocks: Vec<(String, Vec<&FlatItem>)> = Vec::new();
            for (sname, items) in blocks {
                for chunk in items.chunks(MAX_ROWS_PER_COL - 1) {
                    split_blocks.push((sname.clone(), chunk.to_vec()));
                }
            }

            // 把供应商块分配到"栏"：每栏容量 MAX_ROWS_PER_COL（含供应商标题行），放不下切下一栏；满 3 栏则分页
            let mut pages: Vec<Vec<(String, Vec<&FlatItem>, u16)>> = Vec::new(); // 每页一组块，元素 (供应商名, 商品, 栏起始列)
            let mut cur_page: Vec<(String, Vec<&FlatItem>, u16)> = Vec::new();
            let mut col_index = 0usize;
            let mut rows_used = 0usize;

            for (sname, items) in split_blocks {
                let need = 1 + items.len(); // 供应商标题行 + 商品行
                if rows_used > 0 && rows_used + need > MAX_ROWS_PER_COL {
                    col_index += 1;
                    rows_used = 0;
                    if col_index >= 3 {
                        pages.push(std::mem::take(&mut cur_page));
                        col_index = 0;
                    }
                }
                cur_page.push((sname.clone(), items, COL_BASE[col_index]));
                rows_used += need;
                if rows_used >= MAX_ROWS_PER_COL {
                    col_index += 1;
                    rows_used = 0;
                    if col_index >= 3 {
                        pages.push(std::mem::take(&mut cur_page));
                        col_index = 0;
                    }
                }
            }
            if !cur_page.is_empty() {
                pages.push(cur_page);
            }

            for (page_idx, blocks_in_sheet) in pages.iter().enumerate() {
                let worksheet = workbook.add_worksheet();
                worksheet.set_name(format!("第{}页", page_idx + 1).as_str())?;

                // A4 纵向、左边距 1cm（约 0.394 英寸），其余边距 0.5cm
                worksheet.set_portrait();
                worksheet.set_paper_size(9); // A4 paper size
                worksheet.set_margins(0.394, 0.2, 0.2, 0.2, 0.2, 0.2);
                worksheet.set_print_center_horizontally(false);
                worksheet.set_header("");
                worksheet.set_footer("&C第 &P 页，共 &N 页");

                // 列宽：每栏 [序号2.5, 品名规格13, 单位4.5, 今日价6.5]，间隔列1.2，总计 14 列
                let col_widths = [2.5, 13.0, 4.5, 6.5, 1.2, 2.5, 13.0, 4.5, 6.5, 1.2, 2.5, 13.0, 4.5, 6.5];
                for (i, w) in col_widths.iter().enumerate() {
                    worksheet.set_column_width(i as u16, *w)?;
                }

                // 标题 + 日期
                worksheet.merge_range(0, 0, 0, 13, "今日进价采集清单", &title_format)?;
                worksheet.set_row_height(0, 28)?;
                worksheet.merge_range(1, 0, 1, 13, display_date.as_str(), &date_format)?;
                worksheet.set_row_height(1, 16)?;

                // 表头（3 组，组间空一列）
                let headers = ["序号", "品名规格", "单位", "今日价"];
                for base in COL_BASE.iter() {
                    for (hi, h) in headers.iter().enumerate() {
                        worksheet.write_with_format(2, base + hi as u16, *h, &header_format)?;
                    }
                }
                worksheet.set_row_height(2, 22)?;

                // 每栏从第 3 行开始独立向下填充，栏内序号连续递增
                for col_idx in 0..3usize {
                    let base = COL_BASE[col_idx];
                    let col_blocks: Vec<&(String, Vec<&FlatItem>, u16)> = blocks_in_sheet.iter().filter(|b| b.2 == base).collect();
                    if col_blocks.is_empty() {
                        continue;
                    }
                    let mut row = 3u32;
                    for (sname, items, _b) in col_blocks {
                        worksheet.merge_range(row, base, row, base + 3, sname.as_str(), &supplier_format)?;
                        worksheet.set_row_height(row, 18)?;
                        row += 1;
                        // 序号按供应商分组重新从 1 编号
                        let mut seq = 1i64;
                        for it in items {
                            worksheet.write_with_format(row, base, seq as f64, &cell_format)?;
                            worksheet.write_with_format(row, base + 1, it.name_spec.as_str(), &cell_left_format)?;
                            worksheet.write_with_format(row, base + 2, it.base_unit.as_str(), &cell_format)?;
                            match it.price {
                                Some(p) => { worksheet.write_with_format(row, base + 3, p, &cell_right_format)?; }
                                None => { worksheet.write_with_format(row, base + 3, "", &cell_right_format)?; }
                            }
                            worksheet.set_row_height(row, 18)?;
                            row += 1;
                            seq += 1;
                        }
                    }
                }
            }

            let buf = workbook.save_to_buffer()?;
            Ok(buf)
        })();

        match excel_result {
            Ok(buf) => {
                let filename = if has_date {
                    format!("今日进价采集清单_A4_{}.xlsx", date.unwrap_or_default())
                } else {
                    "今日进价采集清单_A4.xlsx".to_string()
                };
                Ok((filename, buf))
            }
            Err(e) => {
                eprintln!("今日进价采集A4导出失败: {}", e);
                Err(UseCaseError::Storage("导出失败".to_string()))
            }
        }
    }

    async fn export_by_category(&self, date: Option<String>, print_values: bool) -> Result<(String, Vec<u8>), UseCaseError> {
        // 行为保持：查询失败旧 unwrap_or_default → 空
        let rows = self.repo.fetch_category_rows(date.as_deref()).await.unwrap_or_default();

        // 按分类排序：分类 sort_key 优先，再按商品名
        let mut items: Vec<serde_json::Value> = rows.iter().map(|r| {
            let category_name = r.category_name.clone().unwrap_or_default();
            let parent_name = r.parent_name.clone().unwrap_or_default();
            let sort_key = crate::service::get_category_sort_key(&category_name, &parent_name);
            serde_json::json!({
                "product_id": r.product_id,
                "product_name": r.product_name.clone().unwrap_or_default(),
                "spec": r.spec.clone().unwrap_or_default(),
                "base_unit": r.base_unit.clone().unwrap_or_default(),
                "total_qty": r.total_qty.unwrap_or(0.0),
                "purchase_price": r.purchase_price.unwrap_or(0.0),
                "max_purchase_price": r.max_purchase_price.unwrap_or(0.0),
                "min_purchase_price": r.min_purchase_price.unwrap_or(0.0),
                "category_name": category_name,
                "sort_key": sort_key,
            })
        }).collect();
        items.sort_by(|a, b| {
            let sk = a["sort_key"].as_i64().unwrap_or(999).cmp(&b["sort_key"].as_i64().unwrap_or(999));
            if sk != std::cmp::Ordering::Equal { return sk; }
            let pn = a["product_name"].as_str().unwrap_or("").cmp(b["product_name"].as_str().unwrap_or(""));
            if pn != std::cmp::Ordering::Equal { return pn; }
            a["product_id"].as_i64().unwrap_or(0).cmp(&b["product_id"].as_i64().unwrap_or(0))
        });

        let has_date = date.is_some();
        let excel_result: Result<Vec<u8>, XlsxError> = (|| {
            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet();

            worksheet.set_landscape();
            worksheet.set_margins(0.2, 0.2, 0.2, 0.2, 0.2, 0.2);
            worksheet.set_print_center_vertically(false);
            worksheet.set_print_center_horizontally(true);
            worksheet.set_header("");
            worksheet.set_footer("&C第 &P 页，共 &N 页");
            worksheet.set_repeat_rows(0, 2)?;

            let title_format = Format::new()
                .set_bold()
                .set_font_size(14)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter);

            let header_format = Format::new()
                .set_bold()
                .set_font_size(10)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin)
                .set_text_wrap();

            let cell_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            let cell_left_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            let date_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Right)
                .set_align(FormatAlign::VerticalCenter);

            let col_widths = [4, 16, 6, 8, 8, 8, 8, 8];
            // 第1列为序号，第2列为品名规格，第3列基本单位，第4列合计数量，第5~7列价格，第8列今日价
            let headers = ["序号", "品名规格", "基本单位", "合计数量", "最高价", "最低价", "最近价", "今日价"];
            let display_date = if has_date {
                date.clone().unwrap_or_default()
            } else {
                Local::now().format("%Y-%m-%d").to_string()
            };
            let max_col = 7u16;

            let mut current_row = 0;
            worksheet.merge_range(current_row, 0, current_row, max_col, "今日进价采集清单（按分类）", &title_format)?;
            worksheet.set_row_height(current_row, 28)?;
            current_row += 1;

            worksheet.merge_range(current_row, 4, current_row, max_col, display_date.as_str(), &date_format)?;
            worksheet.set_row_height(current_row, 14)?;
            current_row += 1;

            for (i, header) in headers.iter().enumerate() {
                worksheet.write_with_format(current_row, i as u16, *header, &header_format)?;
            }
            worksheet.set_row_height(current_row, 20)?;
            current_row += 1;

            // 按分类分组打印分类标题
            let mut last_sort_key: i64 = -1;
            let mut seq = 1i64;
            for item in &items {
                let sort_key = item["sort_key"].as_i64().unwrap_or(999);
                if sort_key != last_sort_key {
                    if last_sort_key != -1 {
                        current_row += 1;
                    }
                    let category_name = item["category_name"].as_str().unwrap_or("未分类");
                    worksheet.merge_range(current_row, 0, current_row, max_col, format!("【{}】", category_name).as_str(), &header_format)?;
                    worksheet.set_row_height(current_row, 20)?;
                    current_row += 1;
                    last_sort_key = sort_key;
                }

                // 导出名称与采购单规则对齐：规格 -> 名称（独立取值不拼接）
                let product_name = item["product_name"].as_str().unwrap_or("");
                let spec = item["spec"].as_str().unwrap_or("").trim();
                let name_with_spec = if spec.is_empty() {
                    product_name.to_string()
                } else {
                    spec.to_string()
                };
                let base_unit = item["base_unit"].as_str().unwrap_or("");
                let total_qty = item["total_qty"].as_f64().unwrap_or(0.0);
                let max_p = item["max_purchase_price"].as_f64().unwrap_or(0.0);
                let min_p = item["min_purchase_price"].as_f64().unwrap_or(0.0);
                let purchase_price = item["purchase_price"].as_f64().unwrap_or(0.0);

                worksheet.write_with_format(current_row, 0, seq as f64, &cell_format)?;
                worksheet.write_with_format(current_row, 1, name_with_spec.as_str(), &cell_left_format)?;
                worksheet.write_with_format(current_row, 2, base_unit, &cell_format)?;
                worksheet.write_with_format(current_row, 3, total_qty, &cell_format)?;
                worksheet.write_with_format(current_row, 4, max_p, &cell_format)?;
                worksheet.write_with_format(current_row, 5, min_p, &cell_format)?;
                worksheet.write_with_format(current_row, 6, purchase_price, &cell_format)?;
                if print_values {
                    worksheet.write_with_format(current_row, 7, purchase_price, &cell_format)?;
                } else {
                    worksheet.write_with_format(current_row, 7, "", &cell_format)?;
                }
                worksheet.set_row_height(current_row, 20.0)?;
                current_row += 1;
                seq += 1;
            }

            for (i, w) in col_widths.iter().enumerate() {
                worksheet.set_column_width(i as u16, *w)?;
            }

            let buf = workbook.save_to_buffer()?;
            Ok(buf)
        })();

        match excel_result {
            Ok(buf) => {
                let filename = if has_date {
                    format!("今日进价采集清单_按分类_{}.xlsx", date.unwrap_or_default())
                } else {
                    "今日进价采集清单_按分类.xlsx".to_string()
                };
                Ok((filename, buf))
            }
            Err(e) => {
                eprintln!("今日进价采集清单(按分类)导出失败: {}", e);
                Err(UseCaseError::Storage("导出失败".to_string()))
            }
        }
    }
}
