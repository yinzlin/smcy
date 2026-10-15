//! 订单增补（分摊增项）用例：创建 / 双向列表 / 删除回滚 / 增补比价 / 调整订单分页
//!
//! 行为保持：列表与统计查询失败吞错兜底；分摊余额 UPDATE 序列失败吞错；
//! 比价的展示归并逻辑（display_name/sort_key/合计）逐字平移。

use serde_json::json;
use std::collections::HashMap;

use domain::error::DomainError;
use domain::port::order_supplement::{
    AdjustedOrdersFilter, OrderSupplementInput, OrderSupplementRepo,
};
use crate::dto::OrderSupplementItemReq;

use crate::error::UseCaseError;
use crate::port::order_supplement::OrderSupplementUseCase;
use crate::service::get_category_sort_key;

#[derive(Clone)]
pub struct OrderSupplementService<R: OrderSupplementRepo> {
    repo: R,
}

impl<R: OrderSupplementRepo> OrderSupplementService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

/// repo 将存储层错误原文存入 DomainError::Storage，此处取回原文拼接旧文案
fn storage_msg(e: DomainError) -> String {
    match e {
        DomainError::Storage(m) => m,
        other => other.to_string(),
    }
}

impl<R: OrderSupplementRepo> OrderSupplementUseCase for OrderSupplementService<R> {
    async fn create(&self, req: OrderSupplementItemReq) -> Result<String, UseCaseError> {
        // HTTP 契约 → domain 端口输入类型（字段同名同序，下游字段访问不变）
        let req = OrderSupplementInput::from(req);
        match self.repo.create(&req).await {
            Ok(id) => {
                // 分摊余额口径：正数(换入/追加/新增)按金额全额消耗余额；
                // replace_remove 冲减仅修改分摊账套明细，不释放余额
                // （被冲减的原明细金额本身即待分摊的耗材来源，其额度由换入商品对冲）
                let pool_delta = if req.operation_type == "replace_remove" { 0.0 } else { req.amount };
                // 行为保持：分摊余额联动失败吞错
                let _ = self.repo.apply_pool_delta(req.source_order_id, pool_delta).await;

                // 冲减后若余额回升（remaining>0），需从"已完成"回退到"分摊中"并清空完结时间
                let _ = self.repo.touch_completed_at(req.source_order_id).await;

                Ok(serde_json::to_string(&json!({ "id": id })).unwrap())
            }
            Err(e) => Err(UseCaseError::Storage(format!("创建增项失败: {}", storage_msg(e)))),
        }
    }

