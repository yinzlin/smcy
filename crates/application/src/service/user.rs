//! 用户域用例：注册/登录/会话/启停/联系方式回填
//!
//! 编排 UserRepo + bcrypt；中文文案与失败吞错行为均与旧 handler 逐字
//! 保持（标注「行为保持」处为旧实现的防御性吞错，维持响应兼容）。

use bcrypt;
use rand;

use domain::port::user::{ActiveUserRow, NewUser, UpdateUser, UserRepo};

use crate::dto::user::{
    ApiMessage, ContactPhoneResp, CreateUserCmd, LoginCheckResp, LoginOutcome, LoginResp,
    LoginUser, UpdateUserCmd, UserDetail, UserDetailResp,
};
use crate::error::UseCaseError;
use crate::port::user::UserUseCase;

#[derive(Clone)]
pub struct UserService<R: UserRepo> {
    repo: R,
}

impl<R: UserRepo> UserService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: UserRepo> UserUseCase for UserService<R> {
    async fn list_active(&self) -> Result<Vec<ActiveUserRow>, UseCaseError> {
        // 行为保持：旧实现查询失败返回空列表
        Ok(self.repo.list_active().await.unwrap_or_default())
    }

    async fn get_detail(&self, id: i64) -> Result<UserDetailResp, UseCaseError> {
        // 行为保持：查询失败视同用户不存在
        let row = self.repo.get_detail(id).await.unwrap_or(None);
        Ok(match row {
            Some(r) => UserDetailResp {
                success: true,
                message: None,
                user: Some(UserDetail {
                    id: r.id,
                    username: r.username,
                    nickname: r.nickname,
                    role: r.role,
                    status: r.status,
                    supplier_id: r.supplier_id,
                    purchaser_id: r.purchaser_id,
                }),
            },
            None => UserDetailResp {
                success: false,
                message: Some("用户不存在".into()),
                user: None,
            },
        })
    }

    async fn get_contact_phone(&self, user_id: i64) -> Result<ContactPhoneResp, UseCaseError> {
        // 行为保持：查询失败返回空串
        let phone = self.repo.get_contact_phone(user_id).await.unwrap_or_default();
        Ok(ContactPhoneResp { success: true, contact_phone: phone })
    }

    async fn set_contact_phone(&self, user_id: i64, phone: String) -> Result<ContactPhoneResp, UseCaseError> {
        if user_id <= 0 {
            return Err(UseCaseError::Unauthorized("未登录".into()));
        }
        // 行为保持：更新失败仍返回成功
        let _ = self.repo.set_contact_phone(user_id, &phone).await;
        Ok(ContactPhoneResp { success: true, contact_phone: phone })
    }

    async fn create(&self, cmd: CreateUserCmd) -> Result<ApiMessage, UseCaseError> {
        if cmd.username.is_empty() {
            return Ok(ApiMessage { success: false, message: "用户名不能为空".into() });
        }
        if cmd.password.is_empty() {
            return Ok(ApiMessage { success: false, message: "密码不能为空".into() });
        }
        // 行为保持：存在性查询失败按不存在处理
        let exists = self.repo.username_exists(&cmd.username).await.unwrap_or(false);
        if exists {
            return Ok(ApiMessage { success: false, message: "用户名已存在".into() });
        }
        let password_hash = bcrypt::hash(&cmd.password, bcrypt::DEFAULT_COST).unwrap();
        // 行为保持：插入失败仍返回创建成功（旧实现 .ok() 吞错）
        let _ = self
            .repo
            .create(&NewUser {
                username: cmd.username,
                password_hash,
                nickname: cmd.nickname,
                role: cmd.role,
                supplier_id: cmd.supplier_id,
                purchaser_id: cmd.purchaser_id,
            })
            .await;
        Ok(ApiMessage { success: true, message: "用户创建成功".into() })
    }

    async fn update(&self, cmd: UpdateUserCmd) -> Result<ApiMessage, UseCaseError> {
        if cmd.username.is_empty() {
            return Ok(ApiMessage { success: false, message: "用户名不能为空".into() });
        }
        if !cmd.password.is_empty() {
            let password_hash = bcrypt::hash(&cmd.password, bcrypt::DEFAULT_COST).unwrap();
            // 行为保持：密码更新失败仍继续
            let _ = self.repo.update_password(cmd.id, &password_hash).await;
        }
        // 行为保持：资料更新失败仍返回成功
        let _ = self
            .repo
            .update_profile(&UpdateUser {
                id: cmd.id,
                username: cmd.username,
                nickname: cmd.nickname,
                role: cmd.role,
                supplier_id: cmd.supplier_id,
                purchaser_id: cmd.purchaser_id,
            })
            .await;
        Ok(ApiMessage { success: true, message: "用户更新成功".into() })
    }

    async fn delete(&self, id: i64) -> Result<ApiMessage, UseCaseError> {
        // 行为保持：删除失败仍返回成功
        let _ = self.repo.delete(id).await;
        Ok(ApiMessage { success: true, message: "用户删除成功".into() })
    }

    async fn set_status(&self, id: i64, status: i64) -> Result<ApiMessage, UseCaseError> {
        // 行为保持：更新失败仍返回成功
        let _ = self.repo.set_status(id, status).await;
        Ok(ApiMessage { success: true, message: "状态更新成功".into() })
    }

    async fn login(&self, username: String, password: String) -> Result<LoginOutcome, UseCaseError> {
        // 行为保持：查询失败视同用户名不存在
        let row = self.repo.find_login(&username).await.unwrap_or(None);
        let row = match row {
            Some(r) => r,
            None => return Err(UseCaseError::Business("用户名不存在".into())),
        };
        if row.status != 1 {
            return Err(UseCaseError::Business("账号已被禁用".into()));
        }
        if !bcrypt::verify(&password, &row.password).unwrap_or(false) {
            return Err(UseCaseError::Business("密码错误".into()));
        }
        // 行为保持：登录时间更新失败不影响登录
        let _ = self.repo.touch_last_login(row.id).await;
        let session_token = format!("{}:{:x}", row.id, rand::random::<u128>());
        Ok(LoginOutcome {
            body: LoginResp {
                success: true,
                message: "登录成功".into(),
                user: Some(LoginUser {
                    id: row.id,
                    username,
                    nickname: row.nickname,
                    role: row.role,
                }),
            },
            session_token,
        })
    }

    async fn login_check(&self, user_id: i64) -> Result<LoginCheckResp, UseCaseError> {
        // 行为保持：查询失败视同未登录
        let row = self.repo.find_active_by_id(user_id).await.unwrap_or(None);
        Ok(match row {
            Some(r) => LoginCheckResp {
                logged_in: true,
                user: Some(LoginUser {
                    id: user_id,
                    username: r.username,
                    nickname: r.nickname,
                    role: r.role,
                }),
            },
            None => LoginCheckResp { logged_in: false, user: None },
        })
    }
}
