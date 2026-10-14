//! 内嵌静态资源（bootstrap / chart.js），编译进二进制，无需外部文件

use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};

const BOOTSTRAP_CSS: &str = include_str!("../static/bootstrap.min.css");
const BOOTSTRAP_JS: &str = include_str!("../static/bootstrap.bundle.min.js");
const CHART_JS: &str = include_str!("../static/chart.umd.min.js");

pub(crate) async fn serve_bootstrap_css() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        BOOTSTRAP_CSS,
    )
}

pub(crate) async fn serve_bootstrap_js() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        BOOTSTRAP_JS,
    )
}

pub(crate) async fn serve_chart_js() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        CHART_JS,
    )
}
