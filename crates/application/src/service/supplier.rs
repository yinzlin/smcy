//! 供应商用例：列表 / 导出 / 导入 / 创建 / 更新 / 删除
//!
//! 行为保持：查询类吞错兜底、错误文案逐字节复刻旧 handler。

use domain::port::supplier::{ImportSupplier, SupplierFilter, SupplierRepo, SupplierRow, UpsertSupplier};

use crate::error::UseCaseError;
use crate::port::supplier::SupplierUseCase;

#[derive(Clone)]
pub struct SupplierService<R: SupplierRepo> {
    repo: R,
}

impl<R: SupplierRepo> SupplierService<R> {
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

impl<R: SupplierRepo> SupplierUseCase for SupplierService<R> {
    async fn list(&self, category_id: Option<i64>, keyword_pattern: String) -> Vec<SupplierRow> {
        // 行为保持：查询失败返回空数组（旧 unwrap_or_default）
        self.repo
            .list(&SupplierFilter { category_id, keyword_pattern })
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
                self.repo.find_category_id(category_name, "supplier").await.unwrap_or(None)
            } else {
                None
            };
            let item = ImportSupplier {
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

    async fn create(&self, cmd: UpsertSupplier) -> Result<String, UseCaseError> {
        match self.repo.create(&cmd).await {
            Ok(()) => Ok("创建成功".to_string()),
            Err(_) => Err(UseCaseError::Storage("创建失败".to_string())),
        }
    }

    async fn update(&self, id: i64, cmd: UpsertSupplier) -> Result<String, UseCaseError> {
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

/// 导出工作簿（自旧 api_supplier_export 移植：列序/列宽/表头逐字节一致；
/// 前 8 列顺序固定不可调整——Excel 导入按列索引解析）
fn build_export_workbook(rows: &[SupplierRow]) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    use rust_xlsxwriter::{Format, FormatAlign, Workbook};

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_format = Format::new()
        .set_bold()
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);

    let headers = ["ID", "名称", "联系人", "电话", "地址", "经营范围", "备注", "分类",
                   "定点供应商", "定点编号", "负责人", "统一社会信用代码", "许可证号", "许可证有效期至",
                   "供货合同编号", "合同开始日期", "合同结束日期"];
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
        worksheet.write(row_idx, 8, if row.is_designated == 1 { "是" } else { "否" })?;
        worksheet.write(row_idx, 9, row.designated_no.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 10, row.legal_person.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 11, row.credit_code.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 12, row.license_no.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 13, row.license_expire.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 14, row.contract_no.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 15, row.contract_start.clone().unwrap_or_default())?;
        worksheet.write(row_idx, 16, row.contract_end.clone().unwrap_or_default())?;
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
    worksheet.set_column_width(8, 10)?;
    worksheet.set_column_width(9, 14)?;
    worksheet.set_column_width(10, 10)?;
    worksheet.set_column_width(11, 22)?;
    worksheet.set_column_width(12, 20)?;
    worksheet.set_column_width(13, 14)?;
    worksheet.set_column_width(14, 16)?;
    worksheet.set_column_width(15, 14)?;
    worksheet.set_column_width(16, 14)?;

    Ok(workbook.save_to_buffer()?)
}
