//! React 前端资源内嵌：crates/frontend/dist 编译进二进制，挂 /app，SPA 路由回退 index.html

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

use axum::{
    extract::Path,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
};
use flate2::write::GzEncoder;
use flate2::Compression;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
struct AppAssets;

pub async fn serve_app_index(headers: HeaderMap) -> impl IntoResponse {
    serve_embedded("index.html", &headers)
}

pub async fn serve_app(Path(path): Path<String>, headers: HeaderMap) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return serve_embedded("index.html", &headers);
    }
    if AppAssets::get(path).is_some() {
        return serve_embedded(path, &headers);
    }
    // SPA 回退：仅对无扩展名的页面路径回 index.html，带扩展名的静态资源缺失直接 404
    let looks_like_asset = path.rsplit('/').next().is_some_and(|s| s.contains('.'));
    if looks_like_asset {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    serve_embedded("index.html", &headers)
}

/// /login 302 到 React 登录页（老登录页保留在 /legacy/login 回退）
pub async fn redirect_app_login() -> impl IntoResponse {
    Redirect::temporary("/app/login")
}

/// /product 302 到 React 商品页（老商品页保留在 /legacy/product 回退）
pub async fn redirect_app_product() -> impl IntoResponse {
    Redirect::temporary("/app/product")
}

/// /purchase 302 到 React 采购订单页（老采购页保留在 /legacy/purchase 回退）
pub async fn redirect_app_purchase() -> impl IntoResponse {
    Redirect::temporary("/app/purchase")
}

/// /sales 302 到 React 销售订单页（老销售页保留在 /legacy/sales 回退）
pub async fn redirect_app_sales() -> impl IntoResponse {
    Redirect::temporary("/app/sales")
}

/// /mobile/today_price 302 到 React 今日进价采集页（老页保留在 /legacy/today_price 回退）
pub async fn redirect_app_today_price() -> impl IntoResponse {
    Redirect::temporary("/app/today-price")
}

/// /query/price_schedule 302 到 React 价格策略页（老页保留在 /legacy/price_schedule 回退）
pub async fn redirect_app_price_schedule() -> impl IntoResponse {
    Redirect::temporary("/app/price-schedule")
}

/// 批量定义旧路由 → 新 React 路由的 302 重定向函数
macro_rules! redirect_app {
    ($name:ident, $target:expr) => {
        pub async fn $name() -> impl IntoResponse {
            Redirect::temporary($target)
        }
    };
}

// 管理类页面
redirect_app!(redirect_app_index, "/app");
redirect_app!(redirect_app_supplier, "/app/supplier");
redirect_app!(redirect_app_purchaser, "/app/purchaser");
redirect_app!(redirect_app_warehouse, "/app/warehouse");
redirect_app!(redirect_app_inventory, "/app/inventory");
redirect_app!(redirect_app_supplement, "/app/supplement");
redirect_app!(redirect_app_user, "/app/user");
redirect_app!(redirect_app_system, "/app/system");
redirect_app!(redirect_app_operation_log, "/app/operation-log");
redirect_app!(redirect_app_backup, "/app/backup");
redirect_app!(redirect_app_restore, "/app/restore");

