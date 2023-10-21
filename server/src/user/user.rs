use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Clone, FromRow)]
pub struct User {
    pub id: i32,
    #[schema(example = r"+852 1234 5678")]
    pub phone: String,
}
