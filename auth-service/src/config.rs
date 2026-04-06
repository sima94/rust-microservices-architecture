pub struct AppConfig {
    pub jwt_secret: String,
    pub access_token_ttl_secs: i64,
    pub refresh_token_ttl_days: i64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            jwt_secret: std::env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            access_token_ttl_secs: std::env::var("ACCESS_TOKEN_TTL_SECS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .expect("ACCESS_TOKEN_TTL_SECS must be a number"),
            refresh_token_ttl_days: std::env::var("REFRESH_TOKEN_TTL_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .expect("REFRESH_TOKEN_TTL_DAYS must be a number"),
        }
    }
}
