//! 商品用例：列表组装 / 详情 / 搜索 / CRUD / 状态切换 / 导出导入 / 单位与价格快照

use chrono::Local;
use common::utils::{round2, sanitize_filename_prefix};
use domain::error::DomainError;
use domain::port::price_schedule::PriceScheduleRepo;
use domain::port::product::{
    DeleteCascadeError, ImportInsertOutcome, ImportProduct, NewProduct, NewProductUnit, PriceSnapshot,
    ProductExportRow, ProductImageStore, ProductListFilter, ProductRepo, RemoteImageStore, UpdateProduct,
};

use crate::error::UseCaseError;
use crate::port::product::ProductUseCase;

/// 提取存储错误上下文（文本契约 500 响应体需透传旧文案）
fn storage_msg(e: &DomainError) -> String {
    match e {
        DomainError::Storage(s) => s.clone(),
        _ => e.to_string(),
    }
}

#[derive(Clone)]
pub struct ProductService<R: ProductRepo, S: RemoteImageStore + ProductImageStore, P: PriceScheduleRepo> {
    repo: R,
    image: S,
    price: P,
}

impl<R: ProductRepo, S: RemoteImageStore + ProductImageStore, P: PriceScheduleRepo> ProductService<R, S, P> {
    pub fn new(repo: R, image: S, price: P) -> Self {
        Self { repo, image, price }
    }

    /// 行为保持：旧 check_basic_not_confirmed 的吞错与文案逻辑
    async fn ensure_not_confirmed(&self, id: i64) -> Result<(), UseCaseError> {
        if id <= 0 {
            return Err(UseCaseError::BadRequest("缺少记录ID".to_string()));
        }
        let status = match self.repo.get_audit_status(id).await {
            Ok(s) => s,
            Err(_) => None, // 行为保持：查询失败视为记录不存在
        };
        match status.as_deref() {
            Some("confirmed") => Err(UseCaseError::BadRequest(
                "该记录已审核，如需修改/删除请先反审核".to_string(),
            )),
            Some(_) => Ok(()),
            None => Err(UseCaseError::NotFound("记录不存在".to_string())),
        }
    }

    /// 商超1/2/3/AI 非零价的平均（四舍五入保留 2 位，与前端 calcSellingPrice 一致）
    fn calc_selling_price(gov_price: Option<f64>, other_prices: &[f64], base_price: f64) -> f64 {
        if let Some(gp) = gov_price {
            if gp > 0.0 {
                return gp;
            }
            if !other_prices.is_empty() {
                return (other_prices.iter().sum::<f64>() / other_prices.len() as f64 * 100.0).round() / 100.0;
            }
            return base_price;
        }
        if !other_prices.is_empty() {
            (other_prices.iter().sum::<f64>() / other_prices.len() as f64 * 100.0).round() / 100.0
        } else {
            base_price
        }
    }
}

impl<R: ProductRepo, S: RemoteImageStore + ProductImageStore, P: PriceScheduleRepo> ProductUseCase for ProductService<R, S, P> {
    async fn list(&self, is_super_admin: bool, f: ProductListFilter) -> Result<serde_json::Value, UseCaseError> {
        // 行为保持：总数查询失败旧 unwrap panic → 500；数据查询失败返回空数组
        let (rows, total) = self
            .repo
            .list(&f)
            .await
            .map_err(|_| UseCaseError::Storage(String::new()))?;

        let mut products: Vec<serde_json::Value> = Vec::new();
        for row in rows {
            // 行为保持：units/prices 查询失败旧 unwrap_or_default → 空
            let unit_rows = self.repo.units_of(row.id).await.unwrap_or_default();
            let units: Vec<serde_json::Value> = unit_rows
                .iter()
                .map(|ur| {
                    serde_json::json!({
                        "id": ur.id,
                        "unit_name": ur.unit_name,
                        "ratio": ur.ratio,
                        "unit_price": ur.unit_price,
                        "purchase_price": if is_super_admin { ur.purchase_price } else { 0.0 },
                        "sort_order": ur.sort_order,
                    })
                })
                .collect();

            let price_rows = self.repo.prices_of(row.id).await.unwrap_or_default();
            let mut prices: Vec<serde_json::Value> = Vec::new();
            let mut gov_price: Option<f64> = None;
            let mut other_prices: Vec<f64> = Vec::new();

            for pr in price_rows {
                prices.push(serde_json::json!({
                    "price_type": pr.price_type.clone(),
                    "price": pr.price,
                }));

                if pr.price_type == "gov_procurement" {
                    gov_price = Some(pr.price);
                } else if pr.price_type == "supermarket_1"
                    || pr.price_type == "supermarket_2"
                    || pr.price_type == "supermarket_3"
                    || pr.price_type == "ai_realtime"
                {
                    if pr.price > 0.0 {
                        other_prices.push(pr.price);
                    }
                }
            }

            let selling_price = Self::calc_selling_price(gov_price, &other_prices, row.base_price);

            products.push(serde_json::json!({
                "id": row.id,
                "name": row.name,
                "spec": row.spec,
                "alias1": row.alias1,
                "alias2": row.alias2,
                "unit": row.unit,
                "base_unit": row.base_unit,
                "base_price": row.base_price,
                "purchase_price": if is_super_admin { row.purchase_price } else { 0.0 },
                "max_purchase_price": if is_super_admin { row.max_purchase_price } else { 0.0 },
                "min_purchase_price": if is_super_admin { row.min_purchase_price } else { 0.0 },
                "markup_rate": row.markup_rate,
                "auto_update_price": row.auto_update_price,
                "image_url": if f.include_thumbs { row.image_url.clone() } else { None },
                "category_id": row.category_id,
                "status": row.status,
                "audit_status": row.audit_status,
                "shelf_life": row.shelf_life,
                "gov_status": row.gov_status,
                "category_name": row.category_name,
                "units": units,
                "prices": prices,
                "selling_price": selling_price,
            }));
        }

        let page_size = f.page_size;
        let total_pages = if page_size > 0 { (total + page_size - 1) / page_size } else { 0 };
        Ok(serde_json::json!({
            "data": products,
            "page": f.page,
            "page_size": page_size,
            "total": total,
            "total_pages": total_pages
        }))
    }

