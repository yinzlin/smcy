//! 价格策略时段用例：时段 CRUD / 批量导入（JSON+Excel）/ 生效价查询 / 销售单补价 / 导出
//!
//! 行为与旧 api_price_schedule_* handler 逐字一致：含错误文案、状态码（RawResponse 逐字节）、
//! 吞错分支、eprintln 日志与写后联动（rebuild 时段连续性 → product_price 快照 → 政采标记 → base_price 同步）。

use calamine::Reader;
use chrono::Local;
use crate::dto::PriceScheduleReq;
use domain::UserCtx;
use domain::port::price_schedule::{PriceScheduleInput, PriceScheduleRepo, ScheduleFilter, ScheduleListRow};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook, XlsxError};

use crate::error::UseCaseError;
use crate::port::price_schedule::{PriceScheduleOutcome, PriceScheduleUseCase};

#[derive(Clone)]
pub struct PriceScheduleService<R: PriceScheduleRepo> {
    repo: R,
}

impl<R: PriceScheduleRepo> PriceScheduleService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

/// 价格类型白名单：与前端 PriceTypeSelect 选项保持一致
const PRICE_SCHEDULE_TYPES: &[&str] = &[
    "gov_procurement",
    "supermarket_1",
    "supermarket_2",
    "supermarket_3",
    "ai_realtime",
    "supplier_contract",
    "purchaser_contract",
];

fn is_valid_price_type(price_type: &str) -> bool {
    PRICE_SCHEDULE_TYPES.contains(&price_type)
}

/// 校验 price_schedule 录入请求的字段合法性，返回错误信息字符串（None 表示通过）
fn validate_price_schedule_req(req: &PriceScheduleInput) -> Option<String> {
    if req.product_id <= 0 {
        return Some("product_id 必须大于 0".to_string());
    }
    if !is_valid_price_type(&req.price_type) {
        return Some(format!(
            "price_type 非法：{}（允许：{}）",
            req.price_type,
            PRICE_SCHEDULE_TYPES.join(", ")
        ));
    }
    // 约定价必须指定对应方
    if req.price_type == "supplier_contract" && req.supplier_id.unwrap_or(0) <= 0 {
        return Some("供应商约定采购价必须选择供应商".to_string());
    }
    if req.price_type == "purchaser_contract" && req.purchaser_id.unwrap_or(0) <= 0 {
        return Some("采购单位约定售价必须选择采购单位".to_string());
    }
    // 通用价格类型不应带 supplier_id / purchaser_id
    if req.price_type != "supplier_contract" && req.supplier_id.is_some() {
        return Some("仅供应商约定采购价可指定 supplier_id".to_string());
    }
    if req.price_type != "purchaser_contract" && req.purchaser_id.is_some() {
        return Some("仅采购单位约定售价可指定 purchaser_id".to_string());
    }
    if !common::utils::is_valid_date_str(&req.effective_date) {
        return Some(format!("effective_date 格式错误：{}", req.effective_date));
    }
    if let Some(end) = &req.end_date {
        if !end.is_empty() && !common::utils::is_valid_date_str(end) {
            return Some(format!("end_date 格式错误：{}", end));
        }
        if !end.is_empty() && end.as_str() < req.effective_date.as_str() {
            return Some("end_date 不能早于 effective_date".to_string());
        }
    }
    if req.price < 0.0 {
        return Some("price 不能为负数".to_string());
    }
    None
}

/// 行级数据权限（自 backend middleware::auth::can_access_sales_order 平移，逻辑逐字一致）
fn can_access_sales_order(user: &UserCtx, order_purchaser_id: i64) -> bool {
    match user.role.as_str() {
        "super_admin" | "admin" => true,
        "purchaser" => user.purchaser_id == order_purchaser_id,
        _ => false,
    }
}

/// ScheduleListRow → 列表/单条共用的 JSON 形态（字段顺序逐字保持）
fn list_row_json(r: &ScheduleListRow) -> serde_json::Value {
    serde_json::json!({
        "id": r.id,
        "product_id": r.product_id,
        "product_name": r.product_name.clone().unwrap_or_default(),
        "spec": r.spec.clone().unwrap_or_default(),
        "unit": r.unit.clone().unwrap_or_default(),
        "alias1": r.alias1.clone().unwrap_or_default(),
        "alias2": r.alias2.clone().unwrap_or_default(),
        "price_type": r.price_type,
        "price": r.price,
        "effective_date": r.effective_date,
        "end_date": r.end_date,
        "source": r.source,
        "remark": r.remark,
        "create_at": r.create_at,
        "supplier_id": r.supplier_id,
        "purchaser_id": r.purchaser_id,
        "supplier_name": r.supplier_name.clone().unwrap_or_default(),
        "purchaser_name": r.purchaser_name.clone().unwrap_or_default(),
    })
}

impl<R: PriceScheduleRepo> PriceScheduleService<R> {
    /// 录入/更新/删除/导入后的统一联动：维护时段连续性 → 同步快照 → 政采标记（eprintln 前缀随调用点变化）
    async fn after_write(&self, product_id: i64, price_type: &str, log_prefix: &str) {
        if let Err(e) = self.repo.rebuild_end_dates(product_id, price_type).await {
            eprintln!("[{}] 维护 end_date 失败: {}", log_prefix, e);
        }
        let _ = self.repo.sync_price_snapshot(product_id, price_type).await;
        // 政采平台价有数值时自动标记政采状态
        let _ = self.repo.auto_mark_gov_status(product_id).await;
    }

