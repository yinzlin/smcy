//! 依赖装配：初始化数据库连接池，执行启动期数据修复，构造六边形 App
//!
//! App 按域持有用例服务（泛型单态化，无 dyn）；handler 从 State<App>
//! 取用例。database::pool() 的全局访问最终仅保留此处一处。

use application::service::basic_audit::BasicAuditService;
use application::service::category::CategoryService;
use application::service::food_accept::FoodAcceptService;
use application::service::inventory::InventoryService;
use application::service::order_allocation::OrderAllocationService;
use application::service::order_supplement::OrderSupplementService;
use application::service::price_schedule::PriceScheduleService;
use application::service::product::ProductService;
use application::service::purchase_document::PurchaseDocumentService;
use application::service::purchase_order::PurchaseOrderService;
use application::service::purchaser::PurchaserService;
use application::service::query_finance::QueryFinanceService;
use application::service::query_purchase::QueryPurchaseService;
use application::service::query_sales::QuerySalesService;
use application::service::query_stats::QueryStatsService;
use application::service::query_stock::QueryStockService;
use application::service::sales_order::SalesOrderService;
use application::service::supplier::SupplierService;
use application::service::system_mgmt::SystemMgmtService;
use application::service::today_price::TodayPriceService;
use application::service::user::UserService;
use application::service::warehouse::WarehouseService;
use database::sqlite::repo::audit::SqliteBasicAuditRepo;
use database::sqlite::repo::category::SqliteCategoryRepo;
use database::sqlite::repo::food_accept::SqliteFoodAcceptRepo;
use database::sqlite::repo::inventory::SqliteInventoryRepo;
use database::sqlite::repo::oplog::SqliteOperationLogRepo;
use database::sqlite::repo::order_allocation::SqliteOrderAllocationRepo;
use database::sqlite::repo::order_supplement::SqliteOrderSupplementRepo;
use database::sqlite::repo::price_schedule::SqlitePriceScheduleRepo;
use database::sqlite::repo::product::SqliteProductRepo;
use database::sqlite::repo::purchase_document::SqlitePurchaseDocumentRepo;
use database::sqlite::repo::purchase_order::SqlitePurchaseOrderRepo;
use database::sqlite::repo::purchaser::SqlitePurchaserRepo;
use database::sqlite::repo::query_finance::SqliteQueryFinanceRepo;
use database::sqlite::repo::query_purchase::SqliteQueryPurchaseRepo;
use database::sqlite::repo::query_sales::SqliteQuerySalesRepo;
use database::sqlite::repo::query_stats::SqliteQueryStatsRepo;
use database::sqlite::repo::query_stock::SqliteQueryStockRepo;
use database::sqlite::repo::sales_order::SqliteSalesOrderRepo;
use database::sqlite::repo::supplier::SqliteSupplierRepo;
use database::sqlite::repo::system_mgmt::SqliteSystemMgmtRepo;
use database::sqlite::repo::today_price::SqliteTodayPriceRepo;
use database::sqlite::repo::user::SqliteUserRepo;
use database::sqlite::repo::warehouse::SqliteWarehouseRepo;
use domain::port::product::{ProductImageStore, RemoteImageStore};

/// 远程图片转存基础设施适配器：导入商品时下载网络图片到本地 uploads（复用旧实现）
#[derive(Clone, Copy)]
pub struct BackendImageStore;

impl RemoteImageStore for BackendImageStore {
    async fn save_remote(&self, url: &str, name_prefix: &str) -> Result<String, String> {
        crate::api::handlers::save_remote_image_to_local(url, name_prefix).await
    }
}

impl ProductImageStore for BackendImageStore {
    async fn save_upload(&self, name_prefix: &str, ext: &str, bytes: &[u8]) -> Result<String, String> {
        // 行为保持：目录创建失败吞错，继续走写文件分支（写失败才报"保存图片失败"）
        tokio::fs::create_dir_all("uploads/products").await.ok();
        let timestamp = chrono::Utc::now().timestamp_millis();
        let random: u32 = rand::random();
        let new_filename = format!("{}_{}_{}.{}", name_prefix, timestamp, random, ext);
        let path = format!("uploads/products/{}", new_filename);
        if tokio::fs::write(&path, bytes).await.is_err() {
            return Err(String::new()); // 用例层映射为"保存图片失败"
        }
        Ok(format!("/api/uploads/products/{}", new_filename))
    }

