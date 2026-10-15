use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder};
use std::collections::HashMap;

// ===== 导出通用辅助 =====
pub fn xlsx_header_format(color: u32) -> Format {
    Format::new()
        .set_bold()
        .set_background_color(Color::RGB(color))
        .set_font_color(Color::White)
        .set_align(FormatAlign::Center)
        .set_border(FormatBorder::Thin)
}

pub fn xlsx_response(buf: Vec<u8>, filename: &str) -> axum::response::Response {
    let content_disposition =
        format!("attachment; filename*=UTF-8''{}", urlencode_filename(filename));
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()),
            (header::CONTENT_DISPOSITION, content_disposition),
        ],
        buf,
    )
        .into_response()
}

pub fn urlencode_filename(name: &str) -> String {
    let mut out = String::new();
    for b in name.as_bytes() {
        let c = *b;
        if c.is_ascii_alphanumeric() || c == b'.' || c == b'-' || c == b'_' {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{:02X}", c));
        }
    }
    out
}

pub fn round2(price: f64) -> f64 {
    (price * 100.0).round() / 100.0
}

pub fn parse_keyword_pattern(params: &HashMap<String, String>) -> String {
    match params.get("keyword").filter(|s| !s.is_empty()) {
        Some(k) => format!("%{}%", k),
        None => "%".to_string(),
    }
}

pub fn parse_csv(content: &str) -> Vec<Vec<String>> {
    let mut result = Vec::new();
    let mut current_row = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if let Some(&next) = chars.peek() {
                    if next == '"' {
                        current_field.push('"');
                        chars.next();
                    } else {
                        in_quotes = false;
                    }
                } else {
                    in_quotes = false;
                }
            }
            '"' => {
                in_quotes = true;
            }
            ',' if !in_quotes => {
                current_row.push(current_field);
                current_field = String::new();
            }
            '\n' if !in_quotes => {
                current_row.push(current_field);
                if !current_row.iter().all(|s| s.is_empty()) {
                    result.push(current_row);
                }
                current_row = Vec::new();
                current_field = String::new();
            }
            '\r' => {}
            _ => {
                current_field.push(c);
            }
        }
    }

    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(current_field);
        result.push(current_row);
    }

    result
}

pub fn sanitize_filename_prefix(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed.chars().take(60).collect()
    }
}

// 将图片URL转换为服务器文件路径（兼容旧格式 /api/product/image/ 与新格式 /api/uploads/...）
pub fn image_url_to_path(url: &str) -> Option<String> {
    if let Some(rest) = url.strip_prefix("/api/uploads/") {
        Some(format!("uploads/{}", rest))
    } else if let Some(rest) = url.strip_prefix("/api/product/image/") {
        Some(format!("uploads/{}", rest))
    } else {
        None
    }
}

pub fn operation_action_label(action: &str) -> String {
    let map = [
        ("purchase_order.create", "创建采购单"),
        ("purchase_order.update", "修改采购单"),
        ("purchase_order.delete", "删除采购单"),
        ("purchase_order.approve", "采购单审核"),
        ("purchase_order.unapprove", "采购单反审核"),
        ("purchase_order.import", "导入采购单"),
        ("sales_order.create", "创建销售单"),
        ("sales_order.update", "修改销售单"),
        ("sales_order.delete", "删除销售单"),
        ("sales_order.approve", "销售单审核"),
        ("sales_order.unapprove", "销售单反审核"),
        ("sales_order.update_status", "销售单状态流转"),
        ("sales_order.adjust_price", "销售单调价"),
        ("sales_order.import", "导入销售单"),
        ("sales_order.correction", "批量修正数量"),
        ("sales_order.generate_purchase", "生成采购订单"),
        ("sales_order.upload_image", "上传订单图片"),
        ("sales_order.delete_image", "删除订单图片"),
        ("purchase_document.upload", "上传采购单据"),
        ("purchase_document.delete", "删除采购单据"),
    ];
    for (k, v) in map {
        if action == k {
            return v.to_string();
        }
    }
    action.to_string()
}

/// 校验日期字符串是否为合法 'YYYY-MM-DD'
pub fn is_valid_date_str(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let bytes = s.as_bytes();
    bytes[4] == b'-' && bytes[7] == b'-'
        && bytes[..4].iter().all(|c| c.is_ascii_digit())
        && bytes[5..7].iter().all(|c| c.is_ascii_digit())
        && bytes[8..10].iter().all(|c| c.is_ascii_digit())
}

/// 清理写入 Excel 的字符串：去除 NUL/控制字符（rust_xlsxwriter 内部以 C 字符串写入，
/// 含 NUL 或异常控制字符会导致进程级 panic）。同时将超长名称截断到合理长度。
pub fn sanitize_xlsx_string(s: String) -> String {
    const MAX_LEN: usize = 200;
    let cleaned: String = s
        .chars()
        .filter(|c| {
            let code = *c as u32;
            // 保留可打印 ASCII、常见中文/Unicode 范围；过滤 NUL 与其他控制字符
            code >= 0x20 || code == 0x09 // 允许 tab
        })
        .collect();
    if cleaned.chars().count() > MAX_LEN {
        let mut out: String = cleaned.chars().take(MAX_LEN).collect();
        out.push('…');
        out
    } else {
        cleaned
    }
}
