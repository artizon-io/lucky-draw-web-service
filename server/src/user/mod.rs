use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::store::Store;
use crate::types::User;

mod test;

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) enum UserError {
    #[schema(example = "User ID already exists")]
    Conflict(String),
    #[schema(example = "User ID doesn't exist")]
    NotFound(String),
}

#[utoipa::path(
    get,
    path = "/user",
    responses(
        (status = 200, description = "List all users successfully", body = [User])
    )
)]
pub(super) async fn list_users(State(store): State<Arc<Store>>) -> Json<Vec<User>> {
    let db_pool = store.lock().await.db_pool.clone();

    let users = sqlx::query_as!(
        User,
        "--sql
            select *
            from users;
        "
    )
    .fetch_all(&db_pool)
    .await
    .unwrap();

    Json(users)
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) struct CreateUserPayload {
    #[schema(example = r"+852 1234 5678")]
    pub phone: String,
}

#[utoipa::path(
    post,
    path = "/user",
    request_body = CreateUserPayload,
    responses(
        (status = 201, description = "User created successfully", body = User),
        (status = 409, description = "Phone number is registered by another user", body = UserError)
    )
)]
pub(super) async fn create_user(
    State(store): State<Arc<Store>>,
    Json(payload): Json<CreateUserPayload>,
) -> impl IntoResponse {
    let db_pool = store.lock().await.db_pool.clone();

    let request = sqlx::query_as!(
        User,
        "--sql
            insert into users (phone)
            values ($1)
            returning *;
        ",
        payload.phone,
    )
    .fetch_one(&db_pool)
    .await;

    match request {
        Ok(new_user) => (StatusCode::CREATED, Json(new_user)).into_response(),
        Err(_) => (
            StatusCode::CONFLICT,
            Json(UserError::Conflict(format!(
                "Phone number {} is registered by another user",
                payload.phone
            ))),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/user/{id}",
    responses(
        (status = 200, description = "Delete user successfully"),
        (status = 404, description = "User not found", body = UserError, example = json!(UserError::NotFound(String::from("User with ID 1 doesn't exist"))))
    ),
    params(
        ("id" = i32, Path, description = "User id")
    )
)]
pub(super) async fn delete_user(
    Path(id): Path<i32>,
    State(store): State<Arc<Store>>,
) -> impl IntoResponse {
    let db_pool = store.lock().await.db_pool.clone();

    let q_result = sqlx::query!(
        "--sql
            delete from users
            where id = $1;
        ",
        id
    )
    .execute(&db_pool)
    .await
    .unwrap();

    match q_result.rows_affected() {
        0 => (
            StatusCode::NOT_FOUND,
            Json(UserError::NotFound(format!(
                "User with ID {id} doesn't exist"
            ))),
        )
            .into_response(),
        _ => StatusCode::OK.into_response(),
    }
}
