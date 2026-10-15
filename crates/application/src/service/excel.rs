use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook, XlsxError};

use common::utils::*;
use domain::port::sales_order::SalesOrderRepo;
use domain::port::user::UserRepo;
use domain::*;
use super::*;

pub async fn build_accept_excel<S: SalesOrderRepo, U: UserRepo>(
    ctx: &UserCtx,
    id: i64,
    reimburse: bool,
    force: bool,
    sales: &S,
    users: &U,
) -> impl IntoResponse {
    // 食材供应人员① 的联系方式：取当前登录用户最近一次保存的"联系方式"（user_account.contact_phone）。
    // 用户未保存过联系方式时为空白，xlsx 中该 cell 保持空。
    // 行为保持：查询失败旧链路吞为空白
    let contact_phone: String = users.get_contact_phone(ctx.user_id).await.unwrap_or_default();

    // 行为保持：查询失败旧 unwrap_or(None) → 订单不存在
    let order_row = sales.accept_excel_order_row(id).await.unwrap_or(None);

    if order_row.is_none() {
        return (StatusCode::NOT_FOUND, "订单不存在").into_response();
    }

    let row = order_row.unwrap();
    let order_no = row.order_no;
    let order_date = row.order_date;
    let discount_rate = row.discount_rate;
    let purchaser_name = row.purchaser_name;

    // 供应商名称与供货车牌号：取自主表字段（前端在新建时默认填充），
    // 主表为空时回退到与前端默认填充完全一致的文本，避免历史数据导出为空白
    // 或与前端显示不一致。
    let supplier_name_raw: Option<String> = row.supplier_company;
    let car_no_raw: Option<String> = row.truck_plate;
    let supplier_name = supplier_name_raw
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "湖南食全味美餐饮管理有限公司".to_string());
    let car_no = car_no_raw
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "湘A·BE9312".to_string());

    // 行为保持：查询失败旧 unwrap_or_default() → 空明细
    let item_rows = sales.accept_excel_item_rows(id).await.unwrap_or_default();

    // 检查是否有金额为零的明细，存在则禁止导出
    if !force {
        let zero_amount_items: Vec<_> = item_rows.iter()
            .filter(|r| r.amount.abs() < 0.001)
            .collect();
        if !zero_amount_items.is_empty() {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                serde_json::to_string(&serde_json::json!({
                    "error": true,
                    "count": zero_amount_items.len(),
                    "message": format!("订单中有 {} 条明细金额为零，不允许导出，请先调整后再试", zero_amount_items.len())
                })).unwrap(),
            ).into_response();
        }
    }

    // 真实口径不合并分摊增项，仅报销口径需要
    // 行为保持：查询失败旧 unwrap_or_default() → 空增项
    let supplement_rows = if reimburse {
        sales.accept_excel_supplement_rows(id).await.unwrap_or_default()
    } else {
        Vec::new()
    };

    use std::collections::HashMap;
    // 元素: (sort_key, food_name, unit, unit_price, quantity, amount, remark, production_date, batch_no, shelf_life)
    let mut item_map: HashMap<i64, (i64, String, String, f64, f64, f64, String, Option<String>, Option<String>, Option<String>)> = HashMap::new();
    // product_id -> 真实明细行 id，用于分摊增项 target_order_item_id 失效时回退匹配
    let mut product_to_key: HashMap<i64, i64> = HashMap::new();
    for r in &item_rows {
        let rid = r.id;
        let pid = r.product_id;
        product_to_key.entry(pid).or_insert(rid);
        let alias2 = r.alias2.clone().unwrap_or_default();
        let product_name = r.product_name.clone();
        let mut spec = r.spec.clone().unwrap_or_default();
        if spec.is_empty() {
            spec = r.product_spec.clone().unwrap_or_default();
        }
        // 导出名称优先级：别称2 -> 规格 -> 名称（仅当别称2与规格都为空时才用名称）
        let food_name = if !alias2.is_empty() {
            alias2
        } else if !spec.is_empty() {
            spec.clone()
        } else {
            product_name
        };
        let unit = r.unit.clone().unwrap_or_default();
        // 打印模板的"规格"列实际为真实订单的"单位"列（件/卷等）
        // 订单明细的 unit 为空时，回退到商品表的基础单位 base_unit
        let unit_for_spec = if !unit.is_empty() {
            unit
        } else {
            r.product_base_unit.clone().unwrap_or_default()
        };
        let original_remark = r.remark.clone().unwrap_or_default();
        // 名称优先级（别称2->规格->名称）三选一独立取值：名称已采用规格时，备注不再重复并入规格
        let remark = if spec.is_empty() || food_name == spec {
            original_remark
        } else if original_remark.is_empty() {
            spec.clone()
        } else {
            format!("{}; {}", spec, original_remark)
        };
        let category_name = r.category_name.clone().unwrap_or_default();
        let parent_name = r.parent_name.clone().unwrap_or_default();
        let sort_key = get_category_sort_key(&category_name, &parent_name);
        let prod_date: Option<String> = r.production_date.clone();
        let batch_no: Option<String> = r.batch_no.clone();
        let shelf_life: Option<String> = r.shelf_life.clone();
        // 政采商品在品名规格前置标记：★=有指导价 ☆=无指导价
        let food_name = match r.gov_status {
            1 => format!("★{}", food_name),
            2 => format!("☆{}", food_name),
            _ => food_name,
        };
        item_map.insert(rid, (sort_key, food_name, unit_for_spec, r.unit_price, r.quantity, r.amount, remark, prod_date, batch_no, shelf_life));
    }

    for r in &supplement_rows {
        let op_type = r.operation_type.clone();
        let target_item_id = r.target_order_item_id;
        let supp_product_id = r.product_id;
        let qty = r.quantity;
        let amt = r.amount;

        // 解析目标明细行 key：优先用 target_order_item_id，失效时用 product_id 回退匹配
        // （销售订单更新会 DELETE+INSERT 导致 sales_order_item.id 变化，旧 target_order_item_id 会失效）
        let resolved_key: Option<i64> = match target_item_id {
            Some(tid) if item_map.contains_key(&tid) => Some(tid),
            _ => product_to_key.get(&supp_product_id).copied(),
        };

        // 替换-冲减：不导出（原被替换商品也需从导出中扣除）
        if op_type == "replace_remove" {
            if let Some(tid) = resolved_key {
                if let Some(entry) = item_map.get_mut(&tid) {
                    let new_qty = entry.4 + qty;
                    let new_amt = entry.5 + amt;
                    // 若原明细被完全冲减（数量或金额归零），从导出中移除
                    if new_qty.abs() < 0.001 || new_amt.abs() < 0.001 {
                        item_map.remove(&tid);
                    } else {
                        *entry = (entry.0, entry.1.clone(), entry.2.clone(), entry.3, new_qty, new_amt, entry.6.clone(), entry.7.clone(), entry.8.clone(), entry.9.clone());
                    }
                }
            }
            continue;
        }

        if op_type == "increase_quantity" {
            if let Some(tid) = resolved_key {
                if let Some(entry) = item_map.get_mut(&tid) {
                    let new_qty = entry.4 + qty;
                    let new_amt = entry.5 + amt;
                    let new_remark = format!("{}（含增项+{}）", entry.6, qty);
                    *entry = (entry.0, entry.1.clone(), entry.2.clone(), entry.3, new_qty, new_amt, new_remark, entry.7.clone(), entry.8.clone(), entry.9.clone());
                }
            }
        } else {
            // new_item 或 replace_add：作为新明细导出，按商品类别归类排序
            let alias2 = r.alias2.clone().unwrap_or_default();
            let product_name = r.product_name.clone();
            let mut spec = r.spec.clone().unwrap_or_default();
            if spec.is_empty() {
                spec = r.product_spec.clone().unwrap_or_default();
            }
            // 导出名称优先级：别称2 -> 规格 -> 名称（仅当别称2与规格都为空时才用名称）
            let food_name = if !alias2.is_empty() {
                alias2
            } else if !spec.is_empty() {
                spec.clone()
            } else {
                product_name
            };
            // 打印模板的"规格"列实际为真实订单的"单位"列（件/卷等）
            // 优先取分摊增项的 unit，为空时回退到商品表的基础单位 base_unit
            let unit = {
                let u = r.unit.clone().unwrap_or_default();
                if !u.is_empty() {
                    u
                } else {
                    r.product_base_unit.clone().unwrap_or_default()
                }
            };
            let category_name = r.category_name.clone().unwrap_or_default();
            let parent_name = r.parent_name.clone().unwrap_or_default();
            let sort_key = get_category_sort_key(&category_name, &parent_name);
            // 名称优先级（别称2->规格->名称）三选一独立取值：名称已采用规格时，备注不再重复并入规格
            let remark = if op_type == "replace_add" {
                // 替换换入：按类别正常导出，不额外标记
                if food_name == spec { String::new() } else { spec.clone() }
            } else {
                // 普通新增增项：保留标记
                let source_remark = r.source_remark.clone().unwrap_or_default();
                if spec.is_empty() || food_name == spec {
                    format!("[增项] {}", source_remark)
                } else {
                    format!("{}; [增项] {}", spec, source_remark)
                }
            };
            // 规格列（C列）填单位（打印模板的"规格"列实际是单位列）
            // 增项无 FIFO 批次快照，生产日期/批号留空；保质期取商品属性（如有）
            let supp_shelf: Option<String> = r.shelf_life.clone();
            // 政采商品在品名规格前置标记：★=有指导价 ☆=无指导价
            let food_name = match r.gov_status {
                1 => format!("★{}", food_name),
                2 => format!("☆{}", food_name),
                _ => food_name,
            };
            item_map.insert(-r.id, (sort_key, food_name, unit, r.unit_price, qty, amt, remark, None, None, supp_shelf));
        }
    }

    // 保留 HashMap key 作为稳定的次级排序依据：
    //   key > 0 = 订单原始明细（key = sales_order_item.id，按录入顺序自增）
    //   key < 0 = 分摊新增/换入明细（key = -order_supplement_item.id）
    // 排序规则：先按类别 sort_key，同类内「原始明细在前(按id升序)、增项在后(按id升序)」。
    // 关键：必须有完全确定的次序，否则 HashMap 迭代随机 → 同一张单每次导出顺序都不同。
    type ItemTuple = (i64, String, String, f64, f64, f64, String, Option<String>, Option<String>, Option<String>);
    let mut items: Vec<(i64, ItemTuple)> = item_map.into_iter().collect();
    items.sort_by(|(ka, va), (kb, vb)| {
        va.0.cmp(&vb.0).then_with(|| {
            let ta = if *ka > 0 { (0, *ka) } else { (1, -*ka) };
            let tb = if *kb > 0 { (0, *kb) } else { (1, -*kb) };
            ta.cmp(&tb)
        })
    });
    let items: Vec<ItemTuple> = items.into_iter().map(|(_, v)| v).collect();

    let accept_total_amount: f64 = items.iter().map(|(_, _, _, _, _, amount, _, _, _, _)| amount).sum();
    let accept_final_amount = accept_total_amount * (1.0 - discount_rate / 100.0);

    let result: Result<Vec<u8>, XlsxError> = (|| {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        worksheet.set_landscape();
        worksheet.set_margins(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        worksheet.set_print_center_vertically(false);
        worksheet.set_print_center_horizontally(true);

        let title_format = Format::new()
            .set_bold()
            .set_font_size(16)
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
            .set_border(FormatBorder::Thin)
            // .set_text_wrap();// 自动换行
            .set_shrink();// 自动缩放

        let cell_right_format = Format::new()
            .set_font_size(10)
            .set_align(FormatAlign::Right)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin);

        let label_format = Format::new()
            .set_font_size(10)
            .set_align(FormatAlign::Right)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin);

        

        

        let money_format = Format::new()
            .set_font_size(10)
            .set_align(FormatAlign::Right)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin)
            .set_num_format("¥#,##0.00");

        let percent_format = Format::new()
            .set_font_size(10)
            .set_align(FormatAlign::Right)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin)
            .set_num_format("0\"%\"");

        let info_format = Format::new()
            .set_font_size(10)
            .set_align(FormatAlign::Left)
            .set_align(FormatAlign::VerticalCenter);

        let col_widths = [4.0, 20.0, 4.0, 6.0, 8.0, 10.0, 10.0, 6.0, 10.0, 10.0, 10.0, 15.0, 10.0];
        for (i, w) in col_widths.iter().enumerate() {
            worksheet.set_column_width(i as u16, *w)?;
        }
        // 右侧联次说明列（竖排文字）：以自动适配宽度模式写入（bestFit），
        // 宽度按竖排单个全角字符 8pt 字身+内边距取 18px
        worksheet.set_column_autofit_width(13, 18)?;

        let headers = [
            "序号".to_string(), "品名规格".to_string(), "单位".to_string(), "数量".to_string(), "单价".to_string(), "总价".to_string(),
            "生产日期\n/批号".to_string(), "保质期".to_string(), "是否有蔬\n菜农残检\n测报告单".to_string(),
            "是否有肉\n类检疫合\n格证".to_string(), "是否异常\n(异味异色)".to_string(),
            "检验情况\n是否合格".to_string(), "备注".to_string(),
        ];

        let items_per_page = 20;
        let total_pages = ((items.len() + items_per_page - 1) / items_per_page) as i32;
        let mut current_row: u32 = 0;

        for page in 0..total_pages {
            let page_title_row = current_row;
            let info_row = current_row + 2;
            let header_row = current_row + 3;
            let first_data_row = current_row + 4;

            worksheet.merge_range(page_title_row, 0, page_title_row, 12, "颍上县公安局机关食堂食材验收单", &title_format)?;
            worksheet.set_row_height(page_title_row, 30)?;

            worksheet.write_with_format(info_row, 0, format!("供应商名称：{}", supplier_name), &info_format)?;
            worksheet.write_with_format(info_row, 6, format!("供货车牌号：{}", car_no), &info_format)?;
            worksheet.write_with_format(info_row, 10, format!("供货时间：{}", order_date), &info_format)?;

            for (i, h) in headers.iter().enumerate() {
                worksheet.write_with_format(header_row, i as u16, h, &header_format)?;
            }
            worksheet.set_row_height(header_row, 35)?;

            let start_idx = page as usize * items_per_page;
            let end_idx = std::cmp::min(start_idx + items_per_page, items.len());
            current_row = first_data_row;

            for (item_idx, (_sort_key, food_name, spec, unit_price, quantity, amount, remark, prod_date, batch_no, shelf_life)) in items[start_idx..end_idx].iter().enumerate() {
                let seq_num = (start_idx + item_idx + 1) as f64;
                worksheet.write_with_format(current_row, 0, seq_num, &cell_format)?;
                worksheet.write_with_format(current_row, 1, food_name, &cell_left_format)?;
                worksheet.write_with_format(current_row, 2, spec, &cell_format)?;
                worksheet.write_with_format(current_row, 3, *quantity, &cell_right_format)?;
                worksheet.write_with_format(current_row, 4, *unit_price, &money_format)?;
                worksheet.write_with_format(current_row, 5, *amount, &money_format)?;
                // 生产日期/批号：确认验收时按 FIFO 写入的快照（有日期带批号，只有一项写一项）
                let prod_cell = match (prod_date.as_deref().map(str::trim).filter(|s| !s.is_empty()), batch_no.as_deref().map(str::trim).filter(|s| !s.is_empty())) {
                    (Some(p), Some(b)) => format!("{}/{}", p, b),
                    (Some(p), None) => p.to_string(),
                    (None, Some(b)) => b.to_string(),
                    (None, None) => String::new(),
                };
                worksheet.write_with_format(current_row, 6, prod_cell, &cell_format)?;
                // 保质期：商品属性（如有）
                worksheet.write_with_format(current_row, 7, shelf_life.as_deref().unwrap_or(""), &cell_format)?;
                worksheet.write_with_format(current_row, 8, "□有 □无", &cell_format)?;
                worksheet.write_with_format(current_row, 9, "□有 □无", &cell_format)?;
                worksheet.write_with_format(current_row, 10, "□有 □无", &cell_format)?;
                worksheet.write_with_format(current_row, 11, "□合格 □不合格", &cell_format)?;
                worksheet.write_with_format(current_row, 12, remark, &cell_left_format)?;

                current_row += 1;
            }

            let blank_rows = (first_data_row + items_per_page as u32 - current_row) as i32;
            for _ in 0..blank_rows {
                for col in 0..13u16 {
                    worksheet.write_with_format(current_row, col, "", &cell_format)?;
                }
                current_row += 1;
            }

            worksheet.merge_range(current_row, 0, current_row, 2, "合计总价：", &label_format)?;
            worksheet.merge_range(current_row, 3, current_row, 5, "", &cell_format)?;
            let purchaser_start_row = current_row;
            let purchaser_label_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Right)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);
            let purchaser_name_format = Format::new()
                .set_font_size(10)
                .set_bold()
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            worksheet.write_with_format(current_row, 3, accept_total_amount, &money_format)?;
            current_row += 1;

            worksheet.merge_range(current_row, 0, current_row, 2, "下浮率：", &label_format)?;
            worksheet.merge_range(current_row, 3, current_row, 5, "", &cell_format)?;
            worksheet.write_with_format(current_row, 3, discount_rate, &percent_format)?;
            current_row += 1;

            worksheet.merge_range(current_row, 0, current_row, 2, "下浮后总价：", &label_format)?;
            worksheet.merge_range(current_row, 3, current_row, 5, "", &cell_format)?;
            worksheet.write_with_format(current_row, 3, accept_final_amount, &money_format)?;
            current_row += 1;

            worksheet.merge_range(purchaser_start_row, 6, purchaser_start_row + 2, 8, "收货单位：", &purchaser_label_format)?;
            worksheet.merge_range(purchaser_start_row, 9, purchaser_start_row + 2, 12, &purchaser_name, &purchaser_name_format)?;

            let forbid_items = vec![
                "禁止采购以下食材：",
                "1、有毒、有害、腐败变质、酸败、霉变、生虫、污秽不洁、混有异物或者其他感官性状异常的食品；",
                "2、无检验检疫合格证明的肉类食品，已过保质期二分之一时间及其他不符合食品标签规定的定型包装食品；",
                "3、无卫生许可证的食品生产经营者供应的食品；",
                "4、禁止采购供应河豚、毛蚶、小海螺等高风险水产品及三文鱼、醉虾、醉蟹等生食水产品；",
                "5、禁止采购散装馅料、肉串及散热熟食制品、卤制品、腌肉、发芽土豆等食品，严禁采购加工制作的豆角（四季豆等）；",
                "6、建议时令蔬菜、瓜果和价格中低档的肉类食品，严禁采购高档食材和反季节蔬菜、瓜果。",
            ];
            for (idx, item) in forbid_items.iter().enumerate() {
                let mut format = Format::new()
                    .set_font_size(8)
                    .set_align(FormatAlign::Left)
                    .set_align(FormatAlign::VerticalCenter);
                if idx == 0 {
                    format = format.set_border_top(FormatBorder::Thin).set_border_left(FormatBorder::Thin).set_border_right(FormatBorder::Thin);
                } else if idx == forbid_items.len() - 1 {
                    format = format.set_border_left(FormatBorder::Thin).set_border_right(FormatBorder::Thin).set_border_bottom(FormatBorder::Thin);
                } else {
                    format = format.set_border_left(FormatBorder::Thin).set_border_right(FormatBorder::Thin);
                }
                worksheet.set_row_height(current_row, 10)?;
                worksheet.merge_range(current_row, 0, current_row, 12, item, &format)?;
                current_row += 1;
            }

            for sig_row in 0..3 {
                let is_first = sig_row == 0;
                let is_last = sig_row == 2;
                
                let mut label_fmt = Format::new()
                    .set_font_size(10)
                    .set_align(FormatAlign::Left)
                    .set_align(FormatAlign::VerticalCenter);
                let mut contact_fmt = Format::new()
                    .set_font_size(10)
                    .set_align(FormatAlign::Right)
                    .set_align(FormatAlign::VerticalCenter);
                let mut supplier_fmt = Format::new()
                    .set_font_size(10)
                    .set_align(FormatAlign::Right)
                    .set_align(FormatAlign::VerticalCenter);
                let mut cell_fmt = Format::new()
                    .set_font_size(10)
                    .set_align(FormatAlign::Left)
                    .set_align(FormatAlign::VerticalCenter);
                let mut last_cell_fmt = Format::new()
                    .set_font_size(10)
                    .set_align(FormatAlign::Left)
                    .set_align(FormatAlign::VerticalCenter)
                    .set_border_right(FormatBorder::Thin);

                if is_first {
                    label_fmt = label_fmt.set_border_top(FormatBorder::Thin).set_border_left(FormatBorder::Thin);
                    contact_fmt = contact_fmt.set_border_top(FormatBorder::Thin);
                    supplier_fmt = supplier_fmt.set_border_top(FormatBorder::Thin);
                    cell_fmt = cell_fmt.set_border_top(FormatBorder::Thin);
                    last_cell_fmt = last_cell_fmt.set_border_top(FormatBorder::Thin);
                } else {
                    label_fmt = label_fmt.set_border_left(FormatBorder::Thin);
                }
                if is_last {
                    label_fmt = label_fmt.set_border_bottom(FormatBorder::Thin);
                    contact_fmt = contact_fmt.set_border_bottom(FormatBorder::Thin);
                    supplier_fmt = supplier_fmt.set_border_bottom(FormatBorder::Thin);
                    cell_fmt = cell_fmt.set_border_bottom(FormatBorder::Thin);
                    last_cell_fmt = last_cell_fmt.set_border_bottom(FormatBorder::Thin);
                }

                let row = current_row;
                worksheet.set_row_height(row, 25)?;
                if sig_row == 0 {
                    worksheet.merge_range(row, 0, row, 1, "食材供应人员①：", &label_fmt)?;
                    worksheet.merge_range(row, 2, row, 3, "联系方式：", &contact_fmt)?;
                    // 食材供应人员①的联系方式：填入当前登录用户最近一次保存的联系方式
                    worksheet.merge_range(row, 4, row, 5, &contact_phone, &cell_fmt)?;
                    worksheet.merge_range(row, 6, row, 7, "公安验收人员①：", &supplier_fmt)?;
                    worksheet.merge_range(row, 8, row, 9, "联系方式：", &contact_fmt)?;
                    worksheet.merge_range(row, 10, row, 12, "", &last_cell_fmt)?;
                } else if sig_row == 1 {
                    worksheet.merge_range(row, 0, row, 1, "食材供应人员②：", &label_fmt)?;
                    worksheet.merge_range(row, 2, row, 3, "联系方式：", &contact_fmt)?;
                    worksheet.merge_range(row, 4, row, 5, "", &cell_fmt)?;
                    worksheet.merge_range(row, 6, row, 7, "公安验收人员②：", &supplier_fmt)?;
                    worksheet.merge_range(row, 8, row, 9, "联系方式：", &contact_fmt)?;
                    worksheet.merge_range(row, 10, row, 12, "", &last_cell_fmt)?;
                } else {
                    worksheet.merge_range(row, 0, row, 1, "食材供应人员③：", &label_fmt)?;
                    worksheet.merge_range(row, 2, row, 3, "联系方式：", &contact_fmt)?;
                    worksheet.merge_range(row, 4, row, 5, "", &cell_fmt)?;
                    worksheet.merge_range(row, 6, row, 7, "厨师①：", &supplier_fmt)?;
                    worksheet.merge_range(row, 8, row, 9, "联系方式：", &contact_fmt)?;
                    worksheet.merge_range(row, 10, row, 12, "", &last_cell_fmt)?;
                }
                current_row += 1;
            }

            let page_info = format!("第{}页，共{}页", page + 1, total_pages);
            let footer_format = Format::new()
                .set_font_size(8)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::Bottom);
            worksheet.merge_range(current_row, 0, current_row, 12, &page_info, &footer_format)?;
            current_row += 1;

            // 页右侧竖排小字：各联功能（空白复写纸用，三联同印，按颜色对号入座）
            // rust_xlsxwriter 0.81 中 set_rotation(270) 内部映射为 xlsx 的 255，即 Excel「文字竖排」
            let copy_note_format = Format::new()
                .set_font_size(8)
                .set_rotation(270)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter);
            worksheet.merge_range(
                page_title_row,
                13,
                current_row - 1,
                13,
                "第一联·存根联　　第二联·客户联　　第三联·财务联",
                &copy_note_format,
            )?;

            if page < total_pages - 1 {
                let _ = worksheet.set_page_breaks(&[current_row]);
            }
        }

        let buf = workbook.save_to_buffer()?;
        Ok(buf)
    })();

    match result {
        Ok(buf) => {
            let filename = if reimburse {
                format!("报销单_{}.xlsx", order_no)
            } else {
                format!("验收单_{}.xlsx", order_no)
            };
            xlsx_response(buf, &filename)
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("生成Excel失败: {}", e)).into_response()
        }
    }
}
