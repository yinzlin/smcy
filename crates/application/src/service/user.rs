
use database::*;
use super::*;

pub async fn get_user_by_id(id: i64) -> Option<UserSimple> {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT nickname, COALESCE(NULLIF(phone, ''), '') as phone FROM user_account WHERE id = ?"
    )
        .bind(id)
        .fetch_optional(pool())
        .await
        .ok()
        .flatten()
        .map(|(nickname, phone)| UserSimple { nickname, phone: phone.unwrap_or_default() })
}




// 一键获取订单明细的最新售价（不写入数据库），返回给前端供用户手动保存












// 商品价格同期走势：返回指定商品在时间段内的进价点（来自采购单实际成交价）和售价点（来自价格变更日志）
// 不应用尾数取整规则——历史原值



// 报销口径汇总：目标单真实明细 + 分摊增项净额，排除耗材分摊来源单本身，避免重计

// 分摊来源统计：列出所有作为分摊来源的订单及其分摊去向








// === 补全缺失的查询 API ===
