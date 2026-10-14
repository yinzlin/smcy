//! 用户域入站端口（UseCase 契约）

use domain::port::user::ActiveUserRow;

use crate::dto::user::{
    ApiMessage, ContactPhoneResp, CreateUserCmd, LoginCheckResp, LoginOutcome, UpdateUserCmd,
    UserDetailResp,
};
use crate::error::UseCaseError;

pub trait UserUseCase: Send + Sync {
    async fn list_active(&self) -> Result<Vec<ActiveUserRow>, UseCaseError>;
    async fn get_detail(&self, id: i64) -> Result<UserDetailResp, UseCaseError>;
    async fn get_contact_phone(&self, user_id: i64) -> Result<ContactPhoneResp, UseCaseError>;
    async fn set_contact_phone(&self, user_id: i64, phone: String) -> Result<ContactPhoneResp, UseCaseError>;
    async fn create(&self, cmd: CreateUserCmd) -> Result<ApiMessage, UseCaseError>;
    async fn update(&self, cmd: UpdateUserCmd) -> Result<ApiMessage, UseCaseError>;
    async fn delete(&self, id: i64) -> Result<ApiMessage, UseCaseError>;
    async fn set_status(&self, id: i64, status: i64) -> Result<ApiMessage, UseCaseError>;
    async fn login(&self, username: String, password: String) -> Result<LoginOutcome, UseCaseError>;
    async fn login_check(&self, user_id: i64) -> Result<LoginCheckResp, UseCaseError>;
}