    /// 批量导入核心写循环（upsert/ignore 两策略；通用价带 NULL 条件），返回 (inserted, skipped, errors, affected_keys)
    async fn import_loop(
        &self,
        validated: &[PriceScheduleInput],
        policy: &str,
        mut errors: Vec<String>,
    ) -> (i64, i64, Vec<String>, std::collections::HashSet<(i64, String)>) {
        let mut inserted = 0i64;
        let mut skipped = 0i64;
        let mut affected_keys: std::collections::HashSet<(i64, String)> = std::collections::HashSet::new();

        for it in validated {
            // 价格统一四舍五入保留两位小数
            let price = (it.price * 100.0).round() / 100.0;
            if policy == "upsert" {
                // 手动 upsert：先查同维度记录是否存在（批量导入仅处理通用价格类型，supplier_id/purchaser_id 均为 NULL）
                let existing_id: Option<i64> = self
                    .repo
                    .find_existing_general(it.product_id, &it.price_type, &it.effective_date)
                    .await
                    .ok()
                    .flatten();
                let result = if let Some(eid) = existing_id {
                    self.repo.update_price_by_id(eid, price, &it.source, &it.remark).await
                } else {
                    self.repo.insert_general(it, price).await
                };
                match result {
                    Ok(_) => {
                        inserted += 1;
                        affected_keys.insert((it.product_id, it.price_type.clone()));
                    }
                    Err(e) => {
                        errors.push(format!("商品 {} / {} / {} : 写入失败 {}", it.product_id, it.price_type, it.effective_date, e));
                    }
                }
            } else {
                // ignore 策略：先查存在
                let exists: i64 = self
                    .repo
                    .count_general(it.product_id, &it.price_type, &it.effective_date)
                    .await
                    .unwrap_or(0);
                if exists > 0 {
                    skipped += 1;
                    continue;
                }
                let result = self.repo.insert_general(it, price).await;
                match result {
                    Ok(_) => {
                        inserted += 1;
                        affected_keys.insert((it.product_id, it.price_type.clone()));
                    }
                    Err(e) => {
                        errors.push(format!("商品 {} / {} / {} : 写入失败 {}", it.product_id, it.price_type, it.effective_date, e));
                    }
                }
            }
        }
        (inserted, skipped, errors, affected_keys)
    }

    /// 批量导入后的统一收尾：rebuild/快照/政采标记 + 受影响商品 base_price 同步
    async fn after_import(&self, affected_keys: &std::collections::HashSet<(i64, String)>, log_prefix: &str) {
        // 重建 end_date + 刷新 product_price 快照
        for (pid, pt) in affected_keys {
            if let Err(e) = self.repo.rebuild_end_dates(*pid, pt).await {
                eprintln!("[{}] 维护 end_date 失败 pid={} pt={}: {}", log_prefix, pid, pt, e);
            }
            let _ = self.repo.sync_price_snapshot(*pid, pt).await;
            // 政采平台价有数值时自动标记政采状态
            let _ = self.repo.auto_mark_gov_status(*pid).await;
        }

        // 同步受影响商品的基础单价（售价），保证商品列表售价 = 编辑页计算售价（政采价优先）
        let affected_pids: std::collections::HashSet<i64> = affected_keys.iter().map(|(pid, _)| *pid).collect();
        for pid in &affected_pids {
            let _ = self.repo.sync_base_price(*pid).await;
        }
    }
}

impl<R: PriceScheduleRepo> PriceScheduleUseCase for PriceScheduleService<R> {
    async fn create(&self, req: PriceScheduleReq) -> Result<PriceScheduleOutcome, UseCaseError> {
        // HTTP 契约 → domain 端口输入类型（字段同名同序，下游字段访问不变）
        let req = PriceScheduleInput::from(req);
        // 价格策略是独立的价格时段数据，与商品基础信息的审核状态无关，
        // 因此不做「商品已审核则禁止修改」的锁定校验（市场价格随时变动，需随时可录入）
        if let Some(err) = validate_price_schedule_req(&req) {
            // 行为保持：create 校验失败返回 400 纯文本
            return Err(UseCaseError::RawResponse(400, err));
        }
        // 价格统一四舍五入保留两位小数
        let price = (req.price * 100.0).round() / 100.0;
        match self.repo.create(&req, price).await {
            Ok(id) => {
                self.after_write(req.product_id, &req.price_type, "price_schedule").await;
                let detail = format!(
                    "录入价格策略：product_id={}, price_type={}, price={}, effective_date={}",
                    req.product_id, req.price_type, req.price, req.effective_date
                );
                Ok(PriceScheduleOutcome::with_log(
                    format!("{{\"id\":{}, \"success\":true}}", id),
                    id.to_string(),
                    detail,
                ))
            }
            Err(e) => {
                let msg = e.to_string();
                // UNIQUE 冲突：返回友好提示
                if msg.contains("UNIQUE") {
                    return Err(UseCaseError::RawResponse(409, format!(
                        "{{\"success\":false, \"message\":\"已存在相同 (商品, 价格类型, 生效日期={}) 的策略，请改用编辑或先删除\"}}",
                        req.effective_date
                    )));
                }
                eprintln!("[price_schedule] 录入失败: {}", msg);
                Err(UseCaseError::RawResponse(500, format!("{{\"success\":false, \"message\":\"录入失败：{}\"}}", msg)))
            }
        }
    }

