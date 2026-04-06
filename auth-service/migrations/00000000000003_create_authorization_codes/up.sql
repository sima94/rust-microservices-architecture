CREATE TABLE authorization_codes (
    id SERIAL PRIMARY KEY,
    code VARCHAR NOT NULL UNIQUE,
    client_id VARCHAR NOT NULL,
    user_id INTEGER NOT NULL REFERENCES auth_users(id),
    redirect_uri VARCHAR NOT NULL,
    scopes VARCHAR NOT NULL,
    code_challenge VARCHAR NOT NULL,
    code_challenge_method VARCHAR NOT NULL DEFAULT 'S256',
    expires_at TIMESTAMP NOT NULL,
    used BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_authorization_codes_code ON authorization_codes(code);
