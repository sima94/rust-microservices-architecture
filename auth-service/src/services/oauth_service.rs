use crate::cache::{self, RedisPool};
use crate::config::AppConfig;
use crate::db::DbPools;
use crate::errors::ServiceError;
use crate::models::dto::*;
use crate::models::{NewAuthorizationCode, NewOAuthClient, NewRefreshToken, OAuthClient};
use crate::repositories::auth_user_repository::AuthUserRepository;
use crate::repositories::authorization_code_repository::AuthorizationCodeRepository;
use crate::repositories::oauth_client_repository::OAuthClientRepository;
use crate::repositories::refresh_token_repository::RefreshTokenRepository;
use crate::services::auth_service::AuthService;
use crate::services::token_service::TokenService;
use actix_web::web;
use argon2::password_hash::PasswordHash;
use argon2::{Argon2, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};

pub struct OAuthService;

impl OAuthService {
    /// Cached client lookup - checks Redis first, falls back to DB
    async fn find_client_cached(
        pools: &DbPools,
        redis: &RedisPool,
        client_id: &str,
    ) -> Result<OAuthClient, ServiceError> {
        let cache_key = cache::oauth_client_cache_key(client_id);

        // Try cache first
        if let Some(client) = cache::get_cached::<OAuthClient>(redis, &cache_key).await {
            println!("Cache HIT for oauth_client:{}", client_id);
            return Ok(client);
        }

        println!("Cache MISS for oauth_client:{}", client_id);

        // Cache miss - query read replica
        let client = OAuthClientRepository::find_by_client_id(&pools.read, client_id)
            .await
            .map_err(|_| ServiceError::InvalidClient("Client not found".into()))?;

        // Store in cache
        cache::set_cached(redis, &cache_key, &client).await;

        Ok(client)
    }

