use axum::extract::{Json, Path, Query, Multipart, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use application::error::UseCaseError;
use application::port::audit::BasicAuditUseCase;
use application::port::category::CategoryUseCase;
use application::port::food_accept::FoodAcceptUseCase;
use application::port::inventory::InventoryUseCase;
use application::port::order_allocation::OrderAllocationUseCase;
use application::port::order_supplement::OrderSupplementUseCase;
use application::port::price_schedule::PriceScheduleUseCase;
use application::port::product::ProductUseCase;
use application::port::purchase_document::PurchaseDocumentUseCase;
use application::port::purchase_order::PurchaseOrderUseCase;
use application::port::purchaser::PurchaserUseCase;
use application::port::query_finance::QueryFinanceUseCase;
use application::port::query_purchase::QueryPurchaseUseCase;
use application::port::query_sales::QuerySalesUseCase;
use application::port::query_stock::QueryStockUseCase;
use application::port::query_stats::QueryStatsUseCase;
use application::port::sales_order::SalesOrderUseCase;
use application::port::supplier::SupplierUseCase;
use application::port::system_mgmt::SystemMgmtUseCase;
use application::port::today_price::TodayPriceUseCase;
use application::port::user::UserUseCase;
use application::port::warehouse::WarehouseUseCase;
use crate::di::App;
use bytes::Bytes;
use calamine::{open_workbook_auto_from_rs, Data, Reader};
use chrono::Local;
use domain::port::audit::BasicTable;
use domain::port::price_schedule::ScheduleFilter;
use domain::port::purchase_order::PurchaseOrderListFilter;
use domain::port::purchaser::UpsertPurchaser;
use domain::port::sales_order::SalesOrderListFilter;
use domain::port::supplier::UpsertSupplier;
use domain::port::today_price::TodayPriceSaveItem;
use common::utils::{xlsx_response, parse_keyword_pattern, parse_csv, is_valid_date_str};
use application::service::generate_order_no;
use application::dto::{
    AcceptReq, CategoryRenameReq, CategoryReq, DeleteReq, LoginReq, OrderSupplementItemReq,
    PriceScheduleBatchReq, PriceScheduleReq, ProductPriceReq, ProductReq, ProductUnitReq,
    ProductUpdateReq, PurchaseOrderReq, PurchaserReq, SalesOrderReq, SupplierReq,
    WarehouseCreateReq, WarehouseUpdateReq,
};
use crate::middleware::auth::check_sales_order_access;
use application::service::build_accept_excel;
use serde_json;

// ---- 用例响应统一映射（六边形 HTTP 适配器侧） ----

fn api_message_json(success: bool, message: &str) -> String {
    serde_json::to_string(&application::dto::user::ApiMessage { success, message: message.to_string() }).unwrap()
}

/// UseCaseError → HTTP 响应：Business=200、Unauthorized=401、Storage=500（中文文案逐字节保持）
fn usecase_error_response(e: UseCaseError) -> (StatusCode, String) {
    match e {
        UseCaseError::Business(m) => (StatusCode::OK, api_message_json(false, &m)),
        UseCaseError::BadRequest(m) => (StatusCode::BAD_REQUEST, api_message_json(false, &m)),
        UseCaseError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, api_message_json(false, &m)),
        UseCaseError::Forbidden(m) => (StatusCode::FORBIDDEN, api_message_json(false, &m)),
        UseCaseError::NotFound(m) => (StatusCode::NOT_FOUND, api_message_json(false, &m)),
        UseCaseError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, api_message_json(false, "服务器内部错误")),
        // 逐字节自定义响应（旧接口非标准错误契约）
        UseCaseError::RawResponse(code, body) => (StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), body),
    }
}

/// UseCaseError → 文本契约响应（旧基础数据接口为纯文本响应体，前端 postAction 直读 text 展示）
fn text_error_response(e: UseCaseError) -> (StatusCode, String) {
    match e {
        UseCaseError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
        UseCaseError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
        UseCaseError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
        UseCaseError::NotFound(m) => (StatusCode::NOT_FOUND, m),
        UseCaseError::Storage(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        UseCaseError::Business(m) => (StatusCode::BAD_REQUEST, m),
        // 逐字节自定义响应（旧接口非标准错误契约）
        UseCaseError::RawResponse(code, body) => (StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), body),
    }
}

/// 用例 Ok(T) 序列化为 200 响应体，Err 走统一错误映射
fn ok_or_err<T: serde::Serialize>(r: Result<T, UseCaseError>) -> (StatusCode, String) {
    match r {
        Ok(v) => (StatusCode::OK, serde_json::to_string(&v).unwrap()),
        Err(e) => usecase_error_response(e),
    }
}

