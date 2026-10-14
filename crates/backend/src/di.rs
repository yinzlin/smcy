//! 依赖装配：初始化数据库连接池，执行启动期数据修复，构造六边形 App
//!
//! App 按域持有用例服务（泛型单态化，无 dyn）；handler 从 State<App>
//! 取用例。database::pool() 的全局访问最终仅保留此处一处。

use application::service::basic_audit::BasicAuditService;
use application::service::category::CategoryService;
use application::service::purchaser::PurchaserService;
use application::service::supplier::SupplierService;
use application::service::user::UserService;
use application::service::warehouse::WarehouseService;
use database::sqlite::repo::audit::SqliteBasicAuditRepo;
use database::sqlite::repo::category::SqliteCategoryRepo;
use database::sqlite::repo::oplog::SqliteOperationLogRepo;
use database::sqlite::repo::purchaser::SqlitePurchaserRepo;
use database::sqlite::repo::supplier::SqliteSupplierRepo;
use database::sqlite::repo::user::SqliteUserRepo;
use database::sqlite::repo::warehouse::SqliteWarehouseRepo;

/// 应用容器：按业务域聚合全部用例服务（axum State 要求 Clone，池为 Arc 克隆零成本）
#[derive(Clone)]
pub struct App {
    pub user: UserService<SqliteUserRepo>,
    pub category: CategoryService<SqliteCategoryRepo>,
    pub warehouse: WarehouseService<SqliteWarehouseRepo>,
    pub audit: BasicAuditService<SqliteBasicAuditRepo, SqliteOperationLogRepo>,
    pub supplier: SupplierService<SqliteSupplierRepo>,
    pub purchaser: PurchaserService<SqlitePurchaserRepo>,
}

/// 初始化数据库；如有待重建的 price_schedule 时段则补跑重建，完成后升级 user_version
pub async fn bootstrap() -> App {
    if let Some(pending) = database::init_pool().await {
        for (product_id, price_type) in pending {
            let _ = application::service::price::rebuild_price_schedule_end_dates(product_id, &price_type).await;
        }
        database::finalize_schedule_rebuild().await;
    }
    let oplog = SqliteOperationLogRepo::new(database::pool().clone());
    App {
        user: UserService::new(SqliteUserRepo::new(database::pool().clone())),
        category: CategoryService::new(SqliteCategoryRepo::new(database::pool().clone())),
        warehouse: WarehouseService::new(SqliteWarehouseRepo::new(database::pool().clone())),
        audit: BasicAuditService::new(SqliteBasicAuditRepo::new(database::pool().clone()), oplog),
        supplier: SupplierService::new(SqliteSupplierRepo::new(database::pool().clone())),
        purchaser: PurchaserService::new(SqlitePurchaserRepo::new(database::pool().clone())),
    }
}