    // --- Client Registration (write) ---
    pub async fn register_client(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        req: ClientRegisterRequest,
    ) -> Result<ClientRegisterResponse, ServiceError> {
        use argon2::PasswordHasher;
        use argon2::password_hash::SaltString;
        use rand::rngs::OsRng;

        let cid = uuid::Uuid::new_v4().to_string();
        let raw_secret: String = (0..32)
            .map(|_| format!("{:02x}", rand::thread_rng().r#gen::<u8>()))
            .collect();

        let salt = SaltString::generate(&mut OsRng);
        let secret_hash = Argon2::default()
            .hash_password(raw_secret.as_bytes(), &salt)
            .map_err(|e| ServiceError::InternalError(format!("Hash error: {}", e)))?
            .to_string();

        let new_client = NewOAuthClient {
            client_id: cid.clone(),
            client_secret_hash: secret_hash,
            client_name: req.client_name.clone(),
            redirect_uri: req.redirect_uri.clone(),
            scopes: req.scopes.clone(),
        };

        let client = OAuthClientRepository::create(&pools.write, new_client)
            .await
            .map_err(ServiceError::from)?;

        // Cache the new client
        cache::set_cached(
            &redis,
            &cache::oauth_client_cache_key(&client.client_id),
            &client,
        )
        .await;

        Ok(ClientRegisterResponse {
            client_id: client.client_id,
            client_secret: raw_secret,
            client_name: client.client_name,
            redirect_uri: client.redirect_uri,
            scopes: client.scopes,
        })
    }

    // --- Authorization Code (PKCE) - reads + write ---
    pub async fn authorize(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        params: AuthorizeParams,
        login: LoginRequest,
    ) -> Result<AuthorizeResponse, ServiceError> {
        if params.response_type != "code" {
            return Err(ServiceError::InvalidRequest(
                "response_type must be 'code'".into(),
            ));
        }
        if params.code_challenge_method != "S256" {
            return Err(ServiceError::InvalidRequest(
                "code_challenge_method must be 'S256'".into(),
            ));
        }

        // Validate client (cached)
        let client = Self::find_client_cached(&pools, &redis, &params.client_id).await?;

        if client.redirect_uri != params.redirect_uri {
            return Err(ServiceError::InvalidRequest("redirect_uri mismatch".into()));
        }

        validate_scopes(&params.scope, &client.scopes)?;

        // Authenticate user (read)
        let user = AuthService::verify_credentials(&pools, &login.email, &login.password).await?;

        // Generate authorization code
        let mut code_bytes = [0u8; 32];
        rand::thread_rng().fill(&mut code_bytes);
        let code_str = hex::encode(code_bytes);

        let new_code = NewAuthorizationCode {
            code: code_str.clone(),
            client_id: params.client_id,
            user_id: user.id,
            redirect_uri: params.redirect_uri,
            scopes: params.scope,
            code_challenge: params.code_challenge,
            code_challenge_method: params.code_challenge_method,
            expires_at: (Utc::now() + Duration::minutes(10)).naive_utc(),
        };

        // Write
        AuthorizationCodeRepository::create(&pools.write, new_code)
            .await
            .map_err(ServiceError::from)?;

        Ok(AuthorizeResponse {
            code: code_str,
            state: params.state,
        })
    }

    // --- Token Exchange ---
    pub async fn token(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        config: web::Data<AppConfig>,
        req: TokenRequest,
    ) -> Result<TokenResponse, ServiceError> {
        match req.grant_type.as_str() {
            "authorization_code" => {
                Self::exchange_authorization_code(pools, redis, config, req).await
            }
            "client_credentials" => {
                Self::exchange_client_credentials(pools, redis, config, req).await
            }
            "refresh_token" => Self::exchange_refresh_token(pools, redis, config, req).await,
            _ => Err(ServiceError::InvalidRequest(format!(
                "Unsupported grant_type: {}",
                req.grant_type
            ))),
        }
    }

    async fn exchange_authorization_code(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        config: web::Data<AppConfig>,
        req: TokenRequest,
    ) -> Result<TokenResponse, ServiceError> {
        let code_val = req
            .code
            .as_deref()
            .ok_or_else(|| ServiceError::InvalidRequest("Missing 'code'".into()))?;
        let redirect = req
            .redirect_uri
            .as_deref()
            .ok_or_else(|| ServiceError::InvalidRequest("Missing 'redirect_uri'".into()))?;
        let verifier = req
            .code_verifier
            .as_deref()
            .ok_or_else(|| ServiceError::InvalidRequest("Missing 'code_verifier'".into()))?;

        // Read operations (cached)
        let client = Self::find_client_cached(&pools, &redis, &req.client_id).await?;
        verify_client_secret(&req.client_secret, &client.client_secret_hash)?;

        let auth_code = AuthorizationCodeRepository::find_by_code(&pools.read, code_val)
            .await
            .map_err(|_| ServiceError::InvalidGrant("Invalid authorization code".into()))?;

        if auth_code.used {
            return Err(ServiceError::InvalidGrant(
                "Authorization code already used".into(),
            ));
        }
        if auth_code.expires_at < Utc::now().naive_utc() {
            return Err(ServiceError::InvalidGrant(
                "Authorization code expired".into(),
            ));
        }
        if auth_code.client_id != req.client_id {
            return Err(ServiceError::InvalidGrant("client_id mismatch".into()));
        }
        if auth_code.redirect_uri != redirect {
            return Err(ServiceError::InvalidGrant("redirect_uri mismatch".into()));
        }

        // PKCE verification
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let computed_challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
        if computed_challenge != auth_code.code_challenge {
            return Err(ServiceError::InvalidGrant(
                "PKCE verification failed".into(),
            ));
        }

        // Write operations
        AuthorizationCodeRepository::mark_used(&pools.write, code_val)
            .await
            .map_err(ServiceError::from)?;

        let user = AuthUserRepository::find_by_id(&pools.read, auth_code.user_id)
            .await
            .map_err(ServiceError::from)?;

        let access_token = TokenService::create_access_token(
            &user.id.to_string(),
            Some(&user.email),
            &req.client_id,
            &auth_code.scopes,
            &config.jwt_secret,
            config.access_token_ttl_secs,
        )
        .map_err(|e| ServiceError::InternalError(format!("JWT error: {}", e)))?;

        let refresh_token_str = TokenService::generate_refresh_token();
        let expires_at = (Utc::now() + Duration::days(config.refresh_token_ttl_days)).naive_utc();

        RefreshTokenRepository::create(
            &pools.write,
            NewRefreshToken {
                token: refresh_token_str.clone(),
                client_id: req.client_id,
                user_id: user.id,
                scopes: auth_code.scopes.clone(),
                expires_at,
            },
        )
        .await
        .map_err(ServiceError::from)?;

        Ok(TokenResponse {
            access_token,
            refresh_token: Some(refresh_token_str),
            token_type: "Bearer".into(),
            expires_in: config.access_token_ttl_secs,
            scope: auth_code.scopes,
        })
    }

    async fn exchange_client_credentials(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        config: web::Data<AppConfig>,
        req: TokenRequest,
    ) -> Result<TokenResponse, ServiceError> {
        let requested_scope = req.scope.as_deref().unwrap_or("read");

        // Read only (cached)
        let client = Self::find_client_cached(&pools, &redis, &req.client_id).await?;
        verify_client_secret(&req.client_secret, &client.client_secret_hash)?;

        validate_scopes(requested_scope, &client.scopes)?;

        let access_token = TokenService::create_access_token(
            &req.client_id,
            None,
            &req.client_id,
            requested_scope,
            &config.jwt_secret,
            config.access_token_ttl_secs,
        )
        .map_err(|e| ServiceError::InternalError(format!("JWT error: {}", e)))?;

        Ok(TokenResponse {
            access_token,
            refresh_token: None,
            token_type: "Bearer".into(),
            expires_in: config.access_token_ttl_secs,
            scope: requested_scope.to_string(),
        })
    }

    async fn exchange_refresh_token(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        config: web::Data<AppConfig>,
        req: TokenRequest,
    ) -> Result<TokenResponse, ServiceError> {
        let refresh_val = req
            .refresh_token
            .as_deref()
            .ok_or_else(|| ServiceError::InvalidRequest("Missing 'refresh_token'".into()))?;

        // Read (cached)
        let client = Self::find_client_cached(&pools, &redis, &req.client_id).await?;
        verify_client_secret(&req.client_secret, &client.client_secret_hash)?;

        let stored = RefreshTokenRepository::find_by_token(&pools.read, refresh_val)
            .await
            .map_err(|_| ServiceError::InvalidGrant("Invalid refresh token".into()))?;

        if stored.expires_at < Utc::now().naive_utc() {
            RefreshTokenRepository::delete_by_token(&pools.write, refresh_val)
                .await
                .ok();
            return Err(ServiceError::InvalidGrant("Refresh token expired".into()));
        }
        if stored.client_id != req.client_id {
            return Err(ServiceError::InvalidGrant("client_id mismatch".into()));
        }

        // Write: rotate tokens
        RefreshTokenRepository::delete_by_token(&pools.write, refresh_val)
            .await
            .map_err(ServiceError::from)?;

        let user = AuthUserRepository::find_by_id(&pools.read, stored.user_id)
            .await
            .map_err(ServiceError::from)?;

        let access_token = TokenService::create_access_token(
            &user.id.to_string(),
            Some(&user.email),
            &req.client_id,
            &stored.scopes,
            &config.jwt_secret,
            config.access_token_ttl_secs,
        )
        .map_err(|e| ServiceError::InternalError(format!("JWT error: {}", e)))?;

        let new_refresh = TokenService::generate_refresh_token();
        let expires_at = (Utc::now() + Duration::days(config.refresh_token_ttl_days)).naive_utc();

        RefreshTokenRepository::create(
            &pools.write,
            NewRefreshToken {
                token: new_refresh.clone(),
                client_id: req.client_id,
                user_id: user.id,
                scopes: stored.scopes.clone(),
                expires_at,
            },
        )
        .await
        .map_err(ServiceError::from)?;

        Ok(TokenResponse {
            access_token,
            refresh_token: Some(new_refresh),
            token_type: "Bearer".into(),
            expires_in: config.access_token_ttl_secs,
            scope: stored.scopes,
        })
    }

    // --- Token Revocation (write) ---
    pub async fn revoke(
        pools: web::Data<DbPools>,
        redis: web::Data<RedisPool>,
        req: RevokeRequest,
    ) -> Result<(), ServiceError> {
        let client = Self::find_client_cached(&pools, &redis, &req.client_id).await?;
        verify_client_secret(&req.client_secret, &client.client_secret_hash)?;

        RefreshTokenRepository::delete_by_token(&pools.write, &req.token)
            .await
            .ok();

        Ok(())
    }
}

fn verify_client_secret(raw_secret: &str, stored_hash: &str) -> Result<(), ServiceError> {
    let parsed_hash = PasswordHash::new(stored_hash)
        .map_err(|e| ServiceError::InternalError(format!("Hash parse error: {}", e)))?;

    Argon2::default()
        .verify_password(raw_secret.as_bytes(), &parsed_hash)
        .map_err(|_| ServiceError::InvalidClient("Client authentication failed".into()))?;

    Ok(())
}

fn validate_scopes(requested: &str, allowed: &str) -> Result<(), ServiceError> {
    let allowed_set: Vec<&str> = allowed.split_whitespace().collect();
    for scope in requested.split_whitespace() {
        if !allowed_set.contains(&scope) {
            return Err(ServiceError::InvalidRequest(format!(
                "Scope '{}' not allowed",
                scope
            )));
        }
    }
    Ok(())
}