    async fn remove_by_url(&self, url: &str) {
        // 行为保持：解析失败/删除失败均吞错
        if let Some(path) = common::utils::image_url_to_path(url) {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        tokio::fs::read(path).await.map_err(|e| e.to_string())
    }
}

/// 应用容器：按业务域聚合全部用例服务（axum State 要求 Clone，池为 Arc 克隆零成本）
///
/// `user_repo` / `oplog_repo` / `sales_order_repo` 为中间件（如 auth.rs 的会话解析、
/// 操作日志、行级数据权限）共享的仓储句柄：中间件不得直连 pool，一律经端口取数。
#[derive(Clone)]
pub struct App {
    pub user_repo: SqliteUserRepo,
    pub oplog_repo: SqliteOperationLogRepo,
    pub sales_order_repo: SqliteSalesOrderRepo,
    pub user: UserService<SqliteUserRepo>,
    pub category: CategoryService<SqliteCategoryRepo>,
    pub warehouse: WarehouseService<SqliteWarehouseRepo>,
    pub audit: BasicAuditService<SqliteBasicAuditRepo, SqliteOperationLogRepo>,
    pub supplier: SupplierService<SqliteSupplierRepo>,
    pub purchaser: PurchaserService<SqlitePurchaserRepo>,
    pub product: ProductService<SqliteProductRepo, BackendImageStore>,
    pub purchase_document: PurchaseDocumentService<SqlitePurchaseDocumentRepo>,
    pub today_price: TodayPriceService<SqliteTodayPriceRepo>,
    pub price_schedule: PriceScheduleService<SqlitePriceScheduleRepo>,
    pub inventory: InventoryService<SqliteInventoryRepo>,
    pub purchase_order: PurchaseOrderService<SqlitePurchaseOrderRepo>,
    pub sales_order: SalesOrderService<SqliteSalesOrderRepo>,
    pub order_allocation: OrderAllocationService<SqliteOrderAllocationRepo>,
    pub order_supplement: OrderSupplementService<SqliteOrderSupplementRepo>,
    pub query_finance: QueryFinanceService<SqliteQueryFinanceRepo>,
    pub query_purchase: QueryPurchaseService<SqliteQueryPurchaseRepo>,
    pub query_sales: QuerySalesService<SqliteQuerySalesRepo>,
    pub query_stats: QueryStatsService<SqliteQueryStatsRepo>,
    pub query_stock: QueryStockService<SqliteQueryStockRepo>,
    pub system_mgmt: SystemMgmtService<SqliteSystemMgmtRepo>,
    pub food_accept: FoodAcceptService<SqliteFoodAcceptRepo>,
}

/// 初始化数据库；如有待重建的 price_schedule 时段则补跑重建，完成后升级 user_version
pub async fn bootstrap() -> App {
    if let Some(pending) = database::init_pool().await {
        for (product_id, price_type) in pending {
            let _ = application::service::price::rebuild_price_schedule_end_dates(product_id, &price_type).await;
        }
        database::finalize_schedule_rebuild().await;
    }
    let pool = database::pool().clone();
    let user_repo = SqliteUserRepo::new(pool.clone());
    let oplog_repo = SqliteOperationLogRepo::new(pool.clone());
    let sales_order_repo = SqliteSalesOrderRepo::new(pool.clone());
    App {
        user_repo: user_repo.clone(),
        oplog_repo: oplog_repo.clone(),
        sales_order_repo: sales_order_repo.clone(),
        user: UserService::new(user_repo.clone()),
        category: CategoryService::new(SqliteCategoryRepo::new(pool.clone())),
        warehouse: WarehouseService::new(SqliteWarehouseRepo::new(pool.clone())),
        audit: BasicAuditService::new(SqliteBasicAuditRepo::new(pool.clone()), oplog_repo.clone()),
        supplier: SupplierService::new(SqliteSupplierRepo::new(pool.clone())),
        purchaser: PurchaserService::new(SqlitePurchaserRepo::new(pool.clone())),
        product: ProductService::new(
            SqliteProductRepo::new(pool.clone()),
            BackendImageStore,
        ),
        purchase_document: PurchaseDocumentService::new(SqlitePurchaseDocumentRepo::new(pool.clone())),
        today_price: TodayPriceService::new(SqliteTodayPriceRepo::new(pool.clone())),
        price_schedule: PriceScheduleService::new(SqlitePriceScheduleRepo::new(pool.clone())),
        inventory: InventoryService::new(SqliteInventoryRepo::new(pool.clone())),
        purchase_order: PurchaseOrderService::new(SqlitePurchaseOrderRepo::new(pool.clone())),
        sales_order: SalesOrderService::new(sales_order_repo.clone()),
        order_allocation: OrderAllocationService::new(SqliteOrderAllocationRepo::new(pool.clone())),
        order_supplement: OrderSupplementService::new(SqliteOrderSupplementRepo::new(pool.clone())),
        query_finance: QueryFinanceService::new(SqliteQueryFinanceRepo::new(pool.clone())),
        query_purchase: QueryPurchaseService::new(SqliteQueryPurchaseRepo::new(pool.clone())),
        query_sales: QuerySalesService::new(SqliteQuerySalesRepo::new(pool.clone())),
        query_stats: QueryStatsService::new(SqliteQueryStatsRepo::new(pool.clone())),
        query_stock: QueryStockService::new(SqliteQueryStockRepo::new(pool.clone())),
        system_mgmt: SystemMgmtService::new(SqliteSystemMgmtRepo::new(pool.clone())),
        food_accept: FoodAcceptService::new(SqliteFoodAcceptRepo::new(pool.clone())),
    }
}
