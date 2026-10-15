use axum::http::{HeaderMap, StatusCode};
use std::collections::HashSet;

use domain::UserCtx;
use domain::port::{UserRepo, OperationLogRepo, SalesOrderRepo};

use crate::di::App;

pub async fn get_user_role(app: &App, headers: &HeaderMap) -> String {
    let session_token = headers.get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find(|s| s.trim().starts_with("session="))
                .map(|s| s.trim().strip_prefix("session=").unwrap_or(""))
        })
        .unwrap_or("");

    if session_token.is_empty() {
        return "anonymous".to_string();
    }

    let parts: Vec<&str> = session_token.split(':').collect();
    if parts.len() < 2 {
        return "anonymous".to_string();
    }

    let user_id = match parts[0].parse::<i64>() {
        Ok(id) => id,
        Err(_) => return "anonymous".to_string(),
    };

    match app.user_repo.find_active_by_id(user_id).await {
        Ok(Some(row)) => row.role,
        _ => "anonymous".to_string(),
    }
}

// ===== 细粒度权限点系统（工程化 RBAC） =====
// 权限点：resource.action，如 purchase_order.update
// 角色 → 权限点映射，super_admin 拥有全部权限

pub fn has_permission_point(role: &str, permission: &str) -> bool {
    const ALL_PERMS: [&str; 24] = [
        "purchase_order.view", "purchase_order.create", "purchase_order.update", "purchase_order.approve", "purchase_order.unapprove", "purchase_order.cancel", "purchase_order.delete",
        "sales_order.view", "sales_order.create", "sales_order.update", "sales_order.approve", "sales_order.unapprove", "sales_order.adjust_price", "sales_order.cancel", "sales_order.delete",
        "query.view", "manage.admin", "manage.user", "manage.system", "manage.backup",
        "supplier", "purchaser",
        "price_schedule.view", "price_schedule.manage",
    ];

    let role_perms: HashSet<&str> = match role {
        "super_admin" => HashSet::from(ALL_PERMS),
        "admin" => HashSet::from(ALL_PERMS),
        "supplier" => HashSet::from([
            "supplier",
            "purchase_order.view", "purchase_order.create", "purchase_order.update", "purchase_order.approve", "purchase_order.cancel",
            "sales_order.view", "query.view",
            "price_schedule.view",
        ]),
        "purchaser" => HashSet::from([
            "purchaser",
            "purchase_order.view",
            "sales_order.view", "sales_order.create", "sales_order.update", "sales_order.approve", "sales_order.adjust_price", "sales_order.cancel",
            "query.view",
            "price_schedule.view",
        ]),
        "user" => HashSet::from(["query.view"]),
        _ => HashSet::new(),
    };
    role_perms.contains(permission)
}

pub async fn get_user_ctx(app: &App, headers: &HeaderMap) -> UserCtx {
    let session_token = headers.get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find(|s| s.trim().starts_with("session="))
                .map(|s| s.trim().strip_prefix("session=").unwrap_or(""))
        })
        .unwrap_or("");

    let empty = UserCtx { role: "anonymous".to_string(), user_id: 0, supplier_id: 0, purchaser_id: 0 };
    if session_token.is_empty() {
        return empty;
    }

    let parts: Vec<&str> = session_token.split(':').collect();
    if parts.len() < 2 {
        return empty;
    }

    let user_id = match parts[0].parse::<i64>() {
        Ok(id) => id,
        Err(_) => return empty,
    };

    match app.user_repo.session_ctx(user_id).await {
        Ok(Some(row)) => UserCtx {
            role: row.role,
            user_id,
            supplier_id: row.supplier_id,
            purchaser_id: row.purchaser_id,
        },
        _ => empty,
    }
}

pub async fn log_operation(
    app: &App,
    user: &UserCtx,
    action: &str,
    target_type: &str,
    target_id: &str,
    detail: &str,
) {
    let _ = app.oplog_repo.log(user, action, target_type, target_id, detail).await;
}

pub fn can_access_purchase_order(user: &UserCtx, order_supplier_id: i64) -> bool {
    match user.role.as_str() {
        "super_admin" | "admin" => true,
        "supplier" => user.supplier_id == order_supplier_id,
        _ => false,
    }
}

pub fn can_access_sales_order(user: &UserCtx, order_purchaser_id: i64) -> bool {
    match user.role.as_str() {
        "super_admin" | "admin" => true,
        "purchaser" => user.purchaser_id == order_purchaser_id,
        _ => false,
    }
}

