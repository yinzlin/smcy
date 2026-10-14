//! 分类用例：列表 / 树 / 创建 / 重命名 / 删除
//!
//! 行为保持：旧 handler 对查询类 SQL 失败采取吞错兜底（unwrap_or_default /
//! unwrap_or(0)），此处逐处复刻并注释。

use domain::port::category::{CategoryRepo, CategoryRow, NewCategory};

use crate::error::UseCaseError;
use crate::port::category::CategoryUseCase;

#[derive(Clone)]
pub struct CategoryService<R: CategoryRepo> {
    repo: R,
}

impl<R: CategoryRepo> CategoryService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: CategoryRepo> CategoryUseCase for CategoryService<R> {
    async fn list(&self) -> Vec<CategoryRow> {
        // 行为保持：查询失败返回空数组（旧 unwrap_or_default）
        self.repo.list_all().await.unwrap_or_default()
    }

    async fn tree(&self, entity_type: &str) -> Vec<serde_json::Value> {
        // 行为保持：查询失败返回空树（旧 unwrap_or_default）
        let rows = self.repo.list_for_tree().await.unwrap_or_default();
        build_tree(&rows, None, entity_type)
    }

    async fn create(
        &self,
        name: String,
        parent_id: Option<i64>,
        entity_type: String,
        sort_order: Option<i32>,
    ) -> Result<(), UseCaseError> {
        let cmd = NewCategory {
            name,
            parent_id,
            entity_type,
            sort_order: sort_order.unwrap_or(0),
        };
        match self.repo.create(&cmd).await {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("创建分类失败: {}", e);
                // 行为保持：旧错误路径仅返回 500 空响应体
                Err(UseCaseError::Storage(String::new()))
            }
        }
    }

    async fn rename(&self, id: i64, name: String) -> Result<String, UseCaseError> {
        match self.repo.rename(id, &name).await {
            Ok(()) => Ok("重命名成功".to_string()),
            Err(e) => {
                eprintln!("重命名分类失败: {}", e);
                Err(UseCaseError::Storage("重命名失败".to_string()))
            }
        }
    }

    async fn delete(&self, id: i64) -> Result<String, UseCaseError> {
        // 行为保持：计数查询失败视为 0 继续执行（旧 unwrap_or(0)）
        let child_count = self.repo.count_children(id).await.unwrap_or(0);
        if child_count > 0 {
            return Err(UseCaseError::BadRequest("该分类下有子分类，无法删除".to_string()));
        }
        for (table, count) in [
            ("supplier", self.repo.count_supplier_refs(id).await.unwrap_or(0)),
            ("purchaser", self.repo.count_purchaser_refs(id).await.unwrap_or(0)),
            ("product", self.repo.count_product_refs(id).await.unwrap_or(0)),
        ] {
            if count > 0 {
                return Err(UseCaseError::BadRequest(format!("该分类已被{}引用，无法删除", table)));
            }
        }
        match self.repo.delete(id).await {
            Ok(()) => Ok("删除成功".to_string()),
            Err(_) => Err(UseCaseError::Storage("删除失败".to_string())),
        }
    }
}

/// 分类树构建（自 common::utils::build_category_tree_json 移植，
/// 输入从 SqliteRow 换为 CategoryRow，JSON 字段顺序与逐层排序逻辑逐字保持）
fn build_tree(rows: &[CategoryRow], parent_id: Option<i64>, entity_type: &str) -> Vec<serde_json::Value> {
    let mut result = vec![];
    for row in rows {
        if row.entity_type != entity_type {
            continue;
        }
        if row.parent_id != parent_id {
            continue;
        }
        let id = row.id;
        let children = build_tree(rows, Some(id), entity_type);
        result.push(serde_json::json!({
            "id": id,
            "name": row.name,
            "parent_id": row.parent_id,
            "sort_order": row.sort_order,
            "children": children,
        }));
    }
    result.sort_by_key(|x| (x["sort_order"].as_i64().unwrap_or(0), x["id"].as_i64().unwrap_or(0)));
    result
}
