use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, Json};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::store::Store;
use crate::types::CampaignCoupon;

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) enum RedeemError {
    #[schema(example = "User or coupon not found")]
    Conflict(String),
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) struct RedeemPayload {
    pub user_id: i32,
    pub coupon_id: i32,
}

#[utoipa::path(
    post,
    path = "/redeem",
    request_body = RedeemPayload,
    responses(
        (status = 200, description = "Coupon redeemed successfully", body = CampaignCoupon),
        (status = 409, description = "Coupon not found, or coupon has already been redeemed", body = RedeemError),
    )
)]
pub(super) async fn redeem_coupon(
    State(store): State<Arc<Store>>,
    Json(payload): Json<RedeemPayload>,
) -> impl IntoResponse {
    let db_pool = store.lock().await.db_pool.clone();

    let query = sqlx::query_as!(
        CampaignCoupon,
        "--sql
            update campaign_coupons
            set redeemed = true
            where id = $1 and redeemed = false
            returning *;
        ",
        payload.coupon_id
    )
    .fetch_one(&db_pool)
    .await;

    match query {
        Err(_) => (
            StatusCode::CONFLICT,
            Json(RedeemError::Conflict(
                "Coupon not found, or it has already been redeemed".to_string(),
            )),
        )
            .into_response(),
        Ok(coupon) => (StatusCode::OK, Json(coupon)).into_response(),
    }
}