// 数据查询——采购查询
redirect_app!(redirect_app_query_purchase_order, "/app/query/purchase-order");
redirect_app!(redirect_app_query_purchase_document, "/app/query/purchase-document");
redirect_app!(redirect_app_query_purchase_price, "/app/query/purchase-price");
redirect_app!(redirect_app_query_purchase_summary, "/app/query/purchase-summary");
redirect_app!(redirect_app_query_supplier_balance, "/app/query/supplier-balance");
// 数据查询——销售查询
redirect_app!(redirect_app_query_sales_order, "/app/query/sales-order");
redirect_app!(redirect_app_query_sales_summary, "/app/query/sales-summary");
redirect_app!(redirect_app_query_sales_price, "/app/query/sales-price");
redirect_app!(redirect_app_query_purchaser_balance, "/app/query/purchaser-balance");
redirect_app!(redirect_app_query_product_rank, "/app/query/product-rank");
redirect_app!(redirect_app_query_reimburse_summary, "/app/query/reimburse-summary");
redirect_app!(redirect_app_query_allocation_source, "/app/query/allocation-source");
redirect_app!(redirect_app_query_order_adjust, "/app/query/order-adjust");
// 数据查询——库存查询
redirect_app!(redirect_app_query_stock_balance, "/app/query/stock-balance");
redirect_app!(redirect_app_query_stock_flow, "/app/query/stock-flow");
redirect_app!(redirect_app_query_stock_summary, "/app/query/stock-summary");
redirect_app!(redirect_app_query_stock_summary_reimburse, "/app/query/stock-summary-reimburse");
redirect_app!(redirect_app_query_stock_warning, "/app/query/stock-warning");
redirect_app!(redirect_app_query_slow_stock, "/app/query/slow-stock");
// 数据查询——财务查询
redirect_app!(redirect_app_query_income_expense, "/app/query/income-expense");
redirect_app!(redirect_app_query_profit_detail, "/app/query/profit-detail");
redirect_app!(redirect_app_query_finance_settlement, "/app/query/finance-settlement");
// 数据查询——统计报表
redirect_app!(redirect_app_query_overview, "/app/query/overview");
redirect_app!(redirect_app_query_category_stats, "/app/query/category-stats");
redirect_app!(redirect_app_query_document_summary, "/app/query/document-summary");

// 移动端分拣页（老页保留在 /legacy/sort* 回退）
redirect_app!(redirect_app_mobile_sort, "/app/mobile/sort");
redirect_app!(redirect_app_mobile_sort_by_purchaser, "/app/mobile/sort-by-purchaser");
redirect_app!(redirect_app_mobile_sort_by_category, "/app/mobile/sort-by-category");
redirect_app!(redirect_app_mobile_sort_by_supplier, "/app/mobile/sort-by-supplier");
redirect_app!(redirect_app_mobile_sort_comprehensive, "/app/mobile/sort-comprehensive");

/// gzip 压缩结果缓存（按资源路径），内嵌资源内容不可变，可安全长期缓存
fn gzip_cache() -> &'static Mutex<HashMap<String, Arc<Vec<u8>>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<Vec<u8>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn accepts_gzip(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("gzip"))
}

/// 仅对文本类资源压缩（图片/字体等已压缩格式再压缩无收益且会增大体积）
fn is_compressible(mime: &str) -> bool {
    mime.starts_with("text/")
        || mime.contains("javascript")
        || mime.contains("json")
        || mime.contains("xml")
        || mime.contains("svg")
        || mime.contains("css")
}

fn gzip_bytes(path: &str, data: &[u8]) -> Arc<Vec<u8>> {
    let cache = gzip_cache();
    let mut map = cache.lock().unwrap();
    if let Some(hit) = map.get(path) {
        return hit.clone();
    }
    let mut enc = GzEncoder::new(Vec::new(), Compression::best());
    let _ = enc.write_all(data);
    let out = Arc::new(enc.finish().unwrap_or_default());
    map.insert(path.to_string(), out.clone());
    out
}

fn serve_embedded(path: &str, headers: &HeaderMap) -> axum::response::Response {
    match AppAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let mime_str = mime.as_ref();
            // index.html 不缓存保证发版即生效；Vite 产物文件名带 hash，可长缓存
            let cache = if path == "index.html" {
                "no-cache"
            } else {
                "public, max-age=31536000, immutable"
            };
            let use_gzip = accepts_gzip(headers) && is_compressible(mime_str);
            let body = if use_gzip {
                let gz = gzip_bytes(path, content.data.as_ref());
                axum::body::Body::from(gz.as_ref().clone())
            } else {
                match content.data {
                    Cow::Borrowed(b) => axum::body::Body::from(b),
                    Cow::Owned(v) => axum::body::Body::from(v),
                }
            };
            let mut builder = axum::response::Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime_str)
                .header(header::CACHE_CONTROL, cache);
            if use_gzip {
                builder = builder
                    .header(header::CONTENT_ENCODING, "gzip")
                    .header(header::VARY, "Accept-Encoding");
            }
            builder.body(body).unwrap()
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, "frontend not built").into_response(),
    }
}
