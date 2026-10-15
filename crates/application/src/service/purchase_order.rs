//! 采购订单用例：创建 / 列表 / 详情 / 乐观锁更新 / 删除 / 结算 / 审核 / 反审核 / 导出 / 打印 / 导入
//!
//! 行为与旧 api_purchase_order_* handler 逐字一致：含错误文案、状态码、吞错分支、
//! 写后联动（商品进价更新/历史重算）与操作日志文案。

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use domain::error::DomainError;
use domain::port::price_schedule::{PriceScheduleRepo, PurchasePriceInput};
use domain::port::purchase_order::*;
use domain::{PurchaseOrderReq, UserCtx};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError};

use crate::error::UseCaseError;
use crate::port::purchase_order::{PurchaseOrderOutcome, PurchaseOrderUseCase};

#[derive(Clone)]
pub struct PurchaseOrderService<R: PurchaseOrderRepo, P: PriceScheduleRepo> {
    repo: R,
    price: P,
}

impl<R: PurchaseOrderRepo, P: PriceScheduleRepo> PurchaseOrderService<R, P> {
    pub fn new(repo: R, price: P) -> Self {
        Self { repo, price }
    }
}

/// 行级数据权限（自 backend middleware::auth::can_access_purchase_order 平移，逻辑逐字一致）：
/// admin/super_admin 可见全部；supplier 仅可见自己绑定的供应商
fn can_access_purchase_order(user: &UserCtx, order_supplier_id: i64) -> bool {
    match user.role.as_str() {
        "super_admin" | "admin" => true,
        "supplier" => user.supplier_id == order_supplier_id,
        _ => false,
    }
}

/// 主表仓库按明细汇总：各行全部同一仓库则记该仓库；否则仓库名去重后以"、"连接（创建/更新共用）
fn derive_main_warehouse(items: &[domain::PurchaseOrderItemReq]) -> (i64, String) {
    let mut wh_id_set: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut wh_names: Vec<String> = Vec::new();
    for it in items {
        let wid = it.warehouse_id.unwrap_or(0);
        let wname = it.warehouse_name.clone().unwrap_or_default();
        if wid > 0 { wh_id_set.insert(wid); }
        if !wname.trim().is_empty() && !wh_names.contains(&wname) { wh_names.push(wname); }
    }
    let main_wh_id = if wh_id_set.len() == 1 { *wh_id_set.iter().next().unwrap() } else { 0 };
    let main_wh_name = wh_names.join("、");
    (main_wh_id, main_wh_name)
}

/// base_quantity 服务端统一重算入参组装（product_unit.ratio 权威），不信任前端值——与销售单同一规则
fn base_inputs_of(items: &[domain::PurchaseOrderItemReq]) -> Vec<(i64, String, f64, f64)> {
    items.iter()
        .map(|it| (it.product_id, it.unit.clone().unwrap_or_default(), it.quantity, it.base_quantity.unwrap_or(0.0)))
        .collect()
}

/// 采购明细 → 进价更新输入映射（domain 自有类型，端口不依赖 HTTP 契约）
fn price_inputs_of(items: &[domain::PurchaseOrderItemReq]) -> Vec<PurchasePriceInput> {
    items.iter()
        .map(|it| PurchasePriceInput {
            product_id: it.product_id,
            unit_price: it.unit_price,
            quantity: it.quantity,
            base_quantity: it.base_quantity,
            amount: it.amount,
        })
        .collect()
}

impl<R: PurchaseOrderRepo, P: PriceScheduleRepo> PurchaseOrderService<R, P> {
    /// 详情/更新/删除/结算/审核/反审核共用护栏：订单存在性 + 行级数据权限
    async fn guard(&self, id: i64, ctx: &UserCtx, not_found: &str, forbidden: &str) -> Result<(i64, String), UseCaseError> {
        let order = self.repo.get_supplier_status(id).await.ok().flatten();
        let (order_supplier_id, order_status) = match order {
            Some(o) => o,
            None => return Err(UseCaseError::NotFound(not_found.to_string())),
        };
        if !can_access_purchase_order(ctx, order_supplier_id) {
            return Err(UseCaseError::Forbidden(forbidden.to_string()));
        }
        Ok((order_supplier_id, order_status))
    }
}

