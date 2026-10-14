//! 依赖装配：初始化数据库连接池，并执行启动期数据修复

/// 初始化数据库；如有待重建的 price_schedule 时段则补跑重建，完成后升级 user_version
pub async fn bootstrap() {
    if let Some(pending) = database::init_pool().await {
        for (product_id, price_type) in pending {
            let _ = application::service::price::rebuild_price_schedule_end_dates(product_id, &price_type).await;
        }
        database::finalize_schedule_rebuild().await;
    }
}
