use sqlx::postgres::PgPoolOptions;

pub type DbPool = sqlx::PgPool;

#[derive(Clone)]
pub struct DbPools {
    pub write: DbPool,
    pub read: DbPool,
}

async fn create_pool(url: &str, max_size: u32) -> DbPool {
    PgPoolOptions::new()
        .max_connections(max_size)
        .connect(url)
        .await
        .expect(&format!("Failed to create pool for {}", url))
}

pub async fn init_pools() -> DbPools {
    let write_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let read_url = std::env::var("DATABASE_READ_URL")
        .unwrap_or_else(|_| write_url.clone());

    let max_size: u32 = std::env::var("DB_POOL_MAX_SIZE")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .expect("DB_POOL_MAX_SIZE must be a valid u32");

    let write = create_pool(&write_url, max_size).await;
    let read = create_pool(&read_url, max_size).await;

    println!("DB pools initialized (write: {}, read: {})",
        if write_url == read_url { "same" } else { "master" },
        if write_url == read_url { "same" } else { "replica" },
    );

    DbPools { write, read }
}

// Keep for backward compatibility (tests, health check)
pub async fn init_pool() -> DbPool {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env file");

    let max_size: u32 = std::env::var("DB_POOL_MAX_SIZE")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .expect("DB_POOL_MAX_SIZE must be a valid u32");

    PgPoolOptions::new()
        .max_connections(max_size)
        .connect(&database_url)
        .await
        .expect("Failed to create pool.")
}
