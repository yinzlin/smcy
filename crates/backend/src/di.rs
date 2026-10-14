//! 依赖装配：初始化数据库连接池，执行启动期数据修复，构造六边形 App
//!
//! App 按域持有用例服务（泛型单态化，无 dyn）；handler 从 State<App>
//! 取用例。database::pool() 的全局访问最终仅保留此处一处。

use application::service::user::UserService;
use database::sqlite::repo::user::SqliteUserRepo;

/// 应用容器：按业务域聚合全部用例服务（axum State 要求 Clone，池为 Arc 克隆零成本）
#[derive(Clone)]
pub struct App {
    pub user: UserService<SqliteUserRepo>,
}

/// 初始化数据库；如有待重建的 price_schedule 时段则补跑重建，完成后升级 user_version
pub async fn bootstrap() -> App {
    if let Some(pending) = database::init_pool().await {
        for (product_id, price_type) in pending {
            let _ = application::service::price::rebuild_price_schedule_end_dates(product_id, &price_type).await;
        }
        database::finalize_schedule_rebuild().await;
    }
    App {
        user: UserService::new(SqliteUserRepo::new(database::pool().clone())),
    }
}