impl<R: PurchaseOrderRepo, P: PriceScheduleRepo> PurchaseOrderUseCase for PurchaseOrderService<R, P> {
    async fn create(&self, req: PurchaseOrderReq, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError> {
        // 仅已审核的供应商允许下单（id<=0 视为通过，避免误伤无关联的零值）
        if req.supplier_id > 0 {
            let status = self.repo.supplier_audit_status(req.supplier_id).await.ok().flatten();
            if status.as_deref() != Some("confirmed") {
                return Err(UseCaseError::BadRequest("该供应商尚未审核通过，暂不能下单".to_string()));
            }
        }

        // 行级数据权限：supplier 只能为自己绑定的供应商创建采购单
        if ctx.role == "supplier" {
            let effective_supplier_id = if req.supplier_id != 0 { req.supplier_id } else { ctx.supplier_id };
            if ctx.supplier_id == 0 || effective_supplier_id != ctx.supplier_id {
                return Err(UseCaseError::Forbidden("供应商账号只能为自己创建采购单".to_string()));
            }
        }

        let (main_wh_id, main_wh_name) = derive_main_warehouse(&req.items);

        let order_id = match self.repo.insert_order(&req, main_wh_id, &main_wh_name).await {
            Ok(id) => id,
            Err(_) => return Err(UseCaseError::Storage("创建失败".to_string())),
        };

        if !req.items.is_empty() {
            let base_qtys = self.repo.compute_base_quantities(&base_inputs_of(&req.items)).await;
            let _ = self.repo.insert_items_batch(order_id, &req.items, &base_qtys).await;
            // 采购入库后更新商品进价（当前/最高/最低）
            let _ = self.price.update_product_purchase_prices(&price_inputs_of(&req.items)).await;
        }

        Ok(PurchaseOrderOutcome::with_log(
            "创建成功".to_string(),
            order_id.to_string(),
            format!("创建采购单 {}（供应商ID={}）", req.order_no, req.supplier_id),
        ))
    }

    async fn list(&self, ctx: &UserCtx, filter: PurchaseOrderListFilter) -> Result<String, UseCaseError> {
        // 行级数据权限：supplier 角色强制只看自己绑定的供应商
        let supplier_id: Option<i64> = if ctx.role == "supplier" {
            if ctx.supplier_id > 0 { Some(ctx.supplier_id) } else { Some(-1) /* 未绑定则查不到任何数据 */ }
        } else {
            filter.supplier_id
        };

        let is_settled_filter: Option<String> = match filter.is_settled.as_deref() {
            Some("0") | Some("1") => filter.is_settled.clone(),
            _ => None,
        };

        // 排序处理
        let order_clause = match filter.sort_field.as_str() {
            "order_no" => format!("po.order_no {}", filter.sort_order),
            "order_date" => format!("po.order_date {}", filter.sort_order),
            "unit_name" => format!("s.name {}", filter.sort_order),
            "status" => format!("po.status {}", filter.sort_order),
            _ => format!("po.id {}", filter.sort_order),
        };

        let normalized = PurchaseOrderListFilter {
            supplier_id,
            is_settled: is_settled_filter,
            ..filter.clone()
        };

        let total = self.repo.count_list(&normalized).await
            .map_err(|e| UseCaseError::Storage(e.to_string()))?;
        let rows = self.repo.list(&normalized, &order_clause).await.unwrap_or_default();

        let orders: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                // 仓库展示规则：
                // 1) 明细里无任何仓库名 → 沿用主表仓库名
                // 2) 明细里只有一个去重仓库名 → 显示该仓库名
                // 3) 明细里有 ≥2 个去重仓库名 → 显示"综合"
                let display_warehouse = if row.item_warehouse_count == 0 {
                    row.warehouse_name.clone().unwrap_or_default()
                } else if row.item_warehouse_count == 1 {
                    row.item_warehouse_names.clone().unwrap_or_default()
                } else {
                    "综合".to_string()
                };

                serde_json::json!({
                    "id": row.id,
                    "order_no": row.order_no,
                    "order_date": row.order_date,
                    "total_amount": row.total_amount,
                    "discount_rate": row.discount_rate,
                    "amount_reduction": row.amount_reduction,
                    "final_amount": row.final_amount,
                    "warehouse_id": row.warehouse_id,
                    "warehouse_name": display_warehouse,
                    "status": row.status,
                    "remark": row.remark,
                    "supplier_name": row.supplier_name,
                    "is_settled": row.is_settled,
                })
            })
            .collect();

