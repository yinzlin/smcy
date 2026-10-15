//! user_account 仓储实现：SQL 与旧 api_user_* / api_login handler 逐字一致

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use domain::error::DomainError;
use domain::port::user::{
    ActiveUserRow, LoginRow, NewUser, SessionCtxRow, SessionUserRow, UpdateUser, UserDetailRow,
    UserRepo,
};

#[derive(Clone)]
pub struct SqliteUserRepo {
    pool: SqlitePool,
}

impl SqliteUserRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(e: sqlx::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

impl UserRepo for SqliteUserRepo {
    async fn list_active(&self) -> Result<Vec<ActiveUserRow>, DomainError> {
        let rows = sqlx::query(
            "SELECT id, username, nickname, phone, role, status, COALESCE(supplier_id,0) as supplier_id, COALESCE(purchaser_id,0) as purchaser_id FROM user_account WHERE status = 1 ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| ActiveUserRow {
                id: r.get("id"),
                username: r.get("username"),
                nickname: r.get::<Option<String>, _>("nickname").unwrap_or_default(),
                phone: r.get::<Option<String>, _>("phone").unwrap_or_default(),
                role: r.get("role"),
                supplier_id: r.get("supplier_id"),
                purchaser_id: r.get("purchaser_id"),
            })
            .collect())
    }

    async fn get_detail(&self, id: i64) -> Result<Option<UserDetailRow>, DomainError> {
        sqlx::query(
            "SELECT id, username, nickname, role, status, COALESCE(supplier_id,0) as supplier_id, COALESCE(purchaser_id,0) as purchaser_id FROM user_account WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .map(|r| {
            Ok(UserDetailRow {
                id: r.get("id"),
                username: r.get("username"),
                nickname: r.get("nickname"),
                role: r.get("role"),
                status: r.get("status"),
                supplier_id: r.get("supplier_id"),
                purchaser_id: r.get("purchaser_id"),
            })
        })
        .transpose()
    }

    async fn username_exists(&self, username: &str) -> Result<bool, DomainError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_account WHERE username = ?)")
            .bind(username)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    async fn create(&self, user: &NewUser) -> Result<(), DomainError> {
        sqlx::query("INSERT INTO user_account (username, password, nickname, role, supplier_id, purchaser_id) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&user.username)
            .bind(&user.password_hash)
            .bind(&user.nickname)
            .bind(&user.role)
            .bind(user.supplier_id)
            .bind(user.purchaser_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn update_password(&self, id: i64, password_hash: &str) -> Result<(), DomainError> {
        sqlx::query("UPDATE user_account SET password = ? WHERE id = ?")
            .bind(password_hash)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn update_profile(&self, user: &UpdateUser) -> Result<(), DomainError> {
        if user.role.is_empty() {
            sqlx::query("UPDATE user_account SET username = ?, nickname = ?, supplier_id = ?, purchaser_id = ?, update_at = datetime('now','localtime') WHERE id = ?")
                .bind(&user.username)
                .bind(&user.nickname)
                .bind(user.supplier_id)
                .bind(user.purchaser_id)
                .bind(user.id)
                .execute(&self.pool)
                .await
                .map_err(storage)?;
        } else {
            sqlx::query("UPDATE user_account SET username = ?, nickname = ?, role = ?, supplier_id = ?, purchaser_id = ?, update_at = datetime('now','localtime') WHERE id = ?")
                .bind(&user.username)
                .bind(&user.nickname)
                .bind(&user.role)
                .bind(user.supplier_id)
                .bind(user.purchaser_id)
                .bind(user.id)
                .execute(&self.pool)
                .await
                .map_err(storage)?;
        }
        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM user_account WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn set_status(&self, id: i64, status: i64) -> Result<(), DomainError> {
        sqlx::query("UPDATE user_account SET status = ?, update_at = datetime('now','localtime') WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn get_contact_phone(&self, id: i64) -> Result<String, DomainError> {
        sqlx::query("SELECT COALESCE(contact_phone, '') as contact_phone FROM user_account WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| r.try_get("contact_phone").unwrap_or_default())
            .ok_or(DomainError::NotFound("用户不存在"))
    }

    async fn set_contact_phone(&self, id: i64, phone: &str) -> Result<(), DomainError> {
        sqlx::query("UPDATE user_account SET contact_phone = ? WHERE id = ?")
            .bind(phone)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn find_login(&self, username: &str) -> Result<Option<LoginRow>, DomainError> {
        sqlx::query("SELECT id, username, password, nickname, role, status FROM user_account WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| {
                Ok(LoginRow {
                    id: r.get("id"),
                    password: r.get("password"),
                    status: r.get("status"),
                    nickname: r.get("nickname"),
                    role: r.get("role"),
                })
            })
            .transpose()
    }

    async fn touch_last_login(&self, id: i64) -> Result<(), DomainError> {
        sqlx::query("UPDATE user_account SET last_login_time = datetime('now','localtime') WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn find_active_by_id(&self, id: i64) -> Result<Option<SessionUserRow>, DomainError> {
        sqlx::query("SELECT id, username, nickname, role FROM user_account WHERE id = ? AND status = 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| {
                Ok(SessionUserRow {
                    username: r.get("username"),
                    nickname: r.get("nickname"),
                    role: r.get("role"),
                })
            })
            .transpose()
    }

    async fn session_ctx(&self, id: i64) -> Result<Option<SessionCtxRow>, DomainError> {
        sqlx::query(
            "SELECT role, COALESCE(supplier_id,0) as supplier_id, COALESCE(purchaser_id,0) as purchaser_id FROM user_account WHERE id = ? AND status = 1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .map(|r| {
            Ok(SessionCtxRow {
                role: r.get("role"),
                supplier_id: r.get("supplier_id"),
                purchaser_id: r.get("purchaser_id"),
            })
        })
        .transpose()
    }
}