pub fn check_api_route_permission(path: &str) -> Option<&str> {
    if path.starts_with("/api/supplier/") {
        Some("supplier")
    } else if path.starts_with("/api/purchaser/") {
        Some("purchaser")
    } else if path.starts_with("/api/product/") {
        Some("manage.admin")
    } else if path.starts_with("/api/warehouse/") {
        Some("manage.admin")
    } else if path.starts_with("/api/inventory/") {
        Some("manage.admin")
    } else if path.starts_with("/api/purchase_order/") {
        if path.ends_with("/create") || path.ends_with("/import") {
            Some("purchase_order.create")
        } else if path.ends_with("/update") {
            Some("purchase_order.update")
        } else if path.ends_with("/approve") {
            Some("purchase_order.approve")
        } else if path.ends_with("/unapprove") {
            Some("purchase_order.unapprove")
        } else if path.ends_with("/delete") {
            Some("purchase_order.delete")
        } else if path.ends_with("/cancel") {
            Some("purchase_order.cancel")
        } else {
            Some("purchase_order.view")
        }
    } else if path.starts_with("/api/purchase_document/") {
        if path.ends_with("/upload") || path.ends_with("/delete") {
            Some("purchase_order.update")
        } else {
            Some("purchase_order.view")
        }
    } else if path.starts_with("/api/sales_order/") {
        if path.ends_with("/create") || path.ends_with("/import") || path.contains("/generate_purchase") {
            Some("sales_order.create")
        } else if path.ends_with("/update") || path.ends_with("/correction") || path.ends_with("/upload_image") || path.ends_with("/delete_image") {
            Some("sales_order.update")
        } else if path.ends_with("/settle") {
            Some("sales_order.update")
        } else if path.ends_with("/approve") {
            Some("sales_order.approve")
        } else if path.ends_with("/unapprove") {
            Some("sales_order.unapprove")
        } else if path.ends_with("/delete") {
            Some("sales_order.delete")
        } else if path.ends_with("/update_prices") {
            Some("sales_order.adjust_price")
        } else if path.ends_with("/update_status") {
            Some("sales_order.approve")
        } else if path.ends_with("/cancel") {
            Some("sales_order.cancel")
        } else {
            Some("sales_order.view")
        }
    } else if path.starts_with("/api/query/purchase") || path.starts_with("/api/query/supplier_balance") {
        Some("query.view")
    } else if path.starts_with("/api/query/sales") || path.starts_with("/api/query/purchaser_balance") || path.starts_with("/api/query/product_rank") {
        Some("query.view")
    } else if path.starts_with("/api/query/stock") || path.starts_with("/api/query/income") || path.starts_with("/api/query/profit") || path.starts_with("/api/query/overview") || path.starts_with("/api/query/category") || path.starts_with("/api/query/document") {
        Some("manage.admin")
    } else if path.starts_with("/api/stock_movement/") {
        Some("manage.admin")
    } else if path.starts_with("/api/query/") {
        Some("query.view")
    } else if path.starts_with("/api/user/") {
        if path == "/api/user/list" {
            Some("query.view")
        } else {
            Some("manage.user")
        }
    } else if path.starts_with("/api/price_schedule/") {
        if path.ends_with("/list")
            || path.ends_with("/history")
            || path.ends_with("/lookup")
            || path.ends_with("/lookup_batch")
            || path.ends_with("/get")
            || path.ends_with("/diagnose")
            || path.ends_with("/export")
            || path.ends_with("/backfill_sales_order")
        {
            Some("price_schedule.view")
        } else {
            Some("price_schedule.manage")
        }
    } else if path.starts_with("/api/system/") {
        Some("manage.system")
    } else if path.starts_with("/api/backup/") {
        Some("manage.backup")
    } else if path.starts_with("/api/restore/") {
        Some("manage.backup")
    } else {
        None
    }
}

pub async fn check_api_permission(app: &App, headers: &HeaderMap, path: &str) -> Result<String, (StatusCode, String)> {
    let ctx = get_user_ctx(app, headers).await;

    if let Some(permission) = check_api_route_permission(path) {
        if !has_permission_point(&ctx.role, permission) {
            return Err((StatusCode::FORBIDDEN, serde_json::to_string(&serde_json::json!({
                "success": false,
                "message": "您没有权限执行此操作"
            })).unwrap()));
        }
    }

    Ok(ctx.role)
}

pub async fn check_sales_order_access(
    app: &App,
    headers: &HeaderMap,
    id: i64,
) -> Result<(), (StatusCode, String)> {
    let ctx = get_user_ctx(app, headers).await;
    let order_purchaser_id = app.sales_order_repo.fetch_purchaser_id_or_neg1(id).await;
    if !can_access_sales_order(&ctx, order_purchaser_id) {
        return Err((StatusCode::FORBIDDEN, "您没有权限查看此订单".to_string()));
    }
    Ok(())
}
