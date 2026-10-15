//! 系统管理用例：系统配置、备份/还原、操作日志查询与导出、数据修复
//!
//! 行为保持：单键配置写入、备份记录插入、文件删除等吞错（unwrap_or_default）
//! 原样保留；workbook 构建 .unwrap() 原样保留；文件系统副作用（backups 目录、
//! VACUUM INTO 产物、food_accept_v3.db 覆盖、2 秒后自动重启调度）在本层直接执行；
//! Excel 文件名/响应头由 HTTP 薄适配层拼接。

use std::collections::HashMap;

use chrono::Local;
use domain::error::DomainError;
use domain::port::system_mgmt::{OperationLogFilter, SystemMgmtRepo};

use crate::error::UseCaseError;
use crate::port::system_mgmt::{BackupDownload, SystemMgmtUseCase};

#[derive(Clone)]
pub struct SystemMgmtService<R: SystemMgmtRepo> {
    repo: R,
}

impl<R: SystemMgmtRepo> SystemMgmtService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

/// 操作日志筛选参数解析（与旧 handler 一致：缺失为空串、两侧 trim）
fn parse_operation_log_filter(params: &HashMap<String, String>) -> OperationLogFilter {
    OperationLogFilter {
        username: params.get("username").map(|s| s.as_str()).unwrap_or("").trim().to_string(),
        action: params.get("action").map(|s| s.as_str()).unwrap_or("").trim().to_string(),
        start_date: params.get("start_date").map(|s| s.as_str()).unwrap_or("").trim().to_string(),
        end_date: params.get("end_date").map(|s| s.as_str()).unwrap_or("").trim().to_string(),
    }
}

/// VACUUM INTO 失败 → 携带原始存储层错误消息的 500（文案由调用场景决定）
fn vacuum_into_error(e: DomainError, prefix: &str) -> UseCaseError {
    match e {
        DomainError::Storage(m) => UseCaseError::Storage(format!("{}{}", prefix, m)),
        other => UseCaseError::from(other),
    }
}

impl<R: SystemMgmtRepo> SystemMgmtUseCase for SystemMgmtService<R> {
    async fn set_config(&self, data: HashMap<String, String>) -> Result<String, UseCaseError> {
        for (key, value) in data {
            // 行为保持：单键保存失败 unwrap_or_default 吞错，继续处理后续键
            self.repo.upsert_system_config(&key, &value).await.unwrap_or_default();
        }
        Ok("设置保存成功".to_string())
    }

    async fn get_config(&self) -> Result<String, UseCaseError> {
        // 行为保持：查询失败 unwrap_or_default 吞错
        let rows = self.repo.list_system_config().await.unwrap_or_default();
        let mut map = serde_json::Map::new();
        for (key, value) in rows {
            map.insert(key, serde_json::Value::String(value));
        }
        Ok(serde_json::to_string(&serde_json::Value::Object(map)).unwrap())
    }

