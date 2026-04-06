CREATE TABLE refresh_tokens (
    id SERIAL PRIMARY KEY,
    token VARCHAR NOT NULL UNIQUE,
    client_id VARCHAR NOT NULL,
    user_id INTEGER NOT NULL REFERENCES auth_users(id),
    scopes VARCHAR NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_refresh_tokens_token ON refresh_tokens(token);