pub async fn api_system_config(State(app): State<App>, Json(data): Json<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.system_mgmt.set_config(data).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

/// GET /api/system/config：返回全部配置键值对（供新前端系统参数页回显）
pub async fn api_system_config_get(State(app): State<App>) -> impl IntoResponse {
    match app.system_mgmt.get_config().await {
        // 行为保持：旧 handler 返回 Json(...)，Content-Type: application/json 与响应体逐字节一致
        Ok(s) => (StatusCode::OK, [("Content-Type", "application/json")], s).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

/// GET /api/backup/list：返回备份记录列表（供新前端备份/恢复页）
pub async fn api_backup_list(State(app): State<App>) -> impl IntoResponse {
    match app.system_mgmt.backup_list().await {
        // 行为保持：旧 handler 返回 Json(...)，Content-Type: application/json 与响应体逐字节一致
        Ok(s) => (StatusCode::OK, [("Content-Type", "application/json")], s).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_operation_log_list(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/system/operation_log").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    match app.system_mgmt.operation_log_list(&params).await {
        Ok(s) => (StatusCode::OK, s),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_operation_log_export(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/system/operation_log/export").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    match app.system_mgmt.export_operation_log(&params).await {
        Ok(data) => {
            let filename = format!("操作日志_{}.xlsx", chrono::Local::now().format("%Y%m%d_%H%M%S"));
            xlsx_response(data, &filename)
        }
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_user_list(State(app): State<App>) -> impl IntoResponse {
    ok_or_err(app.user.list_active().await)
}

pub async fn api_user_get(State(app): State<App>, Path(id): Path<i64>) -> impl IntoResponse {
    ok_or_err(app.user.get_detail(id).await)
}

// 获取当前登录用户最近保存的"联系方式"（销售订单主表单输入框的回填值）
pub async fn api_user_get_contact_phone(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    ok_or_err(app.user.get_contact_phone(ctx.user_id).await)
}

// 更新当前登录用户最近保存的"联系方式"。
// 销售订单保存时同步调用：把用户在主表单输入框填的联系方式持久化，
// 下次打开销售订单页 / 新建时由前端 GET 拉回作为默认值。
pub async fn api_user_set_contact_phone(State(app): State<App>, headers: axum::http::HeaderMap, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let phone = data["contact_phone"].as_str().unwrap_or("").trim().to_string();
    ok_or_err(app.user.set_contact_phone(ctx.user_id, phone).await)
}

pub async fn api_user_create(State(app): State<App>, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    ok_or_err(app.user.create(application::dto::user::CreateUserCmd {
        username: data["username"].as_str().unwrap_or("").to_string(),
        password: data["password"].as_str().unwrap_or("").to_string(),
        nickname: data["nickname"].as_str().unwrap_or("").to_string(),
        role: data["role"].as_str().unwrap_or("user").to_string(),
        supplier_id: data["supplier_id"].as_i64().unwrap_or(0),
        purchaser_id: data["purchaser_id"].as_i64().unwrap_or(0),
    }).await)
}

pub async fn api_user_update(State(app): State<App>, Path(id): Path<i64>, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    ok_or_err(app.user.update(application::dto::user::UpdateUserCmd {
        id,
        username: data["username"].as_str().unwrap_or("").to_string(),
        password: data["password"].as_str().unwrap_or("").to_string(),
        nickname: data["nickname"].as_str().unwrap_or("").to_string(),
        role: data["role"].as_str().unwrap_or("").to_string(),
        supplier_id: data["supplier_id"].as_i64().unwrap_or(0),
        purchaser_id: data["purchaser_id"].as_i64().unwrap_or(0),
    }).await)
}

pub async fn api_user_delete(State(app): State<App>, Path(id): Path<i64>) -> impl IntoResponse {
    ok_or_err(app.user.delete(id).await)
}

pub async fn api_user_status(State(app): State<App>, Path(id): Path<i64>, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    ok_or_err(app.user.set_status(id, data["status"].as_i64().unwrap_or(0)).await)
}

pub async fn api_backup(State(app): State<App>) -> impl IntoResponse {
    match app.system_mgmt.backup().await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_backup_download(State(app): State<App>, Path(id): Path<i64>) -> impl IntoResponse {
    match app.system_mgmt.backup_download(id).await {
        Ok(d) => {
            let filename = d.file_name.split('/').last().unwrap_or(&d.file_name);
            let headers = [
                ("Content-Type", "application/octet-stream"),
                ("Content-Disposition", &format!("attachment; filename=\"{}\"", filename)),
            ];
            (StatusCode::OK, headers, d.content).into_response()
        }
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_backup_delete(State(app): State<App>, Path(id): Path<i64>) -> impl IntoResponse {
    match app.system_mgmt.backup_delete(id).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_restore(State(app): State<App>, Path(id): Path<i64>) -> impl IntoResponse {
    match app.system_mgmt.restore(id).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_restore_file(State(app): State<App>, mut multipart: Multipart) -> impl IntoResponse {
    // multipart 字段解析属 HTTP 传输适配（与 parse_import_rows 同例）；临时文件落盘与主库还原在用例层
    let mut file_bytes: Option<bytes::Bytes> = None;

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        if field.name() != Some("file") {
            continue;
        }

        let bytes = match field.bytes().await {
            Ok(b) => b,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("文件读取失败：{}", e)),
        };
        if bytes.is_empty() {
            return (StatusCode::BAD_REQUEST, "文件内容为空".to_string());
        }

        file_bytes = Some(bytes);
        break;
    }

    let bytes = match file_bytes {
        Some(b) => b,
        None => return (StatusCode::BAD_REQUEST, "未找到文件".to_string()),
    };

    match app.system_mgmt.restore_file(bytes.as_ref()).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_inspect_corrupted_items(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/inspect_corrupted_items").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    match app.system_mgmt.inspect_corrupted_items().await {
        Ok(json_str) => (StatusCode::OK, json_str),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_clean_corrupted_items(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/clean_corrupted_items").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    match app.system_mgmt.clean_corrupted_items().await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_clean_invalid_orders(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/clean_invalid_orders").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    match app.system_mgmt.clean_invalid_orders().await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_supplier_export(State(app): State<App>) -> impl IntoResponse {
    match app.supplier.export_excel().await {
        Ok(data) => (
            StatusCode::OK,
            [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", "attachment; filename=\"suppliers.xlsx\""),
            ],
            data,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

/// Excel/CSV 字节流解析为二维表（传输适配逻辑，保留在 backend 层）
async fn parse_import_rows(content: &Bytes) -> Result<Vec<Vec<String>>, (StatusCode, String)> {
    if content.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        let content_vec = content.to_vec();
        let mut workbook = match open_workbook_auto_from_rs(std::io::Cursor::new(content_vec)) {
            Ok(w) => w,
            Err(e) => return Err((StatusCode::BAD_REQUEST, format!("读取Excel文件失败: {}", e))),
        };
        let sheets = workbook.sheet_names().to_vec();
        if sheets.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Excel文件中没有工作表".to_string()));
        }
        let range = match workbook.worksheet_range(&sheets[0]) {
            Ok(r) => r,
            Err(e) => return Err((StatusCode::BAD_REQUEST, format!("无法读取Excel文件内容: {}", e))),
        };
        Ok(range.rows()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        Data::Empty => "".to_string(),
                        Data::Int(v) => v.to_string(),
                        Data::Float(v) => v.to_string(),
                        Data::String(v) => v.to_string(),
                        Data::Bool(v) => v.to_string(),
                        _ => "".to_string(),
                    })
                    .collect()
            })
            .collect())
    } else {
        let content_str = String::from_utf8_lossy(content).to_string();
        Ok(parse_csv(&content_str))
    }
}

pub async fn api_supplier_import(State(app): State<App>, content: Bytes) -> impl IntoResponse {
    let rows = match parse_import_rows(&content).await {
        Ok(r) => r,
        Err((code, msg)) => return (code, msg).into_response(),
    };
    match app.supplier.import(rows).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_login(State(app): State<App>, Json(data): Json<LoginReq>) -> impl IntoResponse {
    match app.user.login(data.username.clone(), data.password.clone()).await {
        Ok(outcome) => {
            let body = serde_json::to_string(&outcome.body).unwrap();
            axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Set-Cookie", format!("session={}; HttpOnly; Path=/", outcome.session_token))
                .body(axum::body::Body::from(body))
                .unwrap()
        }
        Err(e) => usecase_error_response(e).into_response(),
    }
}

pub async fn api_logout() -> impl IntoResponse {
    let body = serde_json::to_string(&serde_json::json!({
        "success": true,
        "message": "已退出登录"
    })).unwrap();
    
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Set-Cookie", "session=; HttpOnly; Path=/; Max-Age=0")
        .body(axum::body::Body::from(body))
        .unwrap()
}

pub async fn api_login_check(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    let session_token = headers.get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find(|s| s.trim().starts_with("session="))
                .map(|s| s.trim().strip_prefix("session=").unwrap_or(""))
        })
        .unwrap_or("");

    if session_token.is_empty() {
        return (StatusCode::OK, serde_json::to_string(&serde_json::json!({
            "logged_in": false
        })).unwrap());
    }

    let parts: Vec<&str> = session_token.split(':').collect();
    if parts.len() < 2 {
        return (StatusCode::OK, serde_json::to_string(&serde_json::json!({
            "logged_in": false
        })).unwrap());
    }

    let user_id = match parts[0].parse::<i64>() {
        Ok(id) => id,
        Err(_) => {
            return (StatusCode::OK, serde_json::to_string(&serde_json::json!({
                "logged_in": false
            })).unwrap());
        }
    };

    ok_or_err(app.user.login_check(user_id).await)
}

pub async fn api_supplier_list(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let category_id = params.get("category_id").and_then(|v| v.parse::<i64>().ok());
    let keyword_pattern = parse_keyword_pattern(&params);
    let rows = app.supplier.list(category_id, keyword_pattern).await;
    (StatusCode::OK, serde_json::to_string(&rows).unwrap())
}

pub async fn api_supplier_create(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<SupplierReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/supplier/create").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    let cmd = UpsertSupplier {
        name: req.name,
        contact: req.contact,
        phone: req.phone,
        address: req.address,
        business_scope: req.business_scope,
        remark: req.remark,
        category_id: req.category_id,
        is_designated: req.is_designated.unwrap_or(0),
        designated_no: req.designated_no,
        legal_person: req.legal_person,
        credit_code: req.credit_code,
        license_no: req.license_no,
        license_expire: req.license_expire,
        contract_no: req.contract_no,
        contract_start: req.contract_start,
        contract_end: req.contract_end,
    };
    match app.supplier.create(cmd).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_supplier_update(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<SupplierReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/supplier/update").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    let cmd = UpsertSupplier {
        name: req.name,
        contact: req.contact,
        phone: req.phone,
        address: req.address,
        business_scope: req.business_scope,
        remark: req.remark,
        category_id: req.category_id,
        is_designated: req.is_designated.unwrap_or(0),
        designated_no: req.designated_no,
        legal_person: req.legal_person,
        credit_code: req.credit_code,
        license_no: req.license_no,
        license_expire: req.license_expire,
        contract_no: req.contract_no,
        contract_start: req.contract_start,
        contract_end: req.contract_end,
    };
    match app.supplier.update(req.id.unwrap_or(0), cmd).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_supplier_delete(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<DeleteReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/supplier/delete").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    match app.supplier.delete(req.id).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

// ===== 基础数据审核状态管理 =====
// audit_status：pending=待审核，confirmed=已审核
// 审核/反审核仅超级管理员可执行（超级用户始终拥有最高权限），操作记入审计日志
// （set_basic_audit_status / check_basic_not_confirmed 已随六边形化迁移至 application 用例层）

pub async fn api_supplier_approve(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    let id = req["id"].as_i64().unwrap_or(0);
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.audit.set_audit_status(&ctx, BasicTable::Supplier, id, "confirmed", "supplier.approve").await {
        Ok(()) => (StatusCode::OK, "审核成功".to_string()).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}
pub async fn api_supplier_unapprove(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    let id = req["id"].as_i64().unwrap_or(0);
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.audit.set_audit_status(&ctx, BasicTable::Supplier, id, "pending", "supplier.unapprove").await {
        Ok(()) => (StatusCode::OK, "反审核成功".to_string()).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}
pub async fn api_purchaser_approve(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    let id = req["id"].as_i64().unwrap_or(0);
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.audit.set_audit_status(&ctx, BasicTable::Purchaser, id, "confirmed", "purchaser.approve").await {
        Ok(()) => (StatusCode::OK, "审核成功".to_string()).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}
pub async fn api_purchaser_unapprove(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    let id = req["id"].as_i64().unwrap_or(0);
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.audit.set_audit_status(&ctx, BasicTable::Purchaser, id, "pending", "purchaser.unapprove").await {
        Ok(()) => (StatusCode::OK, "反审核成功".to_string()).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}
pub async fn api_product_approve(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    let id = req["id"].as_i64().unwrap_or(0);
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.audit.set_audit_status(&ctx, BasicTable::Product, id, "confirmed", "product.approve").await {
        Ok(()) => (StatusCode::OK, "审核成功".to_string()).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}
pub async fn api_product_unapprove(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    let id = req["id"].as_i64().unwrap_or(0);
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.audit.set_audit_status(&ctx, BasicTable::Product, id, "pending", "product.unapprove").await {
        Ok(()) => (StatusCode::OK, "反审核成功".to_string()).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}
pub async fn api_warehouse_approve(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    let id = req["id"].as_i64().unwrap_or(0);
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.audit.set_audit_status(&ctx, BasicTable::Warehouse, id, "confirmed", "warehouse.approve").await {
        Ok(()) => (StatusCode::OK, "审核成功".to_string()).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}
pub async fn api_warehouse_unapprove(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    let id = req["id"].as_i64().unwrap_or(0);
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.audit.set_audit_status(&ctx, BasicTable::Warehouse, id, "pending", "warehouse.unapprove").await {
        Ok(()) => (StatusCode::OK, "反审核成功".to_string()).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_purchaser_export(State(app): State<App>) -> impl IntoResponse {
    match app.purchaser.export_excel().await {
        Ok(data) => (
            StatusCode::OK,
            [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", "attachment; filename=\"purchasers.xlsx\""),
            ],
            data,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_purchaser_import(State(app): State<App>, content: Bytes) -> impl IntoResponse {
    let rows = match parse_import_rows(&content).await {
        Ok(r) => r,
        Err((code, msg)) => return (code, msg).into_response(),
    };
    match app.purchaser.import(rows).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_purchaser_list(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let category_id = params.get("category_id").and_then(|v| v.parse::<i64>().ok());
    let keyword_pattern = parse_keyword_pattern(&params);
    let rows = app.purchaser.list(category_id, keyword_pattern).await;
    (StatusCode::OK, serde_json::to_string(&rows).unwrap())
}

pub async fn api_purchaser_create(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<PurchaserReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchaser/create").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    let cmd = UpsertPurchaser {
        name: req.name,
        contact: req.contact,
        phone: req.phone,
        address: req.address,
        business_scope: req.business_scope,
        remark: req.remark,
        category_id: req.category_id,
    };
    match app.purchaser.create(cmd).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_purchaser_update(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<PurchaserReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchaser/update").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    let cmd = UpsertPurchaser {
        name: req.name,
        contact: req.contact,
        phone: req.phone,
        address: req.address,
        business_scope: req.business_scope,
        remark: req.remark,
        category_id: req.category_id,
    };
    match app.purchaser.update(req.id.unwrap_or(0), cmd).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_purchaser_delete(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<DeleteReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchaser/delete").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    match app.purchaser.delete(req.id).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_product_toggle_status(State(app): State<App>, Path(id): Path<i64>) -> (StatusCode, String) {
    match app.product.toggle_status(id).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_list(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    // 进价仅超级管理员可见：非 super_admin 返回的进价字段置 0
    let is_super_admin = crate::middleware::auth::get_user_ctx(&app, &headers).await.role == "super_admin";
    let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
    let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20);
    let filter = domain::port::product::ProductListFilter {
        product_id: params.get("id").and_then(|v| v.parse::<i64>().ok()),
        category_id: params.get("category_id").and_then(|v| v.parse::<i64>().ok()),
        keyword_pattern: parse_keyword_pattern(&params),
        gov_status: params.get("gov_status").and_then(|v| v.parse::<i64>().ok()),
        include_thumbs: params.get("thumbs").map(|s| s != "0").unwrap_or(true),
        page,
        page_size,
        offset: (page - 1) * page_size,
    };
    ok_or_err(app.product.list(is_super_admin, filter).await)
}

pub async fn api_product_create(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<ProductReq>) -> impl IntoResponse {
    // 进价仅超级管理员可设置：非 super_admin 创建商品时进价强制为 0
    let is_super_admin = crate::middleware::auth::get_user_ctx(&app, &headers).await.role == "super_admin";
    match app.product.create(is_super_admin, req).await {
        Ok(()) => StatusCode::OK,
        Err(e) => text_error_response(e).0,
    }
}

pub async fn api_product_check_name(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let name = params.get("name").filter(|s| !s.is_empty()).cloned();
    (StatusCode::OK, app.product.check_name(name).await)
}

pub async fn api_product_by_id(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let is_super_admin = crate::middleware::auth::get_user_ctx(&app, &headers).await.role == "super_admin";
    let product_id = params.get("id").and_then(|s| s.parse::<i64>().ok());
    (StatusCode::OK, app.product.by_id(is_super_admin, product_id).await)
}

pub async fn api_product_search(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let is_super_admin = crate::middleware::auth::get_user_ctx(&app, &headers).await.role == "super_admin";
    let keyword = params.get("keyword").filter(|s| !s.is_empty()).cloned();
    // audited=1：仅返回已审核商品（开单商品选择用）
    let only_audited = params.get("audited").map(|s| s == "1").unwrap_or(false);
    (StatusCode::OK, app.product.search(is_super_admin, keyword, only_audited).await)
}

pub async fn api_product_update(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<ProductUpdateReq>) -> impl IntoResponse {
    let role = match crate::middleware::auth::check_api_permission(&app, &headers, "/api/product/update").await {
        Err(e) => return e,
        Ok(role) => role,
    };
    match app.product.update(role == "super_admin", req).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_delete(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/product/delete").await {
        Err(e) => return e,
        Ok(_) => {}
    }

    let id = req["id"].as_i64().unwrap_or(0);
    match app.product.delete(id).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_upload_image(
    State(app): State<App>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let product_id = params.get("product_id").and_then(|s| s.parse::<i64>().ok());
    // 收集 multipart 中 name="file" 的 (文件名, 内容)；next_field 失败 panic 行为保留
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    while let Some(field) = multipart.next_field().await.unwrap() {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().unwrap_or_else(|| "unknown.jpg").to_string();
        let bytes = field.bytes().await.unwrap_or_default().to_vec();
        files.push((filename, bytes));
    }
    match app.product.upload_image(product_id, files).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_delete_image(
    State(app): State<App>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> (StatusCode, String) {
    let product_id = params.get("product_id").and_then(|s| s.parse::<i64>().ok());
    match app.product.delete_image(product_id).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

/// 下载网络图片并转存到本地 uploads/products/，返回本地访问 URL
/// 文件命名与手动上传一致：{商品名前缀}_{时间戳}_{随机数}.{扩展名}
/// 使用 Windows 自带 curl.exe 下载，避免引入额外 HTTP 依赖
pub(crate) async fn save_remote_image_to_local(url: &str, name_prefix: &str) -> Result<String, String> {
    let url_trim = url.trim();
    if url_trim.is_empty() {
        return Err("图片URL为空".to_string());
    }
    if !url_trim.starts_with("http://") && !url_trim.starts_with("https://") {
        return Err("URL必须以 http:// 或 https:// 开头".to_string());
    }

    tokio::fs::create_dir_all("uploads/products")
        .await
        .map_err(|e| format!("创建图片目录失败: {}", e))?;

    let timestamp = chrono::Utc::now().timestamp_millis();
    let random: u32 = rand::random();
    let tmp_path = format!("uploads/products/_tmp_{}_{}.bin", timestamp, random);

    let output = tokio::process::Command::new("curl.exe")
        .args([
            "-L",
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "20",
            "--max-filesize",
            "5242880",
            // 伪 UA：部分图站对非浏览器 UA 返回 403
            "-A",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
            "-o",
            &tmp_path,
            "-w",
            "%{content_type}",
            url_trim,
        ])
        .output()
        .await
        .map_err(|e| format!("调用 curl.exe 失败（系统缺少 curl）: {}", e))?;

    let content_type = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();

    if !output.status.success() {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let reason = if stderr.is_empty() {
            format!("curl 退出码 {:?}", output.status.code())
        } else {
            stderr
        };
        return Err(format!("下载图片失败: {}", reason));
    }

    let meta = tokio::fs::metadata(&tmp_path)
        .await
        .map_err(|e| format!("读取下载文件失败: {}", e))?;
    if meta.len() == 0 {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err("下载的图片内容为空".to_string());
    }
    if meta.len() > 5 * 1024 * 1024 {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err("图片大小不能超过5MB".to_string());
    }

    let ext: String = if content_type.contains("png") {
        "png".to_string()
    } else if content_type.contains("gif") {
        "gif".to_string()
    } else if content_type.contains("webp") {
        "webp".to_string()
    } else if content_type.contains("jpeg") || content_type.contains("jpg") {
        "jpg".to_string()
    } else {
        // Content-Type 不可用时回退：从 URL 路径取扩展名
        let path_part = url_trim.split(['?', '#']).next().unwrap_or("");
        let from_url = path_part
            .rsplit('.')
            .next()
            .map(|e| e.to_lowercase())
            .filter(|e| ["jpg", "jpeg", "png", "gif", "webp"].contains(&e.as_str()));
        match from_url {
            Some(e) => e,
            None => {
                let _ = tokio::fs::remove_file(&tmp_path).await;
                return Err(format!("无法识别图片类型 (Content-Type: {})", content_type));
            }
        }
    };

    let filename = format!("{}_{}.{}.{}", name_prefix, timestamp, random, ext);
    let final_path = format!("uploads/products/{}", filename);
    tokio::fs::rename(&tmp_path, &final_path)
        .await
        .map_err(|e| format!("保存图片失败: {}", e))?;
    Ok(format!("/api/uploads/products/{}", filename))
}

/// 通过网络图片 URL 设置商品图片：下载转存到本地后回填 image_url
pub async fn api_product_image_from_url(
    State(app): State<App>,
    axum::Json(req): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let product_id = req.get("product_id").and_then(|v| v.as_i64());
    let url = req
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    match app.product.image_from_url(product_id, url).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => {
            // 行为保持：400 为 BadRequest，500 为 Storage
            let status = match &e {
                UseCaseError::BadRequest(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            let msg = match e {
                UseCaseError::BadRequest(m) | UseCaseError::Storage(m) => m,
                _ => "服务器内部错误".to_string(),
            };
            (status, serde_json::json!({ "error": msg }).to_string())
        }
    }
}

pub async fn api_sales_order_upload_image(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;

    let order_id = params.get("order_id").and_then(|s| s.parse::<i64>().ok());
    let order_id = match order_id {
        Some(id) => id,
        None => return (StatusCode::BAD_REQUEST, "缺少 order_id 参数".to_string()),
    };

    // 取 multipart 中最后一个 file 字段（旧 handler 逐字段即时处理；前端单 file 场景行为一致）
    let mut file: Option<(String, Vec<u8>)> = None;
    while let Some(field) = multipart.next_field().await.unwrap() {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().unwrap_or_else(|| "unknown.jpg").to_string();
        let bytes = field.bytes().await.unwrap_or_default().to_vec();
        file = Some((filename, bytes));
    }

    let image_type = params.get("type").map(|s| s.as_str()).unwrap_or("");
    match app.sales_order.upload_image(order_id, image_type, file, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_delete_image(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> (StatusCode, String) {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;

    let order_id = params.get("order_id").and_then(|s| s.parse::<i64>().ok());
    let order_id = match order_id {
        Some(id) => id,
        None => return (StatusCode::BAD_REQUEST, "缺少 order_id 参数".to_string()),
    };

    let image_type = params.get("type").map(|s| s.as_str()).unwrap_or("");
    match app.sales_order.delete_image(order_id, image_type, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_purchase_document_list(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let supplier_id = params.get("supplier_id").and_then(|s| s.parse::<i64>().ok());
    let document_date = params.get("document_date").map(|s| s.as_str()).unwrap_or("");
    match app.purchase_document.list(&ctx, supplier_id, document_date).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_purchase_document_upload(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchase_document/upload").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;

    // multipart 字段解析属 HTTP 传输适配（与 api_restore_file 同例）；文件落盘与 DB 写入在用例层
    let mut supplier_id: Option<i64> = None;
    let mut supplier_name: Option<String> = None;
    let mut document_date: Option<String> = None;
    let mut remark: Option<String> = None;
    // 先缓存文件字节与扩展名，待所有字段解析完再按 供应商+日期 前缀写盘
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_ext: String = "jpg".to_string();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let filename = field.file_name().unwrap_or_else(|| "unknown.jpg");
            let ext = std::path::Path::new(filename)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("jpg")
                .to_lowercase();
            if !["jpg", "jpeg", "png", "gif", "webp"].contains(&ext.as_str()) {
                continue;
            }
            let bytes = field.bytes().await.unwrap_or_default();
            if bytes.len() > 10 * 1024 * 1024 {
                continue;
            }
            file_ext = ext;
            file_bytes = Some(bytes.to_vec());
        } else if name == "supplier_id" {
            if let Ok(v) = field.text().await {
                supplier_id = v.parse::<i64>().ok();
            }
        } else if name == "supplier_name" {
            supplier_name = field.text().await.ok();
        } else if name == "document_date" {
            document_date = field.text().await.ok();
        } else if name == "remark" {
            remark = field.text().await.ok();
        }
    }

    if supplier_id.is_none() || supplier_name.is_none() || document_date.is_none() || file_bytes.is_none() {
        return (StatusCode::BAD_REQUEST, "缺少必填参数".to_string());
    }

    match app.purchase_document.upload(
        &ctx,
        supplier_id.unwrap(),
        supplier_name.unwrap().as_str(),
        document_date.unwrap().as_str(),
        remark,
        file_ext.as_str(),
        &file_bytes.unwrap(),
    ).await {
        Ok(outcome) => {
            crate::middleware::auth::log_operation(&app, &ctx, "purchase_document.upload", "purchase_document", &outcome.id.to_string(),
                &outcome.log_detail).await;
            (StatusCode::OK, serde_json::json!({ "id": outcome.id, "url": outcome.url }).to_string())
        },
        Err(e) => text_error_response(e),
    }
}

pub async fn api_purchase_document_delete(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchase_document/delete").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.purchase_document.delete(&ctx, id).await {
        Ok(body) => {
            crate::middleware::auth::log_operation(&app, &ctx, "purchase_document.delete", "purchase_document", &id.to_string(),
                &format!("删除采购单据 ID={}", id)).await;
            (StatusCode::OK, body)
        },
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_get_image(
    State(app): State<App>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    match app.product.read_image(None, filename).await {
        Some((content, mime)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime)],
            content,
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain")],
            "图片不存在".as_bytes().to_vec(),
        )
            .into_response(),
    }
}

pub async fn api_get_uploaded_image(
    State(app): State<App>,
    Path((folder, filename)): Path<(String, String)>,
) -> impl IntoResponse {
    match app.product.read_image(Some(folder), filename).await {
        Some((content, mime)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime)],
            content,
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain")],
            "图片不存在".as_bytes().to_vec(),
        )
            .into_response(),
    }
}

pub async fn api_product_export(State(app): State<App>) -> impl IntoResponse {
    match app.product.export().await {
        Ok(data) => (
            StatusCode::OK,
            [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", "attachment; filename=\"products.xlsx\""),
            ],
            data,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_product_import(State(app): State<App>, content: Bytes) -> impl IntoResponse {
    // Excel/CSV 解析为行列文本（传输格式适配保留在 HTTP 层），行级业务写入在用例层
    let rows: Vec<Vec<String>>;

    if content.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        let content_vec = content.to_vec();
        match open_workbook_auto_from_rs(std::io::Cursor::new(content_vec)) {
            Ok(mut workbook) => {
                let sheets = workbook.sheet_names().to_vec();
                if sheets.is_empty() {
                    return (StatusCode::BAD_REQUEST, "Excel文件中没有工作表".to_string()).into_response();
                }

                let range = match workbook.worksheet_range(&sheets[0]) {
                    Ok(r) => r,
                    Err(e) => return (StatusCode::BAD_REQUEST, format!("无法读取Excel文件内容: {}", e)).into_response(),
                };

                rows = range.rows()
                    .map(|row| {
                        row.iter()
                            .map(|cell| match cell {
                                Data::Empty => "".to_string(),
                                Data::Int(v) => v.to_string(),
                                Data::Float(v) => v.to_string(),
                                Data::String(v) => v.to_string(),
                                Data::Bool(v) => v.to_string(),
                                _ => "".to_string(),
                            })
                            .collect()
                    })
                    .collect();
            }
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("读取Excel文件失败: {}", e)).into_response();
            }
        }
    } else {
        let content_str = String::from_utf8_lossy(&content).to_string();
        rows = parse_csv(&content_str);
    }

    if rows.len() < 2 {
        return (StatusCode::BAD_REQUEST, "文件至少需要包含标题行和一行数据".to_string()).into_response();
    }

    let summary = app.product.import(rows).await;
    (StatusCode::OK, summary).into_response()
}

pub async fn api_product_unit_create(State(app): State<App>, Json(req): Json<ProductUnitReq>) -> (StatusCode, String) {
    match app.product.unit_create(req).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_unit_update(State(app): State<App>, Json(req): Json<ProductUnitReq>) -> (StatusCode, String) {
    match app.product.unit_update(req).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_unit_delete(State(app): State<App>, Json(req): Json<DeleteReq>) -> (StatusCode, String) {
    match app.product.unit_delete(req.id).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_unit_delete_by_product(State(app): State<App>, Json(req): Json<serde_json::Value>) -> (StatusCode, String) {
    let product_id = req["product_id"].as_i64().unwrap_or(0);
    match app.product.unit_delete_by_product(product_id).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_unit_list(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let is_super_admin = crate::middleware::auth::get_user_ctx(&app, &headers).await.role == "super_admin";
    let product_id = params.get("product_id").and_then(|s| s.parse::<i64>().ok());
    (StatusCode::OK, app.product.unit_list(is_super_admin, product_id).await)
}

pub async fn api_product_price_upsert(State(app): State<App>, Json(req): Json<ProductPriceReq>) -> impl IntoResponse {
    match app.product.price_upsert(req).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_product_price_list(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let product_id = match params.get("product_id") {
        Some(v) => v.parse::<i64>().unwrap_or(0),
        None => 0,
    };
    (StatusCode::OK, app.product.price_list(product_id).await)
}

pub async fn api_product_price_delete(State(app): State<App>, Json(req): Json<DeleteReq>) -> (StatusCode, String) {
    match app.product.price_delete(req.id).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_product_price_delete_by_product(State(app): State<App>, Json(req): Json<std::collections::HashMap<String, i64>>) -> impl IntoResponse {
    let product_id = match req.get("product_id") {
        Some(&id) => id,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };
    app.product.price_delete_by_product(product_id).await;
    StatusCode::OK.into_response()
}

pub async fn api_product_sync_base_price(State(app): State<App>, Json(req): Json<std::collections::HashMap<String, i64>>) -> impl IntoResponse {
    let product_id = match req.get("product_id") {
        Some(&id) => id,
        None => return StatusCode::BAD_REQUEST,
    };
    app.product.sync_base_price(product_id).await;
    StatusCode::OK
}

pub async fn api_product_price_log_list(
    State(app): State<App>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let product_id: Option<i64> = params.get("product_id").and_then(|s| s.parse().ok());
    let limit: i64 = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(100);
    (StatusCode::OK, app.product.price_log_list(product_id, limit).await)
}

pub async fn api_category_list(State(app): State<App>) -> impl IntoResponse {
    let rows = app.category.list().await;
    (StatusCode::OK, serde_json::to_string(&rows).unwrap())
}

pub async fn api_category_create(State(app): State<App>, Json(req): Json<CategoryReq>) -> impl IntoResponse {
    match app
        .category
        .create(req.name, req.parent_id, req.entity_type, req.sort_order)
        .await
    {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_category_delete(State(app): State<App>, Json(req): Json<serde_json::Value>) -> (StatusCode, String) {
    let id = req["id"].as_i64().unwrap_or(0);
    match app.category.delete(id).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_category_rename(State(app): State<App>, Json(req): Json<CategoryRenameReq>) -> (StatusCode, String) {
    match app.category.rename(req.id, req.name).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_category_tree(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let entity_type = params.get("entity_type").cloned().unwrap_or_else(|| "product".to_string());
    let tree = app.category.tree(&entity_type).await;
    (StatusCode::OK, serde_json::to_string(&tree).unwrap())
}

pub async fn api_inventory_list(State(app): State<App>) -> impl IntoResponse {
    let rows = app.inventory.list().await;
    (StatusCode::OK, serde_json::to_string(&rows).unwrap())
}

pub async fn api_warehouse_list(State(app): State<App>) -> impl IntoResponse {
    let rows = app.warehouse.list().await;
    (StatusCode::OK, serde_json::to_string(&rows).unwrap())
}

pub async fn api_warehouse_create(State(app): State<App>, Json(req): Json<WarehouseCreateReq>) -> (StatusCode, String) {
    match app
        .warehouse
        .create(req.name, req.code, req.address, req.contact, req.phone, req.sort_order)
        .await
    {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_warehouse_update(State(app): State<App>, Json(req): Json<WarehouseUpdateReq>) -> (StatusCode, String) {
    match app
        .warehouse
        .update(req.id, req.name, req.code, req.address, req.contact, req.phone, req.status, req.sort_order)
        .await
    {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_warehouse_delete(State(app): State<App>, Json(req): Json<std::collections::HashMap<String, i64>>) -> (StatusCode, String) {
    let id = req.get("id").copied().unwrap_or(0);
    match app.warehouse.delete(id).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_order_generate_no(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let default_type = "purchase".to_string();
    let order_type = params.get("type").unwrap_or(&default_type);
    let default_date = Local::now().format("%Y-%m-%d").to_string();
    let order_date = params.get("date").unwrap_or(&default_date);
    
    let order_no = generate_order_no(order_type, order_date, &app.sales_order_repo, &app.purchase_order_repo).await;
    
    (StatusCode::OK, serde_json::to_string(&serde_json::json!({ "order_no": order_no })).unwrap())
}

pub async fn api_product_set_auto_update_price(
    State(app): State<App>,
    Json(req): Json<std::collections::HashMap<String, i64>>,
) -> impl IntoResponse {
    let product_id = match req.get("product_id") {
        Some(&id) => id,
        None => return (StatusCode::BAD_REQUEST, serde_json::json!({"error": "missing product_id"}).to_string()).into_response(),
    };
    let auto = req.get("auto_update_price").copied().unwrap_or(1);
    app.product.set_auto_update_price(product_id, auto).await;
    (StatusCode::OK, serde_json::json!({"ok": true}).to_string()).into_response()
}

pub async fn api_product_batch_set_auto_update_price(
    State(app): State<App>,
    Json(req): Json<std::collections::HashMap<String, i64>>,
) -> impl IntoResponse {
    let auto = req.get("auto_update_price").copied().unwrap_or(1);
    app.product.batch_set_auto_update_price(auto).await;
    (StatusCode::OK, serde_json::json!({"ok": true}).to_string()).into_response()
}

pub async fn api_product_last_purchase_price(
    State(app): State<App>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let product_id: i64 = match params.get("product_id").and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return (StatusCode::BAD_REQUEST, serde_json::json!({"error": "missing product_id"}).to_string()).into_response(),
    };

    match app.product.last_purchase_price(product_id).await {
        Some(body) => (StatusCode::OK, body).into_response(),
        None => (StatusCode::NOT_FOUND, serde_json::json!({"error": "product not found"}).to_string()).into_response(),
    }
}

pub async fn api_purchase_order_create(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<PurchaseOrderReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchase_order/create").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.purchase_order.create(req, &ctx).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                crate::middleware::auth::log_operation(&app, &ctx, "purchase_order.create", "purchase_order", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_purchase_order_list(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let filter = PurchaseOrderListFilter {
        keyword_pattern: parse_keyword_pattern(&params),
        supplier_id: params.get("supplier_id").and_then(|s| s.parse().ok()),
        is_settled: params.get("is_settled").cloned(),
        sort_field: params.get("sort_field").cloned().unwrap_or_else(|| "id".to_string()),
        sort_order: params.get("sort_order").cloned().unwrap_or_else(|| "desc".to_string()),
        page: params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1),
        page_size: params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20),
    };
    match app.purchase_order.list(&ctx, filter).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_purchase_order_detail(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.purchase_order.detail(id, &ctx).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_purchase_order_update(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<PurchaseOrderReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchase_order/update").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.purchase_order.update(req, &ctx).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                crate::middleware::auth::log_operation(&app, &ctx, "purchase_order.update", "purchase_order", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_purchase_order_delete(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchase_order/delete").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.purchase_order.delete(id, &ctx).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                crate::middleware::auth::log_operation(&app, &ctx, "purchase_order.delete", "purchase_order", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

/// 设置采购订单结算状态（0=未结 1=已结），无需版本号，操作列直接调用
pub async fn api_purchase_order_settle(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchase_order/settle").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let is_settled = data["is_settled"].as_i64();
    match app.purchase_order.settle(id, is_settled, &ctx).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                crate::middleware::auth::log_operation(&app, &ctx, "purchase_order.settle", "purchase_order", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

/// FIFO 先进先出：取某商品当前库存中「最早批次」的生产日期/批号。
/// 口径（无批次余额表，纯推导）：
///   批次队列 = 已审核及之后状态的采购单明细行（有 production_date 按其升序，无则按采购单日期），
///   每行量 = base_quantity（缺失时 quantity × ratio，兜底 1）；
///   已消耗 = stock_movement 销售净出库（out 为正、撤销冲销 in 为负）；
///   按队列顺序抵扣消耗后，第一个剩余量 > 0 的批次即为最早批次。
/// 返回 (production_date, batch_no)；无批次数据（无采购明细/全部消耗完）返回 (None, None)。

pub async fn api_purchase_order_approve(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchase_order/approve").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let reason = data["reason"].as_str().unwrap_or("").trim().to_string();
    match app.purchase_order.approve(id, reason, &ctx).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                crate::middleware::auth::log_operation(&app, &ctx, "purchase_order.approve", "purchase_order", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_purchase_order_unapprove(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchase_order/unapprove").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let reason = data["reason"].as_str().unwrap_or("").trim().to_string();
    match app.purchase_order.unapprove(id, reason, &ctx).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                crate::middleware::auth::log_operation(&app, &ctx, "purchase_order.unapprove", "purchase_order", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_purchase_order_export(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // 行级数据权限：supplier 只能导出自己的（强制逻辑在用例层）
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let keyword_pattern = parse_keyword_pattern(&params);
    let supplier_id = params.get("supplier_id").and_then(|s| s.parse().ok());
    match app.purchase_order.export(&ctx, keyword_pattern, supplier_id).await {
        Ok(data) => (
            StatusCode::OK,
            [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", "attachment; filename=\"purchase_orders.xlsx\""),
            ],
            data,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_purchase_order_print_excel(
    State(app): State<App>,
    Path(id): Path<i64>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // 如果传入了 user_id 则优先使用参数里的；否则使用订单里存的（解析失败/非正数均不覆盖）
    let user_id_override = params.get("user_id").or(params.get("userId")).and_then(|s| s.parse::<i64>().ok());
    match app.purchase_order.print_excel(id, user_id_override).await {
        Ok((data, filename)) => xlsx_response(data, &filename),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_purchase_order_import(State(app): State<App>, headers: axum::http::HeaderMap, content: Bytes) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/purchase_order/import").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.purchase_order.import(content.to_vec(), &ctx).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                crate::middleware::auth::log_operation(&app, &ctx, "purchase_order.import", "purchase_order", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.body).into_response()
        }
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_sales_order_detail(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.sales_order.detail(id, &ctx).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_update(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<SalesOrderReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/update").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.sales_order.update(req, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_update_prices(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/update_prices").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.sales_order.update_prices(id, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_delete(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/delete").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.sales_order.delete(id, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_export(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // 行级数据权限：purchaser 只能导出自己采购单位的销售单（强制逻辑在用例层）
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let keyword_pattern = parse_keyword_pattern(&params);
    let purchaser_id = params.get("purchaser_id").and_then(|s| s.parse().ok());
    match app.sales_order.export(&ctx, keyword_pattern, purchaser_id).await {
        Ok(data) => (
            StatusCode::OK,
            [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", "attachment; filename=\"sales_orders.xlsx\""),
            ],
            data,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_sales_order_import(State(app): State<App>, headers: axum::http::HeaderMap, content: Bytes) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/import").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.sales_order.import(content.to_vec(), &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body).into_response()
        }
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_purchase_order(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    // 行级数据权限：supplier 角色强制只看自己绑定的供应商
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.query_purchase.query_orders(&ctx, &params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_purchase_order_export(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    // 行级数据权限：supplier 角色只能导出自己供应商的数据
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.query_purchase.export_orders(&ctx, &params).await {
        Ok(buf) => (
            [
                (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"purchase_orders.xlsx\""),
            ],
            buf,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_purchase_price(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_purchase.price_items(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_purchase_summary(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_purchase.purchase_summary(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_supplier_balance(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    // 行级数据权限：supplier 角色只能看自己绑定的供应商往来
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.query_finance.supplier_balance(&ctx).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_supplier_balance_export(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    // 行级数据权限：supplier 角色只能导出自己绑定的供应商往来
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.query_finance.export_supplier_balance(&ctx).await {
        Ok(buf) => (
            [
                (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"supplier_balance.xlsx\""),
            ],
            buf,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_sales_order(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    // 行级数据权限：purchaser 角色强制只看自己绑定的采购单位
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.query_sales.query_orders(&ctx, &params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_sales_order_export(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    // 行级数据权限：purchaser 角色只能导出自己采购单位的数据
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.query_sales.export_orders(&ctx, &params).await {
        Ok(buf) => (
            [
                (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"sales_orders.xlsx\""),
            ],
            buf,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_product_price_trend(
    State(app): State<App>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    match app.query_purchase.price_trend(&params).await {
        Ok(body) => (StatusCode::OK, body).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

/// 采购比价：按供应商聚合历史采购价，返回加权均价/最近单价/区间/次数
/// 同时读取 product_supplier_price（今日采集价），今日价优先于历史价作为"最近单价"
pub async fn api_query_purchase_price_compare(
    State(app): State<App>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    match app.query_purchase.price_compare(&params).await {
        Ok(body) => (StatusCode::OK, body).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_sales_price(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_sales.price_items(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_sales_summary(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_sales.sales_summary(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_reimburse_summary(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_sales.reimburse_summary(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_allocation_source(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_finance.allocation_source(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_purchaser_balance(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    // 行级数据权限：purchaser 角色只能看自己绑定的采购单位往来
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.query_finance.purchaser_balance(&ctx).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_purchaser_balance_export(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    // 行级数据权限：purchaser 角色只能导出自己绑定的采购单位往来
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.query_finance.export_purchaser_balance(&ctx).await {
        Ok(buf) => (
            [
                (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"purchaser_balance.xlsx\""),
            ],
            buf,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_sales_order_generate_purchase(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/generate_purchase").await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let force = params.get("force").map(|s| s == "1").unwrap_or(false);
    match app.sales_order.generate_purchase(id, force, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")], outcome.body).into_response()
        }
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_product_rank(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_sales.product_rank(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_overview(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stats.overview(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_category_stats(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stats.category_stats(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_document_summary(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stats.document_summary(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_stock_balance(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
    let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
    // 日期非法直接报错，避免时间边界静默出错
    if (!start_date.is_empty() && !is_valid_date_str(start_date))
        || (!end_date.is_empty() && !is_valid_date_str(end_date))
    {
        return (StatusCode::BAD_REQUEST, "日期格式不正确，应为 YYYY-MM-DD".to_string());
    }
    match app.query_stock.stock_balance(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_stock_flow(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stock.stock_flow(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_stock_summary(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stock.stock_summary(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_stock_summary_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stock.export_stock_summary(&params).await {
        Ok(buf) => (
            [
                (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"stock_summary.xlsx\""),
            ],
            buf,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_stock_summary_reimburse(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stock.stock_summary_reimburse(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_stock_summary_reimburse_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stock.export_stock_summary_reimburse(&params).await {
        Ok(buf) => (
            [
                (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"stock_summary_reimburse.xlsx\""),
            ],
            buf,
        ).into_response(),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_stock_warning(State(app): State<App>) -> impl IntoResponse {
    match app.query_stock.stock_warning().await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_slow_stock(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stock.slow_stock(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_income_expense(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_finance.income_expense(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_profit_detail(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_finance.profit_detail(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_purchase_price_export(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    // 单价/金额为进价信息，仅超级管理员可见：非 super_admin 导出时这两列置空
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.query_purchase.export_price_items(&ctx, &params).await {
        Ok(buf) => xlsx_response(buf, "采购台账查询.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_purchase_summary_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_purchase.export_purchase_summary(&params).await {
        Ok(buf) => xlsx_response(buf, "采购汇总统计.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_sales_price_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_sales.export_price_items(&params).await {
        Ok(buf) => xlsx_response(buf, "销售价格查询.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_sales_summary_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_sales.export_sales_summary(&params).await {
        Ok(buf) => xlsx_response(buf, "销售汇总报表.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_product_rank_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_sales.export_product_rank(&params).await {
        Ok(buf) => xlsx_response(buf, "畅销商品排名.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_reimburse_summary_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_sales.export_reimburse_summary(&params).await {
        Ok(buf) => xlsx_response(buf, "报销口径汇总.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_allocation_source_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_finance.export_allocation_source(&params).await {
        Ok(buf) => xlsx_response(buf, "分摊来源统计.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_overview_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stats.export_overview(&params).await {
        Ok(buf) => xlsx_response(buf, "进销存汇总报表.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_stock_balance_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
    let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
    // 日期非法直接报错，避免时间边界静默出错
    if (!start_date.is_empty() && !is_valid_date_str(start_date))
        || (!end_date.is_empty() && !is_valid_date_str(end_date))
    {
        return (StatusCode::BAD_REQUEST, "日期格式不正确，应为 YYYY-MM-DD").into_response();
    }
    let history = !start_date.is_empty() || !end_date.is_empty();
    match app.query_stock.export_stock_balance(&params).await {
        Ok(buf) => xlsx_response(buf, if history { "库存收发存查询.xlsx" } else { "库存余额查询.xlsx" }),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_stock_flow_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let file_name = if params.get("format").map(|s| s.as_str()) == Some("timeline") { "出入库记录-时间线补打.xlsx" } else { "出入库记录.xlsx" };
    match app.query_stock.export_stock_flow(&params).await {
        Ok(buf) => xlsx_response(buf, file_name),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_stock_warning_export(State(app): State<App>) -> impl IntoResponse {
    match app.query_stock.export_stock_warning().await {
        Ok(buf) => xlsx_response(buf, "库存预警.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_slow_stock_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stock.export_slow_stock(&params).await {
        Ok(buf) => xlsx_response(buf, "呆滞库存查询.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_income_expense_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_finance.export_income_expense(&params).await {
        Ok(buf) => xlsx_response(buf, "收支流水查询.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_profit_detail_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_finance.export_profit_detail(&params).await {
        Ok(buf) => xlsx_response(buf, "毛利明细查询.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

/// 财务结算查询 API：合并采购/销售订单，返回应付/已付/未付/下浮/扣减统计
pub async fn api_query_finance_settlement(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_finance.finance_settlement(&params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_query_category_stats_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stats.export_category_stats(&params).await {
        Ok(buf) => xlsx_response(buf, "品类进销存统计.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_query_document_summary_export(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.query_stats.export_document_summary(&params).await {
        Ok(buf) => xlsx_response(buf, "单据汇总查询.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_purchase_document_list_export(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let supplier_id = params.get("supplier_id").and_then(|s| s.parse::<i64>().ok());
    let document_date = params.get("document_date").map(|s| s.as_str()).unwrap_or("");
    match app.purchase_document.list_export(&ctx, supplier_id, document_date).await {
        Ok(buf) => xlsx_response(buf, "采购单据列表.xlsx"),
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_sales_order_create(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<SalesOrderReq>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/create").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.sales_order.create(req, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_list(State(app): State<App>, headers: axum::http::HeaderMap, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let filter = SalesOrderListFilter {
        keyword_pattern: parse_keyword_pattern(&params),
        purchaser_id: params.get("purchaser_id").and_then(|s| s.parse().ok()),
        is_settled: params.get("is_settled").cloned(),
        sort_field: params.get("sort_field").cloned().unwrap_or_else(|| "id".to_string()),
        sort_order: params.get("sort_order").cloned().unwrap_or_else(|| "desc".to_string()),
        page: params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1),
        page_size: params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20),
    };
    match app.sales_order.list(&ctx, filter).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_accept(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.sales_order.accept(id, &ctx).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_by_purchaser(State(app): State<App>, headers: axum::http::HeaderMap, Path(purchaser_id): Path<i64>) -> impl IntoResponse {
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.sales_order.by_purchaser(purchaser_id, &ctx).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_allocation_create(State(app): State<App>, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    match app.order_allocation.create(req).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_allocation_summary(State(app): State<App>, Path(source_order_id): Path<i64>) -> impl IntoResponse {
    match app.order_allocation.summary(source_order_id).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_allocation_allocated_orders(State(app): State<App>) -> impl IntoResponse {
    match app.order_allocation.allocated_orders().await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_allocation_terminate(State(app): State<App>, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    match app.order_allocation.terminate(req).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_allocation_cancel(State(app): State<App>, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    match app.order_allocation.cancel(req).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_allocation_complete(State(app): State<App>, Json(req): Json<serde_json::Value>) -> impl IntoResponse {
    match app.order_allocation.complete(req).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_supplement_create(State(app): State<App>, Json(req): Json<OrderSupplementItemReq>) -> impl IntoResponse {
    match app.order_supplement.create(req).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_supplement_list_by_target(State(app): State<App>, Path(order_id): Path<i64>) -> impl IntoResponse {
    match app.order_supplement.list_by_target(order_id).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_supplement_list_by_source(State(app): State<App>, Path(order_id): Path<i64>) -> impl IntoResponse {
    match app.order_supplement.list_by_source(order_id).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_adjusted_orders(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match app.order_supplement.adjusted_orders(params).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_supplement_delete(State(app): State<App>, Path(id): Path<i64>) -> impl IntoResponse {
    match app.order_supplement.delete(id).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_supplement_compare(State(app): State<App>, Path(order_id): Path<i64>) -> impl IntoResponse {
    match app.order_supplement.compare(order_id).await {
        Ok(body) => (StatusCode::OK, body),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_accept_excel(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let force = params.get("force").map(|s| s == "1").unwrap_or(false);
    match check_sales_order_access(&app, &headers, id).await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    build_accept_excel(&crate::middleware::auth::get_user_ctx(&app, &headers).await, id, true, force, &app.sales_order_repo, &app.user_repo).await.into_response()
}

pub async fn api_sales_order_real_excel(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let force = params.get("force").map(|s| s == "1").unwrap_or(false);
    match check_sales_order_access(&app, &headers, id).await {
        Err(e) => return e.into_response(),
        Ok(_) => {}
    }
    build_accept_excel(&crate::middleware::auth::get_user_ctx(&app, &headers).await, id, false, force, &app.sales_order_repo, &app.user_repo).await.into_response()
}

pub async fn api_sales_order_sort_items(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    match app.sales_order.sort_items(date).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_sort_items_excel(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    match app.sales_order.sort_items_excel(date).await {
        Ok((filename, buf)) => {
            let headers = [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", &format!("attachment; filename=\"{}\"", filename)),
            ];
            (StatusCode::OK, headers, buf).into_response()
        },
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_sales_order_sort_items_by_category(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    match app.sales_order.sort_items_by_category(date).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_sort_items_by_category_excel(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    match app.sales_order.sort_items_by_category_excel(date).await {
        Ok((filename, buf)) => {
            if buf.is_empty() {
                eprintln!("Excel export empty buffer (rows likely empty)");
                return (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "application/json")], "{\"ok\":true,\"empty\":true,\"msg\":\"当日暂无分拣数据\"}").into_response();
            }
            let headers = [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", &format!("attachment; filename=\"{}\"", filename)),
            ];
            (StatusCode::OK, headers, buf).into_response()
        },
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_sales_order_sort_items_by_supplier(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    match app.sales_order.sort_items_by_supplier(date).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_sort_items_by_supplier_excel(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    // 可选：是否输出实量/单价/金额数值。不传或非 1/true 时为打印手填模式（三列留空）
    let print_values = matches!(
        params.get("print_values").map(|v| v.trim().to_lowercase()).as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    );
    match app.sales_order.sort_items_by_supplier_excel(date, print_values).await {
        Ok((filename, buf)) => {
            let headers = [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", &format!("attachment; filename=\"{}\"", filename)),
            ];
            (StatusCode::OK, headers, buf).into_response()
        },
        Err(e) => text_error_response(e).into_response(),
    }
}

// ===== 今日进价采集（按供应商去重合并） =====
// 数据源：当天/指定日期的销售订单明细，按供应商分组，组内按商品合并去重
// 用途：向各供应商采集当日进价，录入后回写 product.purchase_price（最近价），
//       供销售订单生成采购订单时使用正确的进价

pub async fn api_product_today_price_items(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // 今日进价采集仅超级管理员可用
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    if ctx.role != "super_admin" {
        return (StatusCode::FORBIDDEN, serde_json::json!({ "success": false, "message": "只有超级管理员可以使用今日进价采集" }).to_string());
    }
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    let date = if date.is_empty() { None } else { Some(date) };
    (StatusCode::OK, app.today_price.items(date).await)
}

pub async fn api_product_today_price_save(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Json(data): Json<std::collections::HashMap<String, serde_json::Value>>,
) -> impl IntoResponse {
    // 今日进价采集仅超级管理员可录入
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    if ctx.role != "super_admin" {
        return (StatusCode::FORBIDDEN, serde_json::json!({ "success": false, "message": "只有超级管理员可以录入今日进价" }).to_string());
    }
    let raw_items = data.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut items: Vec<TodayPriceSaveItem> = Vec::new();
    for item in raw_items {
        let product_id = item.get("product_id").and_then(|v| v.as_i64());
        let supplier_id = item.get("supplier_id").and_then(|v| v.as_i64());
        let price = item.get("price").and_then(|v| v.as_f64());
        if let (Some(product_id), Some(supplier_id), Some(price)) = (product_id, supplier_id, price) {
            items.push(TodayPriceSaveItem { product_id, supplier_id, price });
        }
    }
    (StatusCode::OK, app.today_price.save(items).await)
}

pub async fn api_product_today_price_excel(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // 今日进价采集仅超级管理员可用
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    if ctx.role != "super_admin" {
        return (StatusCode::FORBIDDEN, serde_json::json!({ "success": false, "message": "只有超级管理员可以使用今日进价采集" }).to_string()).into_response();
    }
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    let date = if date.is_empty() { None } else { Some(date) };
    // print_values=0：今日价列留空，供打印后手写采集；=1：今日价列显示当前最近价
    let print_values = matches!(
        params.get("print_values").map(|v| v.trim().to_lowercase()).as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    );
    match app.today_price.export_excel(date, print_values).await {
        Ok((filename, buf)) => {
            let headers = [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", &format!("attachment; filename=\"{}\"", filename)),
            ];
            (StatusCode::OK, headers, buf).into_response()
        }
        Err(e) => text_error_response(e).into_response(),
    }
}

// ===== 今日进价采集·A4 打印页 =====
// 按供应商分组，A4 纵向三栏排版（每栏四列：序号/品名规格/单位/今日价，栏间空一列，共 14 列），左边距 1cm
pub async fn api_product_today_price_a4(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // 今日进价采集仅超级管理员可用
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    if ctx.role != "super_admin" {
        return (StatusCode::FORBIDDEN, serde_json::json!({ "success": false, "message": "只有超级管理员可以使用今日进价采集" }).to_string()).into_response();
    }
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    let date = if date.is_empty() { None } else { Some(date) };
    // print_values=1：今日价列显示最近价（purchase_price）；=0：留空手写
    let print_values = matches!(
        params.get("print_values").map(|v| v.trim().to_lowercase()).as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    );
    match app.today_price.export_a4(date, print_values).await {
        Ok((filename, buf)) => {
            let headers = [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", &format!("attachment; filename=\"{}\"", filename)),
            ];
            (StatusCode::OK, headers, buf).into_response()
        }
        Err(e) => text_error_response(e).into_response(),
    }
}

// ===== 今日进价采集·按商品分类导出 =====
// 去除采购单位/仓库/供应商分组，按商品分类排序，便于整体采集当日进价
pub async fn api_product_today_price_excel_by_category(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // 今日进价采集仅超级管理员可用
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    if ctx.role != "super_admin" {
        return (StatusCode::FORBIDDEN, serde_json::json!({ "success": false, "message": "只有超级管理员可以使用今日进价采集" }).to_string()).into_response();
    }
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    let date = if date.is_empty() { None } else { Some(date) };
    let print_values = matches!(
        params.get("print_values").map(|v| v.trim().to_lowercase()).as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    );
    match app.today_price.export_by_category(date, print_values).await {
        Ok((filename, buf)) => {
            let headers = [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", &format!("attachment; filename=\"{}\"", filename)),
            ];
            (StatusCode::OK, headers, buf).into_response()
        }
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_sales_order_sort_items_by_purchaser(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    match app.sales_order.sort_items_by_purchaser(date).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_sort_items_by_purchaser_excel(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    match app.sales_order.sort_items_by_purchaser_excel(date).await {
        Ok((filename, buf)) => {
            let headers = [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", &format!("attachment; filename=\"{}\"", filename)),
            ];
            (StatusCode::OK, headers, buf).into_response()
        },
        Err(e) => text_error_response(e).into_response(),
    }
}

pub async fn api_sales_order_sort_comprehensive(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    match app.sales_order.sort_comprehensive(date).await {
        Ok(json) => (StatusCode::OK, json),
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_sort_comprehensive_excel(State(app): State<App>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let date = params.get("date").cloned().unwrap_or_default().trim().to_string();
    match app.sales_order.sort_comprehensive_excel(date).await {
        Ok((filename, buf)) => {
            let headers = [
                ("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                ("Content-Disposition", &format!("attachment; filename=\"{}\"", filename)),
            ];
            (StatusCode::OK, headers, buf).into_response()
        },
        Err(e) => text_error_response(e).into_response(),
    }
}

/// 设置销售订单结算状态（0=未结 1=已结），操作列直接调用
pub async fn api_sales_order_settle(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/settle").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.sales_order.settle(id, data["is_settled"].as_i64(), &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_update_status(State(app): State<App>, headers: axum::http::HeaderMap, Json(req): Json<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/update_status").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let id = req.get("id").and_then(|s| s.parse::<i64>().ok());
    let new_status = req.get("status").cloned();
    match app.sales_order.update_status(id, new_status, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_approve(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/approve").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let reason = data["reason"].as_str().unwrap_or("").trim().to_string();
    match app.sales_order.approve(id, reason, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_unapprove(State(app): State<App>, headers: axum::http::HeaderMap, Path(id): Path<i64>, Json(data): Json<serde_json::Value>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/unapprove").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    let reason = data["reason"].as_str().unwrap_or("").trim().to_string();
    match app.sales_order.unapprove(id, reason, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        }
        Err(e) => text_error_response(e),
    }
}

pub async fn api_sales_order_correction(State(app): State<App>, headers: axum::http::HeaderMap, Json(data): Json<std::collections::HashMap<String, serde_json::Value>>) -> impl IntoResponse {
    match crate::middleware::auth::check_api_permission(&app, &headers, "/api/sales_order/correction").await {
        Err(e) => return e,
        Ok(_) => {}
    }
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;

    let corrections = match data.get("corrections") {
        None => return (StatusCode::BAD_REQUEST, "缺少修正数据".to_string()),
        Some(v) => v.as_array().cloned().unwrap_or_default(),
    };

    match app.sales_order.correction(corrections, &ctx).await {
        Ok(outcome) => {
            for log in &outcome.logs {
                crate::middleware::auth::log_operation(&app, &ctx, &log.action, &log.target_type, &log.target_id, &log.detail).await;
            }
            (StatusCode::OK, outcome.body)
        },
        Err(e) => text_error_response(e),
    }
}

pub async fn api_accept_create(State(app): State<App>, Json(req): Json<AcceptReq>) -> impl IntoResponse {
    // 旧行为：仅状态码（200/500），无响应体文案
    match app.food_accept.create(req).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub async fn api_accept_list(State(app): State<App>) -> impl IntoResponse {
    match app.food_accept.list().await {
        Ok(body) => (StatusCode::OK, body),
        // 行为保持：列表用例实际不会返回 Err（查询失败吞错返回空数组）
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, String::new()),
    }
}

// ====== 价格策略时段管理（政采价/超市比价按生效日期段录入） ======
// 业务逻辑已迁移至 application::service::price_schedule（六边形切片）；
// 此处仅保留 HTTP 薄适配：权限检查 → 入参解析 → 调用用例 → 操作日志。

/// 新增价格策略时段
pub async fn api_price_schedule_create(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PriceScheduleReq>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/create").await {
        return (code, msg).into_response();
    }
    match app.price_schedule.create(req).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
                crate::middleware::auth::log_operation(&app, &ctx, "录入价格策略", "price_schedule", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.json).into_response()
        }
        Err(e) => usecase_error_response(e).into_response(),
    }
}

/// 更新价格策略时段（允许改 price/remark/source/price_type/effective_date；product_id 由 id 锁定）
pub async fn api_price_schedule_update(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PriceScheduleReq>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/update").await {
        return (code, msg).into_response();
    }
    match app.price_schedule.update(req).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
                crate::middleware::auth::log_operation(&app, &ctx, "更新价格策略", "price_schedule", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.json).into_response()
        }
        Err(e) => usecase_error_response(e).into_response(),
    }
}

/// 删除价格策略时段
pub async fn api_price_schedule_delete(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Json(req): Json<DeleteReq>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/delete").await {
        return (code, msg).into_response();
    }
    match app.price_schedule.delete(req.id).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
                crate::middleware::auth::log_operation(&app, &ctx, "删除价格策略", "price_schedule", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.json).into_response()
        }
        Err(e) => usecase_error_response(e).into_response(),
    }
}

/// 查询价格策略列表（支持分页、商品过滤、价格类型过滤、生效日期段过滤）
pub async fn api_price_schedule_list(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/list").await {
        return (code, msg).into_response();
    }

    let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
    let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20).min(200);
    let filter = ScheduleFilter {
        product_id: params.get("product_id").and_then(|s| s.parse().ok()).unwrap_or(0),
        price_type: params.get("price_type").cloned().unwrap_or_default(),
        effective_from: params.get("effective_from").cloned().unwrap_or_default(),
        effective_to: params.get("effective_to").cloned().unwrap_or_default(),
        keyword: params.get("keyword").cloned().unwrap_or_default(),
    };
    (StatusCode::OK, app.price_schedule.list(filter, page, page_size).await).into_response()
}

/// 批量导入价格策略（用于 Excel 导入）
/// conflict_policy: upsert=按 (product, price_type, effective_date) 覆盖；ignore=跳过已存在
pub async fn api_price_schedule_batch_import(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PriceScheduleBatchReq>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/batch_import").await {
        return (code, msg).into_response();
    }
    let outcome = app.price_schedule.batch_import(req.items, req.conflict_policy).await;
    if let Some((target_id, detail)) = outcome.log {
        let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
        crate::middleware::auth::log_operation(&app, &ctx, "批量导入价格策略", "price_schedule", &target_id, &detail).await;
    }
    (StatusCode::OK, outcome.json).into_response()
}

/// 查询某商品在某 price_type 下的全部历史时段（含起止日期）
pub async fn api_price_schedule_history(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/history").await {
        return (code, msg).into_response();
    }
    let product_id: i64 = params.get("product_id").and_then(|s| s.parse().ok()).unwrap_or(0);
    let price_type = params.get("price_type").cloned().unwrap_or_default();
    (StatusCode::OK, app.price_schedule.history(product_id, price_type).await).into_response()
}

/// 按 id 查单条价格策略记录（编辑表单加载使用，不依赖分页/筛选）
/// GET /api/price_schedule/get?id=xx
/// 返回：单条记录的 JSON，找不到返回 success:false
pub async fn api_price_schedule_get(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/lookup").await {
        return (code, msg).into_response();
    }
    let id: i64 = params.get("id").and_then(|s| s.parse().ok()).unwrap_or(0);
    match app.price_schedule.get(id).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err(e) => usecase_error_response(e).into_response(),
    }
}

/// 时段诊断：返回指定商品下所有价格类型的完整时段链（用于排查数据是否正确录入）
/// GET /api/price_schedule/diagnose?product_id=xx
/// 返回：{ success, product_id, product_name, types: { "gov_procurement": [{id, effective_date, end_date, price, source}, ...], ... } }
pub async fn api_price_schedule_diagnose(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/lookup").await {
        return (code, msg).into_response();
    }
    let product_id: i64 = params.get("product_id").and_then(|s| s.parse().ok()).unwrap_or(0);
    match app.price_schedule.diagnose(product_id).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err(e) => usecase_error_response(e).into_response(),
    }
}

/// 批量按指定日期查询商品的 4 类价（gov_procurement / supermarket_1/2/3）
/// POST /api/price_schedule/lookup_batch
/// body: { date: "YYYY-MM-DD", product_ids: [1,2,3,...] }
/// 返回: { success: true, date, prices: { "1": { gov_procurement: 3.2, supermarket_1: 3.5, supermarket_2, supermarket_3 }, ... } }
/// 注意：JSON object key 必须是字符串，i64 序列化时会被 serde_json 转成字符串。
/// 前端应使用 prices[String(pid)] 或 prices[pid]（JS 自动 toString）访问。
pub async fn api_price_schedule_lookup_batch(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    body: String,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/lookup").await {
        return (code, msg).into_response();
    }
    #[derive(serde::Deserialize)]
    struct Req {
        date: String,
        #[serde(default)]
        product_ids: Vec<i64>,
    }
    let req: Req = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!(
                "{{\"success\":false,\"message\":\"JSON 解析失败: {}\"}}", e
            )).into_response();
        }
    };
    match app.price_schedule.lookup_batch(req.date, req.product_ids).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err(e) => usecase_error_response(e).into_response(),
    }
}

/// 按指定日期查询商品在某 price_type 下的生效价（用于销售单录入页"按日期加载价"）
/// 约定价类型需额外传 supplier_id 或 purchaser_id
pub async fn api_price_schedule_lookup(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/lookup").await {
        return (code, msg).into_response();
    }
    let product_id: i64 = params.get("product_id").and_then(|s| s.parse().ok()).unwrap_or(0);
    let price_type = params.get("price_type").cloned().unwrap_or_default();
    let query_date = params.get("query_date").cloned();
    // 约定价类型需额外传 supplier_id 或 purchaser_id
    let party_id: i64 = if price_type == "supplier_contract" {
        params.get("supplier_id").and_then(|s| s.parse().ok()).unwrap_or(0)
    } else {
        params.get("purchaser_id").and_then(|s| s.parse().ok()).unwrap_or(0)
    };
    (StatusCode::OK, app.price_schedule.lookup(product_id, price_type, query_date, party_id).await).into_response()
}

/// 价格策略导入模板：导出 Excel 含"商品名称、商品编码、规格、别名1、别名2、单位、价格类型、生效日期、价格、备注"
/// 供客户先填表再上传批量导入
pub async fn api_price_schedule_export_template(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/export_template").await {
        return (code, msg).into_response();
    }
    match app.price_schedule.export_template().await {
        Ok(buf) => xlsx_response(buf, "价格策略导入模板.xlsx"),
        Err(e) => usecase_error_response(e).into_response(),
    }
}

/// 导出当前已录入的价格策略列表（按筛选条件）
pub async fn api_price_schedule_export(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/export").await {
        return (code, msg).into_response();
    }
    let filter = ScheduleFilter {
        product_id: params.get("product_id").and_then(|s| s.parse().ok()).unwrap_or(0),
        price_type: params.get("price_type").cloned().unwrap_or_default(),
        effective_from: params.get("effective_from").cloned().unwrap_or_default(),
        effective_to: params.get("effective_to").cloned().unwrap_or_default(),
        keyword: params.get("keyword").cloned().unwrap_or_default(),
    };
    match app.price_schedule.export(filter).await {
        Ok(buf) => xlsx_response(buf, "价格策略列表.xlsx"),
        Err(e) => usecase_error_response(e).into_response(),
    }
}

/// 销售单补单专用：按 (order_date, purchaser_id) 匹配历史时段政采价，
/// 仅对那些明细行 unit_price = 0 或 flag 标记为"待补价"的行按当时价重算金额。
/// 其他明细（已有非零价的行）保持不变。
pub async fn api_price_schedule_backfill_sales_order(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/backfill_sales_order").await {
        return (code, msg).into_response();
    }
    let order_id: i64 = params.get("order_id").and_then(|s| s.parse().ok()).unwrap_or(0);
    let force: bool = params.get("force").map(|s| s == "1" || s == "true").unwrap_or(false);
    let price_type: String = params.get("price_type").cloned().unwrap_or_else(|| "gov_procurement".to_string());
    let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
    match app.price_schedule.backfill_sales_order(order_id, force, price_type, &ctx).await {
        Ok(outcome) => {
            if let Some((target_id, detail)) = outcome.log {
                crate::middleware::auth::log_operation(&app, &ctx, "补单按价重算", "sales_order", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.json).into_response()
        }
        Err(e) => usecase_error_response(e).into_response(),
    }
}

/// Excel 批量导入价格策略（multipart/form-data file 字段）
/// 解析 xlsx 后调用 api_price_schedule_batch_import 等价逻辑
/// Excel 列：商品名称、规格、别名1、别名2、单位、价格类型、生效日期、价格、备注
pub async fn api_price_schedule_batch_import_excel(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if let Err((code, msg)) = crate::middleware::auth::check_api_permission(&app, &headers, "/api/price_schedule/batch_import").await {
        return (code, msg).into_response();
    }
    // 收集上传的 xlsx 字节
    let mut bytes: Option<Vec<u8>> = None;
    while let Some(field) = match multipart.next_field().await {
        Ok(f) => f,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("解析上传失败: {}", e)).into_response(),
    } {
        if field.name() == Some("file") {
            match field.bytes().await {
                Ok(b) => bytes = Some(b.to_vec()),
                Err(e) => return (StatusCode::BAD_REQUEST, format!("读取文件失败: {}", e)).into_response(),
            }
        }
    }
    let bytes = match bytes {
        Some(b) => b,
        None => return (StatusCode::BAD_REQUEST, "{\"success\":false, \"message\":\"未上传 file 字段\"}".to_string()).into_response(),
    };

    match app.price_schedule.batch_import_excel(bytes).await {
        Ok(outcome) => {
            let ctx = crate::middleware::auth::get_user_ctx(&app, &headers).await;
            if let Some((target_id, detail)) = outcome.log {
                crate::middleware::auth::log_operation(&app, &ctx, "Excel批量导入价格策略", "price_schedule", &target_id, &detail).await;
            }
            (StatusCode::OK, outcome.json).into_response()
        }
        Err(e) => usecase_error_response(e).into_response(),
    }
}