    async fn check_name(&self, name: Option<String>) -> String {
        let Some(name) = name.filter(|s| !s.is_empty()) else {
            return serde_json::to_string(&Vec::<serde_json::Value>::new())
                .unwrap_or_else(|_| "[]".to_string())
                .into();
        };
        // 行为保持：查询失败返回空数组（旧 unwrap_or_default）
        let rows = self.repo.find_by_name(&name).await.unwrap_or_default();
        let products: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "name": r.name,
                    "spec": r.spec,
                    "unit": r.unit,
                    "base_unit": r.base_unit,
                    "base_price": r.base_price,
                    "category_id": r.category_id,
                    "category_name": r.category_name,
                })
            })
            .collect();
        serde_json::to_string(&products)
            .unwrap_or_else(|_| "[]".to_string())
            .into()
    }

    async fn by_id(&self, is_super_admin: bool, id: Option<i64>) -> String {
        let Some(pid) = id else {
            return serde_json::to_string(&serde_json::json!({}))
                .unwrap_or_else(|_| "{}".to_string())
                .into();
        };
        // 行为保持：查询失败返回 {}（旧 Err 分支）
        let detail = self.repo.get_detail(pid).await.ok().flatten();
        let Some(r) = detail else {
            return serde_json::to_string(&serde_json::json!({}))
                .unwrap_or_else(|_| "{}".to_string())
                .into();
        };
        let product = serde_json::json!({
            "id": r.id,
            "name": r.name,
            "alias1": r.alias1.unwrap_or_default(),
            "alias2": r.alias2.unwrap_or_default(),
            "spec": r.spec.unwrap_or_default(),
            "unit": r.unit.unwrap_or_default(),
            "base_unit": r.base_unit.unwrap_or_default(),
            "base_price": r.base_price,
            "purchase_price": if is_super_admin { r.purchase_price } else { 0.0 },
            "selling_price": r.selling_price,
            "shelf_life": r.shelf_life.unwrap_or_default(),
            "category_name": r.category_name.unwrap_or_default(),
        });
        serde_json::to_string(&product)
            .unwrap_or_else(|_| "{}".to_string())
            .into()
    }

    async fn search(&self, is_super_admin: bool, keyword: Option<String>, only_audited: bool) -> String {
        let Some(keyword) = keyword.filter(|s| !s.is_empty()) else {
            return serde_json::to_string(&Vec::<serde_json::Value>::new())
                .unwrap_or_else(|_| "[]".to_string())
                .into();
        };
        let pattern = format!("%{}%", keyword);
        // 行为保持：查询失败返回空数组（旧 unwrap_or_default）
        let rows = self.repo.search(&pattern, only_audited).await.unwrap_or_default();
        let products: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "name": r.name,
                    "alias1": r.alias1,
                    "alias2": r.alias2,
                    "spec": r.spec,
                    "unit": r.unit,
                    "base_unit": r.base_unit,
                    "base_price": r.base_price,
                    "purchase_price": if is_super_admin { r.purchase_price } else { 0.0 },
                    "selling_price": r.selling_price,
                    "category_name": r.category_name,
                })
            })
            .collect();
        serde_json::to_string(&products)
            .unwrap_or_else(|_| "[]".to_string())
            .into()
    }

    async fn create(&self, is_super_admin: bool, req: domain::ProductReq) -> Result<(), UseCaseError> {
        // 进价仅超级管理员可设置：非 super_admin 创建商品时进价强制为 0
        let base_unit = req
            .base_unit
            .clone()
            .unwrap_or_else(|| req.unit.clone().unwrap_or_else(|| "斤".to_string()));
        let unit = req.unit.clone().unwrap_or_else(|| "".to_string());
        // 价格统一四舍五入保留两位小数
        let base_price = ((req.base_price.unwrap_or(0.0)) * 100.0).round() / 100.0;
        let purchase_price = if is_super_admin {
            ((req.purchase_price.unwrap_or(0.0)) * 100.0).round() / 100.0
        } else {
            0.0
        };
        let cmd = NewProduct {
            name: req.name,
            spec: req.spec,
            alias1: req.alias1,
            alias2: req.alias2,
            unit,
            base_unit,
            base_price,
            purchase_price,
            category_id: req.category_id,
            shelf_life: req.shelf_life,
            gov_status: req.gov_status.unwrap_or(0),
        };
        match self.repo.create(&cmd).await {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("创建商品失败: {:?}", e);
                Err(UseCaseError::Storage(String::new())) // 行为保持：旧 500 空响应体
            }
        }
    }

    async fn toggle_status(&self, id: i64) -> Result<String, UseCaseError> {
        self.ensure_not_confirmed(id).await?;
        // 行为保持：状态查询失败旧 unwrap_or(None) → 记录不存在
        let status = match self.repo.get_status(id).await {
            Ok(s) => s,
            Err(_) => None,
        };
        let Some(current) = status else {
            return Err(UseCaseError::NotFound("商品不存在".to_string()));
        };
        let new_status = if current == 1 { 0 } else { 1 };
        match self.repo.set_status(id, new_status).await {
            Ok(()) => {
                if new_status == 1 {
                    Ok("商品已启用".to_string())
                } else {
                    Ok("商品已停用".to_string())
                }
            }
            Err(_) => Err(UseCaseError::Storage("操作失败".to_string())),
        }
    }

    async fn update(&self, is_super_admin: bool, req: domain::ProductUpdateReq) -> Result<String, UseCaseError> {
        self.ensure_not_confirmed(req.id).await?;
        // 读取旧值用于日志；行为保持：查询失败旧 .ok().flatten() → 默认值
        let snapshot: PriceSnapshot = self
            .repo
            .get_price_snapshot(req.id)
            .await
            .ok()
            .flatten()
            .unwrap_or(PriceSnapshot { base_price: 0.0, purchase_price: 0.0, markup_rate: 0.5, auto_update_price: 0 });
        let (old_base, old_purchase, old_markup, old_auto) =
            (snapshot.base_price, snapshot.purchase_price, snapshot.markup_rate, snapshot.auto_update_price);

        // 进价仅超级管理员可修改：非 super_admin 忽略前端传入的进价，保持原值
        let mut req = req;
        if !is_super_admin {
            req.purchase_price = Some(old_purchase);
        }

        let cmd = UpdateProduct {
            name: req.name.clone(),
            spec: req.spec.clone(),
            alias1: req.alias1.clone(),
            alias2: req.alias2.clone(),
            unit: req.unit.clone(),
            base_unit: req.base_unit.clone(),
            base_price: req.base_price,
            purchase_price: req.purchase_price,
            image_url: req.image_url.clone(),
            category_id: req.category_id,
            markup_rate: req.markup_rate,
            auto_update_price: req.auto_update_price,
            shelf_life: req.shelf_life.clone(),
            gov_status: req.gov_status.unwrap_or(0),
        };
        match self.repo.update(req.id, &cmd).await {
            Ok(()) => {
                let new_purchase = req.purchase_price.unwrap_or(old_purchase);
                let new_base = req.base_price.unwrap_or(old_base);
                // 记录进价变更
                if (old_purchase - new_purchase).abs() >= 0.001 {
                    let _ = self.price.log_price_change(
                        req.id,
                        "purchase_price",
                        old_purchase,
                        new_purchase,
                        "product_update",
                        None,
                        Some("商品编辑修改进价"),
                    )
                    .await;
                }
                // 记录售价变更
                if (old_base - new_base).abs() >= 0.001 {
                    let _ = self.price.log_price_change(
                        req.id,
                        "base_price",
                        old_base,
                        new_base,
                        "product_update",
                        None,
                        Some("商品编辑修改售价"),
                    )
                    .await;
                }
                // 若加成率或 auto_update_price 改变，触发重算
                let new_markup = req.markup_rate.unwrap_or(old_markup);
                let new_auto = req.auto_update_price.unwrap_or(old_auto);
                if (old_markup - new_markup).abs() >= 0.001 || old_auto != new_auto {
                    eprintln!(
                        "[商品编辑] 商品ID={} 加成率/开关变更 旧加成率={:.4} 新加成率={:.4} 旧开关={} 新开关={} 触发售价重算",
                        req.id, old_markup, new_markup, old_auto, new_auto
                    );
                    let _ = self.price.recalc_base_price_by_markup(req.id, "product_update", None).await;
                }
                Ok("更新成功".to_string())
            }
            Err(e) => {
                eprintln!("更新商品失败: {}", e);
                Err(UseCaseError::Storage("更新失败".to_string()))
            }
        }
    }

    async fn delete(&self, id: i64) -> Result<String, UseCaseError> {
        self.ensure_not_confirmed(id).await?;
        let counts = match self.repo.count_refs(id).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("校验商品关联数据失败({}): {}", e.table, e.message);
                return Err(UseCaseError::Storage(format!("校验{}时出错，删除已中止", e.table)));
            }
        };
        let checks = [
            ("库存", counts.inventory),
            ("采购订单明细", counts.purchase_item),
            ("销售订单明细", counts.sales_item),
            ("补货/订单调整明细", counts.supplement_item),
        ];
        for (name, count) in checks {
            if count > 0 {
                return Err(UseCaseError::BadRequest(format!(
                    "该商品存在{}记录（{}条），无法删除，请先处理关联数据",
                    name, count
                )));
            }
        }
        match self.repo.delete_cascade(id).await {
            Ok(rows) if rows > 0 => Ok("删除成功".to_string()),
            Ok(_) => Err(UseCaseError::NotFound("商品不存在".to_string())),
            Err(DeleteCascadeError::Begin(e)) => {
                eprintln!("删除商品开启事务失败: {}", e);
                Err(UseCaseError::Storage("删除失败".to_string()))
            }
            Err(DeleteCascadeError::Cleanup(e)) => {
                eprintln!("清理商品附属数据失败: {}", e);
                Err(UseCaseError::Storage("删除失败：清理关联数据出错".to_string()))
            }
            Err(DeleteCascadeError::Commit(e)) => {
                eprintln!("删除商品提交事务失败: {}", e);
                Err(UseCaseError::Storage("删除失败".to_string()))
            }
            Err(DeleteCascadeError::Delete(e)) => {
                eprintln!("删除商品失败: {}", e);
                Err(UseCaseError::Storage(format!("删除失败：{}", e)))
            }
        }
    }

    async fn export(&self) -> Result<Vec<u8>, UseCaseError> {
        // 行为保持：宽表查询失败旧 unwrap_or_default → 导出仅表头的空簿
        let rows = self.repo.list_for_export().await.unwrap_or_default();
        build_product_export_workbook(&rows)
            .map_err(|e| UseCaseError::Storage(format!("导出失败: {}", e)))
    }

    async fn import(&self, rows: Vec<Vec<String>>) -> String {
        let mut success = 0;
        let mut updated = 0;
        let mut failed = 0;
        // 失败明细：行号 + 商品名 + 原因（用于导入结果反馈）
        let mut failures: Vec<String> = Vec::new();

        for (idx, row) in rows.iter().enumerate().skip(1) {
            // Excel 行号（含表头行，idx 从 1 开始计第一条数据行）
            let excel_row = idx + 1;
            let row_no = format!("第{}行", excel_row);
            if row.len() < 2 {
                failed += 1;
                failures.push(format!("{}：行数据不足2列，无法解析", row_no));
                continue;
            }

            let name = row[1].trim();
            if name.is_empty() {
                failed += 1;
                failures.push(format!("{}：商品名称为空", row_no));
                continue;
            }
            // 明细中的商品名展示（截断避免弹窗过长）
            let name_disp: String = if name.chars().count() > 20 {
                let s: String = name.chars().take(20).collect();
                format!("{}…", s)
            } else {
                name.to_string()
            };

            let category_name = if row.len() > 9 { row[9].trim() } else { "" };
            let category_id: Option<i64> = if !category_name.is_empty() {
                // 行为保持：分类查询失败旧 .ok().flatten() → None
                self.repo.find_category_id(category_name).await.ok().flatten()
            } else {
                None
            };

            let spec = if row.len() > 4 { row[4].trim() } else { "" };
            // 价格统一四舍五入保留两位小数
            let raw_base: f64 = if row.len() > 7 { row[7].trim().parse().unwrap_or(0.0) } else { 0.0 };
            let raw_purchase: f64 = if row.len() > 8 { row[8].trim().parse().unwrap_or(0.0) } else { 0.0 };
            let base_price: f64 = (raw_base * 100.0).round() / 100.0;
            let purchase_price: f64 = (raw_purchase * 100.0).round() / 100.0;
            let alias1 = if row.len() > 2 { row[2].trim() } else { "" };
            let alias2 = if row.len() > 3 { row[3].trim() } else { "" };
            let unit = if row.len() > 5 { row[5].trim() } else { "" };
            let base_unit = if row.len() > 6 { row[6].trim() } else { "斤" };

            let parsed = ImportProduct {
                name: name.to_string(),
                alias1: alias1.to_string(),
                alias2: alias2.to_string(),
                spec: spec.to_string(),
                unit: unit.to_string(),
                base_unit: base_unit.to_string(),
                base_price,
                purchase_price,
                category_id,
            };

            // 商品写入成功后回填的目标商品 ID（用于后续价格列写入）
            let mut target_pid: Option<i64> = None;

            // 第一列为商品 ID：ID 存在则更新对应商品；无 ID 或 ID 不存在则追加
            // ID 解析容错：剔除零宽字符；兼容 "86.0" 式浮点文本；空串=无ID
            let parse_pid = |s: &str| -> Option<i64> {
                let cleaned: String = s
                    .chars()
                    .filter(|c| !matches!(*c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}'))
                    .collect::<String>()
                    .trim()
                    .to_string();
                if cleaned.is_empty() {
                    return None;
                }
                if let Ok(v) = cleaned.parse::<i64>() {
                    return if v > 0 { Some(v) } else { None };
                }
                if let Ok(f) = cleaned.parse::<f64>() {
                    if f.fract() == 0.0 && f > 0.0 && f <= i64::MAX as f64 {
                        return Some(f as i64);
                    }
                }
                None
            };
            let id_str = row[0].trim();
            let parsed_id = parse_pid(id_str);
            if !id_str.is_empty() && parsed_id.is_none() {
                // 填了 ID 但无法识别为数字：明确报失败，避免静默落成"无ID新增"
                failed += 1;
                failures.push(format!(
                    "{} [{}]：ID列「{}」无法识别为有效数字ID（请填写纯数字ID，或留空由系统分配）",
                    row_no, name_disp, id_str
                ));
                continue;
            }
            if let Some(pid) = parsed_id {
                // 行为保持：存在性计数失败旧 unwrap_or(0) → 视为不存在走追加
                let exists = self.repo.count_by_id(pid).await.unwrap_or(0);
                if exists > 0 {
                    // 同 ID：更新对应商品（修改后置为待审核）
                    match self.repo.import_update(pid, &parsed).await {
                        Ok(r) if r > 0 => {
                            updated += 1;
                            target_pid = Some(pid);
                        }
                        Ok(_) => {
                            failed += 1;
                            failures.push(format!("{} [{}]：更新失败（0 行受影响）", row_no, name_disp));
                        }
                        Err(e) => {
                            failed += 1;
                            let es = storage_msg(&e);
                            if es.contains("UNIQUE constraint failed") {
                                // 名称+规格唯一约束冲突：定位占用该键的已有商品，便于合并/改名
                                // 行为保持：定位查询失败旧 .ok().flatten() → None
                                let cpid = self.repo.find_id_by_name_spec(name, spec).await.ok().flatten();
                                match cpid {
                                    Some(c) => failures.push(format!(
                                        "{} [{}]：更新失败，名称+规格与已有商品 ID {} 冲突（同名同规格唯一），请合并档案或修改名称/规格区分",
                                        row_no, name_disp, c
                                    )),
                                    None => failures.push(format!(
                                        "{} [{}]：更新失败，名称+规格与已有商品冲突（同名同规格唯一）",
                                        row_no, name_disp
                                    )),
                                }
                            } else {
                                failures.push(format!("{} [{}]：更新失败：{}", row_no, name_disp, es));
                            }
                        }
                    }
                }
                // exists == 0：该 ID 不存在 → 落到下方追加
            }

            if target_pid.is_none() {
                // 无 ID 或 ID 不存在：追加（按 name+spec 唯一约束去重）
                // 文件带有效 ID 且库中不存在时，显式使用该 ID 创建，保持 ID 一致
                match self.repo.import_insert(parsed_id, &parsed).await {
                    Ok(ImportInsertOutcome::Inserted(rowid)) => {
                        success += 1;
                        target_pid = Some(rowid);
                    }
                    Ok(ImportInsertOutcome::Ignored) => {
                        failed += 1;
                        // INSERT OR IGNORE 被忽略：名称+规格唯一约束冲突
                        // ID 为唯一辨识：如需更新已有商品，请在 ID 列填写其商品ID；如为新商品，请修改名称/规格区分
                        failures.push(format!(
                            "{} [{}]：未提供有效ID，且同名同规格商品已存在（请在ID列填写已有商品ID进行更新，或修改名称/规格区分）",
                            row_no, name_disp
                        ));
                    }
                    Err(e) => {
                        failed += 1;
                        let es = storage_msg(&e);
                        if es.contains("UNIQUE constraint failed") {
                            failures.push(format!(
                                "{} [{}]：新增失败，名称+规格与已有商品冲突（同名同规格唯一），如需更新请填写该商品ID",
                                row_no, name_disp
                            ));
                        } else {
                            failures.push(format!("{} [{}]：新增失败：{}", row_no, name_disp, es));
                        }
                    }
                }
            }

            // 图片URL列（第16列，可选）：下载网络图片转存到本地并回填 image_url
            // 转存失败不阻断该行商品写入，仅记入失败明细
            if let Some(pid) = target_pid {
                let img_url_raw = if row.len() > 15 { row[15].trim() } else { "" };
                if !img_url_raw.is_empty() {
                    match self.image.save_remote(img_url_raw, &sanitize_filename_prefix(name)).await {
                        Ok(local_url) => {
                            let _ = self.repo.set_image_url(pid, &local_url).await;
                        }
                        Err(e) => {
                            failures.push(format!("{} [{}]：图片转存失败：{}", row_no, name_disp, e));
                        }
                    }
                }
            }

            // 价格列写入（仅当商品写入成功）：政采价/商超1/商超2/商超3/AI实时采集价
            // 写入 product_price_schedule（effective_date=今天，当天已有记录则覆盖）+ product_price 快照
            if let Some(pid) = target_pid {
                let today = Local::now().format("%Y-%m-%d").to_string();
                let price_specs = [
                    ("gov_procurement", 10usize),
                    ("supermarket_1", 11usize),
                    ("supermarket_2", 12usize),
                    ("supermarket_3", 13usize),
                    ("ai_realtime", 14usize),
                ];
                for (pt, pi) in price_specs {
                    let raw = if row.len() > pi { row[pi].trim() } else { "" };
                    if raw.is_empty() {
                        continue;
                    }
                    if let Ok(pv) = raw.parse::<f64>() {
                        let pv = (pv * 100.0).round() / 100.0;
                        if pv > 0.0 {
                            // 时段价：今天生效（同一天重复导入则覆盖价格）；写入失败旧行为吞错
                            let _ = self
                                .repo
                                .import_schedule_price(pid, pt, pv, &today, "商品导入")
                                .await;
                            // 快照：商品编辑表单回显的当前价；写入失败旧行为吞错
                            let _ = self
                                .repo
                                .import_price_snapshot(pid, pt, pv, &today, "商品导入")
                                .await;
                            // 维护时段连续性
                            let _ = self.price.rebuild_end_dates(pid, pt).await;
                            // 政采平台价有数值时自动标记政采状态
                            if pt == "gov_procurement" {
                                let _ = self.price.auto_mark_gov_status(pid).await;
                            }
                        }
                    }
                }
            }
        }

        // 汇总结果 + 失败明细（明细最多展示 50 条，避免弹窗过长）
        let summary = format!("导入完成：新增 {} 条，更新 {} 条，失败 {} 条", success, updated, failed);
        if failures.is_empty() {
            summary
        } else {
            let detail: String = failures.iter().take(50).cloned().collect::<Vec<_>>().join("\n");
            let more = if failures.len() > 50 {
                format!("\n……其余 {} 条失败明细已省略", failures.len() - 50)
            } else {
                String::new()
            };
            format!("{}\n失败明细：\n{}{}", summary, detail, more)
        }
    }

    async fn unit_create(&self, req: domain::ProductUnitReq) -> Result<String, UseCaseError> {
        // 商品单位属价格维度（随时变动），不受商品基础信息审核状态锁定
        // 价格统一四舍五入保留两位小数
        let u = NewProductUnit {
            product_id: req.product_id,
            unit_name: req.unit_name,
            ratio: req.ratio,
            unit_price: ((req.unit_price.unwrap_or(0.0)) * 100.0).round() / 100.0,
            purchase_price: ((req.purchase_price.unwrap_or(0.0)) * 100.0).round() / 100.0,
            sort_order: req.sort_order.unwrap_or(0),
        };
        match self.repo.unit_create(&u).await {
            Ok(()) => Ok("创建成功".to_string()),
            Err(e) => {
                let msg = storage_msg(&e);
                eprintln!("创建单位失败: {}", msg);
                Err(UseCaseError::Storage(format!("创建单位失败: {}", msg)))
            }
        }
    }

    async fn unit_update(&self, req: domain::ProductUnitReq) -> Result<String, UseCaseError> {
        // 行为保持：旧 SQL 的 WHERE id 直接绑定请求的 product_id 字段（前端以该字段传单位 id）
        let u = NewProductUnit {
            product_id: req.product_id,
            unit_name: req.unit_name,
            ratio: req.ratio,
            unit_price: ((req.unit_price.unwrap_or(0.0)) * 100.0).round() / 100.0,
            purchase_price: ((req.purchase_price.unwrap_or(0.0)) * 100.0).round() / 100.0,
            sort_order: req.sort_order.unwrap_or(0),
        };
        match self.repo.unit_update(req.product_id, &u).await {
            Ok(()) => Ok("更新成功".to_string()),
            Err(e) => {
                eprintln!("更新单位失败: {}", e);
                Err(UseCaseError::Storage("更新失败".to_string()))
            }
        }
    }

    async fn unit_delete(&self, id: i64) -> Result<String, UseCaseError> {
        match self.repo.unit_delete(id).await {
            Ok(r) if r > 0 => Ok("删除成功".to_string()),
            Ok(_) => Err(UseCaseError::NotFound("单位不存在".to_string())),
            Err(e) => {
                eprintln!("删除单位失败: {}", e);
                Err(UseCaseError::Storage("删除失败".to_string()))
            }
        }
    }

    async fn unit_delete_by_product(&self, product_id: i64) -> Result<String, UseCaseError> {
        // 商品单位属价格维度（随时变动），不受商品基础信息审核状态锁定
        match self.repo.unit_delete_by_product(product_id).await {
            Ok(()) => Ok("删除成功".to_string()),
            Err(e) => {
                eprintln!("删除单位失败: {}", e);
                Err(UseCaseError::Storage("删除失败".to_string()))
            }
        }
    }

    async fn unit_list(&self, is_super_admin: bool, product_id: Option<i64>) -> String {
        let Some(pid) = product_id else {
            return serde_json::to_string(&Vec::<serde_json::Value>::new())
                .unwrap_or_else(|_| "[]".to_string());
        };
        // 行为保持：查询失败旧 unwrap_or_default → 空数组（复用 units_of，输出列为其子集）
        let rows = self.repo.units_of(pid).await.unwrap_or_default();
        let units: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "name": row.unit_name,
                    "ratio": row.ratio,
                    "unit_price": row.unit_price,
                    "purchase_price": if is_super_admin { row.purchase_price } else { 0.0 },
                })
            })
            .collect();
        serde_json::to_string(&units).unwrap_or_else(|_| "[]".to_string())
    }

    async fn price_upsert(&self, req: domain::ProductPriceReq) -> Result<(), UseCaseError> {
        // product_price 为最新价快照（价格策略 schedule 会同步刷新），属价格维度，不受商品审核锁定
        match self
            .repo
            .price_upsert(
                req.product_id,
                &req.price_type,
                req.price.unwrap_or(0.0),
                req.collected_at.as_deref(),
                req.source.as_deref(),
            )
            .await
        {
            Ok(()) => {
                // 政采平台价有数值时，政采平台状态自动标记为「政采商品（有指导价）」
                let _ = self.price.auto_mark_gov_status(req.product_id).await;
                Ok(())
            }
            Err(e) => {
                eprintln!("保存价格失败: {}", storage_msg(&e));
                Err(UseCaseError::Storage(String::new())) // 行为保持：旧 500 空响应体
            }
        }
    }

    async fn price_list(&self, product_id: i64) -> String {
        // 行为保持：查询失败旧 unwrap_or_default → 空数组
        let rows = self.repo.price_rows(product_id).await.unwrap_or_default();
        let prices: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "product_id": r.product_id,
                    "price_type": r.price_type,
                    "price": r.price,
                    "collected_at": r.collected_at,
                    "source": r.source,
                })
            })
            .collect();
        serde_json::to_string(&prices).unwrap_or_else(|_| "[]".to_string())
    }

    async fn price_delete(&self, id: i64) -> Result<String, UseCaseError> {
        match self.repo.price_delete(id).await {
            Ok(r) if r > 0 => Ok("删除成功".to_string()),
            Ok(_) => Err(UseCaseError::NotFound("价格记录不存在".to_string())),
            Err(e) => {
                eprintln!("删除价格失败: {}", e);
                Err(UseCaseError::Storage("删除失败".to_string()))
            }
        }
    }

    async fn price_delete_by_product(&self, product_id: i64) {
        // product_price 为最新价快照（价格策略 schedule 会同步刷新），属价格维度，不受商品审核锁定
        // 行为保持：旧 .ok() 吞错恒 200
        let _ = self.repo.price_delete_by_product(product_id).await;
    }

    async fn sync_base_price(&self, product_id: i64) {
        let _ = self.price.sync_base_price(product_id).await;
    }

    async fn set_auto_update_price(&self, product_id: i64, auto: i64) {
        let _ = self.price.set_auto_update_price(product_id, auto).await;
    }

    async fn batch_set_auto_update_price(&self, auto: i64) {
        let _ = self.price.batch_set_auto_update_price(auto).await;
    }

    async fn last_purchase_price(&self, product_id: i64) -> Option<String> {
        // 行为保持：查询失败/不存在旧 .ok().flatten() → None → 404
        let info = self.repo.get_purchase_info(product_id).await.ok().flatten()?;
        // 同时取出最近一次采购的原始下单单价及单位，便于前端核对是否同基础单位
        // 行为保持：查询失败旧 .ok().flatten() → 回退 (0.0, base_unit)
        let (last_unit_price, last_unit) = match self.repo.get_last_purchase_item(product_id).await.ok().flatten()
        {
            Some(lr) => (lr.unit_price, lr.unit),
            None => (0.0, info.base_unit.clone()),
        };
        let payload = serde_json::json!({
            "purchase_price": info.purchase_price,
            "base_unit": info.base_unit,
            "last_unit_price": last_unit_price,
            "last_unit": last_unit,
        });
        Some(serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()))
    }

    async fn price_log_list(&self, product_id: Option<i64>, limit: i64) -> String {
        // 行为保持：查询失败旧 unwrap_or_default → 空数组
        let rows = self.repo.price_logs(product_id, limit).await.unwrap_or_default();
        let logs: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "product_id": r.product_id,
                    "product_name": r.product_name,
                    "price_type": r.price_type,
                    "old_price": r.old_price,
                    "new_price": r.new_price,
                    "source": r.source,
                    "ref_id": r.ref_id,
                    "remark": r.remark,
                    "changed_at": r.changed_at,
                })
            })
            .collect();
        serde_json::to_string(&logs).unwrap_or_else(|_| "[]".to_string())
    }

    // —— 子步c：图片操作 ——

    async fn upload_image(&self, product_id: Option<i64>, files: Vec<(String, Vec<u8>)>) -> Result<String, UseCaseError> {
        let Some(pid) = product_id else {
            return Err(UseCaseError::BadRequest("缺少 product_id 参数".to_string()));
        };

        // 行为保持：商品名查询失败/不存在均回退 "product"
        let product_name = self.repo.name_of(pid).await.ok().flatten().unwrap_or_else(|| "product".to_string());
        let name_prefix = sanitize_filename_prefix(&product_name);

        let mut file_path = String::new();
        let mut has_file = false;

        for (filename, bytes) in files {
            has_file = true;
            let ext = std::path::Path::new(&filename)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("jpg")
                .to_lowercase();
            if !["jpg", "jpeg", "png", "gif", "webp"].contains(&ext.as_str()) {
                return Err(UseCaseError::BadRequest("不支持的图片格式".to_string()));
            }
            if bytes.len() > 5 * 1024 * 1024 {
                return Err(UseCaseError::BadRequest("图片大小不能超过5MB".to_string()));
            }
            file_path = self
                .image
                .save_upload(&name_prefix, &ext, &bytes)
                .await
                .map_err(|e| UseCaseError::Storage(if e.is_empty() { "保存图片失败".to_string() } else { e }))?;
        }

        if !has_file {
            return Err(UseCaseError::BadRequest("请选择要上传的图片".to_string()));
        }

        // 行为保持：旧代码 UPDATE 失败吞错不响应 500
        let _ = self.repo.set_image_url_opt(pid, Some(&file_path)).await;

        Ok(serde_json::json!({ "url": file_path }).to_string())
    }

    async fn delete_image(&self, product_id: Option<i64>) -> Result<String, UseCaseError> {
        let Some(pid) = product_id else {
            return Err(UseCaseError::BadRequest("缺少 product_id 参数".to_string()));
        };
        // 行为保持：查询失败/无图均不报错，仍返回"删除成功"
        if let Ok(Some(url)) = self.repo.image_url_of(pid).await {
            self.image.remove_by_url(&url).await;
        }
        // 行为保持：清库失败吞错
        let _ = self.repo.set_image_url_opt(pid, None).await;
        Ok("删除成功".to_string())
    }

    async fn image_from_url(&self, product_id: Option<i64>, url: String) -> Result<String, UseCaseError> {
        let Some(pid) = product_id else {
            return Err(UseCaseError::BadRequest("缺少 product_id".to_string()));
        };
        // 行为保持：商品名查询失败/不存在均回退 "product"
        let product_name = self.repo.name_of(pid).await.ok().flatten().unwrap_or_else(|| "product".to_string());
        let name_prefix = sanitize_filename_prefix(&product_name);

        match self.image.save_remote(&url, &name_prefix).await {
            Ok(local_url) => match self.repo.set_image_url_opt(pid, Some(&local_url)).await {
                Ok(_) => Ok(serde_json::json!({ "url": local_url }).to_string()),
                Err(e) => Err(UseCaseError::Storage(format!("更新数据库失败: {}", storage_msg(&e)))),
            },
            Err(e) => Err(UseCaseError::BadRequest(e)),
        }
    }

    async fn read_image(&self, folder: Option<String>, filename: String) -> Option<(Vec<u8>, &'static str)> {
        let path = match &folder {
            Some(f) => {
                // 仅允许已知子目录，防止路径穿越
                let allowed = ["products", "customer_orders", "signed_orders", "purchase_documents"];
                if !allowed.contains(&f.as_str())
                    || filename.contains("..")
                    || filename.contains('/')
                    || filename.contains('\\')
                {
                    return None;
                }
                format!("uploads/{}/{}", f, filename)
            }
            None => format!("uploads/{}", filename),
        };

        let content = self.image.read_file(&path).await.ok()?;
        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("jpg")
            .to_lowercase();
        let mime: &'static str = match ext.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "image/jpeg",
        };
        Some((content, mime))
    }
}

