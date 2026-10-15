//! 系统管理域出站端口（系统配置、备份记录、操作日志查询、数据修复）
//!
//! 行为保持约定：文件系统副作用（backups 目录扫描/创建、备份文件读取/删除、
//! VACUUM INTO 产物落盘、food_accept_v3.db 覆盖还原）属基础设施副作用，
//! 由 application 用例层直接执行，不入出站端口；端口仅覆盖 DB 读写。

use crate::error::DomainError;

/// 备份记录行（backup_record 表）
#[derive(Debug, Clone)]
pub struct BackupRecordRow {
    pub id: i64,
    pub backup_time: String,
    pub file_name: String,
    pub size: i64,
}

/// 操作日志行（operation_log 表，列表/导出查询共用）
#[derive(Debug, Clone)]
pub struct OperationLogRow {
    pub id: i64,
    pub user_id: i64,
    pub username: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub detail: String,
    pub created_at: String,
}

/// 操作日志筛选条件（各字段为已 trim 的值，空串表示不过滤）
#[derive(Debug, Clone, Default)]
pub struct OperationLogFilter {
    pub username: String,
    pub action: String,
    pub start_date: String,
    pub end_date: String,
}

/// 损坏销售明细行（unit_price/quantity 零值检查用）
#[derive(Debug, Clone)]
pub struct CorruptedItemRow {
    pub id: i64,
    pub order_id: i64,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Option<f64>,
    pub quantity: Option<f64>,
    pub amount: f64,
}

pub trait SystemMgmtRepo: Send + Sync {
    // ---- 系统配置（system_config 表） ----
    /// 单键 upsert（旧行为：调用方逐键吞错）
    async fn upsert_system_config(&self, key: &str, value: &str) -> Result<(), DomainError>;
    /// 全部配置键值对（SELECT key, value FROM system_config）
    async fn list_system_config(&self) -> Result<Vec<(String, String)>, DomainError>;

    // ---- 备份记录（backup_record 表） ----
    /// 备份记录列表（ORDER BY backup_time DESC）
    async fn list_backup_records(&self) -> Result<Vec<BackupRecordRow>, DomainError>;
    /// 插入备份记录（旧行为：调用方吞错）
    async fn insert_backup_record(&self, backup_time: &str, file_name: &str, size: i64) -> Result<(), DomainError>;
    /// 按 id 查备份文件名（不存在返回 None）
    async fn get_backup_file_name(&self, id: i64) -> Result<Option<String>, DomainError>;
    /// 按 id 删除备份记录（旧行为：调用方吞错）
    async fn delete_backup_record(&self, id: i64) -> Result<(), DomainError>;

    // ---- VACUUM（备份/清理共用） ----
    /// VACUUM INTO '<path>'（SQL 与旧 handler 逐字一致：路径单引号拼接 + AssertSqlSafe）
    async fn vacuum_into(&self, path: &str) -> Result<(), DomainError>;
    /// VACUUM（旧行为：调用方 let _ = 吞错）
    async fn vacuum(&self) -> Result<(), DomainError>;

    // ---- 操作日志查询（动态 WHERE 在仓储内拼装，SQL 片段与旧 handler 逐字一致） ----
    /// COUNT 查询（旧行为：调用方 unwrap，DB 错误 panic 语义一致）
    async fn count_operation_logs(&self, filter: &OperationLogFilter) -> Result<i64, DomainError>;
    /// 分页查询（ORDER BY id DESC LIMIT ? OFFSET ?；旧行为：调用方 unwrap_or_default 吞错）
    async fn list_operation_logs(&self, filter: &OperationLogFilter, page_size: i64, offset: i64) -> Result<Vec<OperationLogRow>, DomainError>;
    /// 导出全量查询（ORDER BY id DESC，无 LIMIT）
    async fn list_operation_logs_export(&self, filter: &OperationLogFilter) -> Result<Vec<OperationLogRow>, DomainError>;

    // ---- 数据修复（inspect/clean，SQL 与旧 handler 逐字一致） ----
    /// 损坏销售明细检查（零单价/零数量/零金额且品名非空，LIMIT 100）
    async fn list_corrupted_sales_items(&self) -> Result<Vec<CorruptedItemRow>, DomainError>;
    /// 固定 ID 段的损坏明细（id >= 5527 AND id <= 5619）
    async fn list_corrupted_item_ids(&self) -> Result<Vec<i64>, DomainError>;
    /// 无明细的销售订单
    async fn list_sales_orders_without_items(&self) -> Result<Vec<i64>, DomainError>;
    /// 无明细的采购订单
    async fn list_purchase_orders_without_items(&self) -> Result<Vec<i64>, DomainError>;
    /// 采购单位失效的销售订单
    async fn list_sales_orders_with_invalid_purchaser(&self) -> Result<Vec<i64>, DomainError>;
    /// 供应商失效的采购订单
    async fn list_purchase_orders_with_invalid_supplier(&self) -> Result<Vec<i64>, DomainError>;
    /// 商品失效的销售明细
    async fn list_sales_items_with_invalid_product(&self) -> Result<Vec<i64>, DomainError>;
    /// 商品失效的采购明细
    async fn list_purchase_items_with_invalid_product(&self) -> Result<Vec<i64>, DomainError>;
    /// 按 id 删销售明细（旧行为：调用方 .ok() 吞错）
    async fn delete_sales_order_item(&self, id: i64) -> Result<(), DomainError>;
    /// 按 id 删销售订单（旧行为：调用方 .ok() 吞错）
    async fn delete_sales_order(&self, id: i64) -> Result<(), DomainError>;
    /// 按 id 删采购订单（旧行为：调用方 .ok() 吞错）
    async fn delete_purchase_order(&self, id: i64) -> Result<(), DomainError>;
    /// 按订单 id 删销售明细（旧行为：调用方 .ok() 吞错）
    async fn delete_sales_order_items_by_order(&self, order_id: i64) -> Result<(), DomainError>;
    /// 按订单 id 删采购明细（旧行为：调用方 .ok() 吞错）
    async fn delete_purchase_order_items_by_order(&self, order_id: i64) -> Result<(), DomainError>;
    /// 按 id 删采购明细（旧行为：调用方 .ok() 吞错）
    async fn delete_purchase_order_item(&self, id: i64) -> Result<(), DomainError>;
}
