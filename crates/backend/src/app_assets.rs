//! React 前端资源内嵌：crates/frontend/dist 编译进二进制，挂 /app，SPA 路由回退 index.html

use std::borrow::Cow;

use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Redirect},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
struct AppAssets;

pub async fn serve_app_index() -> impl IntoResponse {
    serve_embedded("index.html")
}

pub async fn serve_app(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return serve_embedded("index.html");
    }
    if AppAssets::get(path).is_some() {
        return serve_embedded(path);
    }
    // SPA 回退：仅对无扩展名的页面路径回 index.html，带扩展名的静态资源缺失直接 404
    let looks_like_asset = path.rsplit('/').next().is_some_and(|s| s.contains('.'));
    if looks_like_asset {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    serve_embedded("index.html")
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

fn serve_embedded(path: &str) -> axum::response::Response {
    match AppAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            // index.html 不缓存保证发版即生效；Vite 产物文件名带 hash，可长缓存
            let cache = if path == "index.html" {
                "no-cache"
            } else {
                "public, max-age=31536000, immutable"
            };
            let body = match content.data {
                Cow::Borrowed(b) => axum::body::Body::from(b),
                Cow::Owned(v) => axum::body::Body::from(v),
            };
            axum::response::Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, cache)
                .body(body)
                .unwrap()
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, "frontend not built").into_response(),
    }
}