    async fn list_by_target(&self, order_id: i64) -> Result<String, UseCaseError> {
        // 行为保持：查询失败返回空数组
        let rows = self.repo.list_by_target(order_id).await.unwrap_or_default();

        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "id": row.id,
                    "target_order_id": row.target_order_id,
                    "source_order_id": row.source_order_id,
                    "source_order_no": row.source_order_no.clone().unwrap_or_default(),
                    "source_remark": row.source_remark.clone().unwrap_or_default(),
                    "product_id": row.product_id,
                    "product_name": row.product_name,
                    "alias1": row.alias1.clone().unwrap_or_default(),
                    "alias2": row.alias2.clone().unwrap_or_default(),
                    "spec": row.spec.clone().unwrap_or_default(),
                    "unit": row.unit,
                    "unit_price": row.unit_price,
                    "quantity": row.quantity,
                    "amount": row.amount,
                    "allocate_date": row.allocate_date,
                    "operation_type": row.operation_type,
                    "target_order_item_id": row.target_order_item_id,
                })
            })
            .collect();

        Ok(serde_json::to_string(&items).unwrap())
    }

    async fn list_by_source(&self, order_id: i64) -> Result<String, UseCaseError> {
        // 行为保持：查询失败返回空数组
        let rows = self.repo.list_by_source(order_id).await.unwrap_or_default();

        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "id": row.id,
                    "target_order_id": row.target_order_id,
                    "target_order_no": row.target_order_no.clone().unwrap_or_default(),
                    "source_order_id": row.source_order_id,
                    "source_remark": row.source_remark.clone().unwrap_or_default(),
                    "product_id": row.product_id,
                    "product_name": row.product_name,
                    "alias1": row.alias1.clone().unwrap_or_default(),
                    "alias2": row.alias2.clone().unwrap_or_default(),
                    "spec": row.spec.clone().unwrap_or_default(),
                    "unit": row.unit,
                    "unit_price": row.unit_price,
                    "quantity": row.quantity,
                    "amount": row.amount,
                    "allocate_date": row.allocate_date,
                    "operation_type": row.operation_type,
                })
            })
            .collect();

        Ok(serde_json::to_string(&items).unwrap())
    }

    async fn delete(&self, id: i64) -> Result<String, UseCaseError> {
        // 行为保持：查询失败吞错 → 视为不存在
        let row = self.repo.fetch_for_delete(id).await.unwrap_or(None);

        let row = match row {
            Some(r) => r,
            None => return Err(UseCaseError::NotFound("增项不存在".to_string())),
        };

        // 与创建时镜像：replace_remove 冲减创建时未消耗余额，回滚时也不释放
        let pool_delta = if row.operation_type == "replace_remove" { 0.0 } else { row.amount };

        match self.repo.delete(id).await {
            Ok(rows) => {
                if rows > 0 {
                    // 正数(换入/追加/新增)回滚时 allocated 减回、remaining 加回；
                    // 冲减(replace_remove)回滚时不影响分摊余额
                    // 行为保持：回滚联动三步失败均吞错
                    let _ = self.repo.rollback_pool(row.source_order_id, pool_delta).await;
                    let _ = self.repo.clear_completed_at(row.source_order_id).await;
                    // 若已分摊金额已归零，则删除该分摊方案，回到"未分摊"初始态，
                    // 允许重新勾选明细并再次初始化分摊
                    let _ = self.repo.delete_if_zero(row.source_order_id).await;

                    Ok("回滚成功".to_string())
                } else {
                    Err(UseCaseError::NotFound("增项不存在".to_string()))
                }
            }
            Err(e) => Err(UseCaseError::Storage(format!("回滚失败: {}", storage_msg(e)))),
        }
    }

    async fn compare(&self, order_id: i64) -> Result<String, UseCaseError> {
        // 行为保持：两个查询失败均返回空列表继续计算
        let item_rows = self.repo.list_compare_items(order_id).await.unwrap_or_default();
        let supplement_rows = self
            .repo
            .list_compare_supplements(order_id)
            .await
            .unwrap_or_default();

        let mut item_map: HashMap<i64, serde_json::Value> = HashMap::new();
        let mut real_items: Vec<serde_json::Value> = Vec::new();
        for r in &item_rows {
            let id = r.id;
            let alias2 = r.alias2.clone().unwrap_or_default();
            let product_name = r.product_name.clone();
            let display_name = if alias2.is_empty() {
                product_name.clone()
            } else {
                format!("{}({})", product_name, alias2)
            };
            let spec = r.spec.clone().unwrap_or_default();
            let category_name = r.category_name.clone().unwrap_or_default();
            let parent_name = r.parent_name.clone().unwrap_or_default();
            let sort_key = get_category_sort_key(&category_name, &parent_name);
            let item = json!({
                "id": id,
                "product_id": r.product_id,
                "product_name": product_name,
                "display_name": display_name,
                "alias1": r.alias1.clone().unwrap_or_default(),
                "alias2": alias2,
                "spec": spec,
                "unit": r.unit.clone().unwrap_or_default(),
                "unit_price": r.unit_price,
                "quantity": r.quantity,
                "amount": r.amount,
                "remark": r.remark.clone().unwrap_or_default(),
                "category_name": category_name,
                "parent_name": parent_name,
                "sort_key": sort_key,
                "is_increase": false,
                "is_new": false,
                "is_replaced": false,
                "supplement_quantity": 0.0,
                "supplement_amount": 0.0,
                "total_quantity": r.quantity,
                "total_amount": r.amount,
            });
            item_map.insert(id, item.clone());
            real_items.push(item);
        }

        let mut new_items: Vec<serde_json::Value> = Vec::new();
        for r in &supplement_rows {
            let op_type = r.operation_type.clone();
            let target_item_id = r.target_order_item_id;
            let qty = r.quantity;
            let amt = r.amount;

            if op_type == "increase_quantity" || op_type == "replace_remove" {
                if let Some(tid) = target_item_id {
                    if let Some(existing) = item_map.get_mut(&tid) {
                        let s_qty = existing["supplement_quantity"].as_f64().unwrap_or(0.0) + qty;
                        let s_amt = existing["supplement_amount"].as_f64().unwrap_or(0.0) + amt;
                        let t_qty = existing["quantity"].as_f64().unwrap_or(0.0) + s_qty;
                        let t_amt = existing["amount"].as_f64().unwrap_or(0.0) + s_amt;
                        existing["supplement_quantity"] = serde_json::json!(s_qty);
                        existing["supplement_amount"] = serde_json::json!(s_amt);
                        existing["total_quantity"] = serde_json::json!(t_qty);
                        existing["total_amount"] = serde_json::json!(t_amt);
                        if op_type == "replace_remove" {
                            existing["is_replaced"] = serde_json::json!(true);
                        } else {
                            existing["is_increase"] = serde_json::json!(true);
                        }
                    }
                }
            } else {
                let alias2 = r.alias2.clone().unwrap_or_default();
                let product_name = r.product_name.clone();
                let display_name = if alias2.is_empty() {
                    product_name.clone()
                } else {
                    format!("{}({})", product_name, alias2)
                };
                let spec = r.spec.clone().unwrap_or_default();
                let source_remark = r.source_remark.clone().unwrap_or_default();
                new_items.push(json!({
                    "id": -r.id,
                    "supplement_id": r.id,
                    "product_id": r.product_id,
                    "product_name": product_name,
                    "display_name": display_name,
                    "alias1": r.alias1.clone().unwrap_or_default(),
                    "alias2": alias2,
                    "spec": spec,
                    "unit": r.unit,
                    "unit_price": r.unit_price,
                    "quantity": 0.0,
                    "amount": 0.0,
                    "remark": source_remark,
                    "category_name": "",
                    "parent_name": "",
                    "sort_key": 9999,
                    "is_increase": false,
                    "is_new": true,
                    "supplement_quantity": qty,
                    "supplement_amount": amt,
                    "total_quantity": qty,
                    "total_amount": amt,
                    "allocate_date": r.allocate_date,
                }));
            }
        }

        let mut real_total = 0.0;
        let mut supplement_total = 0.0;
        let mut alloc_total = 0.0;

        let mut combined: Vec<serde_json::Value> = Vec::new();
        for (_, item) in &item_map {
            real_total += item["amount"].as_f64().unwrap_or(0.0);
            supplement_total += item["supplement_amount"].as_f64().unwrap_or(0.0);
            alloc_total += item["total_amount"].as_f64().unwrap_or(0.0);
            combined.push(item.clone());
        }
        for item in &new_items {
            supplement_total += item["supplement_amount"].as_f64().unwrap_or(0.0);
            alloc_total += item["total_amount"].as_f64().unwrap_or(0.0);
            combined.push(item.clone());
        }

        combined.sort_by(|a, b| {
            let sa = a["sort_key"].as_i64().unwrap_or(9999);
            let sb = b["sort_key"].as_i64().unwrap_or(9999);
            sa.cmp(&sb)
        });

        let result = json!({
            "real_total": real_total,
            "supplement_total": supplement_total,
            "allocation_total": alloc_total,
            "items": combined,
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn adjusted_orders(
        &self,
        params: HashMap<String, String>,
    ) -> Result<String, UseCaseError> {
        // 列出所有收到分摊增项/调整的目标订单（target_order_id 指向该订单即视为有变更）
        let page = params
            .get("page")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(1)
            .max(1);
        let page_size = params
            .get("page_size")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(10)
            .clamp(1, 100);
        let keyword = params.get("keyword").cloned().unwrap_or_default();
        let purchaser_id = params.get("purchaser_id").and_then(|v| v.parse::<i64>().ok());
        let sort_order = params.get("sort_order").cloned().unwrap_or_default();

        let filter = AdjustedOrdersFilter {
            keyword_like: if keyword.trim().is_empty() {
                None
            } else {
                Some(format!("%{}%", keyword.trim()))
            },
            purchaser_id,
            sort_order,
            page,
            page_size,
        };

        // 行为保持：总数查询失败视为 0
        let total = self.repo.count_adjusted(&filter).await.unwrap_or(0);
        // 行为保持：合计查询失败视为 (0.0, 0.0)
        let (sum_real, sum_adjust) = self.repo.sum_adjusted(&filter).await.unwrap_or((0.0, 0.0));
        // 行为保持：列表查询失败返回空数组
        let rows = self.repo.list_adjusted(&filter).await.unwrap_or_default();

        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let total: f64 = row.total_amount;
                let adjust: f64 = row.adjust_amount;
                json!({
                    "id": row.id,
                    "order_no": row.order_no,
                    "order_date": row.order_date,
                    "purchaser_name": row.purchaser_name.clone().unwrap_or_default(),
                    "purchaser_id": row.purchaser_id,
                    "total_amount": total,
                    "adjust_amount": adjust,
                    "adjusted_total": total + adjust,
                    "adjust_count": row.adjust_count,
                    "last_adjust_date": row.last_adjust_date.clone().unwrap_or_default(),
                })
            })
            .collect();

        let resp = json!({
            "total": total,
            "page": page,
            "page_size": page_size,
            "total_real_amount": sum_real,
            "total_adjust_amount": sum_adjust,
            "total_adjusted_amount": sum_real + sum_adjust,
            "items": items,
        });

        Ok(resp.to_string())
    }
}
