//! 销售订单用例：创建 / 列表 / by_purchaser / 详情 / 乐观锁更新 / 调价 / 删除 / 结算 / 状态流转 / 审核 / 反审核 / 导出 / 导入
//!
//! 行为与旧 api_sales_order_* handler 逐字一致：含错误文案、状态码、吞错分支、
//! 价格策略时段补价（政采价优先，否则商超1/2/3/AI 非零平均价）与操作日志文案。

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use domain::error::DomainError;
use domain::port::sales_order::*;
use domain::{SalesOrderItemReq, SalesOrderReq, UserCtx};
use rust_xlsxwriter::{Format, FormatAlign, Workbook, XlsxError};

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
