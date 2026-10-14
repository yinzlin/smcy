//! 遗留散装函数（原 service/user.rs，仅剩 get_user_by_id 一项），
//! 待验收单导出切片迁移后删除。

use database::*;

pub struct UserSimple { pub nickname: String, pub phone: String }

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
