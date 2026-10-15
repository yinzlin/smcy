//! 耗材分摊用例：创建 / 摘要 / 已分摊订单 / 终止 / 取消 / 完成
//!
//! 行为保持：EXISTS/SUM 查询失败吞错兜底；cancel/complete 的行查询不吞错
//! （Err → 500 "查询失败: {原始错误}"）；尾差增项写入失败吞错。

use serde_json::json;

use domain::error::DomainError;
use domain::port::order_allocation::OrderAllocationRepo;

use crate::error::UseCaseError;
use crate::port::order_allocation::OrderAllocationUseCase;

#[derive(Clone)]
pub struct OrderAllocationService<R: OrderAllocationRepo> {
    repo: R,
}

impl<R: OrderAllocationRepo> OrderAllocationService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: OrderAllocationRepo> OrderAllocationUseCase for OrderAllocationService<R> {
    async fn create(&self, req: serde_json::Value) -> Result<String, UseCaseError> {
        let source_order_id = req.get("source_order_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let remark = req.get("remark").and_then(|v| v.as_str()).unwrap_or("");

        // 勾选的来源明细 id 列表
        let item_ids: Vec<i64> = req
            .get("source_item_ids")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|x| x.as_i64()).collect())
            .unwrap_or_default();

        if item_ids.is_empty() {
            return Err(UseCaseError::BadRequest("请至少勾选一条明细".to_string()));
        }

        // 检查是否已存在分摊方案，防止重复创建（行为保持：查询失败视为不存在）
        let exists = self.repo.exists_by_source(source_order_id).await.unwrap_or(false);
        if exists {
            return Err(UseCaseError::BadRequest("该订单已有分摊方案".to_string()));
        }

        // 服务端按勾选明细重算分摊总额，确保金额可信（行为保持：查询失败视为 0）
        let total_amount = self
            .repo
            .sum_item_amounts(source_order_id, &item_ids)
            .await
            .unwrap_or(0.0);

        if total_amount <= 0.0 {
            return Err(UseCaseError::BadRequest(
                "勾选明细的金额合计为 0，无法初始化".to_string(),
            ));
        }

        let item_ids_str = item_ids
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");

