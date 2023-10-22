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
use crate::types::Campaign;

mod test;

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) enum CampaignError {
    #[schema(example = "Sum of probabilities of coupon types exceed 1")]
    Conflict(String),
    #[schema(example = "Campaign ID doesn't exist")]
    NotFound(String),
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) struct GetCampaignResult {
    pub coupon_types: Vec<GetCampaignResultCouponType>,
}

#[derive(ToSchema, Clone, Serialize, Deserialize)]
pub struct GetCampaignResultCouponType {
    pub description: String,
    #[schema(example = "0.1")]
    pub probability: f32,
    pub total_quota: Option<i32>,
    pub daily_quota: Option<i32>,
    pub current_quota: Option<i32>,
    pub current_daily_quota: Option<i32>,
}

#[utoipa::path(
    get,
    path = "/campaign/{id}",
    responses(
        (status = 200, description = "Get information about the campaign successfully", body = GetCampaignResult),
        (status = 404, description = "Campaign ID doesn't exist", body = CampaignError)
    )
)]
#[axum::debug_handler]
pub(super) async fn get_campaign(
    Path(id): Path<i32>,
    State(store): State<Arc<Store>>,
) -> impl IntoResponse {
    let db_pool = store.lock().await.db_pool.clone();

    let campaign_coupon_types = sqlx::query_as!(
        GetCampaignResultCouponType,
        "--sql
            select description, probability, total_quota, daily_quota, current_quota, current_daily_quota
            from campaign_coupon_types
            where campaign_id = $1;
        ",
        id
    )
    .fetch_all(&db_pool)
    .await
    .unwrap();

    if campaign_coupon_types.len() == 0 {
        (
            StatusCode::NOT_FOUND,
            Json(CampaignError::NotFound(format!(
                "Campaign ID {} doesn't exist, or campaign doesn't have any coupon types",
                id
            ))),
        )
            .into_response()
    } else {
        (
            StatusCode::OK,
            Json(GetCampaignResult {
                coupon_types: campaign_coupon_types,
            }),
        )
            .into_response()
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) struct CreateCampaignPayload {
    pub coupon_types: Vec<CreateCampaignPayloadCouponType>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) struct CreateCampaignPayloadCouponType {
    #[schema(example = "10% off")]
    pub description: String,
    #[schema(example = "0.1")]
    pub probability: f32,
    #[schema(example = "100")]
    pub total_quota: Option<i32>,
    #[schema(example = "30")]
    pub daily_quota: Option<i32>,
}

#[utoipa::path(
    post,
    path = "/campaign",
    request_body = CreateCampaignPayload,
    responses(
        (status = 201, description = "Campaign created successfully", body = Campaign),
        (status = 409, description = "Sum of probabilities of coupon types exceed 1", body = CampaignError)
    )
)]
pub(super) async fn create_campaign(
    State(store): State<Arc<Store>>,
    Json(payload): Json<CreateCampaignPayload>,
) -> impl IntoResponse {
    let db_pool = store.lock().await.db_pool.clone();

    let total_prob: f32 = payload.coupon_types.iter().map(|t| t.probability).sum();

    if total_prob > 1.0 {
        return (
            StatusCode::CONFLICT,
            Json(CampaignError::Conflict(format!(
                "Sum of probabilities of coupon types in campaign exceed 1: {}",
                total_prob
            ))),
        )
            .into_response();
    }

    let mut tx = db_pool.begin().await.unwrap();

    let new_compaign = sqlx::query_as!(
        Campaign,
        "--sql
            insert into campaigns (id)
            values (default)
            returning *;
        "
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    let campaign_id = new_compaign.id;

    let campaign_ids: Vec<_> = vec![campaign_id; payload.coupon_types.len()];
    let descriptions: Vec<_> = payload
        .coupon_types
        .iter()
        .map(|t| t.description.clone())
        .collect();
    let probabilities: Vec<_> = payload.coupon_types.iter().map(|t| t.probability).collect();
    let total_quotas: Vec<Option<i32>> =
        payload.coupon_types.iter().map(|t| t.total_quota).collect();
    let daily_quotas: Vec<Option<i32>> =
        payload.coupon_types.iter().map(|t| t.daily_quota).collect();

    // https://github.com/launchbadge/sqlx/blob/main/FAQ.md#how-can-i-bind-an-array-to-a-values-clause-how-can-i-do-bulk-inserts
    // https://github.com/launchbadge/sqlx/issues/1893
    #[allow(deprecated)]
    sqlx::query!(
        "--sql
            insert into campaign_coupon_types (campaign_id, description, probability, total_quota, daily_quota, current_quota)
            select * from unnest($1::int[], $2::text[], $3::float4[], $4::int[], $5::int[], $4::int[]);
        ",
        &campaign_ids[..],
        &descriptions[..],
        &probabilities[..],
        &total_quotas[..]: Vec<Option<i32>>,
        &daily_quotas[..]: Vec<Option<i32>>
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    (StatusCode::CREATED, Json(new_compaign)).into_response()
}
