//! 采购单据用例：列表 / 上传（文件落盘 + DB 写入）/ 删除 / 列表导出
//!
//! 行为与旧 api_purchase_document_* handler 逐字一致：含错误文案、状态码、
//! 吞错分支（列表查询 unwrap_or_default、删除前置读吞错、文件删除 let _）、
//! 文件命名规则与操作日志文案；Workbook 构建 .unwrap() 原样保留。

use domain::port::purchase_document::{PurchaseDocumentFilter, PurchaseDocumentRepo};
use domain::UserCtx;
use rust_xlsxwriter::Workbook;

use crate::error::UseCaseError;
use crate::port::purchase_document::{PurchaseDocumentUpload, PurchaseDocumentUseCase};

#[derive(Clone)]
pub struct PurchaseDocumentService<R: PurchaseDocumentRepo> {
    repo: R,
}

impl<R: PurchaseDocumentRepo> PurchaseDocumentService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// 行级数据权限裁决（旧 handler 逐字平移）：supplier 角色只能看/传/删自己绑定的
    /// 供应商单据；supplier_id<=0 兜底 -1（匹配不到任何单据）
    fn effective_supplier_id(ctx: &UserCtx, supplier_id: Option<i64>) -> Option<i64> {
        if ctx.role == "supplier" {
            if ctx.supplier_id > 0 { Some(ctx.supplier_id) } else { Some(-1) }
        } else {
            supplier_id
        }
    }
}

impl<R: PurchaseDocumentRepo> PurchaseDocumentUseCase for PurchaseDocumentService<R> {
    async fn list(&self, ctx: &UserCtx, supplier_id: Option<i64>, document_date: &str) -> Result<String, UseCaseError> {
        let filter = PurchaseDocumentFilter {
            supplier_id: Self::effective_supplier_id(ctx, supplier_id),
            document_date: document_date.to_string(),
        };

        // 行为保持：查询失败 unwrap_or_default 吞错
        let rows = self.repo.list(&filter).await.unwrap_or_default();

        let result: Vec<serde_json::Value> = rows.iter().map(|row| serde_json::json!({
            "id": row.id,
            "supplier_id": row.supplier_id,
            "supplier_name": row.supplier_name,
            "document_date": row.document_date,
            "image_url": row.image_url,
            "remark": row.remark,
            "create_at": row.create_at,
        })).collect();

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn upload(&self, ctx: &UserCtx, supplier_id: i64, supplier_name: &str, document_date: &str, remark: Option<String>, file_ext: &str, file_bytes: &[u8]) -> Result<PurchaseDocumentUpload, UseCaseError> {
        // 行级数据权限：supplier 角色只能上传自己绑定的供应商单据
        if ctx.role == "supplier" {
            if ctx.supplier_id == 0 || supplier_id != ctx.supplier_id {
                return Err(UseCaseError::Forbidden("供应商账号只能为自己上传单据".to_string()));
            }
        }

        let name_prefix = format!("{}_{}", common::utils::sanitize_filename_prefix(supplier_name), document_date);

        tokio::fs::create_dir_all("uploads/purchase_documents").await.ok();
        let timestamp = chrono::Utc::now().timestamp_millis();
        let random: u32 = rand::random();
        let new_filename = format!("{}_{}_{}.{}", name_prefix, timestamp, random, file_ext);
        let path = format!("uploads/purchase_documents/{}", new_filename);

        if tokio::fs::write(&path, file_bytes).await.is_err() {
            return Err(UseCaseError::Storage("保存图片失败".to_string()));
        }
        let saved_url = format!("/api/uploads/purchase_documents/{}", new_filename);

        let id = match self.repo.insert(supplier_id, supplier_name, document_date, &saved_url, remark.as_deref()).await {
            Ok(id) => id,
            Err(_) => return Err(UseCaseError::Storage("保存失败".to_string())),
        };

        Ok(PurchaseDocumentUpload {
            id,
            url: saved_url.clone(),
            log_detail: format!("上传采购单据（供应商ID={}，日期={}）：{}", supplier_id, document_date, saved_url),
        })
    }

    async fn delete(&self, ctx: &UserCtx, id: i64) -> Result<String, UseCaseError> {
        // 行为保持：前置查询失败吞为 None（旧 fetch_optional(...).unwrap_or(None)）
        if let Some((image_url, doc_supplier_id)) = self.repo.get_image_url_and_supplier(id).await {
            // 行级数据权限：supplier 角色只能删除自己供应商的单据
            if ctx.role == "supplier" && doc_supplier_id != ctx.supplier_id {
                return Err(UseCaseError::Forbidden("您没有权限删除此单据".to_string()));
            }
            if let Some(path) = common::utils::image_url_to_path(&image_url) {
                let _ = tokio::fs::remove_file(&path).await;
            }
        } else {
            return Err(UseCaseError::NotFound("单据不存在".to_string()));
        }

        // 行为保持：删除失败 → 500"删除失败"
        self.repo.delete(id).await.map_err(|_| UseCaseError::Storage("删除失败".to_string()))?;

        Ok("删除成功".to_string())
    }

    async fn list_export(&self, ctx: &UserCtx, supplier_id: Option<i64>, document_date: &str) -> Result<Vec<u8>, UseCaseError> {
        let filter = PurchaseDocumentFilter {
            supplier_id: Self::effective_supplier_id(ctx, supplier_id),
            document_date: document_date.to_string(),
        };

        // 行为保持：查询失败 unwrap_or_default 吞错
        let rows = self.repo.list_export(&filter).await.unwrap_or_default();

        let mut workbook = Workbook::new();
        let ws = workbook.add_worksheet();
        ws.set_name("采购单据").unwrap();
        let hf = common::utils::xlsx_header_format(0x4472C4);
        for (c, h) in ["ID", "供应商", "单据日期", "备注", "创建时间"].iter().enumerate() {
            ws.write_with_format(0, c as u16, *h, &hf).unwrap();
        }
        ws.set_column_width(0, 8).unwrap();
        ws.set_column_width(1, 18).unwrap();
        ws.set_column_width(2, 14).unwrap();
        ws.set_column_width(3, 20).unwrap();
        ws.set_column_width(4, 20).unwrap();
        for (i, row) in rows.iter().enumerate() {
            let r = (i + 1) as u32;
            ws.write(r, 0, row.id).unwrap();
            ws.write(r, 1, row.supplier_name.as_str()).unwrap();
            ws.write(r, 2, row.document_date.as_str()).unwrap();
            ws.write(r, 3, row.remark.clone().unwrap_or_default()).unwrap();
            ws.write(r, 4, row.create_at.as_str()).unwrap();
        }
        Ok(workbook.save_to_buffer().unwrap())
    }
}
