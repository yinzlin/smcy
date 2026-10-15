//! 系统管理域入站端口（UseCase 契约）

use std::collections::HashMap;

use crate::error::UseCaseError;

/// 备份文件下载结果（文件名 + 内容字节；文件读取属基础设施副作用，在用例层完成，
/// 响应头由 HTTP 薄适配层拼接）
pub struct BackupDownload {
    pub file_name: String,
    pub content: Vec<u8>,
}

pub trait SystemMgmtUseCase: Send + Sync {
    /// 保存系统配置（逐键 INSERT OR REPLACE，单键失败吞错）→ 200 文本"设置保存成功"
    async fn set_config(&self, data: HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 全部配置键值对 → JSON 对象字符串（HTTP 层保持旧 Json 响应的 application/json 头）
    async fn get_config(&self) -> Result<String, UseCaseError>;
    /// 备份记录列表 → JSON 数组字符串（HTTP 层保持旧 Json 响应的 application/json 头）
    async fn backup_list(&self) -> Result<String, UseCaseError>;
    /// 操作日志分页列表 → 200 JSON 字符串（COUNT 查询失败 unwrap 与旧 handler panic 语义一致）
    async fn operation_log_list(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError>;
    /// 操作日志导出 → xlsx 字节（文件名/响应头由 HTTP 层拼接）
    async fn export_operation_log(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError>;
    /// 执行备份（VACUUM INTO backups/backup_*.db + 插入 backup_record，记录失败吞错）→ 200 文本
    async fn backup(&self) -> Result<String, UseCaseError>;
    /// 备份下载 → 文件名与字节（记录不存在/文件读取失败 → 404"文件不存在"）
    async fn backup_download(&self, id: i64) -> Result<BackupDownload, UseCaseError>;
    /// 删除备份（文件删除与记录删除均吞错）→ 200 文本；记录不存在 → 404"备份不存在"
    async fn backup_delete(&self, id: i64) -> Result<String, UseCaseError>;
    /// 还原备份（复制覆盖 food_accept_v3.db，2 秒后自动重启）→ 200 文本
    async fn restore(&self, id: i64) -> Result<String, UseCaseError>;
    /// 上传还原（字节落盘 temp_backups/temp_restore.db 后复制覆盖主库，2 秒后自动重启）→ 200 文本
    async fn restore_file(&self, data: &[u8]) -> Result<String, UseCaseError>;
    /// 检查损坏销售明细（LIMIT 100）→ JSON 数组字符串（序列化失败 → 500"序列化失败：…"）
    async fn inspect_corrupted_items(&self) -> Result<String, UseCaseError>;
    /// 清理损坏明细（固定 ID 段 5527..=5619 与无明细订单，逐条吞错，最后 VACUUM）→ 200 文本
    async fn clean_corrupted_items(&self) -> Result<String, UseCaseError>;
    /// 清理无效订单（先 VACUUM INTO 备份，六轮修复，逐条吞错，最后 VACUUM）→ 200 文本
    async fn clean_invalid_orders(&self) -> Result<String, UseCaseError>;
}
