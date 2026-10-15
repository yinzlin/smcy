//! 销售订单用例：创建 / 列表 / by_purchaser / 详情 / 乐观锁更新 / 调价 / 删除 / 结算 / 状态流转 / 审核 / 反审核 / 导出 / 导入
//!
//! 行为与旧 api_sales_order_* handler 逐字一致：含错误文案、状态码、吞错分支、
//! 价格策略时段补价（政采价优先，否则商超1/2/3/AI 非零平均价）与操作日志文案。

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use chrono::Local;
use common::utils::sanitize_xlsx_string;
use domain::error::DomainError;
use domain::port::sales_order::*;
use domain::{SalesOrderItemReq, SalesOrderReq, UserCtx};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook, XlsxError};

use super::get_category_sort_key;
use super::price::recalc_base_price_by_markup;
use crate::error::UseCaseError;
use crate::port::sales_order::{SalesOrderLog, SalesOrderOutcome, SalesOrderUseCase};

#[derive(Clone)]
pub struct SalesOrderService<R: SalesOrderRepo> {
    repo: R,
}

impl<R: SalesOrderRepo> SalesOrderService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

/// 行级数据权限（自 backend middleware::auth::can_access_sales_order 平移，逻辑逐字一致）：
/// admin/super_admin 可见全部；purchaser 仅可见自己绑定的采购单位
fn can_access_sales_order(user: &UserCtx, order_purchaser_id: i64) -> bool {
    match user.role.as_str() {
        "super_admin" | "admin" => true,
        "purchaser" => user.purchaser_id == order_purchaser_id,
        _ => false,
    }
}

/// 数量同步兜底：保存时若销售数量为 0 而预售数量 > 0，用预售数量兜底，
/// 避免后续生成采购时数量为 0 的空明细（创建/更新共用）
fn normalize_quantities(items: &mut [SalesOrderItemReq]) {
    for it in items.iter_mut() {
        if it.quantity <= 0.0 && it.pre_sale_quantity.unwrap_or(0.0) > 0.0 {
            it.quantity = it.pre_sale_quantity.unwrap();
        }
    }
}

/// base_quantity 服务端统一重算入参组装（product_unit.ratio 权威），不信任前端值
fn base_inputs_of(items: &[SalesOrderItemReq]) -> Vec<(i64, String, f64, f64)> {
    items.iter()
        .map(|it| (it.product_id, it.unit.clone().unwrap_or_default(), it.quantity, it.base_quantity.unwrap_or(0.0)))
        .collect()
}

impl<R: SalesOrderRepo> SalesOrderService<R> {
    /// 详情/更新/调价/删除/结算/审核/反审核共用护栏：订单存在性 + 行级数据权限
    async fn guard(&self, id: i64, ctx: &UserCtx) -> Result<(i64, String), UseCaseError> {
        let order = self.repo.get_purchaser_status(id).await.ok().flatten();
        let (order_purchaser_id, order_status) = match order {
            Some(o) => o,
            None => return Err(UseCaseError::NotFound("订单不存在".to_string())),
        };
        if !can_access_sales_order(ctx, order_purchaser_id) {
            return Err(UseCaseError::Forbidden("您没有权限操作此订单".to_string()));
        }
        Ok((order_purchaser_id, order_status))
    }

    /// 价格策略时段补价：对 unit_price<=0 的明细按 order_date 查时段价。
    /// 政采价(>0) 优先，否则商超1/2/3/AI 非零平均价；返回补价条数。
    /// 前端已手工填价的保留原值不被覆盖；order_date 非法/为空时不补。
    async fn auto_fill_prices(&self, items: &mut [SalesOrderItemReq], order_date: &str) -> (String, usize) {
        let order_date_short = if order_date.len() >= 10 { order_date[..10].to_string() } else { order_date.to_string() };
        if order_date_short.is_empty() || !common::utils::is_valid_date_str(&order_date_short) {
            return (order_date_short, 0);
        }
        let mut product_ids: Vec<i64> = Vec::new();
        for it in items.iter() {
            if it.product_id > 0 && it.unit_price <= 0.0 {
                product_ids.push(it.product_id);
            }
        }
        if product_ids.is_empty() {
            return (order_date_short, 0);
        }
        let gov_map = crate::service::price::batch_lookup_effective_prices(&product_ids, "gov_procurement", &order_date_short).await;
        let sup1_map = crate::service::price::batch_lookup_effective_prices(&product_ids, "supermarket_1", &order_date_short).await;
        let sup2_map = crate::service::price::batch_lookup_effective_prices(&product_ids, "supermarket_2", &order_date_short).await;
        let sup3_map = crate::service::price::batch_lookup_effective_prices(&product_ids, "supermarket_3", &order_date_short).await;
        let ai_map = crate::service::price::batch_lookup_effective_prices(&product_ids, "ai_realtime", &order_date_short).await;
        let mut auto_filled = 0;
        for it in items.iter_mut() {
            if it.product_id > 0 && it.unit_price <= 0.0 {
                // 政采价(>0) 优先，否则商超1/2/3/AI 非零平均价
                let gov = gov_map.get(&it.product_id).copied().filter(|p| *p > 0.0);
                let new_price = if let Some(g) = gov {
                    g
                } else {
                    let vals: Vec<f64> = [
                        sup1_map.get(&it.product_id).copied().unwrap_or(0.0),
                        sup2_map.get(&it.product_id).copied().unwrap_or(0.0),
                        sup3_map.get(&it.product_id).copied().unwrap_or(0.0),
                        ai_map.get(&it.product_id).copied().unwrap_or(0.0),
                    ].into_iter().filter(|v| *v > 0.0).collect();
                    if vals.is_empty() {
                        0.0
                    } else {
                        (vals.iter().sum::<f64>() / vals.len() as f64 * 100.0).round() / 100.0
                    }
                };
                if new_price > 0.0 {
                    it.unit_price = new_price;
                    // amount 同步：使用现 quantity 重新计算（避免前端传的 0 金额）
                    if it.quantity > 0.0 && it.amount <= 0.0 {
                        it.amount = new_price * it.quantity;
                    }
                    auto_filled += 1;
                }
            }
        }
        (order_date_short, auto_filled)
    }
}

impl<R: SalesOrderRepo> SalesOrderUseCase for SalesOrderService<R> {
    async fn create(&self, mut req: SalesOrderReq, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        // 仅已审核的采购方与商品允许下单（id<=0 视为通过，避免误伤无关联的零值）
        if req.purchaser_id > 0 {
            let status = self.repo.purchaser_audit_status(req.purchaser_id).await.ok().flatten();
            if status.as_deref() != Some("confirmed") {
                return Err(UseCaseError::BadRequest("该采购方尚未审核通过，暂不能下单".to_string()));
            }
        }
        for item in &req.items {
            if item.product_id > 0 {
                let status = self.repo.product_audit_status(item.product_id).await.ok().flatten();
                if status.as_deref() != Some("confirmed") {
                    return Err(UseCaseError::BadRequest(format!("商品「{}」尚未审核通过，暂不能下单", item.product_name)));
                }
            }
        }

        // 行级数据权限：purchaser 只能为自己绑定的采购单位创建销售单
        if ctx.role == "purchaser" {
            let effective_purchaser_id = if req.purchaser_id != 0 { req.purchaser_id } else { ctx.purchaser_id };
            if ctx.purchaser_id == 0 || effective_purchaser_id != ctx.purchaser_id {
                return Err(UseCaseError::Forbidden("采购单位账号只能为自己创建销售单".to_string()));
            }
        }

        // 价格策略时段补价：补录历史销售单时，前端可能传 unit_price=0
        // 此处按 req.order_date 自动从 product_price_schedule 查 (gov_procurement, supermarket_*) 价
        // 仅补 unit_price <= 0 的明细；前端已手工填价的保留原值不被覆盖。
        // 支持补录：order_date 为任意过去日期，自动匹配对应时段价。
        let mut logs: Vec<SalesOrderLog> = Vec::new();
        if !req.items.is_empty() {
            let (order_date_short, auto_filled) = self.auto_fill_prices(&mut req.items, &req.order_date).await;
            if auto_filled > 0 {
                logs.push(SalesOrderLog::new(
                    "价格策略自动补价",
                    "0".to_string(),
                    format!("销售单创建按 order_date={} 自动补价 {} 条", order_date_short, auto_filled),
                ));
            }
        }

        // 数量同步：保存时若销售数量为 0 而预售数量 > 0，用预售数量兜底
        normalize_quantities(&mut req.items);

        let order_id = match self.repo.insert_order(&req).await {
            Ok(id) => id,
            Err(_) => return Err(UseCaseError::Storage("创建失败".to_string())),
        };

        if !req.items.is_empty() {
            let base_qtys = database::sqlite::repo::stock_writer::compute_base_quantities(
                database::pool(), &base_inputs_of(&req.items)).await;
            let _ = self.repo.insert_items_batch(order_id, &req.items, &base_qtys).await;
        }

        logs.push(SalesOrderLog::new(
            "sales_order.create",
            order_id.to_string(),
            format!("创建销售单 {}（采购单位ID={}）", req.order_no, req.purchaser_id),
        ));
        Ok(SalesOrderOutcome::with_logs("创建成功".to_string(), logs))
    }

    async fn list(&self, ctx: &UserCtx, filter: SalesOrderListFilter) -> Result<String, UseCaseError> {
        // 行级数据权限：purchaser 角色强制只看自己绑定的采购单位
        let purchaser_id: Option<i64> = if ctx.role == "purchaser" {
            if ctx.purchaser_id > 0 { Some(ctx.purchaser_id) } else { Some(-1) /* 未绑定则查不到任何数据 */ }
        } else {
            filter.purchaser_id
        };

        let is_settled_filter: Option<String> = match filter.is_settled.as_deref() {
            Some("0") | Some("1") => filter.is_settled.clone(),
            _ => None,
        };

        // 排序处理
        let order_clause = match filter.sort_field.as_str() {
            "order_no" => format!("so.order_no {}", filter.sort_order),
            "order_date" => format!("so.order_date {}", filter.sort_order),
            "unit_name" => format!("p.name {}", filter.sort_order),
            "status" => format!("so.status {}", filter.sort_order),
            _ => format!("so.id {}", filter.sort_order),
        };

        let normalized = SalesOrderListFilter {
            purchaser_id,
            is_settled: is_settled_filter,
            ..filter.clone()
        };

        let total = self.repo.count_list(&normalized).await
            .map_err(|e| UseCaseError::Storage(e.to_string()))?;
        let rows = self.repo.list(&normalized, &order_clause).await.unwrap_or_default();

        let orders: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| serde_json::json!({
                "id": row.id,
                "order_no": row.order_no,
                "order_date": row.order_date,
                "total_amount": row.total_amount,
                "discount_rate": row.discount_rate,
                "amount_reduction": row.amount_reduction,
                "final_amount": row.final_amount,
                "warehouse_id": row.warehouse_id,
                "warehouse_name": row.warehouse_name,
                "status": row.status,
                "remark": row.remark,
                "purchaser_name": row.purchaser_name,
                "is_reimburse": row.supplement_count > 0,
                "is_settled": row.is_settled,
            }))
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

