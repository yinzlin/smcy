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