        match self
            .repo
            .create(source_order_id, total_amount, remark, &item_ids_str)
            .await
        {
            Ok(id) => Ok(serde_json::to_string(&json!({ "id": id, "total_amount": total_amount })).unwrap()),
            Err(e) => Err(UseCaseError::Storage(format!("创建分摊方案失败: {}", storage_msg(e)))),
        }
    }

    async fn summary(&self, source_order_id: i64) -> Result<String, UseCaseError> {
        // 行为保持：查询失败吞错 → 视为不存在 → {"exists": false}
        let row = self.repo.fetch_first_by_source(source_order_id).await.unwrap_or(None);

        let summary = match row {
            None => json!({ "exists": false }),
            Some(r) => {
                let item_ids_str = r.source_item_ids.unwrap_or_default();
                let source_item_ids: Vec<i64> = item_ids_str
                    .split(',')
                    .filter_map(|s| s.trim().parse::<i64>().ok())
                    .collect();
                json!({
                    "exists": true,
                    "id": r.id,
                    "source_order_id": r.source_order_id,
                    "total_amount": r.total_amount,
                    "allocated_amount": r.allocated_amount,
                    "remaining_balance": r.remaining_balance,
                    "status": r.status,
                    "remark": r.remark.unwrap_or_default(),
                    "created_at": r.created_at,
                    "completed_at": r.completed_at.unwrap_or_default(),
                    "source_item_ids": source_item_ids,
                })
            }
        };

        Ok(serde_json::to_string(&summary).unwrap())
    }

    async fn allocated_orders(&self) -> Result<String, UseCaseError> {
        // 行为保持：查询失败返回空数组
        let rows = self.repo.list_allocated_orders().await.unwrap_or_default();

        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "alloc_id": r.alloc_id,
                    "source_order_id": r.source_order_id,
                    "order_no": r.order_no.clone().unwrap_or_default(),
                    "order_date": r.order_date.clone().unwrap_or_default(),
                    "purchaser_id": r.purchaser_id.unwrap_or(0),
                    "purchaser_name": r.purchaser_name.clone().unwrap_or_default(),
                    "total_amount": r.total_amount,
                    "allocated_amount": r.allocated_amount,
                    "remaining_balance": r.remaining_balance,
                    "status": r.status,
                    "remark": r.remark.clone().unwrap_or_default(),
                    "created_at": r.created_at.clone().unwrap_or_default(),
                    "completed_at": r.completed_at.clone().unwrap_or_default(),
                })
            })
            .collect();

        Ok(serde_json::to_string(&items).unwrap())
    }

    async fn terminate(&self, req: serde_json::Value) -> Result<String, UseCaseError> {
        let source_order_id = req.get("source_order_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let remark = req.get("remark").and_then(|v| v.as_str()).unwrap_or("");

        match self.repo.terminate(source_order_id, remark).await {
            Ok(rows) => {
                if rows > 0 {
                    Ok("终止成功".to_string())
                } else {
                    Err(UseCaseError::BadRequest("无法终止：订单不存在或已完成".to_string()))
                }
            }
            Err(e) => Err(UseCaseError::Storage(format!("终止失败: {}", storage_msg(e)))),
        }
    }

    async fn cancel(&self, req: serde_json::Value) -> Result<String, UseCaseError> {
        let source_order_id = req.get("source_order_id").and_then(|v| v.as_i64()).unwrap_or(0);

        // 行为保持：查询失败不吞错（500 "查询失败: {原始错误}"）
        let row = match self.repo.fetch_by_source(source_order_id).await {
            Ok(r) => r,
            Err(e) => return Err(UseCaseError::Storage(format!("查询失败: {}", storage_msg(e)))),
        };
        let row = match row {
            Some(r) => r,
            None => return Err(UseCaseError::NotFound("分摊方案不存在".to_string())),
        };

        let allocated = row.allocated_amount;
        let status = row.status;

        if status == 2 {
            return Err(UseCaseError::BadRequest("分摊方案已完成，无法取消".to_string()));
        }
        if allocated > 0.0001 {
            return Err(UseCaseError::BadRequest(format!(
                "尚有已分摊金额 {:.2} 元，请先回滚全部分摊记录后再取消",
                allocated
            )));
        }

        match self.repo.delete_by_source(source_order_id).await {
            Ok(rows) => {
                if rows > 0 {
                    Ok("取消成功".to_string())
                } else {
                    Err(UseCaseError::BadRequest("无法取消分摊方案".to_string()))
                }
            }
            Err(e) => Err(UseCaseError::Storage(format!("取消失败: {}", storage_msg(e)))),
        }
    }

    async fn complete(&self, req: serde_json::Value) -> Result<String, UseCaseError> {
        let source_order_id = req.get("source_order_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let target_order_id = req.get("target_order_id").and_then(|v| v.as_i64());
        let auto_tail = req.get("auto_tail").and_then(|v| v.as_bool()).unwrap_or(false);

        // 行为保持：查询失败不吞错（500 "查询失败: {原始错误}"）
        let row = match self.repo.fetch_by_source(source_order_id).await {
            Ok(r) => r,
            Err(e) => return Err(UseCaseError::Storage(format!("查询失败: {}", storage_msg(e)))),
        };
        let row = match row {
            Some(r) => r,
            None => return Err(UseCaseError::NotFound("分摊方案不存在".to_string())),
        };

        let alloc_id = row.id;
        let remaining_balance = row.remaining_balance;
        let total_amount = row.total_amount;
        let status = row.status;

        if status == 2 {
            return Err(UseCaseError::BadRequest("分摊方案已完成".to_string()));
        }
        if status == 3 {
            return Err(UseCaseError::BadRequest("分摊方案已终止".to_string()));
        }

        let threshold = 5.0;

        if remaining_balance > threshold && !auto_tail {
            return Err(UseCaseError::BadRequest(format!(
                "剩余 {:.2} 元未分摊，超过尾差限额（±{:.2} 元），请继续分摊或终止方案",
                remaining_balance, threshold
            )));
        }
        if remaining_balance < -threshold && !auto_tail {
            return Err(UseCaseError::BadRequest(format!(
                "已超额分摊 {:.2} 元，超过尾差限额（±{:.2} 元），请回滚部分分摊后再确认",
                remaining_balance.abs(),
                threshold
            )));
        }

        if auto_tail && remaining_balance.abs() > 0.0 {
            if let Some(tid) = target_order_id {
                // 行为保持：目标订单存在性查询失败视为不存在
                let target_exists = self.repo.target_order_exists(tid).await.unwrap_or(false);
                if target_exists {
                    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                    // 行为保持：尾差增项写入失败吞错
                    let _ = self
                        .repo
                        .insert_tail_item(
                            tid,
                            source_order_id,
                            &format!(
                                "耗材分摊尾差（源订单: {:.2} 元中剩余 {:.2} 元）",
                                total_amount, remaining_balance
                            ),
                            remaining_balance,
                            &today,
                        )
                        .await;
                }
            }
        }

        match self.repo.mark_completed(alloc_id).await {
            Ok(_) => Ok(serde_json::to_string(&json!({ "auto_tail": auto_tail && remaining_balance > 0.0 })).unwrap()),
            Err(e) => Err(UseCaseError::Storage(format!("完成分摊失败: {}", storage_msg(e)))),
        }
    }
}

/// repo 将存储层错误原文存入 DomainError::Storage，此处取回原文拼接旧文案
fn storage_msg(e: DomainError) -> String {
    match e {
        DomainError::Storage(m) => m,
        other => other.to_string(),
    }
}