    async fn by_purchaser(&self, purchaser_id: i64, ctx: &UserCtx) -> Result<String, UseCaseError> {
        // 行级数据权限：purchaser 角色只能查自己绑定的采购单位
        let effective_purchaser_id = if ctx.role == "purchaser" {
            if ctx.purchaser_id > 0 { ctx.purchaser_id } else { -1 }
        } else {
            purchaser_id
        };

        let orders = self.repo.list_by_purchaser(effective_purchaser_id).await.unwrap_or_default();

        let order_ids: Vec<i64> = orders.iter().map(|row| row.id).collect();

        let mut allocation_status_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        if !order_ids.is_empty() {
            let rows = self.repo.allocation_status_rows(&order_ids).await.unwrap_or_default();
            for (source_id, status) in rows {
                // 与 summary 保持一致：仅保留最早一条记录
                allocation_status_map.entry(source_id).or_insert(status);
            }
        }

        let result: Vec<serde_json::Value> = orders.iter().map(|row| {
            let allocation_status = allocation_status_map.get(&row.id).copied().unwrap_or(-1);
            serde_json::json!({
                "id": row.id,
                "order_no": row.order_no,
                "order_date": row.order_date,
                "total_amount": row.total_amount,
                "discount_rate": row.discount_rate,
                "discount_total": row.discount_total,
                "status": row.status,
                "allocation_status": allocation_status,
                "purchaser_name": row.purchaser_name.clone().unwrap_or_default(),
                "purchaser_id": row.purchaser_id,
            })
        }).collect();

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn detail(&self, id: i64, ctx: &UserCtx) -> Result<String, UseCaseError> {
        // 行级数据权限：先确认订单存在并校验归属
        let exists = match self.repo.get_detail_purchaser_id(id).await {
            Some(pid) => {
                if !can_access_sales_order(ctx, pid) {
                    return Err(UseCaseError::Forbidden("您没有权限查看此订单".to_string()));
                }
                true
            }
            None => false,
        };

        if !exists {
            return Err(UseCaseError::NotFound(format!("订单不存在 (ID: {})", id)));
        }

        let row = self.repo.get_order_detail(id).await
            .map_err(|e| match e {
                DomainError::Storage(m) => UseCaseError::Storage(format!("查询失败：{}", m)),
                other => UseCaseError::from(other),
            })?;
        let row = match row {
            Some(r) => r,
            None => {
                return Err(UseCaseError::Storage(format!("订单存在但JOIN查询失败 (ID: {})", id)));
            }
        };

        let item_rows = self.repo.list_items(id).await
            .map_err(|e| match e {
                DomainError::Storage(m) => UseCaseError::Storage(format!("查询订单明细失败：{}", m)),
                other => UseCaseError::from(other),
            })?;

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
                "pre_sale_quantity": r.pre_sale_quantity,
                "supplier_id": r.supplier_id,
                "supplier_name": r.supplier_name,
                "production_date": r.production_date,
                "batch_no": r.batch_no,
                "remark": r.remark,
            }))
            .collect();

        let order = serde_json::json!({
            "id": row.id,
            "purchaser_id": row.purchaser_id,
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
            "remark": row.remark,
            "customer_order_image": row.customer_order_image,
            "signed_order_image": row.signed_order_image,
            "supplier_company": row.supplier_company,
            "truck_plate": row.truck_plate,
            "is_settled": row.is_settled,
            "purchaser_name": row.purchaser_name,
            "items": items,
        });

        match serde_json::to_string(&order) {
            Ok(json_str) => Ok(json_str),
            Err(e) => Err(UseCaseError::Storage(format!("序列化订单JSON失败：{}", e))),
        }
    }

    async fn update(&self, mut req: SalesOrderReq, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        // 行级数据权限 + 存在性
        let (_order_purchaser_id, order_status) = self.guard(req.id.unwrap_or(0), ctx).await?;
        // 仅待审核（pending）状态允许修改；已审核/已流转的订单必须反审核后才能修改（防篡改）
        if order_status != "pending" {
            return Err(UseCaseError::BadRequest(format!("当前订单状态为「{}」，仅待审核状态的订单允许修改；已审核订单需管理员反审核后才能修改", order_status)));
        }
        // 价格策略时段补价：编辑时若修改了 order_date 为历史日期，对 unit_price=0 的明细
        // 仍按当前 req.order_date 自动补价。
        let mut logs: Vec<SalesOrderLog> = Vec::new();
        if !req.items.is_empty() {
            let (order_date_short, auto_filled) = self.auto_fill_prices(&mut req.items, &req.order_date).await;
            if auto_filled > 0 {
                logs.push(SalesOrderLog::new(
                    "价格策略自动补价",
                    req.id.unwrap_or(0).to_string(),
                    format!("销售单编辑按 order_date={} 自动补价 {} 条", order_date_short, auto_filled),
                ));
            }
        }
        // 强制乐观锁：req.version 必须传入且 > 0
        let ver = match req.version {
            Some(v) if v > 0 => v,
            _ => return Err(UseCaseError::BadRequest("保存失败：缺少订单版本号，请刷新页面后重试".to_string())),
        };
        // 安全护栏：更新时明细不允许为空（防止并发覆盖导致明细被清空）
        if req.items.is_empty() {
            return Err(UseCaseError::BadRequest("保存失败：订单明细不能为空".to_string()));
        }

        // 数量同步：保存时若销售数量为 0 而预售数量 > 0，用预售数量兜底
        normalize_quantities(&mut req.items);

        let old_order_no = req.order_no.clone();
        let old_total = req.total_amount;
        let old_final = req.final_amount;

        // 服务端按当前单位换算比例统一计算基础数量，不信任前端值
        let base_qtys = database::sqlite::repo::stock_writer::compute_base_quantities(
            database::pool(), &base_inputs_of(&req.items)).await;

        match self.repo.update_order_with_items(&req, ver, &base_qtys).await {
            Ok(SalesOrderUpdateTx::Committed) => {
                let new_version = ver + 1;
                logs.push(SalesOrderLog::new(
                    "sales_order.update",
                    req.id.unwrap_or(0).to_string(),
                    format!("更新销售单 {}（原单号 {}）：金额 {:.2}→{:.2}，下浮后合计 {:.2}→{:.2}，版本 {}→{}",
                        req.order_no, old_order_no, old_total, req.total_amount, old_final, req.final_amount, ver, new_version),
                ));
                Ok(SalesOrderOutcome::with_logs("更新成功".to_string(), logs))
            }
            Ok(SalesOrderUpdateTx::VersionConflict) => Err(UseCaseError::RawResponse(
                409,
                format!("订单已被其他用户修改（版本 {} 已被覆盖），请刷新后重试", ver),
            )),
            Err(e) => Err(UseCaseError::from(e)),
        }
    }

    async fn update_prices(&self, id: i64, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        // 状态约束 + 行级数据权限：调价仅限待配单且归属自己的订单
        let (_order_purchaser_id, order_status) = self.guard(id, ctx).await?;
        if order_status != "pending" {
            return Err(UseCaseError::BadRequest(format!("当前订单状态为「{}」，仅待配单状态的订单允许调整价格", order_status)));
        }

        let items = self.repo.list_item_price_inputs(id).await;

        if items.is_empty() {
            return Err(UseCaseError::NotFound("订单不存在或无明细".to_string()));
        }

        let mut result_items = Vec::new();
        let mut errors = Vec::new();
        let mut new_total = 0.0;

        for (item_id, product_id, quantity) in &items {
            match self.repo.product_base_price(*product_id).await {
                Some(new_price) if new_price > 0.0 => {
                    let new_amount = new_price * quantity;
                    new_total += new_amount;
                    result_items.push(serde_json::json!({
                        "item_id": item_id,
                        "product_id": product_id,
                        "unit_price": new_price,
                        "amount": new_amount,
                    }));
                }
                Some(_) => {
                    errors.push(format!("商品ID {} 售价为0，跳过", product_id));
                }
                None => {
                    errors.push(format!("商品ID {} 未找到", product_id));
                }
            }
        }

        let (discount_rate, amount_reduction) = self.repo.fetch_discount_and_reduction(id).await;

        let discount_amount = ((new_total * (1.0 - discount_rate / 100.0)) * 100.0).round() / 100.0;
        let new_final = ((discount_amount - amount_reduction) * 100.0).round() / 100.0;

        let resp = serde_json::json!({
            "items": result_items,
            "total_amount": new_total,
            "final_amount": new_final.max(0.0),
            "errors": errors,
        });
        let logs = vec![SalesOrderLog::new(
            "sales_order.adjust_price",
            id.to_string(),
            format!("一键获取销售单 {} 最新售价，新合计={}，错误数={}", id, new_total, errors.len()),
        )];
        Ok(SalesOrderOutcome::with_logs(serde_json::to_string(&resp).unwrap(), logs))
    }

    async fn delete(&self, id: i64, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        // 行级数据权限 + 状态约束
        let (_order_purchaser_id, order_status) = self.guard(id, ctx).await?;
        if order_status != "pending" {
            return Err(UseCaseError::BadRequest(format!("当前订单状态为「{}」，仅待配单状态的订单允许删除", order_status)));
        }

        if let Err(e) = self.repo.delete_items(id).await {
            let err_str = match e {
                DomainError::Storage(m) => m,
                other => other.to_string(),
            };
            if err_str.contains("foreign key constraint") || err_str.contains("FOREIGN KEY") {
                return Err(UseCaseError::BadRequest("删除失败：订单明细存在外键约束冲突，请检查关联数据".to_string()));
            }
            return Err(UseCaseError::Storage(format!("删除订单明细失败：{}", err_str)));
        }

        match self.repo.delete_order(id).await {
            Ok(_) => {
                let logs = vec![SalesOrderLog::new(
                    "sales_order.delete",
                    id.to_string(),
                    format!("删除销售单 ID={}", id),
                )];
                Ok(SalesOrderOutcome::with_logs("删除成功".to_string(), logs))
            }
            Err(e) => {
                let err_str = match e {
                    DomainError::Storage(m) => m,
                    other => other.to_string(),
                };
                if err_str.contains("foreign key constraint") || err_str.contains("FOREIGN KEY") {
                    Err(UseCaseError::BadRequest("删除失败：存在外键约束冲突，请检查关联数据".to_string()))
                } else {
                    Err(UseCaseError::Storage(format!("删除失败：{}", err_str)))
                }
            }
        }
    }

    async fn settle(&self, id: i64, is_settled: Option<i64>, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        let (_order_purchaser_id, order_status) = self.guard(id, ctx).await?;
        // 审核后需先反审核
        if order_status != "pending" {
            return Err(UseCaseError::RawResponse(409, "该单据已审核，需先反审核后才能修改结算状态".to_string()));
        }

        let is_settled: i64 = is_settled.unwrap_or(0);
        if is_settled != 0 && is_settled != 1 {
            return Err(UseCaseError::BadRequest("参数无效".to_string()));
        }

        match self.repo.set_settled(id, is_settled).await {
            Ok(rows) if rows > 0 => {
                let label = if is_settled == 1 { "已结" } else { "未结" };
                let logs = vec![SalesOrderLog::new(
                    "sales_order.settle",
                    id.to_string(),
                    format!("设置销售单结算状态为「{}」", label),
                )];
                Ok(SalesOrderOutcome::with_logs("操作成功".to_string(), logs))
            }
            _ => Err(UseCaseError::RawResponse(409, "订单不存在或状态已变化".to_string())),
        }
    }

    async fn update_status(&self, id: Option<i64>, new_status: Option<String>, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        if id.is_none() || new_status.is_none() {
            return Err(UseCaseError::BadRequest("缺少参数".to_string()));
        }

        let id = id.unwrap();
        let new_status = new_status.unwrap();

        let valid_statuses = vec!["pending", "confirmed", "sorting", "sorted", "delivering", "delivered", "accepted"];
        if !valid_statuses.contains(&new_status.as_str()) {
            return Err(UseCaseError::BadRequest("无效状态".to_string()));
        }

        let current_status = self.repo.fetch_status_or_pending(id).await;

        // 行级数据权限：仅可操作归属自己的销售单
        let order_purchaser_id = self.repo.fetch_purchaser_id_or_neg1(id).await;
        if !can_access_sales_order(ctx, order_purchaser_id) {
            return Err(UseCaseError::Forbidden("您没有权限操作此订单".to_string()));
        }

        // 状态机：待审核 → 已审核 → 已验收（审核后直接确认验收，分拣/配送/送达节点合并）
        let allowed_transitions = match current_status.as_str() {
            "pending" => vec!["confirmed", "sorting"],
            "confirmed" => vec!["pending", "accepted"],
            "sorting" => vec!["pending", "sorted"],
            "sorted" => vec!["sorting", "delivering"],
            "delivering" => vec!["sorted", "delivered"],
            "delivered" => vec!["delivering", "accepted"],
            "accepted" => vec!["delivered"],
            _ => vec![],
        };

        if !allowed_transitions.contains(&new_status.as_str()) {
            return Err(UseCaseError::BadRequest(format!("状态不允许从 {} 转换到 {}", current_status, new_status)));
        }

        match self.repo.transition_status(id, &current_status, &new_status).await {
            Ok(SalesOrderTransitionTx::Committed) => {
                let logs = vec![SalesOrderLog::new(
                    "sales_order.update_status",
                    id.to_string(),
                    format!("销售单 {} 状态 {} -> {}", id, current_status, new_status),
                )];
                Ok(SalesOrderOutcome::with_logs("状态更新成功".to_string(), logs))
            }
            Ok(SalesOrderTransitionTx::StateConflict) => Err(UseCaseError::RawResponse(
                409,
                "订单状态已变化，请刷新后重试".to_string(),
            )),
            Err(e) => Err(UseCaseError::from(e)),
        }
    }

    async fn approve(&self, id: i64, reason: String, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        let (_order_purchaser_id, order_status) = self.guard(id, ctx).await?;
        if order_status == "confirmed" {
            return Err(UseCaseError::BadRequest("订单已审核，请勿重复操作".to_string()));
        }
        if order_status != "pending" {
            return Err(UseCaseError::BadRequest(format!("当前订单状态为「{}」，仅待审核状态的订单允许审核", order_status)));
        }

        match self.repo.approve(id).await {
            Ok(rows) if rows > 0 => {
                let logs = vec![SalesOrderLog::new(
                    "sales_order.approve",
                    id.to_string(),
                    format!("审核通过销售单 ID={}（{}）", id, if reason.is_empty() { "无备注" } else { &reason }),
                )];
                Ok(SalesOrderOutcome::with_logs("审核成功，订单已锁定；确认验收时出库".to_string(), logs))
            }
            _ => Err(UseCaseError::RawResponse(409, "订单状态已变化，请刷新后重试".to_string())),
        }
    }

    async fn unapprove(&self, id: i64, reason: String, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        if reason.is_empty() {
            return Err(UseCaseError::BadRequest("反审核必须填写原因".to_string()));
        }

        let (_order_purchaser_id, order_status) = self.guard(id, ctx).await?;
        // 超级管理员拥有任何时刻的反审核权限；其他角色仅允许对已审核（confirmed）订单反审核
        let is_super_admin = ctx.role == "super_admin";
        if !is_super_admin && order_status != "confirmed" {
            return Err(UseCaseError::BadRequest(format!("当前订单状态为「{}」，仅已审核状态的订单允许反审核", order_status)));
        }

        match self.repo.unapprove(id, is_super_admin, order_status == "accepted").await {
            Ok(SalesOrderUnapproveTx::Committed) => {
                let logs = vec![SalesOrderLog::new(
                    "sales_order.unapprove",
                    id.to_string(),
                    format!("反审核销售单 ID={}，原因：{}", id, reason),
                )];
                Ok(SalesOrderOutcome::with_logs("反审核成功，订单已解锁".to_string(), logs))
            }
            Ok(SalesOrderUnapproveTx::StateConflict) => Err(UseCaseError::RawResponse(
                409,
                "订单状态已变化，请刷新后重试".to_string(),
            )),
            Err(e) => Err(UseCaseError::from(e)),
        }
    }

    async fn export(&self, ctx: &UserCtx, keyword_pattern: String, purchaser_id: Option<i64>) -> Result<Vec<u8>, UseCaseError> {
        // 行级数据权限 + 筛选：purchaser 强制只看自己，其他角色可按 purchaser_id 过滤
        let purchaser_id: Option<i64> = if ctx.role == "purchaser" {
            if ctx.purchaser_id > 0 { Some(ctx.purchaser_id) } else { Some(-1) }
        } else {
            purchaser_id
        };

        let rows = self.repo.export_rows(&keyword_pattern, purchaser_id).await.unwrap_or_default();
        build_export_workbook(rows).map_err(|e| UseCaseError::Storage(format!("导出失败: {}", e)))
    }

    async fn import(&self, bytes: Vec<u8>, _ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
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
            let purchaser_name = if order_row.len() > 3 { order_row[3].trim() } else { "" };
            let purchaser_id = if !purchaser_name.is_empty() {
                self.repo.find_purchaser_id_by_name(purchaser_name).await.ok().flatten()
            } else {
                None
            };

            if purchaser_id.is_none() {
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

            let main = SalesOrderImportMain {
                order_no: order_no.clone(),
                purchaser_id: purchaser_id.unwrap(),
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

                            let import_item = SalesOrderImportItem {
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

        let logs = vec![SalesOrderLog::new(
            "sales_order.import",
            String::new(),
            format!("导入销售单：成功 {} 条，失败 {} 条", success, failed),
        )];
        Ok(SalesOrderOutcome::with_logs(
            format!("导入完成：成功 {} 条，失败 {} 条", success, failed),
            logs,
        ))
    }

    async fn accept(&self, id: i64, ctx: &UserCtx) -> Result<String, UseCaseError> {
        let order = match self.repo.accept_order_row(id).await.ok().flatten() {
            Some(o) => o,
            None => return Err(UseCaseError::NotFound("订单不存在".to_string())),
        };

        // 行级数据权限：仅可查看归属自己的销售单验收单
        if !can_access_sales_order(ctx, order.purchaser_id) {
            return Err(UseCaseError::Forbidden("您没有权限查看此订单".to_string()));
        }
        let discount_rate = order.discount_rate;

        // 1) 真实明细（带分类信息，用于排序）
        let item_rows = self.repo.accept_item_rows(id).await.unwrap_or_default();

        let mut item_map: std::collections::HashMap<i64, (i64, String, String, String, f64, f64, f64, String, i64)> = std::collections::HashMap::new(); // sort_key, food_name, spec, unit, unit_price, quantity, amount, remark, original_id
        // product_id -> 真实明细行 id，用于分摊增项 target_order_item_id 失效时回退匹配
        let mut product_to_key: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for r in &item_rows {
            let rid = r.id;
            let pid = r.product_id;
            product_to_key.entry(pid).or_insert(rid);
            let alias2 = r.alias2.clone().unwrap_or_default();
            let product_name = r.product_name.clone();
            let food_name = if alias2.is_empty() {
                product_name.clone()
            } else {
                format!("{}({})", product_name, alias2)
            };
            let unit = r.unit.clone().unwrap_or_default();
            let spec = r.spec.clone().unwrap_or_default();
            let original_remark = r.remark.clone().unwrap_or_default();
            let category_name = r.category_name.clone().unwrap_or_default();
            let parent_name = r.parent_name.clone().unwrap_or_default();
            let sort_key = crate::service::get_category_sort_key(&category_name, &parent_name);
            item_map.insert(rid, (sort_key, food_name, spec, unit, r.unit_price, r.quantity, r.amount, original_remark, rid));
        }

        // 2) 合并分摊增项（与验收单导出 Excel 一致的逻辑）
        let supplement_rows = self.repo.accept_supplement_rows(id).await.unwrap_or_default();

        for r in &supplement_rows {
            let op_type = r.operation_type.as_str();
            let target_item_id = r.target_order_item_id;
            let supp_product_id = r.product_id;
            let qty = r.quantity;
            let amt = r.amount;
            let alias2 = r.alias2.clone().unwrap_or_default();
            let product_name = r.product_name.clone();

            // 解析目标明细行 key：优先用 target_order_item_id，失效时用 product_id 回退匹配
            let resolved_key: Option<i64> = match target_item_id {
                Some(tid) if item_map.contains_key(&tid) => Some(tid),
                _ => product_to_key.get(&supp_product_id).copied(),
            };

            // 替换-冲减：负数金额，若归零则移除
            if op_type == "replace_remove" {
                if let Some(tid) = resolved_key {
                    if let Some(entry) = item_map.get_mut(&tid) {
                        let new_qty = entry.4 + qty;
                        let new_amt = entry.6 + amt;
                        if new_qty.abs() < 0.001 || new_amt.abs() < 0.001 {
                            item_map.remove(&tid);
                        } else {
                            entry.4 = new_qty;
                            entry.6 = new_amt;
                        }
                    }
                }
                continue;
            }

            // 追加数量：叠加到原明细
            if op_type == "increase_quantity" {
                if let Some(tid) = resolved_key {
                    if let Some(entry) = item_map.get_mut(&tid) {
                        let new_qty = entry.4 + qty;
                        let new_amt = entry.6 + amt;
                        let new_remark = format!("{}（含增项+{}）", entry.7, qty);
                        entry.4 = new_qty;
                        entry.6 = new_amt;
                        entry.7 = new_remark;
                    }
                }
            } else {
                // new_item 或 replace_add：作为新明细
                let food_name = if alias2.is_empty() { product_name.clone() } else { format!("{}({})", product_name, alias2) };
                let unit = r.unit.clone();
                let spec = r.spec.clone().unwrap_or_default();
                // 通过 product_id 反查分类名做排序
                let (cat_name, parent_name) = match self.repo.product_category_names(r.product_id).await.ok().flatten() {
                    Some((c, p)) => (c.unwrap_or_default(), p.unwrap_or_default()),
                    None => (String::new(), String::new()),
                };
                let sort_key = crate::service::get_category_sort_key(&cat_name, &parent_name);
                let remark = if op_type == "replace_add" {
                    spec.clone()
                } else {
                    if spec.is_empty() { "[增项]".to_string() } else { format!("{}; [增项]", spec) }
                };
                item_map.insert(-r.id, (sort_key, food_name, spec, unit, r.unit_price, qty, amt, remark, 0));
            }
        }

        let mut items_vec: Vec<_> = item_map.into_values().collect();
        items_vec.sort_by(|a, b| a.0.cmp(&b.0));

        // 3) 按合并后明细重算验收金额
        let accept_total_amount: f64 = items_vec.iter().map(|item| item.6).sum();
        let accept_final_amount = ((accept_total_amount * (1.0 - discount_rate / 100.0)) * 100.0).round() / 100.0;

        // 4) 输出 JSON（字段名与原接口一致）
        let items: Vec<serde_json::Value> = items_vec
            .into_iter()
            .map(|(_sort_key, food_name, _spec, unit, unit_price, quantity, amount, remark, original_id)| {
                serde_json::json!({
                    "id": original_id,
                    "product_id": 0,
                    "product_name": food_name.clone(),
                    "food_name": food_name,
                    "alias2": "",
                    "spec": unit,
                    "unit": unit,
                    "unit_price": unit_price,
                    "quantity": quantity,
                    "amount": amount,
                    "remark": remark,
                })
            })
            .collect();

        let supplier_name = "湖南食全味美餐饮管理有限公司".to_string();
        let car_no = "湘A·NY360".to_string();

        let accept_data = serde_json::json!({
            "id": order.id,
            "order_no": order.order_no,
            "order_date": order.order_date,
            "total_amount": accept_total_amount,
            "discount_rate": discount_rate,
            "final_amount": accept_final_amount,
            "remark": order.remark,
            "purchaser_name": order.purchaser_name,
            "purchaser_address": order.purchaser_address,
            "supplier_name": supplier_name,
            "car_no": car_no,
            "items": items,
        });

        Ok(serde_json::to_string(&accept_data).unwrap())
    }

    async fn generate_purchase(&self, id: i64, force: bool, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        // 本次生成中被排除（已审核/已处理）的采购订单数量，仅作提示
        let mut skipped_count: usize = 0;

        let so_row = match self.repo.generate_order_row(id).await.ok().flatten() {
            Some(g) => g,
            None => return Err(UseCaseError::RawResponse(404, serde_json::json!({ "message": "销售订单不存在" }).to_string())),
        };

        // 行级数据权限：仅可为自己采购单位的销售单生成采购订单
        if !can_access_sales_order(ctx, so_row.purchaser_id) {
            return Err(UseCaseError::RawResponse(403, "您没有权限操作此订单".to_string()));
        }
        let order_date = so_row.order_date.clone();
        let so_warehouse_id = so_row.warehouse_id;
        let so_warehouse_name = so_row.warehouse_name.clone();

        // 重复生成检查：是否已存在本销售单关联的采购订单
        // 已处理（非 pending）的采购订单受保护，不允许覆盖；
        // pending 状态在 force=true 时会被删除并重新合并，避免用户重复点击产生重复明细
        //
        // 检测范围（必须全部命中才算"已存在"）：
        //   1) 主表 source_sales_order_id = id（首次创建时的销售单）
        //   2) 明细中 source_sales_order_id = id（严格限定本销售单，避免误伤其他销售单/手动添加的明细）
        //   3) 当天同供应商已存在的非取消 PO（合并场景：首次创建来自其他销售单）
        // 三路任一命中即视为本销售单已生成过
        let mut existed: Vec<GenerateExistedPo> = self.repo.existing_po_rows(id).await.unwrap_or_default();

        // 兜底：当天同供应商已存在待分拣 PO（按"供应商+日期"合并到他人 PO 的场景）
        // 已审核/已处理的 PO 不视为"已生成过"，不参与合并也不拦截首次生成
        if existed.is_empty() {
            let involved_suppliers: Vec<i64> = self.repo.involved_supplier_ids(id).await.unwrap_or_default();
            if !involved_suppliers.is_empty() {
                existed = self.repo.pending_po_pool(&order_date, &involved_suppliers).await.unwrap_or_default();
            }
        }

        // force 路径需要的旧明细（按 po 收集），仅当 force=true 时填充
        // supplier_items 循环读取以做 UPDATE/INSERT/DELETE
        let mut old_items_by_po: std::collections::HashMap<i64, Vec<GenerateOldItemRow>> = std::collections::HashMap::new();

        // 把"销售单 (P,U) -> PO 明细 id"索引提到外层作用域，
        // supplier_items 循环需要用它做"按主键 UPDATE"判定
        let mut snapshot_by_pu: std::collections::HashMap<(i64, String, i64), i64> = std::collections::HashMap::new();

        if !existed.is_empty() {
            let status_text = |s: &str| match s {
                "pending" => "待分拣",
                "sorting" => "分拣中",
                "sorted" => "已分拣",
                "delivering" => "配送中",
                "delivered" => "已送达",
                "accepted" => "已验收",
                "settled" => "已结算",
                _ => "未知",
            };
            let pending: Vec<&GenerateExistedPo> = existed.iter().filter(|x| x.status == "pending").collect();
            let processed: Vec<&GenerateExistedPo> = existed.iter().filter(|x| x.status != "pending").collect();
            skipped_count = processed.len();

            // 已审核/已处理的采购订单受保护：仅排除，不覆盖。
            // 若本销售单关联的采购订单均已处理（已审核），则整单排除，不做任何修改。
            if pending.is_empty() && !processed.is_empty() {
                let detail = processed.iter()
                    .map(|x| format!("{}（{}，状态：{}）", x.order_no, x.supplier_name, status_text(&x.status)))
                    .collect::<Vec<_>>().join("\n");
                return Ok(SalesOrderOutcome::with_logs(
                    serde_json::json!({
                        "error": true,
                        "excluded": true,
                        "excluded_count": skipped_count,
                        "message": format!("该销售订单关联的 {} 张采购订单均已审核/处理（{}），本次已排除，未作修改。\n如需调整，请到采购订单页面手动处理对应单据。", skipped_count, detail)
                    }).to_string(),
                    vec![],
                ));
            }

            if !force {
                let detail = pending.iter()
                    .map(|x| format!("{}（{}）", x.order_no, x.supplier_name))
                    .collect::<Vec<_>>().join("\n");
                let mut warn = format!("该销售订单已生成过 {} 张采购订单（均为待分拣状态）：{}\n继续操作将对存在的采购订单对应的明细进行增删改（按新销售单明细），是否继续？", pending.len(), detail);
                if skipped_count > 0 {
                    let skip_detail = processed.iter()
                        .map(|x| format!("{}（{}，状态：{}）", x.order_no, x.supplier_name, status_text(&x.status)))
                        .collect::<Vec<_>>().join("\n");
                    warn += &format!("\n另有 {} 张采购订单已审核/处理（{}），本次将排除、不覆盖。", skipped_count, skip_detail);
                }
                return Ok(SalesOrderOutcome::with_logs(
                    serde_json::json!({
                        "warning": true,
                        "count": pending.len(),
                        "excluded_count": skipped_count,
                        "message": warn
                    }).to_string(),
                    vec![],
                ));
            }

            // force=true 准备：
            // 不再"先删后插"，改为"对现存明细做 CRUD"。
            // 1) 收集本销售单所贡献的现存 PO
            // 2) 这些 PO 中本销售单已有的明细(source=id OR 0)暂存为 old_items_by_po
            // 3) supplier_items 循环里据此做 UPDATE/INSERT/DELETE
            // 注：已删除的 PO 不在范围内，无须处理

            // 收集本销售单所贡献的 PO（按主表 source_sales_order_id 命中）
            let mut affected_po_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
            for x in &pending {
                affected_po_ids.insert(x.id);
            }

            // 兜底：按 supplier_id + order_date 找出当天所有非取消 PO，让重算也能覆盖到
            // （例如本销售单明细合并到他人创建的 PO 时）
            let po_pool: Vec<(i64, i64)> = self.repo.po_pool_for_date(&order_date).await.unwrap_or_default();

            // 计算该销售单涉及的 supplier_id 集合（来自明细），用于过滤受影响的 PO
            let mut involved_supplier_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
            let preview_items: Vec<i64> = self.repo.supplier_ids_of_order(id).await.unwrap_or_default();
            for sid in &preview_items {
                if *sid > 0 {
                    involved_supplier_ids.insert(*sid);
                }
            }
            for (po_id, sid) in &po_pool {
                if involved_supplier_ids.contains(sid) {
                    affected_po_ids.insert(*po_id);
                }
            }

            // 拉取每个 affected PO 中**本销售单**已有的明细（严格按 source_sales_order_id = id）
            // 严格匹配是为了不误伤其他销售单（other_so）贡献的明细：
            //   - other_so 贡献的明细保持其 source=other_so，不参与本轮"补充"逻辑
            //   - api_purchase_order_update 重插时已用主表 source 兜底回填了归属，
            //     所以本销售单贡献的明细其 source=id 是稳定可识别的
            // 匹配键用 COALESCE(sales_unit, unit, '') 而非 sales_unit：purchase_order_update
            // 重插时 sales_unit 字段丢失（前端表单不传），但 unit 仍保留，匹配用 unit 才能识别。
            // 暂存为 po_id -> Vec<(id, product_id, unit_key, quantity, amount)>
            for po_id in &affected_po_ids {
                let rows: Vec<GenerateOldItemRow> = self.repo.old_items_by_po(*po_id, id).await.unwrap_or_default();
                if !rows.is_empty() {
                    old_items_by_po.insert(*po_id, rows);
                }
            }

            // 读取本销售单"曾生成过的 PO 明细 id 快照"（用于跨调用追踪）：
            // 解决 force=true 路径下"用户从 PO 中删了明细 → to_consume 池因 source=id 过滤
            // 而为 0 → 重新 INSERT 全部 3 条"造成的重复 BUG。
            // 快照是一组 PO 明细主键 id。每次 force 结束时，会用"本次实际 UPDATE/INSERT
            // 的 PO 明细 id 集合"覆盖写回，所以它始终代表"本销售单最新一次生成时
            // 写入/更新的 PO 明细"。
            // 匹配策略：
            //   - 销售单 (P,U) 在快照中：按主键 UPDATE（id 在 DB 中消失时退化为 INSERT）
            //   - 销售单 (P,U) 不在快照中：INSERT 新条目（source=本单）
            //   - 快照中存在但销售单 (P,U) 不再出现：从快照中清掉（PO 中对应行保留，不删）
            //   - PO 中 source=本单 但不在新快照的明细：保留不动（兜底防误删）
            let snapshot_str: Option<String> = self.repo.snapshot_str(id).await;
            let snapshot_ids: Vec<i64> = snapshot_str
                .as_deref()
                .and_then(|s| serde_json::from_str::<Vec<i64>>(s).ok())
                .unwrap_or_default();

            // 把快照 id 翻译成 (P,U) -> po_item_id 索引（仅 source=本单的 id 纳入）
            // 同一 (P,U) 在快照中只可能 1 条（销售单明细与 PO 明细 1:1 关系）
            // snapshot_by_pu 已在外层定义
            if !snapshot_ids.is_empty() {
                let snap_rows: Vec<GenerateSnapshotRow> = self.repo.snapshot_rows(id, &snapshot_ids).await.unwrap_or_default();
                // 索引用 (P, U, warehouse_id) 三元组：相同 (P,U) 在不同 warehouse 的明细
                // 是不同行，跨仓库复用会破坏"不同仓库出库同商品同时保留"的业务规则。
                for r in snap_rows {
                    snapshot_by_pu.insert((r.product_id, r.unit_key, r.warehouse_id), r.id);
                }
            }
        }

        // 本轮 force 实际写入/更新的 PO 明细 id（循环结束后回写 sales_order 快照）
        // 提到外层作用域以供 supplier_items 循环后的快照回写使用
        let mut new_snapshot_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

        // 取销售订单明细（含销售单位 unit、备注）
        // ORDER BY soi.product_name, soi.id 使采购订单明细顺序与供应商分拣导出一致（按品名排序）
        let item_rows = self.repo.generate_item_rows(id).await.unwrap_or_default();

        // 按供应商分组明细；每个 (supplier_id, product_id, warehouse_id, sales_unit) 唯一
        // 单条销售明细条目的结构：(product_id, product_name, alias1, alias2, spec, unit, quantity, pre_sale_quantity, unit_price, base_unit, base_price, remark)
        let mut supplier_items: std::collections::HashMap<i64, Vec<(i64, String, String, String, String, String, f64, f64, f64, String, f64, String)>> = std::collections::HashMap::new();

        for r in &item_rows {
            let supplier_id = r.supplier_id;
            if supplier_id == 0 {
                continue;
            }
            let product_id = r.product_id;
            let product_name = r.product_name.clone();
            let alias1 = r.alias1.clone().unwrap_or_default();
            let alias2 = r.alias2.clone().unwrap_or_default();
            let spec = r.spec.clone().unwrap_or_default();
            let unit = r.unit.clone().unwrap_or_default();
            let quantity = r.quantity;
            // ordered_quantity 由销售订单的预售数量生成：两者需保持同步
            let ordered_quantity = r.pre_sale_quantity.unwrap_or(quantity);
            let purchase_price = r.purchase_price;
            let base_unit = r.base_unit.clone().unwrap_or_default();
            let base_price = r.base_price;
            let remark = r.remark.clone().unwrap_or_default();

            // 基础单位采购价：分供应商今日价 > 商品进价 > 商品基础单价
            let base_unit_price = if purchase_price > 0.0 { purchase_price } else { base_price };
            // 销售单位为大单位（箱/件等）时的换算，与手工开采购单 updateUnit 同口径：
            // 该单位有专属采购价则用之，否则 基础单位价 × 换算比例
            let (unit_ratio, unit_purchase_price): (f64, f64) = self.repo.unit_ratio_price(product_id, &unit).await.ok().flatten().unwrap_or((1.0, 0.0));
            let unit_price = ((if unit_purchase_price > 0.0 { unit_purchase_price } else { base_unit_price * unit_ratio }) * 100.0).round() / 100.0;

            supplier_items.entry(supplier_id).or_insert_with(Vec::new).push(
                (product_id, product_name, alias1, alias2, spec, unit, quantity, ordered_quantity, unit_price, base_unit, base_price, remark)
            );
        }

        if supplier_items.is_empty() {
            return Err(UseCaseError::RawResponse(400, serde_json::json!({ "message": "销售订单中没有供应商信息，无法生成采购订单" }).to_string()));
        }

        let mut created_count = 0;
        let mut merged_count = 0;
        // 跟踪已处理的 PO ID（force 清理时用于计算差集）
        let mut processed_po_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

        for (supplier_id, items) in supplier_items {
            // 计算该供应商所有明细的合计（用于新建 PO 时初始化主表金额）
            let mut total_amount: f64 = 0.0;
            let wh_id_set: std::collections::HashSet<i64> = std::collections::HashSet::new();
            let mut wh_names: Vec<String> = Vec::new();
            for (product_id, product_name, alias1, alias2, spec, unit, quantity, ordered_quantity, unit_price, _base_unit, _base_price, remark) in &items {
                total_amount += quantity * unit_price;
                if !wh_names.contains(unit) && !unit.trim().is_empty() {
                    wh_names.push(unit.clone());
                }
                let _ = (product_id, product_name, alias1, alias2, spec, ordered_quantity, remark);
            }
            // 主表仓库从销售订单的 warehouse 取（销售单的仓库为唯一入口仓库）
            let main_wh_id = so_warehouse_id;
            let main_wh_name = so_warehouse_name.clone();
            let _ = (wh_id_set, wh_names, total_amount);

            // 查找当天同供应商是否已有待分拣的采购订单（已审核/已处理的排除，只合并到未审核的）
            let existing_po: Option<i64> = self.repo.find_pending_po_of_supplier(supplier_id, &order_date).await.ok().flatten();

            let po_id: i64 = if let Some(existing_id) = existing_po {
                existing_id
            } else {
                let supplier_name_result = self.repo.supplier_name(supplier_id).await.ok().flatten();
                let _supplier_name = match supplier_name_result {
                    Some(sr) => sr,
                    None => "未知供应商".to_string(),
                };

                let order_no = crate::service::order_no::generate_order_no("purchase", &order_date).await;

                match self.repo.insert_po_main(supplier_id, &order_no, &order_date, main_wh_id, &main_wh_name, id).await {
                    Ok(new_id) => {
                        created_count += 1;
                        new_id
                    }
                    Err(e) => {
                        let m = match e {
                            DomainError::Storage(m) => m,
                            other => other.to_string(),
                        };
                        return Err(UseCaseError::RawResponse(500, serde_json::json!({ "message": format!("创建采购订单失败: {}", m) }).to_string()));
                    }
                }
            };
            processed_po_ids.insert(po_id);

            // 拉取 PO 中本销售单已存在的明细（按 product_id+sales_unit 去重，仅本销售单贡献的）
            // 这样其他销售单贡献的明细不会被错误合并
            // 注意：force=true 路径下，old_items_by_po 已包含待 diff 的旧明细，此处复用之
            let existing_items: Vec<(i64, f64, String)> = if let Some(old_rows) = old_items_by_po.get(&po_id) {
                old_rows.iter().map(|o| (o.product_id, o.quantity, o.unit_key.clone())).collect()
            } else {
                self.repo.existing_po_items(po_id, id).await.unwrap_or_default()
            };

            // "补充式"语义（force=true 路径），按 sales_order.generated_purchase_item_ids 快照驱动：
            //   1) 销售单 (P,U) 在快照中（且 id 仍在 PO） → 按主键 UPDATE 同步
            //   2) 销售单 (P,U) 在快照中（但 id 已从 PO 消失/被删） → 退化为 INSERT 补回
            //   3) 销售单 (P,U) 不在快照中，但 PO 中有同 (P,U) 的"孤儿"（其他 source，可能是老代码
            //      INSERT fallback 留下的 source=主表兜底） → UPDATE 复用并把 source 改成本销售单
            //      （关键：避免与孤儿并存形成 (P,U) 重复）
            //   4) 销售单 (P,U) 不在快照中且 PO 中无同 (P,U) → INSERT 新条目（source=本单）
            //   5) 快照中存在但销售单 (P,U) 不再出现 → 从快照中清掉（PO 中对应行保留，不删）
            //   6) PO 中 source=本单 但不在新快照的明细：保留不动（兜底防误删）
            // 这样既保证"用户从 PO 删的明细能补回"（快照覆盖到被删的 id），
            // 又不会因老代码 INSERT fallback 留下的 source=主表兜底孤儿"被新代码当成本销售单的",
            // 进而形成 (P,U) 重复插入。
            // 采购明细与销售明细一一对应：同单同商品多行（补采行为）各自生成一条采购明细，
            // 已写入的明细 id 记入 consumed，后续同 (P,U,wh) 行的孤儿匹配必须排除它们，
            // 否则后行会复用并覆盖前行（数量丢失、备注被吞）。
            let mut updated_lines: i64 = 0;
            let mut added_lines: i64 = 0;
            let mut delta_amount: f64 = 0.0; // UPDATE 同步：金额增量 = 新 - 旧
            let mut consumed_po_item_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

            for (product_id, product_name, alias1, alias2, spec, unit, quantity, ordered_quantity, unit_price, _base_unit, _base_price, remark) in items {
                let amount = ((quantity * unit_price) * 100.0).round() / 100.0;
                // base_quantity 按 product_unit.ratio 现算（销售开"件"时采购入库基础数量必须乘比例），
                // 与手工开单（create/update 服务端重算）和审核写流水兜底保持同一口径
                let base_quantity = self.repo.unit_ratio(product_id, &unit).await.ok().flatten()
                    .map(|ratio| ((quantity * ratio) * 100.0).round() / 100.0)
                    .unwrap_or(quantity);
                let sales_unit = unit.clone();
                // 1) 快照中找 (P,U, warehouse) → 尝试按主键 UPDATE
                // key 加 warehouse_id：跨仓库同 (P,U) 是不同行，必须区分
                let mut updated_via_snapshot = false;
                let key = (product_id, sales_unit.trim().to_string(), main_wh_id);
                if let Some(snap_po_id) = snapshot_by_pu.get(&key).copied() {
                    snapshot_by_pu.remove(&key);
                    let new_amount = quantity * unit_price;
                    // 读取旧 amount 以计算差量（保持主表金额正确）
                    let old_amount: f64 = self.repo.po_item_amount(snap_po_id, po_id).await;
                    let w = GeneratePoItemWrite {
                        product_id,
                        product_name: product_name.clone(),
                        alias1: alias1.clone(),
                        alias2: alias2.clone(),
                        spec: spec.clone(),
                        unit: unit.clone(),
                        unit_price,
                        quantity,
                        base_quantity,
                        amount: new_amount,
                        ordered_quantity,
                        remark: remark.clone(),
                        warehouse_id: main_wh_id,
                        warehouse_name: main_wh_name.clone(),
                        sales_unit: sales_unit.clone(),
                    };
                    let affected = self.repo.update_po_item_via_snapshot(po_id, snap_po_id, &w).await;
                    if affected > 0 {
                        // 真正命中 DB 中存在的明细 → UPDATE 成功
                        new_snapshot_ids.insert(snap_po_id);
                        consumed_po_item_ids.insert(snap_po_id);
                        updated_via_snapshot = true;
                        delta_amount += new_amount - old_amount; // 差量计入主表
                        updated_lines += 1;
                    }
                    // affected == 0 → 快照 id 在 DB 中已被删，退化到下面的"孤儿复用"或 INSERT 分支
                }

                if !updated_via_snapshot {
                    // 2) 快照命中但 id 已被删 或 3) 快照无此 (P,U) →
                    //    先在 PO 中找同 (P,U,**warehouse**) 的"孤儿"（任意 source；通常 source=主表兜底），
                    //    有就 UPDATE 复用并把 source 改成本销售单；没有才 INSERT。
                    // 这是修复"老代码 INSERT fallback 留下 source=主表兜底孤儿"的二次重复 BUG 的关键。
                    //
                    // 关键：匹配键必须包含 warehouse_id！否则"不同仓库出库同 (P,U) 商品"场景下，
                    // SO Y force 生成时会错误地 UPDATE SO X 留下的 wh=X 的孤儿，把 SO Y 的
                    // 明细（数量/仓库等）覆盖到 SO X 的行上，导致不同仓库的明细被吞并成最后一条。
                    // 业务上要求"不同仓库出库的相同商品在同一个 PO 中要同时保留"，
                    // 所以仓库是行级区分维度，不能跨仓库复用。
                    let new_amount = ((quantity * unit_price) * 100.0).round() / 100.0;
                    // 孤儿匹配：同 (P,U,warehouse_id) 任意 source；优先选 source=本单的（极端兜底），
                    // 否则按 id 最小的（最早插入的）复用。
                    // 排除本轮已写入的明细（同单同商品多行 1:1 生成，各行互不复用）。
                    let exclude_ids: Vec<i64> = consumed_po_item_ids.iter().copied().collect();
                    let orphan_id: Option<i64> = self.repo.find_orphan_item(po_id, product_id, sales_unit.trim(), main_wh_id, &exclude_ids, id).await;

                    if let Some(orphan_po_id) = orphan_id {
                        // 读取旧 amount 以计算差量
                        let old_amount: f64 = self.repo.po_item_amount(orphan_po_id, po_id).await;
                        let w = GeneratePoItemWrite {
                            product_id,
                            product_name: product_name.clone(),
                            alias1: alias1.clone(),
                            alias2: alias2.clone(),
                            spec: spec.clone(),
                            unit: unit.clone(),
                            unit_price,
                            quantity,
                            base_quantity,
                            amount: new_amount,
                            ordered_quantity,
                            remark: remark.clone(),
                            warehouse_id: main_wh_id,
                            warehouse_name: main_wh_name.clone(),
                            sales_unit: sales_unit.clone(),
                        };
                        // UPDATE 复用孤儿：同步字段 + 把 source 改成本销售单
                        let affected = self.repo.update_po_item_orphan(po_id, orphan_po_id, id, &w).await;
                        if affected > 0 {
                            new_snapshot_ids.insert(orphan_po_id);
                            consumed_po_item_ids.insert(orphan_po_id);
                            delta_amount += new_amount - old_amount;
                            updated_lines += 1;
                            // 不加 added_lines：这是复用，不是新增
                            continue; // 处理下一个销售单明细
                        }
                        // UPDATE 0 行（极端情况：行已被并发删除）→ 回落到 INSERT
                    }

                    // 4) 快照无此 (P,U) 且 PO 中无孤儿 → INSERT 新条目（source=本单）
                    let w = GeneratePoItemWrite {
                        product_id,
                        product_name: product_name.clone(),
                        alias1: alias1.clone(),
                        alias2: alias2.clone(),
                        spec: spec.clone(),
                        unit: unit.clone(),
                        unit_price,
                        quantity,
                        base_quantity,
                        amount,
                        ordered_quantity,
                        remark: remark.clone(),
                        warehouse_id: main_wh_id,
                        warehouse_name: main_wh_name.clone(),
                        sales_unit: sales_unit.clone(),
                    };
                    if let Ok(new_po_id) = self.repo.insert_po_item(po_id, id, &w).await {
                        if new_po_id > 0 {
                            new_snapshot_ids.insert(new_po_id);
                            consumed_po_item_ids.insert(new_po_id);
                        }
                    }
                    delta_amount += amount;
                    added_lines += 1;
                }
            }
            // 注意：snapshot_by_pu 中剩余的项（快照有但销售单 (P,U) 不再出现）
            // → 不写入 new_snapshot_ids（让它们从快照中清掉，PO 中对应行保留，不删）

            // 同步更新主表总金额：仅按"补充式"的差量调整（新增/UPDATE 金额变化）
            if delta_amount.abs() > 0.0001 {
                self.repo.adjust_po_totals(po_id, delta_amount).await;
            }

            // "补充式"语义：不删主表（即使主表对本单没有可见贡献，也要保留 PO 整体给其他销售单）

            merged_count += updated_lines + added_lines;
            let _ = existing_items; // 兼容旧变量（CRUD 路径下不再使用）
        }

        // 把本次 force 实际写入/更新的 PO 明细 id 写回 sales_order 快照列
        // （只有 force=true 路径填充了 new_snapshot_ids；非 force 走警告返回，不会到这里）
        if !new_snapshot_ids.is_empty() {
            let mut ids: Vec<i64> = new_snapshot_ids.into_iter().collect();
            ids.sort();
            let json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string());
            self.repo.write_snapshot(id, &json).await;
        }

        // "补充式"语义：不再强制清理/删除其他销售单主导的 PO。
        // 即使用户在当前销售单上移除了某个供应商的全部明细，该供应商若还有其他销售单贡献的明细，
        // 也不应被本流程删掉——PO 的归属以"明细来源"为准，不以"主表创建者"为准。

        let logs = vec![SalesOrderLog::new(
            "sales_order.generate_purchase",
            id.to_string(),
            format!("由销售单生成采购订单，新建 {} 张，合并明细 {} 条", created_count, merged_count),
        )];

        let msg = if created_count > 0 && merged_count > 0 {
            format!("成功生成 {} 张采购订单，并合并 {} 条相同明细", created_count, merged_count)
        } else if created_count > 0 {
            format!("成功生成 {} 张采购订单", created_count)
        } else {
            format!("已将销售明细合并到已有采购订单（{} 条合并）", merged_count)
        };
        let msg = if skipped_count > 0 {
            format!("{}（另排除 {} 张已审核/已处理的采购订单）", msg, skipped_count)
        } else {
            msg
        };
        Ok(SalesOrderOutcome::with_logs(
            serde_json::json!({
                "count": created_count,
                "merged": merged_count,
                "excluded_count": skipped_count,
                "message": msg
            }).to_string(),
            logs,
        ))
    }

    async fn upload_image(&self, order_id: i64, image_type: &str, file: Option<(String, Vec<u8>)>, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        // 行级数据权限：仅可为自己采购单位的销售单上传图片
        let order_purchaser_id = self.repo.get_detail_purchaser_id(order_id).await.unwrap_or(-1);
        if !can_access_sales_order(ctx, order_purchaser_id) {
            return Err(UseCaseError::Forbidden("您没有权限操作此订单".to_string()));
        }

        let (folder, prefix, column) = match image_type {
            "customer" => ("customer_orders", "客户订单", "customer_order_image"),
            "signed" => ("signed_orders", "签字验收单", "signed_order_image"),
            _ => return Err(UseCaseError::BadRequest("无效的图片类型".to_string())),
        };

        // 获取销售单位（采购方）名称和订单日期作为前缀
        let (purchaser_name, order_date) = match self.repo.image_purchaser_info(order_id).await {
            Some((name, date)) => (name.unwrap_or_else(|| "purchaser".to_string()), date.unwrap_or_else(|| "nodate".to_string())),
            None => ("purchaser".to_string(), "nodate".to_string()),
        };

        let name_prefix = format!("{}_{}_{}", common::utils::sanitize_filename_prefix(&purchaser_name), order_date, prefix);
        let full_folder = format!("uploads/{}", folder);
        tokio::fs::create_dir_all(&full_folder).await.ok();

        let (filename, bytes) = match file {
            Some(f) => f,
            None => return Err(UseCaseError::BadRequest("请选择要上传的图片".to_string())),
        };

        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("jpg")
            .to_lowercase();

        if !["jpg", "jpeg", "png", "gif", "webp"].contains(&ext.as_str()) {
            return Err(UseCaseError::BadRequest("不支持的图片格式".to_string()));
        }

        let timestamp = chrono::Utc::now().timestamp_millis();
        let random: u32 = rand::random();
        let new_filename = format!("{}_{}_{}.{}", name_prefix, timestamp, random, ext);
        let path = format!("uploads/{}/{}", folder, new_filename);

        if bytes.len() > 5 * 1024 * 1024 {
            return Err(UseCaseError::BadRequest("图片大小不能超过5MB".to_string()));
        }

        if tokio::fs::write(&path, &bytes).await.is_err() {
            return Err(UseCaseError::Storage("保存图片失败".to_string()));
        }

        let file_path = format!("/api/uploads/{}/{}", folder, new_filename);

        self.repo.update_image_column(order_id, column, &file_path).await;

        Ok(SalesOrderOutcome::with_logs(
            serde_json::json!({ "url": file_path }).to_string(),
            vec![SalesOrderLog::new("sales_order.upload_image", order_id.to_string(),
                format!("上传{}图片：{}", if image_type == "customer" { "客户订单" } else { "签字验收单" }, file_path))],
        ))
    }

    async fn delete_image(&self, order_id: i64, image_type: &str, ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        // 行级数据权限：仅可操作归属自己的销售单
        let order_purchaser_id = self.repo.get_detail_purchaser_id(order_id).await.unwrap_or(-1);
        if !can_access_sales_order(ctx, order_purchaser_id) {
            return Err(UseCaseError::Forbidden("您没有权限操作此订单".to_string()));
        }

        let column = match image_type {
            "customer" => "customer_order_image",
            "signed" => "signed_order_image",
            _ => return Err(UseCaseError::BadRequest("无效的图片类型".to_string())),
        };

        if let Some(Some(image_url)) = self.repo.image_url_of(order_id, column).await {
            if let Some(path) = common::utils::image_url_to_path(&image_url) {
                let _ = tokio::fs::remove_file(&path).await;
            }
        }

        self.repo.clear_image_column(order_id, column).await;

        Ok(SalesOrderOutcome::with_logs(
            "删除成功".to_string(),
            vec![SalesOrderLog::new("sales_order.delete_image", order_id.to_string(),
                format!("删除{}图片", if image_type == "customer" { "客户订单" } else { "签字验收单" }))],
        ))
    }

    // ===== 分拣清单（sort*）：聚合/排序/xlsx 构建自旧 handler 逐字平移 =====

    async fn sort_items(&self, date: String) -> Result<String, UseCaseError> {
        let rows = self.repo.sort_item_rows(&date).await.unwrap_or_default();

        let mut items_map: std::collections::HashMap<i64, serde_json::Value> = std::collections::HashMap::new();

        for r in &rows {
            let product_id = r.product_id;
            let existing = items_map.entry(product_id).or_insert_with(|| {
                serde_json::json!({
                    "id": r.id,
                    "product_id": product_id,
                    "product_name": r.product_name.clone(),
                    "unit": r.unit.clone().unwrap_or_default(),
                    "unit_price": r.unit_price,
                    "total_quantity": 0.0,
                    "total_amount": 0.0,
                    "purchaser_names": Vec::new() as Vec<String>,
                    "order_nos": Vec::new() as Vec<String>,
                    "remarks": Vec::new() as Vec<String>,
                })
            });

            existing["total_quantity"] = serde_json::json!(existing["total_quantity"].as_f64().unwrap_or(0.0) + r.quantity);
            existing["total_amount"] = serde_json::json!(existing["total_amount"].as_f64().unwrap_or(0.0) + r.amount);

            let purchaser_name = r.purchaser_name.clone().unwrap_or_default();
            let purchasers = existing["purchaser_names"].as_array_mut().unwrap();
            if !purchasers.contains(&serde_json::json!(purchaser_name)) {
                purchasers.push(serde_json::json!(purchaser_name));
            }

            let order_no = r.order_no.clone().unwrap_or_default();
            let orders = existing["order_nos"].as_array_mut().unwrap();
            if !orders.contains(&serde_json::json!(order_no)) {
                orders.push(serde_json::json!(order_no));
            }

            let remark = r.remark.clone().unwrap_or_default();
            if !remark.is_empty() {
                let remarks = existing["remarks"].as_array_mut().unwrap();
                if !remarks.contains(&serde_json::json!(remark)) {
                    remarks.push(serde_json::json!(remark));
                }
            }
        }

        let mut items: Vec<serde_json::Value> = items_map.values()
            .map(|v| {
                let mut v = v.clone();
                let purchasers: Vec<String> = v["purchaser_names"].as_array().unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap_or_default().to_string())
                    .collect();
                v["purchaser_names"] = serde_json::json!(purchasers.join("; "));

                let orders: Vec<String> = v["order_nos"].as_array().unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap_or_default().to_string())
                    .collect();
                v["order_nos"] = serde_json::json!(orders.join("; "));

                let remarks: Vec<String> = v["remarks"].as_array().unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap_or_default().to_string())
                    .collect();
                v["remarks"] = serde_json::json!(remarks.join("; "));
                v
            })
            .collect();
        items.sort_by(|a, b| a["product_name"].as_str().unwrap_or("").cmp(b["product_name"].as_str().unwrap_or("")));

        Ok(serde_json::to_string(&items).unwrap())
    }

    async fn sort_items_excel(&self, date: String) -> Result<(String, Vec<u8>), UseCaseError> {
        let rows = self.repo.sort_item_excel_rows(&date).await.unwrap_or_default();

        let mut items_map: std::collections::HashMap<i64, serde_json::Value> = std::collections::HashMap::new();

        for r in &rows {
            let product_id = r.product_id;
            // 导出名称与采购单规则对齐：规格 -> 名称（独立取值不拼接）
            let display_name = {
                let spec = r.spec.clone().unwrap_or_default();
                let spec = spec.trim().to_string();
                if spec.is_empty() { r.product_name.clone() } else { spec }
            };
            let existing = items_map.entry(product_id).or_insert_with(|| {
                serde_json::json!({
                    "id": r.id,
                    "product_id": product_id,
                    "product_name": display_name,
                    "unit": r.unit.clone().unwrap_or_default(),
                    "unit_price": r.unit_price,
                    "total_quantity": 0.0,
                    "total_amount": 0.0,
                    "purchaser_names": Vec::new() as Vec<String>,
                    "order_nos": Vec::new() as Vec<String>,
                    "remarks": Vec::new() as Vec<String>,
                })
            });

            existing["total_quantity"] = serde_json::json!(existing["total_quantity"].as_f64().unwrap_or(0.0) + r.quantity);
            existing["total_amount"] = serde_json::json!(existing["total_amount"].as_f64().unwrap_or(0.0) + r.amount);

            let purchaser_name = r.purchaser_name.clone().unwrap_or_default();
            let purchasers = existing["purchaser_names"].as_array_mut().unwrap();
            if !purchasers.contains(&serde_json::json!(purchaser_name)) {
                purchasers.push(serde_json::json!(purchaser_name));
            }

            let order_no = r.order_no.clone().unwrap_or_default();
            let orders = existing["order_nos"].as_array_mut().unwrap();
            if !orders.contains(&serde_json::json!(order_no)) {
                orders.push(serde_json::json!(order_no));
            }

            let remark = r.remark.clone().unwrap_or_default();
            if !remark.is_empty() {
                let remarks = existing["remarks"].as_array_mut().unwrap();
                if !remarks.contains(&serde_json::json!(remark)) {
                    remarks.push(serde_json::json!(remark));
                }
            }
        }

        let mut items: Vec<serde_json::Value> = items_map.values()
            .map(|v| {
                let mut v = v.clone();
                let purchasers: Vec<String> = v["purchaser_names"].as_array().unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap_or_default().to_string())
                    .collect();
                v["purchaser_names"] = serde_json::json!(purchasers.join("; "));

                let orders: Vec<String> = v["order_nos"].as_array().unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap_or_default().to_string())
                    .collect();
                v["order_nos"] = serde_json::json!(orders.join("; "));

                let remarks: Vec<String> = v["remarks"].as_array().unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap_or_default().to_string())
                    .collect();
                v["remarks"] = serde_json::json!(remarks.join("; "));
                v
            })
            .collect();
        // 固定排序：先按商品名称，同名时按 product_id 兜底，保证每次导出顺序完全一致
        items.sort_by(|a, b| a["product_name"].as_str().unwrap_or("").cmp(b["product_name"].as_str().unwrap_or(""))
            .then_with(|| a["product_id"].as_i64().unwrap_or(0).cmp(&b["product_id"].as_i64().unwrap_or(0))));

        let excel_result: Result<Vec<u8>, XlsxError> = (|| {
            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet();

            worksheet.set_landscape();
            worksheet.set_margins(0.2, 0.2, 0.2, 0.2, 0.2, 0.2);
            worksheet.set_print_center_vertically(false);
            worksheet.set_print_center_horizontally(true);

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
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::VerticalCenter);

            let cell_left_format = Format::new()
                .set_font_size(10)
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter);

            let price_format = Format::new()
                .set_font_size(10)
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Right)
                .set_num_format("0.00");

            worksheet.merge_range(0, 0, 0, 6, "采购分拣清单", &title_format)?;
            worksheet.set_row_height(0, 28)?;

            let headers = ["序号", "商品名称", "单位", "单价", "数量", "金额", "采购单位"];
            let mut current_row = 2;
            for (i, h) in headers.iter().enumerate() {
                worksheet.write_with_format(current_row, i as u16, *h, &header_format)?;
            }
            current_row += 1;

            let mut index = 1;
            for item in &items {
                worksheet.write_with_format(current_row, 0, index as f64, &cell_format)?;
                worksheet.write_with_format(current_row, 1, item["product_name"].as_str().unwrap_or_default(), &cell_left_format)?;
                worksheet.write_with_format(current_row, 2, item["unit"].as_str().unwrap_or_default(), &cell_format)?;
                worksheet.write_with_format(current_row, 3, item["unit_price"].as_f64().unwrap_or(0.0), &price_format)?;
                worksheet.write_with_format(current_row, 4, item["total_quantity"].as_f64().unwrap_or(0.0), &cell_format)?;
                worksheet.write_with_format(current_row, 5, item["total_amount"].as_f64().unwrap_or(0.0), &price_format)?;
                worksheet.write_with_format(current_row, 6, item["purchaser_names"].as_str().unwrap_or_default(), &cell_left_format)?;
                current_row += 1;
                index += 1;
            }

            worksheet.write_with_format(current_row, 0, "合计", &header_format)?;
            worksheet.merge_range(current_row, 0, current_row, 4, "", &header_format)?;
            let total_amount: f64 = items.iter().map(|item| item["total_amount"].as_f64().unwrap_or(0.0)).sum();
            worksheet.write_with_format(current_row, 5, total_amount, &price_format)?;

            worksheet.set_column_width(0, 10)?;
            worksheet.set_column_width(1, 30)?;
            worksheet.set_column_width(2, 12)?;
            worksheet.set_column_width(3, 12)?;
            worksheet.set_column_width(4, 12)?;
            worksheet.set_column_width(5, 12)?;
            worksheet.set_column_width(6, 30)?;

            let buf = workbook.save_to_buffer()?;
            Ok(buf)
        })();

        match excel_result {
            Ok(buf) => Ok(("采购分拣清单.xlsx".to_string(), buf)),
            Err(e) => {
                eprintln!("Excel export error: {}", e);
                Err(UseCaseError::Storage("导出失败".to_string()))
            }
        }
    }

    async fn sort_items_by_category(&self, date: String) -> Result<String, UseCaseError> {
        let has_date = !date.is_empty();
        let rows = self.repo.sort_category_rows(&date).await.unwrap_or_default();
        eprintln!("sort_by_category_excel rows={}, date='{}', has_date={}", rows.len(), date, has_date);

        let mut category_map: std::collections::HashMap<(String, i64), std::collections::HashMap<(String, i64), Vec<serde_json::Value>>> = std::collections::HashMap::new();

        for r in &rows {
            let category_name = sanitize_xlsx_string(r.category_name.clone().unwrap_or_else(|| "未分类".to_string()));
            let category_id = r.category_id;
            let purchaser_name = sanitize_xlsx_string(r.purchaser_name.clone());
            let purchaser_id = r.purchaser_id;

            let purchaser_map = category_map.entry((category_name, category_id)).or_insert_with(std::collections::HashMap::new);
            let purchaser_items = purchaser_map.entry((purchaser_name, purchaser_id)).or_insert_with(Vec::new);

            purchaser_items.push(serde_json::json!({
                "item_id": r.item_id,
                "product_id": r.product_id,
                "product_name": r.product_name.clone(),
                "unit": r.unit.clone().unwrap_or_default(),
                "unit_price": r.unit_price,
                "quantity": r.quantity,
                "amount": r.amount,
                "remark": r.remark.clone().unwrap_or_default(),
                "order_no": r.order_no.clone().unwrap_or_default(),
            }));
        }

        let mut result: Vec<serde_json::Value> = Vec::new();
        for ((category_name, category_id), purchaser_map) in category_map {
            let mut purchasers: Vec<serde_json::Value> = Vec::new();
            for ((purchaser_name, purchaser_id), items) in purchaser_map {
                let total_qty: f64 = items.iter().map(|item| item["quantity"].as_f64().unwrap_or(0.0)).sum();
                purchasers.push(serde_json::json!({
                    "purchaser_name": purchaser_name,
                    "purchaser_id": purchaser_id,
                    "items": items,
                    "total_quantity": total_qty,
                }));
            }
            // 名称优先（业务阅读顺序），同名按 id 兜底
            purchasers.sort_by(|a, b| a["purchaser_name"].as_str().unwrap_or("").cmp(b["purchaser_name"].as_str().unwrap_or(""))
                .then_with(|| a["purchaser_id"].as_i64().unwrap_or(0).cmp(&b["purchaser_id"].as_i64().unwrap_or(0))));

            let total_qty: f64 = purchasers.iter().map(|p| p["total_quantity"].as_f64().unwrap_or(0.0)).sum();
            result.push(serde_json::json!({
                "category_name": category_name,
                "category_id": category_id,
                "purchasers": purchasers,
                "total_quantity": total_qty,
            }));
        }

        result.sort_by(|a, b| a["category_name"].as_str().unwrap_or("").cmp(b["category_name"].as_str().unwrap_or(""))
            .then_with(|| a["category_id"].as_i64().unwrap_or(0).cmp(&b["category_id"].as_i64().unwrap_or(0))));

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn sort_items_by_category_excel(&self, date: String) -> Result<(String, Vec<u8>), UseCaseError> {
        let rows = self.repo.sort_category_excel_rows(&date).await.unwrap_or_default();

        // key 带 id：同名时靠 id 确定唯一顺序，避免每次导出顺序漂移
        let mut category_map: std::collections::HashMap<(String, i64), std::collections::HashMap<(String, i64), Vec<serde_json::Value>>> = std::collections::HashMap::new();

        for r in &rows {
            let category_name = sanitize_xlsx_string(r.category_name.clone().unwrap_or_else(|| "未分类".to_string()));
            let category_id = r.category_id;
            let purchaser_name = sanitize_xlsx_string(r.purchaser_name.clone());
            let purchaser_id = r.purchaser_id;

            // 导出名称与采购单规则对齐：规格 -> 名称（独立取值不拼接）
            let display_name = {
                let spec = r.spec.clone().unwrap_or_default();
                let spec = spec.trim().to_string();
                if spec.is_empty() { r.product_name.clone() } else { spec }
            };

            let purchaser_map = category_map.entry((category_name, category_id)).or_insert_with(std::collections::HashMap::new);
            let purchaser_items = purchaser_map.entry((purchaser_name, purchaser_id)).or_insert_with(Vec::new);

            purchaser_items.push(serde_json::json!({
                "product_id": r.product_id,
                "product_name": sanitize_xlsx_string(display_name),
                "unit": sanitize_xlsx_string(r.unit.clone().unwrap_or_default()),
                "quantity": r.quantity,
                "pre_sale_quantity": r.pre_sale_quantity.unwrap_or(0.0),
                "amount": r.amount.unwrap_or(0.0),
                "remark": sanitize_xlsx_string(r.remark.clone().unwrap_or_default()),
                "order_no": sanitize_xlsx_string(r.order_no.clone().unwrap_or_default()),
            }));
        }

        let mut result: Vec<serde_json::Value> = Vec::new();
        for ((category_name, category_id), purchaser_map) in category_map {
            let mut purchasers: Vec<serde_json::Value> = Vec::new();
            for ((purchaser_name, purchaser_id), items) in purchaser_map {
                let total_qty: f64 = items.iter().map(|item| item["quantity"].as_f64().unwrap_or(0.0)).sum();
                purchasers.push(serde_json::json!({
                    "purchaser_name": purchaser_name,
                    "purchaser_id": purchaser_id,
                    "items": items,
                    "total_quantity": total_qty,
                }));
            }
            // 名称优先（业务阅读顺序），同名按 id 兜底
            purchasers.sort_by(|a, b| a["purchaser_name"].as_str().unwrap_or("").cmp(b["purchaser_name"].as_str().unwrap_or(""))
                .then_with(|| a["purchaser_id"].as_i64().unwrap_or(0).cmp(&b["purchaser_id"].as_i64().unwrap_or(0))));

            let total_qty: f64 = purchasers.iter().map(|p| p["total_quantity"].as_f64().unwrap_or(0.0)).sum();
            result.push(serde_json::json!({
                "category_name": category_name,
                "category_id": category_id,
                "purchasers": purchasers,
                "total_quantity": total_qty,
            }));
        }

        result.sort_by(|a, b| a["category_name"].as_str().unwrap_or("").cmp(b["category_name"].as_str().unwrap_or(""))
            .then_with(|| a["category_id"].as_i64().unwrap_or(0).cmp(&b["category_id"].as_i64().unwrap_or(0))));

        // 用 catch_unwind 包裹 Excel 构建过程，避免单个坏数据导致整个进程 panic 崩溃
        let build_excel = || -> Result<Vec<u8>, XlsxError> {
            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet();

            worksheet.set_landscape();
            worksheet.set_margins(0.2, 0.2, 0.2, 0.2, 0.2, 0.2);
            worksheet.set_print_center_vertically(false);
            worksheet.set_print_center_horizontally(true);

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

            let cat_hunxian_format = Format::new()
                .set_bold()
                .set_font_size(12)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#DC2626")
                .set_font_color("#FFFFFF");

            let cat_xianshu_format = Format::new()
                .set_bold()
                .set_font_size(12)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#16A34A")
                .set_font_color("#FFFFFF");

            let cat_liangyou_format = Format::new()
                .set_bold()
                .set_font_size(12)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#1D4ED8")
                .set_font_color("#FFFFFF");

            let cat_douzhi_format = Format::new()
                .set_bold()
                .set_font_size(12)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#CA8A04")
                .set_font_color("#FFFFFF");

            let cat_fenmian_format = Format::new()
                .set_bold()
                .set_font_size(12)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#64748B")
                .set_font_color("#FFFFFF");

            let cat_shuiguo_format = Format::new()
                .set_bold()
                .set_font_size(12)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#EA580C")
                .set_font_color("#FFFFFF");

            let cat_other_format = Format::new()
                .set_bold()
                .set_font_size(12)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#6B7280")
                .set_font_color("#FFFFFF");

            let purchaser_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#E5E7EB")
                .set_font_color("#374151");

            let col_widths = [6.0, 18.0, 8.0, 10.0, 20.0, 20.0];
            for (i, w) in col_widths.iter().enumerate() {
                worksheet.set_column_width(i as u16, *w)?;
            }

            let mut current_row = 0;
            worksheet.merge_range(current_row, 0, current_row, 5, "采购分拣清单（按分类）", &title_format)?;
            worksheet.set_row_height(current_row, 28)?;
            current_row += 2;

            let headers = ["序号", "商品名称", "单位", "数量", "备注", "采购单位"];
            for (i, header) in headers.iter().enumerate() {
                worksheet.write_with_format(current_row, i as u16, *header, &header_format)?;
            }
            current_row += 1;

            let mut seq = 1;
            for cat in &result {
                let cat_name = cat["category_name"].as_str().unwrap_or("未分类");

                let cat_format = match () {
                    _ if cat_name.contains("荤鲜") => &cat_hunxian_format,
                    _ if cat_name.contains("鲜蔬") => &cat_xianshu_format,
                    _ if cat_name.contains("粮油") || cat_name.contains("干调") => &cat_liangyou_format,
                    _ if cat_name.contains("豆制品") => &cat_douzhi_format,
                    _ if cat_name.contains("粉面") => &cat_fenmian_format,
                    _ if cat_name.contains("水果") => &cat_shuiguo_format,
                    _ => &cat_other_format,
                };

                let cat_title = format!("【{}】", cat_name);
                worksheet.merge_range(current_row, 0, current_row, 5, cat_title.as_str(), cat_format)?;
                worksheet.set_row_height(current_row, 22)?;
                current_row += 1;

                if let Some(purchasers) = cat["purchasers"].as_array() {
                    for purchaser in purchasers {
                        let purchaser_name = purchaser["purchaser_name"].as_str().unwrap_or("");
                        let purchaser_title = format!("├── {}", purchaser_name);
                        worksheet.merge_range(current_row, 0, current_row, 5, purchaser_title.as_str(), &purchaser_format)?;
                        worksheet.set_row_height(current_row, 20)?;
                        current_row += 1;

                        if let Some(items) = purchaser["items"].as_array() {
                            for item in items {
                                let product_name = item["product_name"].as_str().unwrap_or("");
                                let unit = item["unit"].as_str().unwrap_or("");
                                let quantity = item["quantity"].as_f64().unwrap_or(0.0);
                                let remark = item["remark"].as_str().unwrap_or("");

                                worksheet.write_with_format(current_row, 0, seq as f64, &cell_format)?;
                                worksheet.write_with_format(current_row, 1, product_name, &cell_left_format)?;
                                worksheet.write_with_format(current_row, 2, unit, &cell_format)?;
                                worksheet.write_with_format(current_row, 3, quantity, &cell_format)?;
                                worksheet.write_with_format(current_row, 4, remark, &cell_left_format)?;
                                worksheet.write_with_format(current_row, 5, purchaser_name, &cell_left_format)?;
                                current_row += 1;
                                seq += 1;
                            }
                        }
                    }
                }

                current_row += 1;
            }

            let buf = workbook.save_to_buffer()?;
            Ok(buf)
        };
        let excel_result: Result<Vec<u8>, String> = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(build_excel)) {
            Ok(r) => r.map_err(|e| format!("{:?}", e)),
            Err(p) => Err(format!("Excel 构建 panic: {:?}", p)),
        };

        let buf: Vec<u8> = match excel_result {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Excel export error: {}", e);
                return Err(UseCaseError::Storage("导出失败".to_string()));
            }
        };
        Ok(("purchase_sort_by_category.xlsx".to_string(), buf))
    }

    async fn sort_items_by_supplier(&self, date: String) -> Result<String, UseCaseError> {
        // 无日期：当前待分拣（pending/sorting）；有日期：检索该日期的历史分拣清单
        let rows = self.repo.sort_supplier_rows(&date).await.unwrap_or_default();

        let mut supplier_map: std::collections::HashMap<(String, i64), std::collections::HashMap<(String, i64), Vec<serde_json::Value>>> = std::collections::HashMap::new();

        for r in &rows {
            let supplier_id_key = r.supplier_id;
            let supplier_name = r.supplier_name.clone().unwrap_or_else(|| {
                if supplier_id_key == 0 { "未分配供应商".to_string() } else { format!("供应商{}", supplier_id_key) }
            });
            let purchaser_name = r.purchaser_name.clone();
            let purchaser_id = r.purchaser_id;

            let purchaser_map = supplier_map.entry((supplier_name, supplier_id_key)).or_insert_with(std::collections::HashMap::new);
            let purchaser_items = purchaser_map.entry((purchaser_name, purchaser_id)).or_insert_with(Vec::new);

            purchaser_items.push(serde_json::json!({
                "item_id": r.item_id,
                "order_id": r.order_id,
                "product_id": r.product_id,
                "product_name": r.product_name.clone(),
                "unit": r.unit.clone().unwrap_or_default(),
                "unit_price": r.unit_price,
                "purchase_price": r.purchase_price,
                "quantity": r.quantity,
                "amount": r.amount,
                "remark": r.remark.clone().unwrap_or_default(),
                "order_no": r.order_no.clone().unwrap_or_default(),
            }));
        }

        let mut result: Vec<serde_json::Value> = Vec::new();
        for ((supplier_name, supplier_id), purchaser_map) in supplier_map {
            let mut purchasers: Vec<serde_json::Value> = Vec::new();
            for ((purchaser_name, purchaser_id), items) in purchaser_map {
                let total_qty: f64 = items.iter().map(|item| item["quantity"].as_f64().unwrap_or(0.0)).sum();
                purchasers.push(serde_json::json!({
                    "purchaser_name": purchaser_name,
                    "purchaser_id": purchaser_id,
                    "items": items,
                    "total_quantity": total_qty,
                }));
            }
            purchasers.sort_by(|a, b| a["purchaser_name"].as_str().unwrap_or("").cmp(b["purchaser_name"].as_str().unwrap_or(""))
                .then_with(|| a["purchaser_id"].as_i64().unwrap_or(0).cmp(&b["purchaser_id"].as_i64().unwrap_or(0))));

            let total_qty: f64 = purchasers.iter().map(|p| p["total_quantity"].as_f64().unwrap_or(0.0)).sum();
            result.push(serde_json::json!({
                "supplier_name": supplier_name,
                "supplier_id": supplier_id,
                "purchasers": purchasers,
                "total_quantity": total_qty,
            }));
        }

        result.sort_by(|a, b| a["supplier_name"].as_str().unwrap_or("").cmp(b["supplier_name"].as_str().unwrap_or(""))
            .then_with(|| a["supplier_id"].as_i64().unwrap_or(0).cmp(&b["supplier_id"].as_i64().unwrap_or(0))));

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn sort_items_by_supplier_excel(&self, date: String, print_values: bool) -> Result<(String, Vec<u8>), UseCaseError> {
        // 无日期：当前待分拣（pending/sorting）；有日期：检索该日期的历史分拣清单
        let has_date = !date.is_empty();
        let rows = self.repo.sort_supplier_excel_rows(&date).await.unwrap_or_default();

        // key 带 id：同名时靠 id 确定唯一顺序
        let mut supplier_map: std::collections::HashMap<(String, i64), std::collections::HashMap<(String, i64), Vec<serde_json::Value>>> = std::collections::HashMap::new();

        for r in &rows {
            let supplier_id_key = r.supplier_id;
            let supplier_name = sanitize_xlsx_string(r.supplier_name.clone().unwrap_or_else(|| {
                if supplier_id_key == 0 { "未分配供应商".to_string() } else { format!("供应商{}", supplier_id_key) }
            }));
            let purchaser_name = sanitize_xlsx_string(r.purchaser_name.clone());
            let purchaser_id = r.purchaser_id;

            // 导出名称与采购单规则对齐：规格 -> 名称（独立取值不拼接）
            let display_name = {
                let spec = r.spec.clone().unwrap_or_default();
                let spec = spec.trim().to_string();
                if spec.is_empty() { r.product_name.clone() } else { spec }
            };

            let purchaser_map = supplier_map.entry((supplier_name, supplier_id_key)).or_insert_with(std::collections::HashMap::new);
            let purchaser_items = purchaser_map.entry((purchaser_name, purchaser_id)).or_insert_with(Vec::new);

            purchaser_items.push(serde_json::json!({
                "product_id": r.product_id,
                "product_name": sanitize_xlsx_string(display_name),
                "unit": sanitize_xlsx_string(r.unit.clone().unwrap_or_default()),
                "quantity": r.quantity,
                "pre_sale_quantity": r.pre_sale_quantity.unwrap_or(0.0),
                "amount": r.amount.unwrap_or(0.0),
                "unit_price": r.purchase_price,
                "remark": sanitize_xlsx_string(r.remark.clone().unwrap_or_default()),
                "order_no": sanitize_xlsx_string(r.order_no.clone().unwrap_or_default()),
            }));
        }

        let mut result: Vec<serde_json::Value> = Vec::new();
        for ((supplier_name, supplier_id), purchaser_map) in supplier_map {
            let mut purchasers: Vec<serde_json::Value> = Vec::new();
            for ((purchaser_name, purchaser_id), items) in purchaser_map {
                let total_qty: f64 = items.iter().map(|item| item["quantity"].as_f64().unwrap_or(0.0)).sum();
                purchasers.push(serde_json::json!({
                    "purchaser_name": purchaser_name,
                    "purchaser_id": purchaser_id,
                    "items": items,
                    "total_quantity": total_qty,
                }));
            }
            purchasers.sort_by(|a, b| a["purchaser_name"].as_str().unwrap_or("").cmp(b["purchaser_name"].as_str().unwrap_or(""))
                .then_with(|| a["purchaser_id"].as_i64().unwrap_or(0).cmp(&b["purchaser_id"].as_i64().unwrap_or(0))));

            let total_qty: f64 = purchasers.iter().map(|p| p["total_quantity"].as_f64().unwrap_or(0.0)).sum();
            result.push(serde_json::json!({
                "supplier_name": supplier_name,
                "supplier_id": supplier_id,
                "purchasers": purchasers,
                "total_quantity": total_qty,
            }));
        }

        result.sort_by(|a, b| a["supplier_name"].as_str().unwrap_or("").cmp(b["supplier_name"].as_str().unwrap_or(""))
            .then_with(|| a["supplier_id"].as_i64().unwrap_or(0).cmp(&b["supplier_id"].as_i64().unwrap_or(0))));

        // 用 catch_unwind 包裹 Excel 构建过程，避免单个坏数据导致整个进程 panic 崩溃
        let build_excel = || -> Result<Vec<u8>, XlsxError> {
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

            let cell_right_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Right)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);

            let purchaser_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#E5E7EB")
                .set_font_color("#374151");

            let col_widths = [4, 16, 4, 6, 6, 6, 8, 8];
            let headers = ["序号", "品名规格", "单位", "订量", "实量", "单价", "金额", "备注"];
            let display_date = if has_date {
                date.clone()
            } else {
                Local::now().format("%Y-%m-%d").to_string()
            };

            let date_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Right)
                .set_align(FormatAlign::VerticalCenter);

            let summary_format = Format::new()
                .set_bold()
                .set_font_size(10)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter);

            let summary_right_format = Format::new()
                .set_bold()
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

            let grand_total_right_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Right)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#E5E7EB")
                .set_font_color("#374151");

            let max_col = 7u16;

            if result.is_empty() {
                let worksheet = workbook.add_worksheet();
                worksheet.set_name("无数据")?;
                worksheet.merge_range(0, 0, 0, max_col, "暂无分拣数据", &title_format)?;
            } else {
                for supplier in &result {
                    let supplier_name = supplier["supplier_name"].as_str().unwrap_or("未分配供应商");
                    // Excel sheet 名称最多 31 个字符，且禁止 \ / ? * [ ] : 字符。
                    // 按字符（而非字节）安全截断，避免中文供应商名按字节切片时 panic
                    let sheet_name: String = supplier_name
                        .chars()
                        .filter(|c| !matches!(c, '\\' | '/' | '?' | '*' | '[' | ']' | ':'))
                        .take(31)
                        .collect();
                    let worksheet = workbook.add_worksheet();
                    worksheet.set_name(sheet_name.as_str())?;

                    worksheet.set_landscape();
                    // 上边距1cm，下边距1.27cm留出页脚间隙，页脚1cm，左右为0
                    worksheet.set_margins(0.0, 0.0, 0.4, 0.5, 0.0, 0.2);
                    worksheet.set_print_center_vertically(false);
                    worksheet.set_print_center_horizontally(true);
                    worksheet.set_header("");
                    worksheet.set_footer("&C第 &P 页，共 &N 页");
                    // 每页重复打印标题、日期、表头(行0-2)，保证多页时每页都有页头和日期
                    worksheet.set_repeat_rows(0, 2)?;

                    for (i, w) in col_widths.iter().enumerate() {
                        worksheet.set_column_width(i as u16, *w)?;
                    }

                    let mut current_row = 0;
                    let title = format!("{} - 采购分拣清单", supplier_name);
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

                    let mut grand_total_items = 0i64;
                    let mut grand_total_amount = 0.0;

                    if let Some(purchasers) = supplier["purchasers"].as_array() {
                        for purchaser in purchasers {
                            let purchaser_name = purchaser["purchaser_name"].as_str().unwrap_or("");
                            let purchaser_title = format!("├── {}", purchaser_name);
                            worksheet.merge_range(current_row, 0, current_row, max_col, purchaser_title.as_str(), &purchaser_format)?;
                            worksheet.set_row_height(current_row, 20)?;
                            current_row += 1;

                            if let Some(items) = purchaser["items"].as_array() {
                                // 按单位分组（保持插入顺序）
                                let mut unit_groups: Vec<(String, Vec<&serde_json::Value>)> = Vec::new();
                                for item in items {
                                    let unit = item["unit"].as_str().unwrap_or("").to_string();
                                    if let Some(pos) = unit_groups.iter().position(|(u, _)| u == &unit) {
                                        unit_groups[pos].1.push(item);
                                    } else {
                                        unit_groups.push((unit, vec![item]));
                                    }
                                }

                                let mut purchaser_total_items = 0i64;
                                let mut purchaser_total_amount = 0.0;
                                let mut purchaser_seq = 1;

                                for (unit, group_items) in &unit_groups {
                                    let mut unit_amount = 0.0;
                                    for item in group_items {
                                        let product_name = item["product_name"].as_str().unwrap_or("");
                                        let quantity = item["quantity"].as_f64().unwrap_or(0.0);
                                        let pre_sale_quantity = item["pre_sale_quantity"].as_f64().unwrap_or(0.0);
                                        let stored_amount = item["amount"].as_f64().unwrap_or(0.0);
                                        let remark = item["remark"].as_str().unwrap_or("");
                                        // 单价：使用基础单位的进价（来自 product.purchase_price），而不是售价
                                        let unit_price = item["unit_price"].as_f64().unwrap_or(0.0);
                                        // 金额：实量 × 进价（含数值导出时以实量与进价之积为准）
                                        let row_amount = if print_values { quantity * unit_price } else { stored_amount };

                                        unit_amount += row_amount;

                                        worksheet.write_with_format(current_row, 0, purchaser_seq as f64, &cell_format)?;
                                        worksheet.write_with_format(current_row, 1, product_name, &cell_left_format)?;
                                        worksheet.write_with_format(current_row, 2, unit.as_str(), &cell_format)?;
                                        worksheet.write_with_format(current_row, 3, pre_sale_quantity, &cell_format)?;
                                        if print_values {
                                            worksheet.write_with_format(current_row, 4, quantity, &cell_format)?;
                                            worksheet.write_with_format(current_row, 5, unit_price, &cell_format)?;
                                            worksheet.write_with_format(current_row, 6, row_amount, &cell_right_format)?;
                                        } else {
                                            worksheet.write_with_format(current_row, 4, "", &cell_format)?;
                                            worksheet.write_with_format(current_row, 5, "", &cell_format)?;
                                            worksheet.write_with_format(current_row, 6, "", &cell_right_format)?;
                                        }
                                        worksheet.write_with_format(current_row, 7, remark, &cell_left_format)?;
                                        worksheet.set_row_height(current_row, 20.0)?;//设置供应商分拣页面的导出xlsx的行高
                                        current_row += 1;
                                        purchaser_seq += 1;
                                    }
                                    let unit_count = group_items.len();
                                    purchaser_total_items += unit_count as i64;
                                    purchaser_total_amount += unit_amount;
                                }

                                // 采购单位小计
                                let purchaser_total = format!("小计: 包装数量 {}", purchaser_total_items);
                                worksheet.merge_range(current_row, 0, current_row, 5, purchaser_total.as_str(), &summary_format)?;
                                if print_values {
                                    worksheet.write_with_format(current_row, 6, purchaser_total_amount, &summary_right_format)?;
                                } else {
                                    worksheet.write_with_format(current_row, 6, "", &summary_right_format)?;
                                }
                                worksheet.set_row_height(current_row, 18)?;
                                current_row += 1;

                                grand_total_items += purchaser_total_items;
                                grand_total_amount += purchaser_total_amount;
                            }
                        }
                    }

                    // 供应商总计
                    let grand_total = format!("总计: 包装数量 {}", grand_total_items);
                    worksheet.merge_range(current_row, 0, current_row, 5, grand_total.as_str(), &grand_total_format)?;
                    if print_values {
                        worksheet.write_with_format(current_row, 6, grand_total_amount, &grand_total_right_format)?;
                    } else {
                        worksheet.write_with_format(current_row, 6, "", &grand_total_right_format)?;
                    }
                    worksheet.set_row_height(current_row, 22)?;
                }
            }

            let buf = workbook.save_to_buffer()?;
            Ok(buf)
        };
        let excel_result: Result<Vec<u8>, String> = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(build_excel)) {
            Ok(r) => r.map_err(|e| format!("{:?}", e)),
            Err(p) => Err(format!("Excel 构建 panic: {:?}", p)),
        };

        match excel_result {
            Ok(buf) => {
                let filename = if has_date {
                    format!("采购分拣清单_按供应商_{}.xlsx", date)
                } else {
                    "采购分拣清单_按供应商.xlsx".to_string()
                };
                Ok((filename, buf))
            }
            Err(e) => {
                eprintln!("Excel export error: {}", e);
                Err(UseCaseError::Storage("导出失败".to_string()))
            }
        }
    }

    async fn sort_items_by_purchaser(&self, date: String) -> Result<String, UseCaseError> {
        let rows = self.repo.sort_purchaser_rows(&date).await.unwrap_or_default();

        let mut purchaser_map: std::collections::HashMap<i64, serde_json::Value> = std::collections::HashMap::new();

        for r in &rows {
            let purchaser_id = r.purchaser_id;
            let purchaser_name = r.purchaser_name.clone().unwrap_or_default();

            let purchaser = purchaser_map.entry(purchaser_id).or_insert_with(|| {
                serde_json::json!({
                    "purchaser_id": purchaser_id,
                    "purchaser_name": purchaser_name,
                    "items": Vec::new() as Vec<serde_json::Value>,
                    "total_amount": 0.0,
                    "total_quantity": 0.0,
                })
            });

            purchaser["total_quantity"] = serde_json::json!(purchaser["total_quantity"].as_f64().unwrap_or(0.0) + r.quantity);
            purchaser["total_amount"] = serde_json::json!(purchaser["total_amount"].as_f64().unwrap_or(0.0) + r.amount);

            let items = purchaser["items"].as_array_mut().unwrap();
            items.push(serde_json::json!({
                "id": r.id,
                "product_id": r.product_id,
                "product_name": r.product_name.clone(),
                "unit": r.unit.clone().unwrap_or_default(),
                "unit_price": r.unit_price,
                "quantity": r.quantity,
                "amount": r.amount,
                "order_no": r.order_no.clone().unwrap_or_default(),
                "remark": r.remark.clone().unwrap_or_default(),
            }));
        }

        let mut purchasers: Vec<serde_json::Value> = purchaser_map.values().cloned().collect();
        purchasers.sort_by(|a, b| a["purchaser_name"].as_str().unwrap_or("").cmp(b["purchaser_name"].as_str().unwrap_or(""))
            .then_with(|| a["purchaser_id"].as_i64().unwrap_or(0).cmp(&b["purchaser_id"].as_i64().unwrap_or(0))));

        Ok(serde_json::to_string(&purchasers).unwrap())
    }

    async fn sort_items_by_purchaser_excel(&self, date: String) -> Result<(String, Vec<u8>), UseCaseError> {
        let rows = self.repo.sort_purchaser_excel_rows(&date).await.unwrap_or_default();
        let price_rows = self.repo.sort_purchaser_price_rows().await.unwrap_or_default();

        let mut price_map: std::collections::HashMap<i64, (f64, f64, f64)> = std::collections::HashMap::new();
        for r in &price_rows {
            let product_id = r.product_id;
            let max_price = r.max_price.unwrap_or(0.0);
            let min_price = r.min_price.unwrap_or(0.0);
            let latest_price = r.latest_price.unwrap_or(0.0);
            price_map.insert(product_id, (max_price, min_price, latest_price));
        }

        let mut purchaser_map: std::collections::HashMap<i64, serde_json::Value> = std::collections::HashMap::new();

        for r in &rows {
            let purchaser_id = r.purchaser_id;
            let purchaser_name = r.purchaser_name.clone().unwrap_or_default();
            let product_id = r.product_id;

            let purchaser = purchaser_map.entry(purchaser_id).or_insert_with(|| {
                serde_json::json!({
                    "purchaser_id": purchaser_id,
                    "purchaser_name": purchaser_name,
                    "items": Vec::new() as Vec<serde_json::Value>,
                    "total_amount": 0.0,
                    "total_quantity": 0.0,
                })
            });

            purchaser["total_quantity"] = serde_json::json!(purchaser["total_quantity"].as_f64().unwrap_or(0.0) + r.quantity);
            purchaser["total_amount"] = serde_json::json!(purchaser["total_amount"].as_f64().unwrap_or(0.0) + r.amount);

            let category_name = r.category_name.clone().unwrap_or_default();
            let parent_name = r.parent_name.clone().unwrap_or_default();
            let sort_key = get_category_sort_key(&category_name, &parent_name);

            let (max_price, min_price, latest_price) = price_map.get(&product_id).copied().unwrap_or((0.0, 0.0, 0.0));

            // 导出名称与采购单规则对齐：规格 -> 名称（独立取值不拼接）
            let display_name = {
                let spec = r.spec.clone().unwrap_or_default();
                let spec = spec.trim().to_string();
                if spec.is_empty() { r.product_name.clone() } else { spec }
            };

            let items = purchaser["items"].as_array_mut().unwrap();
            items.push(serde_json::json!({
                "id": r.id,
                "product_id": product_id,
                "product_name": display_name,
                "unit": r.unit.clone().unwrap_or_default(),
                "unit_price": r.unit_price,
                "quantity": r.quantity,
                "amount": r.amount,
                "order_no": r.order_no.clone().unwrap_or_default(),
                "remark": r.remark.clone().unwrap_or_default(),
                "sort_key": sort_key,
                "max_price": max_price,
                "min_price": min_price,
                "latest_price": latest_price,
                "selling_price": r.unit_price,
            }));
        }

        let mut purchasers: Vec<serde_json::Value> = purchaser_map.values().cloned().collect();
        // 固定排序：按采购单位名称排序，保证每次导出顺序一致
        purchasers.sort_by(|a, b| a["purchaser_name"].as_str().unwrap_or("").cmp(b["purchaser_name"].as_str().unwrap_or(""))
            .then_with(|| a["purchaser_id"].as_i64().unwrap_or(0).cmp(&b["purchaser_id"].as_i64().unwrap_or(0))));
        for p in purchasers.iter_mut() {
            let items = p["items"].as_array_mut().unwrap();
            // 分类序优先，同类按品名、明细 id 兜底
            items.sort_by(|a, b| a["sort_key"].as_i64().unwrap_or(999).cmp(&b["sort_key"].as_i64().unwrap_or(999))
                .then_with(|| a["product_name"].as_str().unwrap_or("").cmp(b["product_name"].as_str().unwrap_or("")))
                .then_with(|| a["id"].as_i64().unwrap_or(0).cmp(&b["id"].as_i64().unwrap_or(0))));
        }

        let excel_result: Result<Vec<u8>, XlsxError> = (|| {
            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet();

            worksheet.set_landscape();
            worksheet.set_margins(0.2, 0.2, 0.2, 0.2, 0.2, 0.2);
            worksheet.set_print_center_vertically(false);
            worksheet.set_print_center_horizontally(true);

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

            let section_title_format = Format::new()
                .set_bold()
                .set_font_size(12)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#E5E7EB");

            let cell_format = Format::new()
                .set_font_size(10)
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::VerticalCenter);

            let cell_left_format = Format::new()
                .set_font_size(10)
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter);

            let price_format = Format::new()
                .set_font_size(10)
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Right)
                .set_num_format("0.00");

            worksheet.merge_range(0, 0, 0, 8, "按单位分拣清单", &title_format)?;
            worksheet.set_row_height(0, 28)?;

            let headers = ["序号", "商品名称", "单位", "数量", "备注", "历史最高", "历史最低", "历史最近", "售价"];
            let mut current_row = 2;

            for purchaser in &purchasers {
                current_row += 1;
                worksheet.merge_range(current_row, 0, current_row, 8, purchaser["purchaser_name"].as_str().unwrap_or_default(), &section_title_format)?;

                current_row += 1;
                for (i, h) in headers.iter().enumerate() {
                    worksheet.write_with_format(current_row, i as u16, *h, &header_format)?;
                }

                let mut index = 1;
                let items = purchaser["items"].as_array().unwrap();
                for item in items {
                    current_row += 1;
                    worksheet.write_with_format(current_row, 0, index as f64, &cell_format)?;
                    worksheet.write_with_format(current_row, 1, item["product_name"].as_str().unwrap_or_default(), &cell_left_format)?;
                    worksheet.write_with_format(current_row, 2, item["unit"].as_str().unwrap_or_default(), &cell_format)?;
                    worksheet.write_with_format(current_row, 3, item["quantity"].as_f64().unwrap_or(0.0), &cell_format)?;
                    worksheet.write_with_format(current_row, 4, item["remark"].as_str().unwrap_or_default(), &cell_left_format)?;
                    worksheet.write_with_format(current_row, 5, item["max_price"].as_f64().unwrap_or(0.0), &price_format)?;
                    worksheet.write_with_format(current_row, 6, item["min_price"].as_f64().unwrap_or(0.0), &price_format)?;
                    worksheet.write_with_format(current_row, 7, item["latest_price"].as_f64().unwrap_or(0.0), &price_format)?;
                    worksheet.write_with_format(current_row, 8, item["selling_price"].as_f64().unwrap_or(0.0), &price_format)?;
                    index += 1;
                }

                current_row += 2;
            }

            worksheet.set_column_width(0, 10)?;
            worksheet.set_column_width(1, 30)?;
            worksheet.set_column_width(2, 12)?;
            worksheet.set_column_width(3, 12)?;
            worksheet.set_column_width(4, 20)?;
            worksheet.set_column_width(5, 12)?;
            worksheet.set_column_width(6, 12)?;
            worksheet.set_column_width(7, 12)?;
            worksheet.set_column_width(8, 12)?;

            let buf = workbook.save_to_buffer()?;
            Ok(buf)
        })();

        match excel_result {
            Ok(buf) => Ok(("按单位分拣清单.xlsx".to_string(), buf)),
            Err(e) => {
                eprintln!("Excel export error: {}", e);
                Err(UseCaseError::Storage("导出失败".to_string()))
            }
        }
    }

    async fn sort_comprehensive(&self, date: String) -> Result<String, UseCaseError> {
        let rows = self.repo.sort_comprehensive_rows(&date).await.unwrap_or_default();

        #[derive(Debug, Clone)]
        struct CategoryData {
            id: i64,
            name: String,
            items: Vec<serde_json::Value>,
        }

        #[derive(Debug, Clone)]
        struct PurchaserData {
            id: i64,
            name: String,
            categories: Vec<CategoryData>,
            total_amount: f64,
        }

        let mut purchaser_map: std::collections::HashMap<i64, PurchaserData> = std::collections::HashMap::new();

        for r in &rows {
            let purchaser_id = r.purchaser_id;
            let purchaser_name = r.purchaser_name.clone().unwrap_or_default();
            let category_id = r.category_id;
            let category_name = r.category_name.clone().unwrap_or_else(|| "未分类".to_string());

            let purchaser = purchaser_map.entry(purchaser_id).or_insert_with(|| PurchaserData {
                id: purchaser_id,
                name: purchaser_name,
                categories: Vec::new(),
                total_amount: 0.0,
            });

            purchaser.total_amount += r.amount;

            let category = purchaser.categories.iter_mut()
                .find(|c| c.id == category_id);

            let category_items = match category {
                Some(c) => &mut c.items,
                None => {
                    purchaser.categories.push(CategoryData {
                        id: category_id,
                        name: category_name.clone(),
                        items: Vec::new(),
                    });
                    &mut purchaser.categories.last_mut().unwrap().items
                }
            };

            let existing_idx = category_items.iter().position(|item| item["product_id"].as_i64() == Some(r.product_id));

            if let Some(idx) = existing_idx {
                let item = &mut category_items[idx];
                let current_qty = item["quantity"].as_f64().unwrap_or(0.0);
                let current_amount = item["amount"].as_f64().unwrap_or(0.0);
                item["quantity"] = serde_json::json!(current_qty + r.quantity);
                item["amount"] = serde_json::json!(current_amount + r.amount);

                let order_nos: Vec<String> = item["order_nos"].as_array().unwrap_or(&vec![]).iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect();
                let new_order = r.order_no.clone().unwrap_or_default();
                if !order_nos.contains(&new_order) {
                    let mut new_orders = order_nos;
                    new_orders.push(new_order);
                    item["order_nos"] = serde_json::json!(new_orders);
                }

                let remark = r.remark.clone().unwrap_or_default();
                if !remark.is_empty() {
                    let existing_remarks: Vec<String> = item["remarks"].as_array().unwrap_or(&vec![]).iter()
                        .map(|v| v.as_str().unwrap_or("").to_string())
                        .collect();
                    if !existing_remarks.contains(&remark) {
                        let mut new_remarks = existing_remarks;
                        new_remarks.push(remark);
                        item["remarks"] = serde_json::json!(new_remarks);
                    }
                }
            } else {
                let remark = r.remark.clone().unwrap_or_default();
                category_items.push(serde_json::json!({
                    "id": r.id,
                    "product_id": r.product_id,
                    "product_name": r.product_name.clone(),
                    "unit": r.unit.clone().unwrap_or_default(),
                    "unit_price": r.unit_price,
                    "quantity": r.quantity,
                    "amount": r.amount,
                    "order_nos": vec![r.order_no.clone().unwrap_or_default()],
                    "remarks": if remark.is_empty() { vec![] } else { vec![remark] },
                }));
            }
        }

        let mut result: Vec<serde_json::Value> = Vec::new();
        for (_, purchaser) in purchaser_map {
            let mut categories_json: Vec<serde_json::Value> = Vec::new();
            for cat in purchaser.categories {
                categories_json.push(serde_json::json!({
                    "category_id": cat.id,
                    "category_name": cat.name,
                    "items": cat.items,
                }));
            }
            categories_json.sort_by(|a, b| a["category_name"].as_str().unwrap_or("").cmp(b["category_name"].as_str().unwrap_or(""))
                .then_with(|| a["category_id"].as_i64().unwrap_or(0).cmp(&b["category_id"].as_i64().unwrap_or(0))));

            result.push(serde_json::json!({
                "purchaser_id": purchaser.id,
                "purchaser_name": purchaser.name,
                "categories": categories_json,
                "total_amount": purchaser.total_amount,
            }));
        }

        result.sort_by(|a, b| a["purchaser_name"].as_str().unwrap_or("").cmp(b["purchaser_name"].as_str().unwrap_or(""))
            .then_with(|| a["purchaser_id"].as_i64().unwrap_or(0).cmp(&b["purchaser_id"].as_i64().unwrap_or(0))));

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn sort_comprehensive_excel(&self, date: String) -> Result<(String, Vec<u8>), UseCaseError> {
        let rows = self.repo.sort_comprehensive_excel_rows(&date).await.unwrap_or_default();

        #[derive(Debug, Clone)]
        struct CategoryData {
            id: i64,
            name: String,
            items: Vec<serde_json::Value>,
        }

        #[derive(Debug, Clone)]
        struct PurchaserData {
            id: i64,
            name: String,
            categories: Vec<CategoryData>,
        }

        let mut purchaser_map: std::collections::HashMap<i64, PurchaserData> = std::collections::HashMap::new();

        for r in &rows {
            let purchaser_id = r.purchaser_id;
            let purchaser_name = r.purchaser_name.clone().unwrap_or_default();
            let category_id = r.category_id;
            let category_name = r.category_name.clone().unwrap_or_else(|| "未分类".to_string());

            let purchaser = purchaser_map.entry(purchaser_id).or_insert_with(|| PurchaserData {
                id: purchaser_id,
                name: purchaser_name,
                categories: Vec::new(),
            });

            let category = purchaser.categories.iter_mut()
                .find(|c| c.id == category_id);

            let category_items = match category {
                Some(c) => &mut c.items,
                None => {
                    purchaser.categories.push(CategoryData {
                        id: category_id,
                        name: category_name.clone(),
                        items: Vec::new(),
                    });
                    &mut purchaser.categories.last_mut().unwrap().items
                }
            };

            let existing_idx = category_items.iter().position(|item| item["product_id"].as_i64() == Some(r.product_id));

            if let Some(idx) = existing_idx {
                let item = &mut category_items[idx];
                let current_qty = item["quantity"].as_f64().unwrap_or(0.0);
                item["quantity"] = serde_json::json!(current_qty + r.quantity);

                let order_nos: Vec<String> = item["order_nos"].as_array().unwrap_or(&vec![]).iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect();
                let new_order = r.order_no.clone().unwrap_or_default();
                if !order_nos.contains(&new_order) {
                    let mut new_orders = order_nos;
                    new_orders.push(new_order);
                    item["order_nos"] = serde_json::json!(new_orders);
                }

                let existing_remarks: Vec<String> = item["remarks"].as_array().unwrap_or(&vec![]).iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let new_remark = r.remark.clone().unwrap_or_default();
                if !new_remark.is_empty() && !existing_remarks.contains(&new_remark) {
                    let mut new_remarks = existing_remarks;
                    new_remarks.push(new_remark);
                    item["remarks"] = serde_json::json!(new_remarks);
                }
            } else {
                let remark = r.remark.clone().unwrap_or_default();
                // 导出名称与采购单规则对齐：规格 -> 名称（独立取值不拼接）
                let display_name = {
                    let spec = r.spec.clone().unwrap_or_default();
                    let spec = spec.trim().to_string();
                    if spec.is_empty() { r.product_name.clone() } else { spec }
                };
                category_items.push(serde_json::json!({
                    "id": r.id,
                    "product_id": r.product_id,
                    "product_name": display_name,
                    "unit": r.unit.clone().unwrap_or_default(),
                    "quantity": r.quantity,
                    "order_nos": vec![r.order_no.clone().unwrap_or_default()],
                    "remarks": if remark.is_empty() { vec![] } else { vec![remark] },
                }));
            }
        }

        let mut result: Vec<PurchaserData> = purchaser_map.into_values().collect();
        result.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
        for p in &mut result {
            p.categories.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
        }

        let excel_result: Result<Vec<u8>, XlsxError> = (|| {
            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet();

            worksheet.set_landscape();
            worksheet.set_margins(0.2, 0.2, 0.2, 0.2, 0.2, 0.2);
            worksheet.set_print_center_vertically(false);
            worksheet.set_print_center_horizontally(true);

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

            let purchaser_format = Format::new()
                .set_bold()
                .set_font_size(12)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#0EA5E9")
                .set_font_color("#FFFFFF");

            let cat_hunxian_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#DC2626")
                .set_font_color("#FFFFFF");

            let cat_xianshu_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#16A34A")
                .set_font_color("#FFFFFF");

            let cat_liangyou_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#1D4ED8")
                .set_font_color("#FFFFFF");

            let cat_douzhi_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#CA8A04")
                .set_font_color("#FFFFFF");

            let cat_fenmian_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#64748B")
                .set_font_color("#FFFFFF");

            let cat_shuiguo_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#EA580C")
                .set_font_color("#FFFFFF");

            let cat_other_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_background_color("#6B7280")
                .set_font_color("#FFFFFF");

            let col_widths = [6.0, 20.0, 8.0, 10.0, 20.0, 20.0];
            for (i, w) in col_widths.iter().enumerate() {
                worksheet.set_column_width(i as u16, *w)?;
            }

            let mut current_row = 0;
            let title = format!("采购分拣清单（综合）");
            worksheet.merge_range(current_row, 0, current_row, 5, title.as_str(), &title_format)?;
            worksheet.set_row_height(current_row, 28)?;
            current_row += 2;

            let headers = ["序号", "商品名称", "单位", "数量", "备注", "订单号"];
            for (i, header) in headers.iter().enumerate() {
                worksheet.write_with_format(current_row, i as u16, *header, &header_format)?;
            }
            current_row += 1;

            let mut seq = 1;

            for purchaser in &result {
                let p_title = format!("【采购单位：{}】", purchaser.name);
                worksheet.merge_range(current_row, 0, current_row, 5, p_title.as_str(), &purchaser_format)?;
                worksheet.set_row_height(current_row, 22)?;
                current_row += 1;

                for cat in &purchaser.categories {
                    let cat_format = match () {
                        _ if cat.name.contains("荤鲜") => &cat_hunxian_format,
                        _ if cat.name.contains("鲜蔬") => &cat_xianshu_format,
                        _ if cat.name.contains("粮油") || cat.name.contains("干调") => &cat_liangyou_format,
                        _ if cat.name.contains("豆制品") => &cat_douzhi_format,
                        _ if cat.name.contains("粉面") => &cat_fenmian_format,
                        _ if cat.name.contains("水果") => &cat_shuiguo_format,
                        _ => &cat_other_format,
                    };

                    let cat_title = format!("【{}】", cat.name);
                    worksheet.merge_range(current_row, 0, current_row, 5, cat_title.as_str(), cat_format)?;
                    worksheet.set_row_height(current_row, 18)?;
                    current_row += 1;

                    for item in &cat.items {
                        let product_name = item["product_name"].as_str().unwrap_or("");
                        let unit = item["unit"].as_str().unwrap_or("");
                        let quantity = item["quantity"].as_f64().unwrap_or(0.0);

                        let order_nos: Vec<String> = item["order_nos"].as_array().unwrap_or(&vec![]).iter()
                            .map(|v| v.as_str().unwrap_or("").to_string())
                            .collect();

                        let remarks: Vec<String> = item["remarks"].as_array().unwrap_or(&vec![]).iter()
                            .map(|v| v.as_str().unwrap_or("").to_string())
                            .collect();

                        worksheet.write_with_format(current_row, 0, seq as f64, &cell_format)?;
                        worksheet.write_with_format(current_row, 1, product_name, &cell_left_format)?;
                        worksheet.write_with_format(current_row, 2, unit, &cell_format)?;
                        worksheet.write_with_format(current_row, 3, quantity, &cell_format)?;
                        worksheet.write_with_format(current_row, 4, remarks.join(", "), &cell_left_format)?;
                        worksheet.write_with_format(current_row, 5, order_nos.join(", "), &cell_left_format)?;
                        current_row += 1;
                        seq += 1;
                    }
                }

                current_row += 1;
            }

            let buf = workbook.save_to_buffer()?;
            Ok(buf)
        })();

        match excel_result {
            Ok(buf) => Ok(("采购分拣清单_综合.xlsx".to_string(), buf)),
            Err(e) => {
                eprintln!("Excel export error: {}", e);
                Err(UseCaseError::Storage("导出失败".to_string()))
            }
        }
    }

    async fn correction(&self, corrections: Vec<serde_json::Value>, _ctx: &UserCtx) -> Result<SalesOrderOutcome, UseCaseError> {
        let mut updated_count = 0i64;

        for item in corrections {
            let item_id = item.get("id").and_then(|v| v.as_i64());
            let product_id = item.get("product_id").and_then(|v| v.as_i64());
            // 数量与单价独立可选：只更新提供的字段，金额按当前生效的单价×数量重算
            let quantity = item.get("quantity").and_then(|v| v.as_f64())
                .map(|q| (q * 100.0).round() / 100.0);
            let unit_price = item.get("unit_price").and_then(|v| v.as_f64())
                .map(|p| (p * 100.0).round() / 100.0);
            // 进价修正：更新商品进价（product.purchase_price），供采购订单生成/导出使用
            let purchase_price = item.get("purchase_price").and_then(|v| v.as_f64())
                .map(|p| (p * 100.0).round() / 100.0);

            if quantity.is_none() && unit_price.is_none() && purchase_price.is_none() {
                continue;
            }

            if let Some(pp) = purchase_price {
                // 由明细定位商品，再回写商品进价
                let product_id: Option<i64> = if let Some(iid) = item_id {
                    self.repo.correction_product_id_by_item(iid).await
                } else {
                    product_id
                };
                if let Some(pid) = product_id {
                    if let Some(n) = self.repo.correction_update_product_price(pp, pid).await {
                        updated_count += n as i64;
                    }
                    // 若商品开启了自动售价更新，按新进价联动重算售价
                    recalc_base_price_by_markup(pid, "sales_order_correction", None).await;
                }
            }

            if let Some(item_id) = item_id {
                if let Some(n) = self.repo.correction_update_item_by_id(quantity, unit_price, item_id).await {
                    updated_count += n as i64;
                }
            } else if let Some(product_id) = product_id {
                if let Some(n) = self.repo.correction_update_item_by_product(quantity, unit_price, product_id).await {
                    updated_count += n as i64;
                }
            }
        }

        Ok(SalesOrderOutcome::with_logs(
            format!("成功修正 {} 条记录", updated_count),
            vec![SalesOrderLog::new("sales_order.correction", String::new(),
                format!("批量修正分拣明细（数量/售价/进价），共修正 {} 条记录", updated_count))],
        ))
    }
}

