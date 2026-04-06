CREATE TABLE oauth_clients (
    id SERIAL PRIMARY KEY,
    client_id VARCHAR NOT NULL UNIQUE,
    client_secret_hash VARCHAR NOT NULL,
    client_name VARCHAR NOT NULL,
    redirect_uri VARCHAR NOT NULL,
    scopes VARCHAR NOT NULL DEFAULT 'read',
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_oauth_clients_client_id ON oauth_clients(client_id);
