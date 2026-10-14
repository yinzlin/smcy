//! 仓库用例：列表 / 创建 / 更新 / 删除
//!
//! 行为保持：审核锁定校验与库存计数查询失败时的吞错兜底逐处复刻。

use domain::error::DomainError;
use domain::port::warehouse::{NewWarehouse, UpdateWarehouse, WarehouseRepo, WarehouseRow};

use crate::error::UseCaseError;
use crate::port::warehouse::WarehouseUseCase;

#[derive(Clone)]
pub struct WarehouseService<R: WarehouseRepo> {
    repo: R,
}

impl<R: WarehouseRepo> WarehouseService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// 行为保持：旧 check_basic_not_confirmed 对状态查询失败 unwrap_or(None)
    /// → 按"记录不存在"处理；且要求 id > 0。
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

impl<R: WarehouseRepo> WarehouseUseCase for WarehouseService<R> {
    async fn list(&self) -> Vec<WarehouseRow> {
        // 行为保持：查询失败返回空数组（旧 unwrap_or_default）
        self.repo.list_all().await.unwrap_or_default()
    }

    async fn create(
        &self,
        name: String,
        code: Option<String>,
        address: Option<String>,
        contact: Option<String>,
        phone: Option<String>,
        sort_order: Option<i32>,
    ) -> Result<String, UseCaseError> {
        let cmd = NewWarehouse {
            name,
            code,
            address,
            contact,
            phone,
            sort_order: sort_order.unwrap_or(0),
        };
        match self.repo.create(&cmd).await {
            Ok(()) => Ok("创建成功".to_string()),
            Err(DomainError::Conflict(_)) => {
                Err(UseCaseError::BadRequest("仓库名称或编号已存在".to_string()))
            }
            Err(e) => {
                eprintln!("创建仓库失败: {}", e);
                Err(UseCaseError::Storage("创建失败".to_string()))
            }
        }
    }

    async fn update(
        &self,
        id: i64,
        name: String,
        code: Option<String>,
        address: Option<String>,
        contact: Option<String>,
        phone: Option<String>,
        status: Option<i32>,
        sort_order: Option<i32>,
    ) -> Result<String, UseCaseError> {
        self.ensure_not_confirmed(id).await?;
        let cmd = UpdateWarehouse {
            id,
            name,
            code,
            address,
            contact,
            phone,
            status: status.unwrap_or(1),
            sort_order: sort_order.unwrap_or(0),
        };
        match self.repo.update(&cmd).await {
            Ok(()) => Ok("更新成功".to_string()),
            Err(DomainError::Conflict(_)) => {
                Err(UseCaseError::BadRequest("仓库名称或编号已存在".to_string()))
            }
            Err(e) => {
                eprintln!("更新仓库失败: {}", e);
                Err(UseCaseError::Storage("更新失败".to_string()))
            }
        }
    }

    async fn delete(&self, id: i64) -> Result<String, UseCaseError> {
        if id == 1 {
            return Err(UseCaseError::BadRequest("默认仓库无法删除".to_string()));
        }
        self.ensure_not_confirmed(id).await?;
        // 行为保持：库存计数查询失败视为 0 继续执行（旧 unwrap_or(0)）
        let count = self.repo.count_inventory(id).await.unwrap_or(0);
        if count > 0 {
            return Err(UseCaseError::BadRequest("该仓库存在库存记录，无法删除".to_string()));
        }
        match self.repo.delete(id).await {
            Ok(()) => Ok("删除成功".to_string()),
            Err(_) => Err(UseCaseError::Storage("删除失败".to_string())),
        }
    }
}
