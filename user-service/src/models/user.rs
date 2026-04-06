use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct NewUser {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateUser {
    pub name: Option<String>,
    pub email: Option<String>,
}
