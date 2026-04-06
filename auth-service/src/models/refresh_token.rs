use chrono::NaiveDateTime;

#[derive(Debug, sqlx::FromRow)]
pub struct RefreshToken {
    pub id: i32,
    pub token: String,
    pub client_id: String,
    pub user_id: i32,
    pub scopes: String,
    pub expires_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

pub struct NewRefreshToken {
    pub token: String,
    pub client_id: String,
    pub user_id: i32,
    pub scopes: String,
    pub expires_at: NaiveDateTime,
}
