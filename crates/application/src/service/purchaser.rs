//! 采购单位用例：列表 / 导出 / 导入 / 创建 / 更新 / 删除
//!
//! 行为保持：查询类吞错兜底、错误文案逐字节复刻旧 handler。

use domain::port::purchaser::{ImportPurchaser, PurchaserFilter, PurchaserRepo, PurchaserRow, UpsertPurchaser};

use crate::error::UseCaseError;
use crate::port::purchaser::PurchaserUseCase;

#[derive(Clone)]
pub struct PurchaserService<R: PurchaserRepo> {
    repo: R,
}

impl<R: PurchaserRepo> PurchaserService<R> {
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
}

impl<R: PurchaserRepo> PurchaserUseCase for PurchaserService<R> {
    async fn list(&self, category_id: Option<i64>, keyword_pattern: String) -> Vec<PurchaserRow> {
        // 行为保持：查询失败返回空数组（旧 unwrap_or_default）
        self.repo
            .list(&PurchaserFilter { category_id, keyword_pattern })
            .await
            .unwrap_or_default()
    }

    async fn export_excel(&self) -> Result<Vec<u8>, UseCaseError> {
        // 行为保持：查询失败返回空数组（旧 unwrap_or_default）
        let rows = self.repo.list_for_export().await.unwrap_or_default();
        build_export_workbook(&rows).map_err(|e| UseCaseError::Storage(format!("导出失败: {}", e)))
    }

    async fn import(&self, rows: Vec<Vec<String>>) -> Result<String, UseCaseError> {
        if rows.len() < 2 {
            return Err(UseCaseError::BadRequest("文件至少需要包含标题行和一行数据".to_string()));
        }
        let mut success = 0;
        let mut failed = 0;
        for row in rows.iter().skip(1) {
            if row.len() < 2 {
                failed += 1;
                continue;
            }
            let name = row[1].trim();
            if name.is_empty() {
                failed += 1;
                continue;
            }
            let category_name = if row.len() > 7 { row[7].trim() } else { "" };
            // 行为保持：分类查询失败视为 None（旧 .ok().flatten()）
            let category_id = if !category_name.is_empty() {
                self.repo.find_category_id(category_name, "purchaser").await.unwrap_or(None)
            } else {
                None
            };
            let item = ImportPurchaser {
                name: name.to_string(),
                contact: if row.len() > 2 { row[2].trim().to_string() } else { String::new() },
                phone: if row.len() > 3 { row[3].trim().to_string() } else { String::new() },
                address: if row.len() > 4 { row[4].trim().to_string() } else { String::new() },
                business_scope: if row.len() > 5 { row[5].trim().to_string() } else { String::new() },
                remark: if row.len() > 6 { row[6].trim().to_string() } else { String::new() },
                category_id,
            };
            match self.repo.import_row(&item).await {
                Ok(inserted) => {
                    if inserted {
                        success += 1;
                    } else {
                        failed += 1;
                    }
                }
                Err(_) => {
                    failed += 1;
                }
            }
        }
        Ok(format!("导入完成：成功 {} 条，失败 {} 条", success, failed))
    }

    async fn create(&self, cmd: UpsertPurchaser) -> Result<String, UseCaseError> {
        match self.repo.create(&cmd).await {
            Ok(()) => Ok("创建成功".to_string()),
            Err(_) => Err(UseCaseError::Storage("创建失败".to_string())),
        }
    }

    async fn update(&self, id: i64, cmd: UpsertPurchaser) -> Result<String, UseCaseError> {
        self.ensure_not_confirmed(id).await?;
        match self.repo.update(id, &cmd).await {
            Ok(()) => Ok("更新成功".to_string()),
            Err(_) => Err(UseCaseError::Storage("更新失败".to_string())),
        }
    }

    async fn delete(&self, id: i64) -> Result<String, UseCaseError> {
        self.ensure_not_confirmed(id).await?;
        match self.repo.delete(id).await {
            Ok(()) => Ok("删除成功".to_string()),
            Err(_) => Err(UseCaseError::Storage("删除失败".to_string())),
        }
    }
}

/// 导出工作簿（自旧 api_purchaser_export 移植：列序/列宽/表头逐字节一致）
fn build_export_workbook(rows: &[PurchaserRow]) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    use rust_xlsxwriter::{Format, FormatAlign, Workbook};

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_format = Format::new()
        .set_bold()
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);

    let headers = ["ID", "名称", "联系人", "电话", "地址", "经营范围", "备注", "分类"];
    for (i, &header) in headers.iter().enumerate() {
        worksheet.write_with_format(0, i as u16, header, &header_format)?;
    }

    let mut row_idx = 1;
    for row in rows {
        worksheet.write(row_idx, 0, row.id)?;
        worksheet.write(row_idx, 1, &row.name)?;
        worksheet.write(row_idx, 2, row.contact.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 3, row.phone.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 4, row.address.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 5, row.business_scope.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 6, row.remark.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 7, row.category_name.clone().unwrap_or_default())?;
        row_idx += 1;
    }

    worksheet.set_column_width(0, 8)?;
    worksheet.set_column_width(1, 18)?;
    worksheet.set_column_width(2, 12)?;
    worksheet.set_column_width(3, 15)?;
    worksheet.set_column_width(4, 25)?;
    worksheet.set_column_width(5, 20)?;
    worksheet.set_column_width(6, 20)?;
    worksheet.set_column_width(7, 12)?;

    Ok(workbook.save_to_buffer()?)
}
