//! 采购单据域出站端口（purchase_document 表）
//!
//! 行为保持约定：单据图片文件落盘/删除（uploads/purchase_documents）属基础设施
//! 副作用，由 application 用例层直接执行（与 system_mgmt 同例）；端口仅覆盖 DB
//! 读写。列表查询失败吞为空结果的 unwrap_or_default 语义由用例层保持。

use crate::error::DomainError;

/// 单据行（api_purchase_document_list 全列）
#[derive(Debug, Clone)]
pub struct PurchaseDocumentRow {
    pub id: i64,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub document_date: String,
    pub image_url: String,
    pub remark: Option<String>,
    pub create_at: String,
}

/// 单据导出行（api_purchase_document_list_export，不含 image_url）
#[derive(Debug, Clone)]
pub struct PurchaseDocumentExportRow {
    pub id: i64,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub document_date: String,
    pub remark: Option<String>,
    pub create_at: String,
}

/// 列表筛选（supplier_id 为 HTTP 原始值，supplier 角色强制在用例层完成；
/// document_date 空串表示不过滤）
#[derive(Debug, Clone, Default)]
pub struct PurchaseDocumentFilter {
    pub supplier_id: Option<i64>,
    pub document_date: String,
}

pub trait PurchaseDocumentRepo: Send + Sync {
    /// 列表（动态 WHERE 分支与 ORDER BY 拼装在仓储内，SQL 片段与旧 handler 逐字一致）
    async fn list(&self, filter: &PurchaseDocumentFilter) -> Result<Vec<PurchaseDocumentRow>, DomainError>;
    /// 导出查询（列同旧 handler，无 image_url；SQL 逐字一致）
    async fn list_export(&self, filter: &PurchaseDocumentFilter) -> Result<Vec<PurchaseDocumentExportRow>, DomainError>;
    /// 插入单据，返回 last_insert_rowid
    async fn insert(&self, supplier_id: i64, supplier_name: &str, document_date: &str, image_url: &str, remark: Option<&str>) -> Result<i64, DomainError>;
    /// 删除前置读：image_url 与 supplier_id（fetch_optional，错误吞为 None 由用例层 404）
    async fn get_image_url_and_supplier(&self, id: i64) -> Option<(String, i64)>;
    /// 删除单据（旧行为：失败由用例层映射 500"删除失败"）
    async fn delete(&self, id: i64) -> Result<(), DomainError>;
}
