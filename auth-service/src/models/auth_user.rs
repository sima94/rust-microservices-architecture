use chrono::NaiveDateTime;
use serde::Serialize;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuthUser {
    pub id: i32,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: NaiveDateTime,
}

pub struct NewAuthUser {
    pub email: String,
    pub password_hash: String,
}