/// 导出工作簿（自旧 api_product_export 移植：列序/列宽/表头/留白规则逐字节一致）
fn build_product_export_workbook(rows: &[ProductExportRow]) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    use rust_xlsxwriter::{Format, FormatAlign, Workbook};

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_format = Format::new()
        .set_bold()
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);

    let headers = ["ID", "名称", "下订名称(别称1)", "配单名称(别称2)", "规格", "显示单位", "基础单位", "基准单价", "进价", "分类", "政采价", "商超1", "商超2", "商超3", "AI实时采集价", "计算售价"];
    for (i, &header) in headers.iter().enumerate() {
        worksheet.write_with_format(0, i as u16, header, &header_format)?;
    }

    let mut row_idx = 1;
    for row in rows {
        let gov = row.gov_price;
        let sm1 = row.sm1_price;
        let sm2 = row.sm2_price;
        let sm3 = row.sm3_price;
        let ai = row.ai_price;
        let base_price = row.base_price;
        let purchase_price = row.purchase_price;
        // 计算售价：政采价优先；否则商超1/2/3/AI 中非零价的平均（不做取整，计算原值）
        let selling = if gov > 0.0 {
            gov
        } else {
            let vals = [sm1, sm2, sm3, ai];
            let (sum, cnt) = vals
                .iter()
                .fold((0.0f64, 0usize), |(s, c), &v| if v > 0.0 { (s + v, c + 1) } else { (s, c) });
            if cnt == 0 {
                0.0
            } else {
                round2(sum / cnt as f64)
            }
        };
        // 基础单价：非零计算售价则同步为计算售价，否则取基础单价原值
        let base_out = if selling > 0.0 { selling } else { base_price };
        // 基础单位按实际导出
        let base_unit = row.base_unit.clone().unwrap_or_default();

        worksheet.write(row_idx, 0, row.id)?;
        worksheet.write(row_idx, 1, &row.name)?;
        worksheet.write(row_idx, 2, row.alias1.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 3, row.alias2.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 4, row.spec.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 5, &row.unit)?;
        worksheet.write(row_idx, 6, &base_unit)?;
        // 价格类值 <=0 时单元格留白
        if base_out > 0.0 {
            worksheet.write(row_idx, 7, base_out)?;
        } else {
            worksheet.write(row_idx, 7, "")?;
        }
        if purchase_price > 0.0 {
            worksheet.write(row_idx, 8, purchase_price)?;
        } else {
            worksheet.write(row_idx, 8, "")?;
        }
        worksheet.write(row_idx, 9, row.category_name.clone().unwrap_or_default())?;
        for (col, v) in [(10u16, gov), (11u16, sm1), (12u16, sm2), (13u16, sm3), (14u16, ai), (15u16, selling)] {
            if v > 0.0 {
                worksheet.write(row_idx, col, v)?;
            } else {
                worksheet.write(row_idx, col, "")?;
            }
        }
        row_idx += 1;
    }

    worksheet.set_column_width(0, 8)?;
    worksheet.set_column_width(1, 20)?;
    worksheet.set_column_width(2, 18)?;
    worksheet.set_column_width(3, 18)?;
    worksheet.set_column_width(4, 12)?;
    worksheet.set_column_width(5, 8)?;
    worksheet.set_column_width(6, 10)?;
    worksheet.set_column_width(7, 12)?;
    worksheet.set_column_width(8, 10)?;
    worksheet.set_column_width(9, 12)?;
    worksheet.set_column_width(10, 10)?;
    worksheet.set_column_width(11, 10)?;
    worksheet.set_column_width(12, 10)?;
    worksheet.set_column_width(13, 10)?;
    worksheet.set_column_width(14, 14)?;
    worksheet.set_column_width(15, 12)?;

    workbook.save_to_buffer()
}