    async fn backup_list(&self) -> Result<String, UseCaseError> {
        // 行为保持：查询失败 unwrap_or_default 吞错
        let rows = self.repo.list_backup_records().await.unwrap_or_default();
        let list: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "backup_time": r.backup_time,
                    "file_name": r.file_name,
                    "size": r.size,
                })
            })
            .collect();
        Ok(serde_json::to_string(&list).unwrap())
    }

    async fn operation_log_list(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
        let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20).min(100);
        let offset = (page - 1) * page_size;

        let filter = parse_operation_log_filter(params);

        // 行为保持：COUNT 查询失败 unwrap（与旧 handler 相同的 panic 语义）
        let total = self.repo.count_operation_logs(&filter).await.unwrap();

        // 行为保持：数据查询失败 unwrap_or_default 吞错
        let rows = self.repo.list_operation_logs(&filter, page_size, offset).await.unwrap_or_default();

        let list: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                let action = r.action.clone();
                serde_json::json!({
                    "id": r.id,
                    "user_id": r.user_id,
                    "username": r.username,
                    "action": action,
                    "action_label": common::utils::operation_action_label(&action),
                    "target_type": r.target_type,
                    "target_id": r.target_id,
                    "detail": r.detail,
                    "created_at": r.created_at,
                })
            })
            .collect();

        let result = serde_json::json!({
            "data": list,
            "page": page,
            "page_size": page_size,
            "total": total,
            "total_pages": (total + page_size - 1) / page_size
        });
        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_operation_log(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let filter = parse_operation_log_filter(params);
        // 行为保持：查询失败 unwrap_or_default 吞错
        let rows = self.repo.list_operation_logs_export(&filter).await.unwrap_or_default();

        let mut workbook = rust_xlsxwriter::Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("操作日志").unwrap();

        let header_format = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0x2E75B6))
            .set_font_color(rust_xlsxwriter::Color::White)
            .set_align(rust_xlsxwriter::FormatAlign::Center)
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        let headers = ["ID", "操作时间", "操作人", "动作", "目标类型", "目标ID", "详情"];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_with_format(0, col as u16, *header, &header_format).unwrap();
        }

        for (i, row) in rows.iter().enumerate() {
            let r = (i + 1) as u32;
            let action = row.action.clone();
            let target_type = row.target_type.clone();
            let type_label = match target_type.as_str() {
                "purchase_order" => "采购单",
                "sales_order" => "销售单",
                "purchase_document" => "采购单据",
                _ => &target_type,
            };
            worksheet.write(r, 0, row.id).unwrap();
            worksheet.write(r, 1, row.created_at.clone()).unwrap();
            worksheet.write(r, 2, row.username.clone()).unwrap();
            worksheet.write(r, 3, common::utils::operation_action_label(&action)).unwrap();
            worksheet.write(r, 4, type_label).unwrap();
            worksheet.write(r, 5, row.target_id.clone()).unwrap();
            worksheet.write(r, 6, row.detail.clone()).unwrap();
        }

        worksheet.set_column_width(0, 8).unwrap();
        worksheet.set_column_width(1, 20).unwrap();
        worksheet.set_column_width(2, 14).unwrap();
        worksheet.set_column_width(3, 18).unwrap();
        worksheet.set_column_width(4, 12).unwrap();
        worksheet.set_column_width(5, 10).unwrap();
        worksheet.set_column_width(6, 60).unwrap();

        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn backup(&self) -> Result<String, UseCaseError> {
        use std::fs;
        use std::path::Path;

        let now = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_dir = "backups";
        if !Path::new(backup_dir).exists() {
            fs::create_dir_all(backup_dir).unwrap_or_default();
        }

        let backup_file = format!("{}/backup_{}.db", backup_dir, now);

        if let Err(e) = self.repo.vacuum_into(&backup_file).await {
            return Err(vacuum_into_error(e, "备份失败："));
        }
        if let Ok(size) = fs::metadata(&backup_file) {
            // 行为保持：备份记录插入失败 unwrap_or_default 吞错
            self.repo.insert_backup_record(&now, &backup_file, size.len() as i64).await.unwrap_or_default();
            Ok(format!("备份成功，文件大小：{} 字节", size.len()))
        } else {
            Err(UseCaseError::Storage("备份文件创建失败".to_string()))
        }
    }

    async fn backup_download(&self, id: i64) -> Result<BackupDownload, UseCaseError> {
        // 行为保持：记录查询失败 unwrap_or_default 吞错（与记录不存在同走 404）
        let row = self.repo.get_backup_file_name(id).await.unwrap_or_default();

        if let Some(file_name) = row {
            if let Ok(content) = std::fs::read(&file_name) {
                return Ok(BackupDownload { file_name, content });
            }
        }
        Err(UseCaseError::NotFound("文件不存在".to_string()))
    }

    async fn backup_delete(&self, id: i64) -> Result<String, UseCaseError> {
        // 行为保持：记录查询失败 unwrap_or_default 吞错（与记录不存在同走 404）
        let row = self.repo.get_backup_file_name(id).await.unwrap_or_default();

        if let Some(file_name) = row {
            std::fs::remove_file(&file_name).unwrap_or_default();
            // 行为保持：记录删除失败 unwrap_or_default 吞错
            self.repo.delete_backup_record(id).await.unwrap_or_default();
            Ok("删除成功".to_string())
        } else {
            Err(UseCaseError::NotFound("备份不存在".to_string()))
        }
    }

    async fn restore(&self, id: i64) -> Result<String, UseCaseError> {
        // 行为保持：记录查询失败 unwrap_or_default 吞错（与记录不存在同走 404）
        let row = self.repo.get_backup_file_name(id).await.unwrap_or_default();

        if let Some(file_name) = row {
            match std::fs::copy(&file_name, "food_accept_v3.db") {
                Ok(_) => {
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        if let Ok(exe_path) = std::env::current_exe() {
                            let _ = std::process::Command::new(&exe_path)
                                .spawn();
                        }
                        std::process::exit(0);
                    });
                    Ok("恢复成功，系统将在2秒后自动重启，请稍后刷新页面".to_string())
                }
                Err(e) => Err(UseCaseError::Storage(format!("恢复失败：{}", e))),
            }
        } else {
            Err(UseCaseError::NotFound("备份不存在".to_string()))
        }
    }

    async fn restore_file(&self, data: &[u8]) -> Result<String, UseCaseError> {
        use std::fs;

        let backup_dir = "temp_backups";
        if !std::path::Path::new(backup_dir).exists() {
            fs::create_dir_all(backup_dir).unwrap_or_default();
        }

        let temp_file = format!("{}/temp_restore.db", backup_dir);

        match fs::write(&temp_file, data) {
            Ok(_) => {
                match fs::copy(&temp_file, "food_accept_v3.db") {
                    Ok(_) => {
                        fs::remove_file(&temp_file).unwrap_or_default();
                        tokio::spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            if let Ok(exe_path) = std::env::current_exe() {
                                let _ = std::process::Command::new(&exe_path)
                                    .spawn();
                            }
                            std::process::exit(0);
                        });
                        Ok("恢复成功，系统将在2秒后自动重启，请稍后刷新页面".to_string())
                    }
                    Err(e) => {
                        fs::remove_file(&temp_file).unwrap_or_default();
                        Err(UseCaseError::Storage(format!("恢复失败：{}", e)))
                    }
                }
            }
            Err(e) => Err(UseCaseError::Storage(format!("保存临时文件失败：{}", e))),
        }
    }

    async fn inspect_corrupted_items(&self) -> Result<String, UseCaseError> {
        // 行为保持：查询失败 unwrap_or_default 吞错
        let rows = self.repo.list_corrupted_sales_items().await.unwrap_or_default();

        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| serde_json::json!({
                "id": r.id,
                "order_id": r.order_id,
                "product_id": r.product_id,
                "product_name": r.product_name,
                "unit": r.unit,
                "unit_price": r.unit_price,
                "quantity": r.quantity,
                "amount": r.amount,
            }))
            .collect();

        match serde_json::to_string(&items) {
            Ok(json_str) => Ok(json_str),
            Err(e) => Err(UseCaseError::Storage(format!("序列化失败：{}", e))),
        }
    }

    async fn clean_corrupted_items(&self) -> Result<String, UseCaseError> {
        // 行为保持：查询失败 unwrap_or_default、删除失败 .ok()、VACUUM let _ = 吞错
        let corrupted_ids = self.repo.list_corrupted_item_ids().await.unwrap_or_default();

        let count = corrupted_ids.len();

        for id in &corrupted_ids {
            self.repo.delete_sales_order_item(*id).await.ok();
        }

        let no_item_sales = self.repo.list_sales_orders_without_items().await.unwrap_or_default();

        for id in &no_item_sales {
            self.repo.delete_sales_order(*id).await.ok();
        }

        let _ = self.repo.vacuum().await;

        Ok(format!("清理完成，共删除 {} 条损坏的订单明细记录", count))
    }

    async fn clean_invalid_orders(&self) -> Result<String, UseCaseError> {
        use std::fs;
        use std::path::Path;
        let now = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_dir = "backups";
        if !Path::new(backup_dir).exists() {
            fs::create_dir_all(backup_dir).unwrap_or_default();
        }
        let backup_file = format!("{}/backup_before_clean_{}.db", backup_dir, now);
        if let Err(e) = self.repo.vacuum_into(&backup_file).await {
            return Err(vacuum_into_error(e, "清理前备份失败："));
        }

        let mut deleted_count = 0;

        // 行为保持：查询失败 unwrap_or_default、删除失败 .ok() 吞错
        let no_item_sales = self.repo.list_sales_orders_without_items().await.unwrap_or_default();

        for id in &no_item_sales {
            self.repo.delete_sales_order(*id).await.ok();
            deleted_count += 1;
        }

        let no_item_purchase = self.repo.list_purchase_orders_without_items().await.unwrap_or_default();

        for id in &no_item_purchase {
            self.repo.delete_purchase_order(*id).await.ok();
            deleted_count += 1;
        }

        let invalid_purchaser_sales = self.repo.list_sales_orders_with_invalid_purchaser().await.unwrap_or_default();

        for id in &invalid_purchaser_sales {
            self.repo.delete_sales_order_items_by_order(*id).await.ok();
            self.repo.delete_sales_order(*id).await.ok();
            deleted_count += 1;
        }

        let invalid_supplier_purchase = self.repo.list_purchase_orders_with_invalid_supplier().await.unwrap_or_default();

        for id in &invalid_supplier_purchase {
            self.repo.delete_purchase_order_items_by_order(*id).await.ok();
            self.repo.delete_purchase_order(*id).await.ok();
            deleted_count += 1;
        }

        let invalid_product_sales_items = self.repo.list_sales_items_with_invalid_product().await.unwrap_or_default();

        for id in &invalid_product_sales_items {
            self.repo.delete_sales_order_item(*id).await.ok();
        }

        let invalid_product_purchase_items = self.repo.list_purchase_items_with_invalid_product().await.unwrap_or_default();

        for id in &invalid_product_purchase_items {
            self.repo.delete_purchase_order_item(*id).await.ok();
        }

        let no_item_after_clean_sales = self.repo.list_sales_orders_without_items().await.unwrap_or_default();

        for id in &no_item_after_clean_sales {
            self.repo.delete_sales_order(*id).await.ok();
            deleted_count += 1;
        }

        let no_item_after_clean_purchase = self.repo.list_purchase_orders_without_items().await.unwrap_or_default();

        for id in &no_item_after_clean_purchase {
            self.repo.delete_purchase_order(*id).await.ok();
            deleted_count += 1;
        }

        let _ = self.repo.vacuum().await;

        Ok(format!("清理完成，共删除 {} 条无效订单。清理前已备份到 {}", deleted_count, backup_file))
    }
}