        let page = filter.page;
        let page_size = filter.page_size;
        let result = serde_json::json!({
            "data": orders,
            "page": page,
            "page_size": page_size,
            "total": total,
            "total_pages": (total + page_size - 1) / page_size
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn detail(&self, id: i64, ctx: &UserCtx) -> Result<String, UseCaseError> {
        let row = self.repo.get_order_detail(id).await.ok().flatten();
        let row = match row {
            Some(r) => r,
            None => return Err(UseCaseError::NotFound("订单不存在".to_string())),
        };
        // 行级数据权限：supplier 只能看自己的
        if !can_access_purchase_order(ctx, row.supplier_id) {
            return Err(UseCaseError::Forbidden("您没有权限查看此订单".to_string()));
        }

        let item_rows = self.repo.list_items(id).await.unwrap_or_default();
        let items: Vec<serde_json::Value> = item_rows
            .iter()
            .map(|r| serde_json::json!({
                "id": r.id,
                "product_id": r.product_id,
                "product_name": r.product_name,
                "alias1": r.alias1,
                "alias2": r.alias2,
                "spec": r.spec,
                "unit": r.unit,
                "unit_price": r.unit_price,
                "quantity": r.quantity,
                "base_quantity": r.base_quantity,
                "amount": r.amount,
                "ordered_quantity": r.ordered_quantity,
                "remark": r.remark,
                "warehouse_id": r.warehouse_id,
                "warehouse_name": r.warehouse_name,
                "production_date": r.production_date,
                "batch_no": r.batch_no,
            }))
            .collect();

        let order = serde_json::json!({
            "id": row.id,
            "supplier_id": row.supplier_id,
            "order_no": row.order_no,
            "order_date": row.order_date,
            "total_amount": row.total_amount,
            "discount_rate": row.discount_rate,
            "amount_reduction": row.amount_reduction,
            "final_amount": row.final_amount,
            "warehouse_id": row.warehouse_id,
            "warehouse_name": row.warehouse_name,
            "status": row.status,
            "version": row.version,
            "is_settled": row.is_settled,
            "remark": row.remark,
            "supplier_name": row.supplier_name,
            "items": items,
        });

        Ok(serde_json::to_string(&order).unwrap())
    }

    async fn update(&self, req: PurchaseOrderReq, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError> {
        // 行级数据权限：先查订单所属供应商
        let (_supplier_id, order_status) = self.guard(req.id.unwrap_or(0), ctx, "订单不存在", "您没有权限操作此订单").await?;
        // 仅待审核（pending）状态的订单允许修改；已审核/已流转/已作废的订单必须反审核后才能修改（防篡改）
        if order_status != "pending" {
            return Err(UseCaseError::BadRequest(format!("当前订单状态为「{}」，仅待审核状态的订单允许修改；已审核订单需管理员反审核后才能修改", order_status)));
        }
        // 强制乐观锁：req.version 必须传入且 > 0。
        // 前端一定传 version（从 loadOrderDetail 取得），如果缺失说明调用方不是前端合法路径，直接拒绝。
        let ver = match req.version {
            Some(v) if v > 0 => v,
            _ => return Err(UseCaseError::BadRequest("保存失败：缺少订单版本号，请刷新页面后重试".to_string())),
        };
        // 安全护栏：更新时明细不允许为空（防止并发覆盖导致明细被清空）
        if req.items.is_empty() {
            return Err(UseCaseError::BadRequest("保存失败：订单明细不能为空".to_string()));
        }

        // 主表仓库按明细汇总（与创建逻辑一致）
        let (main_wh_id, main_wh_name) = derive_main_warehouse(&req.items);

        // base_quantity 服务端统一重算（product_unit.ratio 权威），不信任前端值——与销售单同一规则
        let base_qtys = self.repo.compute_base_quantities(&base_inputs_of(&req.items)).await;

        match self.repo.update_order_with_items(&req, main_wh_id, &main_wh_name, ver, &base_qtys).await {
            Ok(()) => {}
            Err(DomainError::Conflict(_)) => {
                return Err(UseCaseError::RawResponse(409, format!("订单已被其他用户修改（版本 {} 已被覆盖），请刷新后重试", ver)));
            }
            Err(e) => return Err(e.into()),
        }
        let new_version = ver + 1;
        // 行为保持：旧实现从 req 回读"原值"（old_* 与提交值恒等，逐字保留该口径）
        let old_order_no = req.order_no.clone();
        let old_total = req.total_amount;
        let old_final = req.final_amount;

        if !req.items.is_empty() {
            // 采购单更新后同步商品进价（当前/最高/最低）
            let _ = self.price.update_product_purchase_prices(&price_inputs_of(&req.items)).await;
        }
        Ok(PurchaseOrderOutcome::with_log(
            "更新成功".to_string(),
            req.id.unwrap_or(0).to_string(),
            format!("更新采购单 {}（原单号 {}）：金额 {:.2}→{:.2}，下浮后合计 {:.2}→{:.2}，版本 {}→{}",
                req.order_no, old_order_no, old_total, req.total_amount, old_final, req.final_amount, ver, new_version),
        ))
    }

    async fn delete(&self, id: i64, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError> {
        // 行级数据权限 + 状态约束
        let (_supplier_id, order_status) = self.guard(id, ctx, "订单不存在", "您没有权限操作此订单").await?;
        if order_status != "pending" {
            return Err(UseCaseError::BadRequest(format!("当前订单状态为「{}」，仅待审核状态的订单允许删除；已审核订单需管理员反审核后才能删除", order_status)));
        }

        // 删除前收集本单涉及的商品，便于删除后从剩余采购历史重算最近/最高/最低进价
        let affected_products = self.repo.distinct_product_ids(id).await.unwrap_or_default();

        let _ = self.repo.delete_items_of_order(id).await;
        match self.repo.delete_order(id).await {
            Ok(()) => {
                // 删除成功后，对涉及的商品从剩余采购历史重算进价（回滚/刷新最高/最低/最近价）
                for pid in affected_products {
                    let _ = self.price.recalc_product_purchase_prices_from_history(pid).await;
                }
                Ok(PurchaseOrderOutcome::with_log(
                    "删除成功".to_string(),
                    id.to_string(),
                    format!("删除采购单 ID={}", id),
                ))
            }
            Err(_) => Err(UseCaseError::Storage("删除失败".to_string())),
        }
    }

    async fn settle(&self, id: i64, is_settled: Option<i64>, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError> {
        let (_supplier_id, order_status) = self.guard(id, ctx, "订单不存在", "您没有权限操作此订单").await?;
        // 审核后需先反审核
        if order_status != "pending" {
            return Err(UseCaseError::RawResponse(409, "该单据已审核，需先反审核后才能修改结算状态".to_string()));
        }

        let is_settled: i64 = is_settled.unwrap_or(0);
        if is_settled != 0 && is_settled != 1 {
            return Err(UseCaseError::BadRequest("参数无效".to_string()));
        }

        match self.repo.set_settled(id, is_settled).await {
            Ok(affected) if affected > 0 => {
                let label = if is_settled == 1 { "已结" } else { "未结" };
                Ok(PurchaseOrderOutcome::with_log(
                    "操作成功".to_string(),
                    id.to_string(),
                    format!("设置采购单结算状态为「{}」", label),
                ))
            }
            _ => Err(UseCaseError::RawResponse(409, "订单不存在或状态已变化".to_string())),
        }
    }

    async fn approve(&self, id: i64, reason: String, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError> {
        let (_supplier_id, order_status) = self.guard(id, ctx, "订单不存在", "您没有权限操作此订单").await?;
        if order_status == "confirmed" {
            return Err(UseCaseError::BadRequest("订单已审核，请勿重复操作".to_string()));
        }
        if order_status != "pending" {
            return Err(UseCaseError::BadRequest(format!("当前订单状态为「{}」，仅待审核状态的订单允许审核", order_status)));
        }

        match self.repo.approve_tx(id).await {
            Ok(()) => Ok(PurchaseOrderOutcome::with_log(
                "审核成功，订单已锁定".to_string(),
                id.to_string(),
                format!("审核通过采购单 ID={}（{}）", id, if reason.is_empty() { "无备注" } else { &reason }),
            )),
            Err(DomainError::Conflict(_)) => {
                Err(UseCaseError::RawResponse(409, "订单状态已变化，请刷新后重试".to_string()))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn unapprove(&self, id: i64, reason: String, ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError> {
        if reason.is_empty() {
            return Err(UseCaseError::BadRequest("反审核必须填写原因".to_string()));
        }

        let (_supplier_id, order_status) = self.guard(id, ctx, "订单不存在", "您没有权限操作此订单").await?;
        // 超级管理员拥有任何时刻的反审核权限；其他角色仅允许对已审核（confirmed）订单反审核
        let is_super_admin = ctx.role == "super_admin";
        if !is_super_admin && order_status != "confirmed" {
            return Err(UseCaseError::BadRequest(format!("当前订单状态为「{}」，仅已审核状态的订单允许反审核", order_status)));
        }

        match self.repo.unapprove_tx(id, is_super_admin).await {
            Ok(()) => Ok(PurchaseOrderOutcome::with_log(
                "反审核成功，订单已解锁".to_string(),
                id.to_string(),
                format!("反审核采购单 ID={}，原因：{}", id, reason),
            )),
            Err(DomainError::Conflict(_)) => {
                Err(UseCaseError::RawResponse(409, "订单状态已变化，请刷新后重试".to_string()))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn export(&self, ctx: &UserCtx, keyword_pattern: String, supplier_id: Option<i64>) -> Result<Vec<u8>, UseCaseError> {
        // 行级数据权限 + 筛选：supplier 强制只看自己，其他角色可按 supplier_id 过滤
        let supplier_id: Option<i64> = if ctx.role == "supplier" {
            if ctx.supplier_id > 0 { Some(ctx.supplier_id) } else { Some(-1) }
        } else {
            supplier_id
        };

        let rows = self.repo.export_rows(&keyword_pattern, supplier_id).await.unwrap_or_default();
        build_export_workbook(rows).map_err(|e| UseCaseError::Storage(format!("导出失败: {}", e)))
    }

    async fn print_excel(&self, id: i64, user_id_override: Option<i64>) -> Result<(Vec<u8>, String), UseCaseError> {
        let order = match self.repo.get_print_order(id).await.ok().flatten() {
            Some(o) => o,
            None => return Err(UseCaseError::NotFound("采购订单不存在".to_string())),
        };
        let items = self.repo.get_print_items(id).await.unwrap_or_default();

        // 如果传入了 user_id 则优先使用参数里的；否则使用订单里存的
        let (mut handler_name, mut handler_phone) = (
            order.user_name.clone().unwrap_or_default(),
            order.handler_phone.clone().unwrap_or_default(),
        );
        if let Some(uid) = user_id_override {
            if uid > 0 {
                if let Some((nickname, phone)) = self.repo.user_nickname_phone(uid).await.ok().flatten() {
                    handler_name = nickname;
                    handler_phone = phone;
                }
            }
        }
        let export_filename = format!("采购单_{}.xlsx", order.order_no);

        let data = build_print_workbook(&order, &items, &handler_name, &handler_phone)
            .map_err(|e| UseCaseError::Storage(format!("导出失败: {}", e)))?;
        Ok((data, export_filename))
    }

    async fn import(&self, bytes: Vec<u8>, _ctx: &UserCtx) -> Result<PurchaseOrderOutcome, UseCaseError> {
        let rows: Vec<Vec<String>>;

        if bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
            match open_workbook_auto_from_rs(std::io::Cursor::new(bytes)) {
                Ok(mut workbook) => {
                    let sheets = workbook.sheet_names().to_vec();
                    if sheets.is_empty() {
                        return Err(UseCaseError::BadRequest("Excel文件中没有工作表".to_string()));
                    }

                    let range = match workbook.worksheet_range(&sheets[0]) {
                        Ok(r) => r,
                        Err(e) => return Err(UseCaseError::BadRequest(format!("无法读取Excel文件内容: {}", e))),
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
                    return Err(UseCaseError::BadRequest(format!("读取Excel文件失败: {}", e)));
                }
            }
        } else {
            let content_str = String::from_utf8_lossy(&bytes).to_string();
            rows = common::utils::parse_csv(&content_str);
        }

        if rows.len() < 2 {
            return Err(UseCaseError::BadRequest("文件至少需要包含标题行和一行数据".to_string()));
        }

        let mut orders: std::collections::HashMap<String, (Vec<String>, Vec<Vec<String>>)> = std::collections::HashMap::new();

        for row in rows.iter().skip(1) {
            if row.len() < 3 {
                continue;
            }

            let order_no = row[1].trim().to_string();
            if order_no.is_empty() {
                continue;
            }

            if !orders.contains_key(&order_no) {
                orders.insert(order_no.clone(), (row.clone(), Vec::new()));
            }

            if row.len() > 9 && !row[9].trim().is_empty() {
                let item: Vec<String> = row[9..].to_vec();
                orders.get_mut(&order_no).unwrap().1.push(item);
            }
        }

        let mut success = 0;
        let mut failed = 0;

        for (order_no, (order_row, items)) in orders {
            let supplier_name = if order_row.len() > 3 { order_row[3].trim() } else { "" };
            let supplier_id = if !supplier_name.is_empty() {
                self.repo.find_supplier_id_by_name(supplier_name).await.ok().flatten()
            } else {
                None
            };

            if supplier_id.is_none() {
                failed += 1;
                continue;
            }

            let order_date = if order_row.len() > 2 { order_row[2].trim() } else { "" };
            if order_date.is_empty() {
                failed += 1;
                continue;
            }

            let total_amount: f64 = if order_row.len() > 4 { order_row[4].trim().parse().unwrap_or(0.0) } else { 0.0 };
            let discount_rate: f64 = if order_row.len() > 5 { order_row[5].trim().parse().unwrap_or(0.0) } else { 0.0 };
            let final_amount: f64 = if order_row.len() > 6 { order_row[6].trim().parse().unwrap_or(0.0) } else { 0.0 };
            let remark = if order_row.len() > 8 { order_row[8].trim() } else { "" };

            let main = PurchaseOrderImportMain {
                order_no: order_no.clone(),
                supplier_id: supplier_id.unwrap(),
                order_date: order_date.to_string(),
                total_amount,
                discount_rate,
                final_amount,
                remark: remark.to_string(),
            };
            let result = self.repo.insert_import_order(&main).await;

            match result {
                Ok((order_id, affected)) => {
                    if affected > 0 {
                        for item in items {
                            if item.len() < 1 {
                                continue;
                            }

                            let product_name = if item.len() > 0 { item[0].trim() } else { "" };
                            let alias1 = if item.len() > 1 { item[1].trim() } else { "" };
                            let alias2 = if item.len() > 2 { item[2].trim() } else { "" };
                            let spec = if item.len() > 3 { item[3].trim() } else { "" };
                            let unit = if item.len() > 4 { item[4].trim() } else { "个" };
                            let unit_price: f64 = if item.len() > 5 { item[5].trim().parse().unwrap_or(0.0) } else { 0.0 };
                            let quantity: f64 = if item.len() > 6 { item[6].trim().parse().unwrap_or(0.0) } else { 0.0 };
                            let amount: f64 = if item.len() > 8 { item[8].trim().parse().unwrap_or(0.0) } else { 0.0 };
                            let item_remark = if item.len() > 9 { item[9].trim() } else { "" };

                            let product_id: i64 = self.repo.find_product_id_by_name_spec(product_name, spec)
                                .await.ok().flatten().unwrap_or(0);

                            // base_quantity 服务端按 product_unit.ratio 重算（有 ratio 时权威，否则回退模板值或 quantity）
                            let computed_bq = if product_id > 0 {
                                self.repo.unit_ratio(product_id, unit).await.ok().flatten()
                                    .map(|ratio| ((quantity * ratio) * 100.0).round() / 100.0)
                            } else { None };
                            let bq = computed_bq.unwrap_or_else(|| {
                                let from_sheet = if item.len() > 7 { item[7].trim().parse().unwrap_or(0.0) } else { 0.0 };
                                if from_sheet > 0.0 { from_sheet } else { quantity }
                            });

                            let import_item = PurchaseOrderImportItem {
                                product_id,
                                product_name: product_name.to_string(),
                                alias1: alias1.to_string(),
                                alias2: alias2.to_string(),
                                spec: spec.to_string(),
                                unit: unit.to_string(),
                                unit_price,
                                quantity,
                                base_quantity: bq,
                                amount,
                                remark: item_remark.to_string(),
                            };
                            let _ = self.repo.insert_import_item(order_id, &import_item).await;
                        }
                        success += 1;
                    } else {
                        failed += 1;
                    }
                }
                Err(_) => {
                    failed += 1;
                }
            }
        }

        Ok(PurchaseOrderOutcome::with_log(
            format!("导入完成：成功 {} 条，失败 {} 条", success, failed),
            String::new(),
            format!("导入采购单：成功 {} 条，失败 {} 条", success, failed),
        ))
    }
}

/// 导出采购单列表工作簿（自旧 build_purchase_order_export_workbook 平移，Vec<SqliteRow> → DTO）
fn build_export_workbook(rows: Vec<PurchaseOrderExportRow>) -> Result<Vec<u8>, XlsxError> {
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
        let item_wh: String = row.item_warehouse_name.clone().unwrap_or_default().trim().to_string();
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
        group_amount += row.amount.unwrap_or(0.0);

        worksheet.write(row_idx, 0, row.id)?;
        worksheet.write(row_idx, 1, row.order_no.clone())?;
        worksheet.write(row_idx, 2, row.order_date.clone())?;
        worksheet.write(row_idx, 3, row.supplier_name.clone())?;
        worksheet.write(row_idx, 4, row.total_amount)?;
        worksheet.write(row_idx, 5, row.discount_rate)?;
        worksheet.write(row_idx, 6, row.final_amount)?;
        worksheet.write(row_idx, 7, row.status.clone())?;
        worksheet.write(row_idx, 8, row.remark.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 9, row.product_name.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 10, row.alias1.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 11, row.alias2.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 12, row.spec.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 13, row.unit.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 14, row.ordered_quantity.unwrap_or(0.0))?;
        worksheet.write(row_idx, 15, row.quantity.unwrap_or(0.0))?;
        worksheet.write(row_idx, 16, row.unit_price.unwrap_or(0.0))?;
        worksheet.write(row_idx, 17, row.base_quantity.unwrap_or(0.0))?;
        worksheet.write(row_idx, 18, row.amount.unwrap_or(0.0))?;
        worksheet.write(row_idx, 19, row.item_remark.clone().unwrap_or_default())?;
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
}

/// 导出采购单（打印模板样式）：单张采购单按打印模板格式导出（自旧 api_purchase_order_print_excel 平移）
fn build_print_workbook(
    order: &PurchaseOrderPrintRow,
    items: &[PurchaseOrderPrintItemRow],
    handler_name: &str,
    handler_phone: &str,
) -> Result<Vec<u8>, XlsxError> {
    let title_format = Format::new()
        .set_bold()
        .set_font_size(16)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);
    let header_format = Format::new()
        .set_bold()
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin);
    let cell_center = Format::new()
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin);
    // 合并单元格左对齐格式（默认不加自动换行）
    let info_left = Format::new()
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter);
    // 合并单元格左对齐格式（自动换行，仅用于 E2:F3 地址/长文本）
    let info_left_wrap = Format::new()
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();
    let sum_left_noline = Format::new()
        .set_bold()
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter);
    // 最后一行的"最终合计"靠右布局
    let sum_right_noline = Format::new()
        .set_bold()
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter);
    // 打印导出：分组标题（仓库）格式，灰底加粗
    let print_group_title = Format::new()
        .set_bold()
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_background_color("#E5E7EB")
        .set_font_color("#374151");
    // 打印导出：分组小计 A:D（保留 A 列左外框、右边框去掉，与 E:F 合并之间无竖线）
    let print_summary_format = Format::new()
        .set_bold()
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_border_left(FormatBorder::Thin)
        .set_border_right(FormatBorder::None)
        .set_border_top(FormatBorder::Thin)
        .set_border_bottom(FormatBorder::Thin);
    // 打印导出：小计 E:F 合并（左边框去掉，与小计文本之间无竖线；F 列右外框保留）
    let print_summary_right = Format::new()
        .set_bold()
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_border_left(FormatBorder::None)
        .set_border_right(FormatBorder::Thin)
        .set_border_top(FormatBorder::Thin)
        .set_border_bottom(FormatBorder::Thin)
        .set_num_format("¥#,##0.00");
    // 打印导出：总计 A:D 灰底（A 列左外框保留、右边框去掉）
    let print_grand_total_label = Format::new()
        .set_bold()
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_background_color("#E5E7EB")
        .set_font_color("#374151")
        .set_border_left(FormatBorder::Thin)
        .set_border_right(FormatBorder::None)
        .set_border_top(FormatBorder::Thin)
        .set_border_bottom(FormatBorder::Thin);
    // 打印导出：总计 E:F 合并 灰底（左边框去掉；F 列右外框保留）
    let print_grand_total_amount = Format::new()
        .set_bold()
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_background_color("#E5E7EB")
        .set_font_color("#374151")
        .set_border_left(FormatBorder::None)
        .set_border_right(FormatBorder::Thin)
        .set_border_top(FormatBorder::Thin)
        .set_border_bottom(FormatBorder::Thin)
        .set_num_format("¥#,##0.00");
    // 货币格式（¥ 前缀，右对齐）
    let currency_right = Format::new()
        .set_num_format("¥#,##0.00")
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin);
    let cell_left = Format::new()
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin);
    let cell_right = Format::new()
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin);

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("采购单")?;
    // 页面设置：241-2S 两层两等份，0 左右边距，水平居中，横向
    ws.set_landscape();
    ws.set_margins(0.0, 0.0, 0.0, 0.4, 0.0, 0.0);
    ws.set_print_center_horizontally(true);

    // 列宽（用户指定，cm→px 按 96dpi 换算，1cm≈38px）：A=4cm B=1cm C=1cm D=2cm E=2cm F=2cm
    ws.set_column_width_pixels(0, 150)?;
    ws.set_column_width_pixels(1, 40)?;
    ws.set_column_width_pixels(2, 50)?;
    ws.set_column_width_pixels(3, 80)?;
    ws.set_column_width_pixels(4, 80)?;
    ws.set_column_width_pixels(5, 50)?;

    // 行 0: 标题（采购单），合并 A-F
    ws.merge_range(0, 0, 0, 5, "采购单", &title_format)?;
    ws.set_row_height(0, 28)?;

    // 准备主表数据用于页眉
    let order_no = order.order_no.clone();
    let order_date = order.order_date.clone();
    let supplier_name = order.supplier_name.clone().unwrap_or_default();
    let supplier_phone = order.supplier_phone.clone().unwrap_or_default();
    let supplier_addr = order.supplier_address.clone().unwrap_or_default();
    let remark_val = order.remark.clone().unwrap_or_default();

    // ---- 排版：A+B / C+D / E+F 三段两等份 ----
    // 每行 6 列：A(20)+B(5)=25, C(6)+D(8)=14, E(10)+F(18)=28
    // 参考截图：订单号+日期+地址（地址跨两行）、供应商+联系、经手人+联系+备注
    // -------------------------------------------------------------------

    // 行 1: 间隔
    ws.set_row_height(1, 6)?;

    // 行 2: A+B="订单号：xxx", C+D="日期：xxx"
    let cell_ab2 = format!("订单号：{}", order_no);
    let cell_cd2 = format!("日期：{}", order_date);
    ws.merge_range(2, 0, 2, 1, cell_ab2.as_str(), &info_left)?;
    ws.merge_range(2, 2, 2, 3, cell_cd2.as_str(), &info_left)?;

    // 行 3: A+B="供应商：xxx", C+D="联系：xxx"
    let cell_ab3 = format!("供应商：{}", supplier_name);
    let cell_cd3 = format!("联系：{}", supplier_phone);
    ws.merge_range(3, 0, 3, 1, cell_ab3.as_str(), &info_left)?;
    ws.merge_range(3, 2, 3, 3, cell_cd3.as_str(), &info_left)?;

    // 行 2~3: E+F 合并（跨两行）写"地址：xxx"，靠左自动换行
    let cell_ef23 = format!("地址：{}", supplier_addr);
    ws.merge_range(2, 4, 3, 5, cell_ef23.as_str(), &info_left_wrap)?;

    // 行 4: A+B="经手人：xxx", C+D="联系：xxx", E+F="备注：xxx"
    let cell_ab4 = format!("经手人：{}", handler_name);
    let cell_cd4 = format!("联系：{}", handler_phone);
    let cell_ef4 = format!("备注：{}", remark_val);
    ws.merge_range(4, 0, 4, 1, cell_ab4.as_str(), &info_left)?;
    ws.merge_range(4, 2, 4, 3, cell_cd4.as_str(), &info_left)?;
    ws.merge_range(4, 4, 4, 5, cell_ef4.as_str(), &info_left)?;

    // 行 5: 间隔
    ws.set_row_height(5, 6)?;

    // 行 6: 表头
    let header_row = 6u32;
    let headers = ["品名规格", "单位", "数量", "单价", "金额", "备注"];
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(header_row, i as u16, *h, &header_format)?;
    }
    ws.set_row_height(header_row, 22)?;

    // ---- 打印分页设置 ----
    // 页头（每一页都显示标题+主表+间隔+表头）：重复行 0~6
    let _ = ws.set_repeat_rows(0, 6);
    // 页脚：居中"第 X 页，共 Y 页"
    ws.set_footer("&C第 &P 页，共 &N 页");

    // ---- 明细按仓库分组：分组标题 + 明细 + 小计 + 总计 ----
    // 预估行数：每组 = 1（标题） + N（明细） + 1（小计），加最后 1（总计）
    let mut cur_row: u32 = 7; // 紧接表头行 6
    let mut grand_item_count: i64 = 0;
    let mut grand_amount: f64 = 0.0;

    // 按仓库分组（保持 SQL 排序顺序：warehouse_name, id）
    let mut last_wh: Option<String> = None;
    let mut group_buf: Vec<&PurchaseOrderPrintItemRow> = Vec::new();
    let mut finished_groups: Vec<(String, Vec<&PurchaseOrderPrintItemRow>)> = Vec::new();

    // 先把所有分组聚合好，便于最后按顺序统一输出（避免在循环内同时写小计/总计造成行索引管理复杂）
    for it in items {
        let wh = it.warehouse_name.trim().to_string();
        if last_wh.as_deref() != Some(wh.as_str()) {
            if let Some(prev) = last_wh.take() {
                let prev_display = if prev.is_empty() { "未指定".to_string() } else { prev.clone() };
                finished_groups.push((prev_display, std::mem::take(&mut group_buf)));
            }
            last_wh = Some(wh);
            group_buf.push(it);
        } else {
            group_buf.push(it);
        }
    }
    if let Some(prev) = last_wh.take() {
        let prev_display = if prev.is_empty() { "未指定".to_string() } else { prev.clone() };
        finished_groups.push((prev_display, group_buf));
    }

    for (wh_display, group_items) in &finished_groups {
        // 分组标题
        let title = format!("├── {}", wh_display);
        ws.merge_range(cur_row, 0, cur_row, 5, title.as_str(), &print_group_title)?;
        ws.set_row_height(cur_row, 20)?;
        cur_row += 1;
        for it in group_items {
            // 导出名称优先级：规格 -> 名称（独立取值不拼接，规格为空时才用名称）
            let name_spec = match it.spec.as_deref().map(str::trim) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => it.product_name.clone(),
            };
            ws.write_with_format(cur_row, 0, name_spec, &cell_left)?;
            ws.write_with_format(cur_row, 1, it.unit.clone().unwrap_or_default(), &cell_center)?;
            ws.write_with_format(cur_row, 2, it.quantity, &cell_right)?;
            ws.write_with_format(cur_row, 3, it.unit_price, &currency_right)?;
            ws.write_with_format(cur_row, 4, it.amount, &currency_right)?;
            ws.write_with_format(cur_row, 5, it.remark.clone().unwrap_or_default(), &cell_left)?;
            ws.set_row_height(cur_row, 22)?;
            cur_row += 1;
            grand_item_count += 1;
            grand_amount += it.amount;
        }
        // 分组小计：A:D 合并写文本，E:F 合并写金额（合并单元格的左竖线去除）
        let group_amount: f64 = group_items.iter().map(|x| x.amount).sum();
        let subtotal = format!("小计: 包装数量 {}", group_items.len());
        ws.merge_range(cur_row, 0, cur_row, 3, subtotal.as_str(), &print_summary_format)?;
        ws.merge_range(cur_row, 4, cur_row, 5, "", &print_summary_right)?;
        ws.write_with_format(cur_row, 4, group_amount, &print_summary_right)?;
        ws.set_row_height(cur_row, 20)?;
        cur_row += 1;
    }

    // 总计：A:D 合并写文本，E:F 合并写金额（合并单元格的左竖线去除，整行灰底）
    let grand_total = format!("总计: 包装数量 {}", grand_item_count);
    ws.merge_range(cur_row, 0, cur_row, 3, grand_total.as_str(), &print_grand_total_label)?;
    ws.merge_range(cur_row, 4, cur_row, 5, "", &print_grand_total_amount)?;
    ws.write_with_format(cur_row, 4, grand_amount, &print_grand_total_amount)?;
    ws.set_row_height(cur_row, 22)?;
    cur_row += 1;

    let sum_row = cur_row;

    // ---- 合计排版（A+B | C+D 靠左；E+F 最终合计靠右）----
    let cell_sum = format!("合计金额: ¥{:.2}", order.total_amount);
    let cell_discount = format!("折减金额: ¥{:.2}", order.amount_reduction);
    let cell_final = format!("最终合计: ¥{:.2}", order.final_amount);
    ws.merge_range(sum_row, 0, sum_row, 1, cell_sum.as_str(), &sum_left_noline)?;
    ws.merge_range(sum_row, 2, sum_row, 3, cell_discount.as_str(), &sum_left_noline)?;
    ws.merge_range(sum_row, 4, sum_row, 5, cell_final.as_str(), &sum_right_noline)?;

    wb.save_to_buffer()
}
