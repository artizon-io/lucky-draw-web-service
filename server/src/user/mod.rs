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
pub use user::User;

mod user;

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) enum UserError {
    #[schema(example = "User already exists")]
    Conflict(String),
    #[schema(example = "id = 1")]
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
    let db_pool = store.lock().await.clone();

    let q = r"--sql
        select *
        from users;
    ";

    let users = sqlx::query_as(q).fetch_all(&db_pool).await.unwrap();

    Json(users)
}

#[utoipa::path(
    post,
    path = "/user",
    request_body = User,
    responses(
        (status = 201, description = "User created successfully", body = User),
        (status = 409, description = "User already exists", body = UserError)
    )
)]
pub(super) async fn create_user(
    State(store): State<Arc<Store>>,
    Json(user): Json<User>,
) -> impl IntoResponse {
    let db_pool = store.lock().await.clone();

    let q = "--sql
        insert into users (id, phone)
        values ($1, $2)
        returning *;
    ";

    let q_result = sqlx::query_as::<_, User>(q)
        .bind(&user.id)
        .bind(&user.phone)
        .fetch_one(&db_pool)
        .await;

    match q_result {
        Err(_) => (
            StatusCode::CONFLICT,
            Json(UserError::Conflict(format!(
                "user already exists: {}",
                user.id
            ))),
        )
            .into_response(),
        Ok(user) => (StatusCode::CREATED, Json(user)).into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/user/{id}",
    responses(
        (status = 200, description = "Delete user successfully"),
        (status = 404, description = "User not found", body = UserError, example = json!(UserError::NotFound(String::from("id = 1"))))
    ),
    params(
        ("id" = i32, Path, description = "User id")
    )
)]
pub(super) async fn delete_user(
    Path(id): Path<i32>,
    State(store): State<Arc<Store>>,
) -> impl IntoResponse {
    let db_pool = store.lock().await.clone();

    let q = "--sql
        delete from users
        where id = $1;
    ";

    let q_result = sqlx::query(q).bind(id).execute(&db_pool).await.unwrap();

    match q_result.rows_affected() {
        0 => (
            StatusCode::NOT_FOUND,
            Json(UserError::NotFound(format!("id = {id}"))),
        )
            .into_response(),
        _ => StatusCode::OK.into_response(),
    }
}
