// Re-export shared cache functions
pub use shared::cache::*;

// Service-specific cache keys
pub fn user_cache_key(id: i32) -> String {
    format!("user:{}", id)
}

pub fn users_list_cache_key() -> String {
    "users:list".to_string()
}