    async fn update(&self, req: PriceScheduleReq) -> Result<PriceScheduleOutcome, UseCaseError> {
        // HTTP 契约 → domain 端口输入类型（字段同名同序，下游字段访问不变）
        let req = PriceScheduleInput::from(req);
        let id = match req.id {
            Some(i) if i > 0 => i,
            _ => return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"id 必填\"}".to_string())),
        };
        if req.price < 0.0 {
            return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"price 不能为负数\"}".to_string()));
        }
        if !is_valid_price_type(&req.price_type) {
            return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"price_type 非法\"}".to_string()));
        }
        if req.effective_date.trim().is_empty() {
            return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"effective_date 必填\"}".to_string()));
        }
        // 先取出原记录，用于比较 (product_id, price_type) 是否有变化 → 需要重算 end_date 连续性
        let old = self.repo.get_product_type(id).await.ok().flatten();
        let (old_product_id, old_price_type) = match old {
            Some(v) => v,
            None => {
                return Err(UseCaseError::RawResponse(404, "{\"success\":false, \"message\":\"记录不存在\"}".to_string()));
            }
        };

        match self.repo.update(id, &req).await {
            Ok(n) if n > 0 => {
                // 维护变更涉及的 (product, price_type) 端点的 end_date 连续性
                // 旧组合 + 新组合各算一次（避免某条孤立时段残留）
                let mut touched: Vec<(i64, String)> = Vec::new();
                touched.push((old_product_id, old_price_type.clone()));
                if old_price_type != req.price_type || old_product_id != req.product_id {
                    touched.push((req.product_id, req.price_type.clone()));
                }
                for (pid, pt) in &touched {
                    if let Err(e) = self.repo.rebuild_end_dates(*pid, pt).await {
                        eprintln!("[price_schedule] 维护 end_date 失败: {}", e);
                    }
                }
                // 同步 product_price 快照（旧组合 + 新组合）
                for (pid, pt) in &touched {
                    let _ = self.repo.sync_price_snapshot(*pid, pt).await;
                    // 政采平台价有数值时自动标记政采状态
                    let _ = self.repo.auto_mark_gov_status(*pid).await;
                }

                let detail = format!(
                    "product_id={}, price_type={}({}->{}), price={}, effective_date={}",
                    req.product_id, req.price_type, old_price_type, req.price_type, req.price, req.effective_date
                );
                Ok(PriceScheduleOutcome::with_log(
                    "{\"success\":true}".to_string(),
                    id.to_string(),
                    detail,
                ))
            }
            Ok(_) => Err(UseCaseError::RawResponse(404, "{\"success\":false, \"message\":\"价格策略不存在\"}".to_string())),
            Err(e) => {
                eprintln!("[price_schedule] 更新失败: {}", e);
                Err(UseCaseError::RawResponse(500, format!("{{\"success\":false, \"message\":\"{}\"}}", e)))
            }
        }
    }

    async fn delete(&self, id: i64) -> Result<PriceScheduleOutcome, UseCaseError> {
        // 先读取 (product_id, price_type) 后续重建 end_date
        let row = self.repo.get_product_type(id).await.ok().flatten();
        match self.repo.delete(id).await {
            Ok(n) if n > 0 => {
                if let Some((pid, pt)) = row {
                    self.after_write(pid, &pt, "price_schedule").await;
                }
                Ok(PriceScheduleOutcome::with_log(
                    "{\"success\":true}".to_string(),
                    id.to_string(),
                    String::new(),
                ))
            }
            Ok(_) => Err(UseCaseError::RawResponse(404, "{\"success\":false, \"message\":\"价格策略不存在\"}".to_string())),
            Err(e) => {
                eprintln!("[price_schedule] 删除失败: {}", e);
                Err(UseCaseError::RawResponse(500, format!("{{\"success\":false, \"message\":\"{}\"}}", e)))
            }
        }
    }

    async fn list(&self, filter: ScheduleFilter, page: i64, page_size: i64) -> String {
        let offset = (page - 1) * page_size;
        // 行为保持：总数/列表查询失败吞为 0/空
        let total: i64 = self.repo.count_list(&filter).await.unwrap_or(0);
        let rows = self.repo.list(&filter, page_size, offset).await.unwrap_or_default();

        let items: Vec<serde_json::Value> = rows.iter().map(list_row_json).collect();
        let result = serde_json::json!({
            "data": items,
            "page": page,
            "page_size": page_size,
            "total": total,
            "total_pages": if page_size > 0 { (total + page_size - 1) / page_size } else { 0 },
        });
        serde_json::to_string(&result).unwrap()
    }

    async fn batch_import(&self, items: Vec<PriceScheduleReq>, conflict_policy: Option<String>) -> PriceScheduleOutcome {
        // HTTP 契约 → domain 端口输入类型（字段同名同序，下游字段访问不变）
        let items: Vec<PriceScheduleInput> = items.into_iter().map(PriceScheduleInput::from).collect();
        let policy = conflict_policy.unwrap_or_else(|| "upsert".to_string());

        // 预校验：相同 (product, price_type, effective_date) 在本次请求内去重
        let mut seen: std::collections::HashSet<(i64, String, String)> = std::collections::HashSet::new();
        let mut validated: Vec<PriceScheduleInput> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        for it in items {
            if let Some(e) = validate_price_schedule_req(&it) {
                errors.push(format!("商品 {} / {} / {} : {}", it.product_id, it.price_type, it.effective_date, e));
                continue;
            }
            let key = (it.product_id, it.price_type.clone(), it.effective_date.clone());
            if !seen.insert(key) {
                errors.push(format!("商品 {} / {} / {} : 同一批次内重复", it.product_id, it.price_type, it.effective_date));
                continue;
            }
            validated.push(it);
        }

        let (inserted, skipped, errors, affected_keys) = self.import_loop(&validated, &policy, errors).await;
        self.after_import(&affected_keys, "price_schedule batch").await;

        let detail = format!(
            "批量导入价格策略：inserted={}, skipped={}, errors={}",
            inserted, skipped, errors.len()
        );
        let result = serde_json::json!({
            "success": true,
            "inserted": inserted,
            "skipped": skipped,
            "errors": errors,
            "total": validated.len(),
        });
        PriceScheduleOutcome::with_log(
            serde_json::to_string(&result).unwrap(),
            "0".to_string(),
            detail,
        )
    }

    async fn history(&self, product_id: i64, price_type: String) -> String {
        if product_id <= 0 || price_type.is_empty() {
            let empty = serde_json::json!({ "data": [] });
            return serde_json::to_string(&empty).unwrap();
        }
        // 行为保持：查询失败吞为空
        let rows = self.repo.history(product_id, &price_type).await.unwrap_or_default();
        let items: Vec<serde_json::Value> = rows.iter().map(|r| {
            serde_json::json!({
                "id": r.id,
                "price": r.price,
                "effective_date": r.effective_date,
                "end_date": r.end_date,
                "source": r.source,
                "remark": r.remark,
                "create_at": r.create_at,
            })
        }).collect();
        let result = serde_json::json!({ "data": items });
        serde_json::to_string(&result).unwrap()
    }

    async fn get(&self, id: i64) -> Result<String, UseCaseError> {
        if id <= 0 {
            return Err(UseCaseError::RawResponse(400, "{\"success\":false,\"message\":\"id 必填\"}".to_string()));
        }
        // 行为保持：查询失败吞为 None
        let row_opt = self.repo.get_by_id(id).await.ok().flatten();
        match row_opt {
            Some(r) => {
                let item = list_row_json(&r);
                Ok(serde_json::json!({"success": true, "data": item}).to_string())
            }
            None => Ok("{\"success\":false,\"message\":\"记录不存在\"}".to_string()),
        }
    }

    async fn diagnose(&self, product_id: i64) -> Result<String, UseCaseError> {
        if product_id <= 0 {
            return Err(UseCaseError::RawResponse(400, "{\"success\":false,\"message\":\"product_id 必填\"}".to_string()));
        }
        // 行为保持：查询失败吞为默认值
        let product_name: String = self.repo.product_name(product_id).await.ok().flatten().unwrap_or_default();
        let rows = self.repo.diagnose(product_id).await.unwrap_or_default();
        let mut types: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        for r in rows {
            let item = serde_json::json!({
                "id": r.id,
                "price": r.price,
                "effective_date": r.effective_date,
                "end_date": r.end_date,
                "source": r.source,
                "remark": r.remark,
                "supplier_id": r.supplier_id,
                "purchaser_id": r.purchaser_id,
            });
            types
                .entry(r.price_type)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()))
                .as_array_mut()
                .map(|a| a.push(item));
        }
        let result = serde_json::json!({
            "success": true,
            "product_id": product_id,
            "product_name": product_name,
            "types": serde_json::Value::Object(types),
        });
        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn lookup_batch(&self, date: String, product_ids: Vec<i64>) -> Result<String, UseCaseError> {
        if product_ids.is_empty() {
            return Ok(serde_json::json!({"success": true, "date": date, "prices": {}}).to_string());
        }
        let date = date.trim().to_string();
        if date.is_empty() {
            return Err(UseCaseError::RawResponse(400, "{\"success\":false,\"message\":\"date 不能为空\"}".to_string()));
        }
        let types = ["gov_procurement", "supermarket_1", "supermarket_2", "supermarket_3", "ai_realtime"];
        // 用 serde_json::Map 显式以字符串为 key，避免 i64 数字 key 在不同 serde_json 版本下被当成 array index
        let mut out: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        for &pid in &product_ids {
            let mut entry = serde_json::Map::new();
            for t in types {
                // 行为保持：查询失败吞为空 map
                let m = self.repo.batch_lookup_effective(&[pid], t, &date).await.unwrap_or_default();
                let v = m.get(&pid).copied().unwrap_or(0.0);
                entry.insert(t.to_string(), serde_json::json!(v));
            }
            out.insert(pid.to_string(), serde_json::Value::Object(entry));
        }
        let result = serde_json::json!({
            "success": true,
            "date": date,
            "prices": serde_json::Value::Object(out),
        });
        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn lookup(&self, product_id: i64, price_type: String, query_date: Option<String>, party_id: i64) -> String {
        let query_date = query_date.unwrap_or_else(|| {
            Local::now().format("%Y-%m-%d").to_string()
        });
        if product_id <= 0 || price_type.is_empty() {
            return serde_json::to_string(&serde_json::json!({
                "matched": false, "price": 0.0, "effective_date": null
            })).unwrap();
        }

        // 约定价走独立查询函数
        let found = if price_type == "supplier_contract" || price_type == "purchaser_contract" {
            if party_id <= 0 {
                return serde_json::to_string(&serde_json::json!({
                    "matched": false, "price": 0.0, "effective_date": null,
                    "message": if price_type == "supplier_contract" { "缺少 supplier_id" } else { "缺少 purchaser_id" }
                })).unwrap();
            }
            self.repo.lookup_contract(product_id, &price_type, party_id, &query_date).await.ok().flatten()
        } else {
            self.repo.lookup_effective(product_id, &price_type, &query_date).await.ok().flatten()
        };

        match found {
            Some((price, schedule_id, eff_date)) => {
                let result = serde_json::json!({
                    "matched": price > 0.0,
                    "price": price,
                    "schedule_id": schedule_id,
                    "effective_date": eff_date,
                    "query_date": query_date,
                });
                serde_json::to_string(&result).unwrap()
            }
            None => {
                let result = serde_json::json!({
                    "matched": false, "price": 0.0, "effective_date": null, "query_date": query_date
                });
                serde_json::to_string(&result).unwrap()
            }
        }
    }

    async fn export_template(&self) -> Result<Vec<u8>, UseCaseError> {
        let result: Result<Vec<u8>, XlsxError> = (|| {
            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet();

            let header_format = Format::new()
                .set_bold()
                .set_font_size(11)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin)
                .set_background_color("#E5E7EB");
            let note_format = Format::new()
                .set_font_size(10)
                .set_font_color("#6B7280");

            // 顶部说明
            worksheet.merge_range(0, 0, 0, 8,
                "价格策略导入模板 - 政采价/超市比价按生效日期段录入", &note_format)?;
            worksheet.merge_range(1, 0, 1, 8,
                "说明：price_type 取值 gov_procurement / supermarket_1 / supermarket_2 / supermarket_3 / ai_realtime；effective_date 必填 YYYY-MM-DD；product_name 必须与商品表完全一致", &note_format)?;

            let headers = ["商品名称*", "规格", "别名1", "别名2", "单位", "价格类型*", "生效日期*(YYYY-MM-DD)", "价格*", "备注"];
            for (i, h) in headers.iter().enumerate() {
                worksheet.write_with_format(3, i as u16, *h, &header_format)?;
            }
            worksheet.set_column_width(0, 20.0)?;
            worksheet.set_column_width(1, 15.0)?;
            worksheet.set_column_width(2, 15.0)?;
            worksheet.set_column_width(3, 15.0)?;
            worksheet.set_column_width(4, 8.0)?;
            worksheet.set_column_width(5, 18.0)?;
            worksheet.set_column_width(6, 22.0)?;
            worksheet.set_column_width(7, 10.0)?;
            worksheet.set_column_width(8, 25.0)?;

            // 示例行：方便用户对格式
            let examples: &[(&str, &str, &str, &str, &str, &str, &str, &str, &str)] = &[
                ("示例:白萝卜", "", "", "", "斤", "gov_procurement", "2026-01-01", "2.50", "示例：可删除本行后填入实际数据"),
                ("示例:白萝卜", "", "", "", "斤", "gov_procurement", "2026-08-15", "3.20", "示例：价调整时段2"),
            ];
            for (i, row) in examples.iter().enumerate() {
                // row 是 &(&str,...) 9 元组；解构成单个 val 数组
                let vals: [&str; 9] = [
                    row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8,
                ];
                for (j, val) in vals.iter().enumerate() {
                    worksheet.write_string((4 + i) as u32, j as u16, *val)?;
                }
            }

            workbook.save_to_buffer()
        })();

        result.map_err(|e| UseCaseError::RawResponse(500, format!("生成失败: {}", e)))
    }

    async fn export(&self, filter: ScheduleFilter) -> Result<Vec<u8>, UseCaseError> {
        // 行为保持：查询失败吞为空
        let rows = self.repo.export_list(&filter).await.unwrap_or_default();

        let result: Result<Vec<u8>, XlsxError> = (|| {
            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet();
            let header_format = Format::new()
                .set_bold()
                .set_font_size(10)
                .set_align(FormatAlign::Center)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);
            let cell_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Left)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin);
            let price_format = Format::new()
                .set_font_size(10)
                .set_align(FormatAlign::Right)
                .set_align(FormatAlign::VerticalCenter)
                .set_border(FormatBorder::Thin)
                .set_num_format("0.00");

            let headers = ["ID", "商品ID", "商品名称", "规格", "别名1", "别名2", "单位", "价格类型", "生效日期", "失效日期", "价格", "来源", "备注"];
            for (i, h) in headers.iter().enumerate() {
                worksheet.write_with_format(0, i as u16, *h, &header_format)?;
            }
            for (idx, r) in rows.iter().enumerate() {
                let row = (idx + 1) as u32;
                worksheet.write_with_format(row, 0, r.id, &cell_format)?;
                worksheet.write_with_format(row, 1, r.product_id, &cell_format)?;
                worksheet.write_with_format(row, 2, r.product_name.clone().unwrap_or_default(), &cell_format)?;
                worksheet.write_with_format(row, 3, r.spec.clone().unwrap_or_default(), &cell_format)?;
                worksheet.write_with_format(row, 4, r.alias1.clone().unwrap_or_default(), &cell_format)?;
                worksheet.write_with_format(row, 5, r.alias2.clone().unwrap_or_default(), &cell_format)?;
                worksheet.write_with_format(row, 6, r.unit.clone().unwrap_or_default(), &cell_format)?;
                worksheet.write_with_format(row, 7, r.price_type.as_str(), &cell_format)?;
                worksheet.write_with_format(row, 8, r.effective_date.as_str(), &cell_format)?;
                worksheet.write_with_format(row, 9, r.end_date.clone().unwrap_or_default(), &cell_format)?;
                worksheet.write_with_format(row, 10, r.price, &price_format)?;
                worksheet.write_with_format(row, 11, r.source.clone().unwrap_or_default(), &cell_format)?;
                worksheet.write_with_format(row, 12, r.remark.clone().unwrap_or_default(), &cell_format)?;
            }
            for c in 0..13u16 {
                worksheet.set_column_width(c, 14.0)?;
            }
            worksheet.set_column_width(2, 22.0)?;
            workbook.save_to_buffer()
        })();

        result.map_err(|e| UseCaseError::RawResponse(500, format!("生成失败: {}", e)))
    }

    async fn backfill_sales_order(&self, order_id: i64, force: bool, price_type: String, ctx: &UserCtx) -> Result<PriceScheduleOutcome, UseCaseError> {
        if order_id <= 0 {
            return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"order_id 必填\"}".to_string()));
        }
        if !is_valid_price_type(&price_type) {
            return Err(UseCaseError::RawResponse(400, format!("{{\"success\":false, \"message\":\"price_type 非法：{}\"}}", price_type)));
        }

        // 行级数据权限
        let purchaser_id: i64 = self.repo.sales_order_purchaser_id(order_id).await.ok().flatten().unwrap_or(0);
        if purchaser_id == 0 {
            return Err(UseCaseError::RawResponse(404, "{\"success\":false, \"message\":\"销售单不存在\"}".to_string()));
        }
        if !can_access_sales_order(ctx, purchaser_id) {
            return Err(UseCaseError::RawResponse(403, "{\"success\":false, \"message\":\"您没有权限操作此销售单\"}".to_string()));
        }
        let order_date: String = self.repo.sales_order_order_date(order_id).await.unwrap_or_default();
        // order_date 取前 10 位 'YYYY-MM-DD'
        let order_date_short = if order_date.len() >= 10 { order_date[..10].to_string() } else { order_date.clone() };

        // 拉取该订单所有明细（行为保持：查询失败吞为空）
        let items = self.repo.sales_order_items(order_id).await.unwrap_or_default();

        if items.is_empty() {
            // 行为保持：订单无明细时早退且不记操作日志
            return Ok(PriceScheduleOutcome::without_log(
                "{\"success\":true, \"updated\":0, \"message\":\"订单无明细\"}".to_string(),
            ));
        }

        // 收集 product_id
        let product_ids: Vec<i64> = items.iter().map(|r| r.product_id).filter(|p| *p > 0).collect();
        let price_map = self.repo.batch_lookup_effective(&product_ids, &price_type, &order_date_short).await.unwrap_or_default();

        let mut updated = 0i64;
        let mut kept = 0i64;
        let mut missing: Vec<String> = Vec::new();

        for r in &items {
            let item_id = r.id;
            let product_id = r.product_id;
            let product_name = &r.product_name;
            let unit = r.unit.clone().unwrap_or_default();
            let qty: f64 = r.quantity;
            let cur_price: f64 = r.unit_price;

            // force=true 时所有行都重算；否则仅 unit_price <= 0 的行重算
            let need_update = force || cur_price <= 0.0;
            if !need_update {
                kept += 1;
                continue;
            }

            let new_price = price_map.get(&product_id).copied().unwrap_or(0.0);
            if new_price <= 0.0 {
                missing.push(format!("{}({})", product_name, unit));
                continue;
            }
            let new_amount = new_price * qty;
            let _ = self.repo.update_sales_item_price(item_id, new_price, new_amount).await;
            updated += 1;
        }

        // 重算主表 total_amount / amount_reduction / final_amount（按 discount_rate）
        let discount_rate: f64 = self.repo.sales_order_discount_rate(order_id).await.unwrap_or(0.0);
        let total_amount: f64 = self.repo.sales_order_total_amount(order_id).await.unwrap_or(0.0);
        let total_amount = ((total_amount) * 100.0).round() / 100.0;
        let amount_reduction = ((total_amount * (discount_rate / 100.0)) * 100.0).round() / 100.0;
        let final_amount = ((total_amount - amount_reduction) * 100.0).round() / 100.0;
        let _ = self.repo.update_sales_order_amounts(order_id, total_amount, amount_reduction, final_amount).await;

        let detail = format!(
            "补单按 order_date={} 应用 {} 价：updated={}, kept={}, missing={}",
            order_date_short, price_type, updated, kept, missing.len()
        );

        let result = serde_json::json!({
            "success": true,
            "order_id": order_id,
            "order_date": order_date_short,
            "price_type": price_type,
            "updated": updated,
            "kept": kept,
            "missing": missing,
        });
        Ok(PriceScheduleOutcome::with_log(
            serde_json::to_string(&result).unwrap(),
            order_id.to_string(),
            detail,
        ))
    }

    async fn batch_import_excel(&self, bytes: Vec<u8>) -> Result<PriceScheduleOutcome, UseCaseError> {
        // 解析 xlsx
        let cursor = std::io::Cursor::new(bytes);
        let mut workbook = match calamine::open_workbook_auto_from_rs(cursor) {
            Ok(wb) => wb,
            Err(e) => return Err(UseCaseError::RawResponse(400, format!("打开 Excel 失败: {}", e))),
        };
        let sheet_names = workbook.sheet_names();
        if sheet_names.is_empty() {
            return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"Excel 中无 sheet\"}".to_string()));
        }
        let range = match workbook.worksheet_range(&sheet_names[0]) {
            Ok(r) => r,
            Err(e) => return Err(UseCaseError::RawResponse(400, format!("读取 sheet 失败: {}", e))),
        };

        // 期望表头：商品名称*、规格、别名1、别名2、单位、价格类型*、生效日期*、价格*、备注
        // 模板顶部有说明行（标题/说明在前几行，表头位于第 4 行），故在前 10 行内动态定位表头行，
        // 兼容直接使用下载模板导入（此前只读第 1 行导致报"缺少「商品名称」列"）
        let rows: Vec<Vec<calamine::Data>> = range.rows().map(|r| r.to_vec()).collect();
        let find_col_exact = |row: &[calamine::Data], name: &str| -> Option<usize> {
            for (i, cell) in row.iter().enumerate() {
                let s = cell_to_string(cell);
                if s.replace('*', "").trim() == name {
                    return Some(i);
                }
            }
            None
        };
        let find_col_prefix = |row: &[calamine::Data], name: &str| -> Option<usize> {
            for (i, cell) in row.iter().enumerate() {
                let s = cell_to_string(cell);
                if s.replace('*', "").trim().starts_with(name) {
                    return Some(i);
                }
            }
            None
        };
        let (header_idx, header) = match rows.iter().enumerate().take(10).find(|(_, r)| find_col_exact(r, "商品名称").is_some()) {
            Some((i, r)) => (i, r),
            None => return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"缺少「商品名称」列\"}".to_string())),
        };
        // 先精确匹配，再前缀匹配（兼容"生效日期*(YYYY-MM-DD)"等带后缀表头）
        let col_idx_of = |name: &str| -> Option<usize> {
            find_col_exact(header, name).or_else(|| find_col_prefix(header, name))
        };
        let idx_name = match col_idx_of("商品名称") {
            Some(i) => i,
            None => return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"缺少「商品名称」列\"}".to_string())),
        };
        let idx_spec = col_idx_of("规格");
        let idx_alias1 = col_idx_of("别名1");
        let idx_alias2 = col_idx_of("别名2");
        let idx_unit = col_idx_of("单位");
        let idx_pt = match col_idx_of("价格类型") {
            Some(i) => i,
            None => return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"缺少「价格类型」列\"}".to_string())),
        };
        let idx_eff = match col_idx_of("生效日期") {
            Some(i) => i,
            None => return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"缺少「生效日期」列\"}".to_string())),
        };
        let idx_price = match col_idx_of("价格") {
            Some(i) => i,
            None => return Err(UseCaseError::RawResponse(400, "{\"success\":false, \"message\":\"缺少「价格」列\"}".to_string())),
        };
        let idx_remark = col_idx_of("备注");

        // 解析行：从表头行的下一行开始（表头可能位于第 4 行而非第 1 行）
        let mut items: Vec<PriceScheduleInput> = Vec::new();
        let mut parse_errors: Vec<String> = Vec::new();
        for (row_idx, row) in rows.iter().enumerate().skip(header_idx + 1) {
            // 跳过全空行
            if row.iter().all(|c| cell_to_string(c).trim().is_empty()) {
                continue;
            }
            let get = |i: Option<usize>| -> String {
                i.and_then(|j| row.get(j)).map(cell_to_string).unwrap_or_default().trim().to_string()
            };
            let product_name = get(Some(idx_name));
            if product_name.is_empty() || product_name.starts_with("示例") {
                continue;
            }
            let price_type = get(Some(idx_pt));
            let eff_date_raw = get(Some(idx_eff));
            let price_str = get(Some(idx_price));
            let remark = get(idx_remark);
            let spec = get(idx_spec);
            let alias1 = get(idx_alias1);
            let alias2 = get(idx_alias2);
            let unit = get(idx_unit);

            // 标准化生效日期：支持 "2026-01-01" / "2026/01/01" / Excel 序列号
            let eff_date = normalize_date_string(&eff_date_raw);
            // 价格：支持 "¥2.50" / "2.5" / "2,500.00" 等
            let price_clean: String = price_str.chars().filter(|c| *c != '¥' && *c != ',' && *c != '￥' && *c != ' ').collect();
            let price: f64 = price_clean.parse().unwrap_or(0.0);

            // 用 product_name + spec + alias1 + alias2 匹配商品（四步回退；查询失败吞为 None）
            let matched_pid: Option<i64> = if !product_name.is_empty() {
                self.repo.find_product_id(&product_name, &spec, &alias1, &alias2).await.ok().flatten()
            } else {
                None
            };
            let pid = match matched_pid {
                Some(p) => p,
                None => {
                    parse_errors.push(format!("第 {} 行：商品「{}」未找到", row_idx + 1, product_name));
                    continue;
                }
            };

            items.push(PriceScheduleInput {
                id: None,
                product_id: pid,
                price_type,
                price,
                effective_date: eff_date,
                end_date: None,
                source: Some("excel_import".to_string()),
                remark: if remark.is_empty() { None } else { Some(remark) },
                supplier_id: None,
                purchaser_id: None,
            });
            // 抑制未用 unit 警告：单位列仅用于辅助展示，不写入 schedule
            let _ = unit;
        }

        if items.is_empty() && !parse_errors.is_empty() {
            let result = serde_json::json!({
                "success": false,
                "inserted": 0,
                "skipped": 0,
                "errors": parse_errors,
                "total": 0,
            });
            // 行为保持：全部解析失败时早退且不记操作日志
            return Ok(PriceScheduleOutcome::without_log(serde_json::to_string(&result).unwrap()));
        }

        // 复用 batch_import 的核心逻辑
        let mut seen: std::collections::HashSet<(i64, String, String)> = std::collections::HashSet::new();
        let mut validated: Vec<PriceScheduleInput> = Vec::new();
        let mut errors = parse_errors;
        for it in items {
            if let Some(e) = validate_price_schedule_req(&it) {
                errors.push(format!("商品 {} / {} / {} : {}", it.product_id, it.price_type, it.effective_date, e));
                continue;
            }
            let key = (it.product_id, it.price_type.clone(), it.effective_date.clone());
            if !seen.insert(key) {
                errors.push(format!("商品 {} / {} / {} : 同一批次内重复", it.product_id, it.price_type, it.effective_date));
                continue;
            }
            validated.push(it);
        }

        let mut inserted = 0i64;
        let mut skipped = 0i64;
        let mut affected_keys: std::collections::HashSet<(i64, String)> = std::collections::HashSet::new();

        for it in &validated {
            // 价格统一四舍五入保留两位小数
            let price = (it.price * 100.0).round() / 100.0;
            let exists: i64 = self
                .repo
                .count_by_key(it.product_id, &it.price_type, &it.effective_date)
                .await
                .unwrap_or(0);
            if exists > 0 {
                // 存在则更新 price / source / remark（避免与手工录入冲突，采用 upsert 语义）
                match self.repo.update_price_by_key(it.product_id, &it.price_type, &it.effective_date, price, &it.source, &it.remark).await {
                    Ok(n) if n > 0 => {
                        inserted += 1;
                        affected_keys.insert((it.product_id, it.price_type.clone()));
                    }
                    Ok(_) => { skipped += 1; }
                    Err(e) => errors.push(format!("商品 {} / {} / {} : 更新失败 {}", it.product_id, it.price_type, it.effective_date, e)),
                }
            } else {
                match self.repo.insert_excel(it, price).await {
                    Ok(_) => {
                        inserted += 1;
                        affected_keys.insert((it.product_id, it.price_type.clone()));
                    }
                    Err(e) => errors.push(format!("商品 {} / {} / {} : 写入失败 {}", it.product_id, it.price_type, it.effective_date, e)),
                }
            }
        }

        self.after_import(&affected_keys, "price_schedule excel import").await;

        let detail = format!("Excel 批量导入价格策略：inserted={}, skipped={}, errors={}", inserted, skipped, errors.len());
        let result = serde_json::json!({
            "success": true,
            "inserted": inserted,
            "skipped": skipped,
            "errors": errors,
            "total": validated.len(),
        });
        Ok(PriceScheduleOutcome::with_log(
            serde_json::to_string(&result).unwrap(),
            "0".to_string(),
            detail,
        ))
    }
}

