use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, Json};
use hyper::StatusCode;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::store::Store;
use crate::types::{CampaignCoupon, CampaignCouponType, Draw};

use rand::distributions::WeightedIndex;
use rand::prelude::*;

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) enum DrawError {
    #[schema(example = "User has already drawn from this campaign today")]
    Conflict(String),
    #[schema(example = "Campaign doesn't exist")]
    NotFound(String),
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) struct DrawPayload {
    pub user_id: i32,
    pub campaign_id: i32,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(super) struct DrawResult {
    pub maybe_coupon: Option<CampaignCoupon>,
}

#[utoipa::path(
    post,
    path = "/draw",
    request_body = DrawPayload,
    responses(
        (status = 200, description = "Draw from campaign successfully", body = DrawResult),
        (status = 409, description = "User has already drawn from this campaign today", body = DrawError)
    )
)]
#[axum::debug_handler]
pub(super) async fn draw(
    State(store): State<Arc<Store>>,
    Json(payload): Json<DrawPayload>,
) -> impl IntoResponse {
    let db_pool = store.lock().await.db_pool.clone();
    let redis = &mut store
        .lock()
        .await
        .redis
        .get_async_connection()
        .await
        .unwrap();

    let today_date = chrono::Utc::now().naive_utc().date();

    let enrolled_campaigns_cache_key =
        format!("user-{}:enrolled-campaigns:{}", payload.user_id, today_date);

    let enrolled_campaigns_cache: Vec<String> = redis
        .get(enrolled_campaigns_cache_key.clone())
        .await
        .unwrap();

    print!(
        r#"
Cache {}
{:#?}
        "#,
        enrolled_campaigns_cache_key, enrolled_campaigns_cache
    );

    if enrolled_campaigns_cache.contains(&payload.campaign_id.to_string()) {
        return (
            StatusCode::CONFLICT,
            Json(DrawError::Conflict(
                "User has already enrolled in this campaign. Come again tommorrow".to_string(),
            )),
        )
            .into_response();
    }

    let mut tx = db_pool.begin().await.unwrap();

    let drawn: bool = sqlx::query_scalar!(
        "--sql
            select exists(
                select *
                from draws
                where user_id = $1 and campaign_id = $2 and date = $3
            );
        ",
        payload.user_id,
        payload.campaign_id,
        today_date
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap()
    .unwrap_or(false);

    if drawn {
        let _: String = redis
            .rpush(
                enrolled_campaigns_cache_key.clone(),
                payload.campaign_id.to_string(),
            )
            .await
            .unwrap();

        tx.rollback().await.unwrap();

        return (
            StatusCode::CONFLICT,
            Json(DrawError::Conflict(
                "User has already drawn from this campaign. Come again tommorow".to_string(),
            )),
        )
            .into_response();
    }

    let coupon_types_cache_key = format!("campaign-{}:prob-dist", payload.campaign_id);

    let maybe_coupon_types_cache: Option<Vec<String>> =
        redis.get(coupon_types_cache_key.clone()).await.unwrap();

    let (coupon_type_ids, mut coupon_type_probabilities): (Vec<i32>, Vec<f32>) =
        if let Some(coupon_types_cache) = maybe_coupon_types_cache {
            print!(
                r#"
Cache {}
{:#?}
                "#,
                coupon_types_cache_key, coupon_types_cache
            );

            coupon_types_cache
                .iter()
                .map(|t| {
                    let v: Vec<&str> = t.split(":").collect();
                    (v[0].parse::<i32>().unwrap(), v[1].parse::<f32>().unwrap())
                })
                .unzip()
        } else {
            let coupon_types = sqlx::query_as!(
                CampaignCouponType,
                "--sql
                    select *
                    from campaign_coupon_types
                    where campaign_id = $1;
                ",
                payload.campaign_id
            )
            .fetch_all(&mut *tx)
            .await
            .unwrap();

            if coupon_types.len() == 0 {
                tx.rollback().await.unwrap();

                return (
                    StatusCode::NOT_FOUND,
                    Json(DrawError::NotFound(
                        "Campaign or user doesn't exist or there is no coupon types in the campaign"
                            .to_string(),
                    )),
                )
                    .into_response();
            }

            let ids = coupon_types.iter().map(|t| t.id).collect::<Vec<i32>>();

            let probabilities = coupon_types
                .iter()
                .map(|t| t.probability)
                .collect::<Vec<f32>>();

            let cache: Vec<String> = ids
                .iter()
                .zip(probabilities.iter())
                .map(|(&a, &b)| format!("{}:{}", a, b))
                .collect();

            print!(
                r#"
Writing to cache {}
{:#?}
                "#,
                coupon_types_cache_key, cache
            );

            let _: String = redis
                .set(coupon_types_cache_key.clone(), cache)
                .await
                .unwrap();

            (ids, probabilities)
        };

    coupon_type_probabilities.push(1.0 - coupon_type_probabilities.iter().sum::<f32>());

    let distribution = WeightedIndex::new(&coupon_type_probabilities).unwrap();
    let mut rng = rand::rngs::StdRng::from_entropy();

    let index = distribution.sample(&mut rng);

    print!(
        r#"
Result of draw:
Coupon types pool: {:#.3?}
Index: {}
        "#,
        coupon_type_probabilities, index
    );

    if index + 1 == coupon_type_probabilities.len() {
        tx.rollback().await.unwrap();
        return (StatusCode::OK, Json(DrawResult { maybe_coupon: None })).into_response();
    }

    let coupon_type_id = &coupon_type_ids[index];

    let query = sqlx::query_as!(
        CampaignCouponType,
        "--sql
            update campaign_coupon_types
            set last_drawn_date = case
                when (last_drawn_date is not null and last_drawn_date != CURRENT_DATE) then CURRENT_DATE
                else last_drawn_date
            end,
            current_daily_quota = case
                when (last_drawn_date is not null and last_drawn_date != CURRENT_DATE) then daily_quota - 1
                else current_daily_quota - 1
            end,
            current_quota = current_quota - 1
            where id = $1
            returning *;
        ",
        coupon_type_id
    )
    .fetch_one(&mut *tx)
    .await;

    print!(
        r#"
Appending new entry to cache {}
{:#?}
        "#,
        enrolled_campaigns_cache_key, payload.campaign_id
    );

    let _: i32 = redis
        .append(
            enrolled_campaigns_cache_key.clone(),
            payload.campaign_id.to_string(),
        )
        .await
        .unwrap();

    if let Err(_) = query {
        tx.rollback().await.unwrap();

        sqlx::query!(
            "--sql
                insert into draws (user_id, campaign_id, campaign_coupon_id)
                values ($1, $2, null);
            ",
            payload.user_id,
            payload.campaign_id
        )
        .execute(&db_pool)
        .await
        .unwrap();

        return (StatusCode::OK, Json(DrawResult { maybe_coupon: None })).into_response();
    }

    let coupon = sqlx::query_as!(
        CampaignCoupon,
        "--sql
            insert into campaign_coupons (redeem_code, campaign_coupon_type_id)
            values ($1, $2)
            returning *;
        ",
        String::from(Uuid::new_v4()),
        coupon_type_id
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    sqlx::query_as!(
        Draw,
        "--sql
            insert into draws (user_id, campaign_id, campaign_coupon_id)
            values ($1, $2, $3)
            returning *;
        ",
        payload.user_id,
        payload.campaign_id,
        coupon.id
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    (
        StatusCode::OK,
        Json(DrawResult {
            maybe_coupon: Some(coupon),
        }),
    )
        .into_response()
}
