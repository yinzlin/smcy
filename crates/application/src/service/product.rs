//! 商品用例：列表组装 / 详情 / 搜索 / CRUD / 状态切换
//!
//! 行为保持：查询类吞错兜底、错误文案与响应体逐字节复刻旧 api_product_* handler。

use domain::port::product::{
    DeleteCascadeError, NewProduct, PriceSnapshot, ProductListFilter, ProductRepo, UpdateProduct,
};

use crate::error::UseCaseError;
use crate::port::product::ProductUseCase;

#[derive(Clone)]
pub struct ProductService<R: ProductRepo> {
    repo: R,
}

impl<R: ProductRepo> ProductService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
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

impl<R: ProductRepo> ProductUseCase for ProductService<R> {
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
                    crate::service::price::log_price_change(
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
                    crate::service::price::log_price_change(
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
                    crate::service::price::recalc_base_price_by_markup(req.id, "product_update", None).await;
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
}