/// 将 calamine Data 单元格转字符串
fn cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::String(s) => s.clone(),
        calamine::Data::Float(f) => {
            // 可能是 Excel 日期序列号（44000+）
            if f.fract() == 0.0 && *f > 30000.0 && *f < 80000.0 {
                // 转 YYYY-MM-DD
                let serial = *f as i64;
                if let Some(dt) = excel_serial_to_date(serial) {
                    return dt;
                }
            }
            f.to_string()
        }
        calamine::Data::Int(i) => i.to_string(),
        calamine::Data::Bool(b) => b.to_string(),
        calamine::Data::DateTime(dt) => {
            // Excel 日期单元格（WPS/Excel 会把 2026-9-1 自动转成真日期）：按序列号转 YYYY-MM-DD，忽略时间部分
            let f = dt.as_f64();
            let serial = f.floor() as i64;
            if (30000..80000).contains(&serial) {
                if let Some(d) = excel_serial_to_date(serial) {
                    return d;
                }
            }
            dt.to_string()
        }
        calamine::Data::DateTimeIso(s) => s.clone(),
        calamine::Data::DurationIso(s) => s.clone(),
        calamine::Data::Error(e) => format!("{:?}", e),
        calamine::Data::Empty => String::new(),
    }
}

/// Excel 日期序列号 → 'YYYY-MM-DD'
/// 序列号 1 = 1900-01-01（注意 Excel 1900 闰年 bug，故 60 以下要 +1 修正）
fn excel_serial_to_date(serial: i64) -> Option<String> {
    // 简化算法：使用 chrono 偏移
    use chrono::{Duration, NaiveDate};
    let base = NaiveDate::from_ymd_opt(1899, 12, 30)?;
    let dt = base + Duration::days(serial);
    Some(dt.format("%Y-%m-%d").to_string())
}

/// 标准化日期字符串：支持 "2026-01-01" / "2026/01/01" / Excel 序列号
fn normalize_date_string(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() { return String::new(); }
    if common::utils::is_valid_date_str(s) { return s.to_string(); }
    // 替换 / 为 -
    let normalized = s.replace('/', "-");
    if common::utils::is_valid_date_str(&normalized) { return normalized; }
    // 尝试纯数字序列号
    if let Ok(serial) = s.parse::<i64>() {
        if let Some(d) = excel_serial_to_date(serial) {
            return d;
        }
    }
    // 尝试 "2026-01-01 00:00:00" 形式
    if let Some(idx) = s.find(' ') {
        let part = &s[..idx];
        if common::utils::is_valid_date_str(part) { return part.to_string(); }
    }
    s.to_string()
}
