use chrono::NaiveDateTime;

#[derive(Debug, sqlx::FromRow)]
pub struct AuthorizationCode {
    pub id: i32,
    pub code: String,
    pub client_id: String,
    pub user_id: i32,
    pub redirect_uri: String,
    pub scopes: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub expires_at: NaiveDateTime,
    pub used: bool,
    pub created_at: NaiveDateTime,
}

pub struct NewAuthorizationCode {
    pub code: String,
    pub client_id: String,
    pub user_id: i32,
    pub redirect_uri: String,
    pub scopes: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub expires_at: NaiveDateTime,
}
