//! 采购单据域入站端口（UseCase 契约）

use domain::UserCtx;

use crate::error::UseCaseError;

/// 单据上传用例返回体：JSON 响应字段 + 操作日志参数（HTTP 层补记 log_operation）
pub struct PurchaseDocumentUpload {
    pub id: i64,
    pub url: String,
    pub log_detail: String,
}

pub trait PurchaseDocumentUseCase: Send + Sync {
    /// 单据列表（supplier 角色强制只看自己绑定供应商，supplier_id<=0 兜底 -1；
    /// 查询失败吞为空数组，JSON 字段序逐字节保持）
    async fn list(&self, ctx: &UserCtx, supplier_id: Option<i64>, document_date: &str) -> Result<String, UseCaseError>;
    /// 上传单据（supplier 行级权限 + 按供应商+日期前缀落盘 uploads/purchase_documents + DB 写入；
    /// multipart 解析与缺参校验在 HTTP 层完成）
    async fn upload(&self, ctx: &UserCtx, supplier_id: i64, supplier_name: &str, document_date: &str, remark: Option<String>, file_ext: &str, file_bytes: &[u8]) -> Result<PurchaseDocumentUpload, UseCaseError>;
    /// 删除单据（行级权限 + 本地文件删除吞错；不存在 → 404"单据不存在"）
    async fn delete(&self, ctx: &UserCtx, id: i64) -> Result<String, UseCaseError>;
    /// 列表导出 xlsx 字节（supplier 角色强制；文件名/响应头由 HTTP 层拼接）
    async fn list_export(&self, ctx: &UserCtx, supplier_id: Option<i64>, document_date: &str) -> Result<Vec<u8>, UseCaseError>;
}