/// 导出销售单列表工作簿（自旧 api_sales_order_export 平移，Vec<SqliteRow> → DTO）
fn build_export_workbook(rows: Vec<SalesOrderExportRow>) -> Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_format = Format::new()
        .set_bold()
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);

    let headers = ["订单ID", "订单号", "订单日期", "采购单位", "总金额", "下浮率(%)", "下浮后合计", "状态", "备注", "商品名称", "下订名称(别称1)", "配单名称(别称2)", "规格", "单位", "预售数量", "数量", "单价", "基本数量", "金额", "商品备注"];
    for (i, &header) in headers.iter().enumerate() {
        worksheet.write_with_format(0, i as u16, header, &header_format)?;
    }

    let mut row_idx = 1;
    for row in rows {
        worksheet.write(row_idx, 0, row.id)?;
        worksheet.write(row_idx, 1, row.order_no.clone())?;
        worksheet.write(row_idx, 2, row.order_date.clone())?;
        worksheet.write(row_idx, 3, row.purchaser_name.clone())?;
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
        worksheet.write(row_idx, 14, row.pre_sale_quantity.unwrap_or(0.0))?;
        worksheet.write(row_idx, 15, row.quantity.unwrap_or(0.0))?;
        worksheet.write(row_idx, 16, row.unit_price.unwrap_or(0.0))?;
        worksheet.write(row_idx, 17, row.base_quantity.unwrap_or(0.0))?;
        worksheet.write(row_idx, 18, row.amount.unwrap_or(0.0))?;
        worksheet.write(row_idx, 19, row.item_remark.clone().unwrap_or_default())?;
        row_idx += 1;
    }

    worksheet.set_column_width(0, 8)?;
    worksheet.set_column_width(1, 18)?;
    worksheet.set_column_width(2, 12)?;
    worksheet.set_column_width(3, 15)?;
    worksheet.set_column_width(4, 10)?;
    worksheet.set_column_width(5, 12)?;
    worksheet.set_column_width(6, 12)?;
    worksheet.set_column_width(7, 8)?;
    worksheet.set_column_width(8, 15)?;
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
