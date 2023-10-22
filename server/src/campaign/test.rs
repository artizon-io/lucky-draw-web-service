#[cfg(test)]
mod tests {
    use crate::{
        campaign::{CreateCampaignPayload, CreateCampaignPayloadCouponType},
        create_app, create_store,
    };

    use axum::{
        body::Body,
        http::{self, Method, Request, StatusCode},
    };
    use redis::AsyncCommands;
    use serde_json::json;
    use tower::ServiceExt;
    use uuid::Uuid;

    #[tokio::test]
    async fn create_campaign_fail_if_prob_exceed_1() {
        let app = create_app().await;

        let create_campaign_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/campaign")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&CreateCampaignPayload {
                            coupon_types: vec![
                                CreateCampaignPayloadCouponType {
                                    description: "50%".to_string(),
                                    probability: 0.5,
                                    total_quota: None,
                                    daily_quota: None,
                                },
                                CreateCampaignPayloadCouponType {
                                    description: "30%".to_string(),
                                    probability: 0.3,
                                    total_quota: None,
                                    daily_quota: None,
                                },
                                CreateCampaignPayloadCouponType {
                                    description: "30%".to_string(),
                                    probability: 0.3,
                                    total_quota: None,
                                    daily_quota: None,
                                },
                            ],
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_campaign_response.status(), StatusCode::CONFLICT);

        let body = hyper::body::to_bytes(create_campaign_response.into_body())
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            body["Conflict"],
            format!(
                "Sum of probabilities of coupon types in campaign exceed 1: {}",
                1.1
            )
        );
    }

    #[tokio::test]
    async fn create_campaign_and_draw_coupon() {
        let app = create_app().await;

        let create_campaign_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/campaign")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&CreateCampaignPayload {
                            coupon_types: vec![
                                CreateCampaignPayloadCouponType {
                                    description: "100%".to_string(),
                                    probability: 1.0,
                                    total_quota: Some(50),
                                    daily_quota: Some(10),
                                },
                                CreateCampaignPayloadCouponType {
                                    description: "0%".to_string(),
                                    probability: 0.0,
                                    total_quota: None,
                                    daily_quota: None,
                                },
                                CreateCampaignPayloadCouponType {
                                    description: "0%".to_string(),
                                    probability: 0.0,
                                    total_quota: None,
                                    daily_quota: None,
                                },
                            ],
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_campaign_response.status(), StatusCode::CREATED);

        let body = hyper::body::to_bytes(create_campaign_response.into_body())
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let campaign_id = body["id"].as_i64().unwrap();
        let campaign_id: i32 = campaign_id.try_into().unwrap();

        let store = create_store().await;
        let db_pool = store.lock().await.db_pool.clone();
        let redis = &mut store
            .lock()
            .await
            .redis
            .get_async_connection()
            .await
            .unwrap();

        // Create 2 temp users for testing

        let random_phones: Vec<String> = (0..2)
            .map(|_| Uuid::new_v4().to_string()[..20].to_owned())
            .collect();

        let users = sqlx::query!(
            "--sql
                insert into users (phone)
                select * from unnest($1::text[])
                returning id;
            ",
            &random_phones[..],
        )
        .fetch_all(&db_pool)
        .await
        .unwrap();

        let user_id = users[0].id;
        let user_2_id = users[1].id;

        // Check if /draw POST endpoint works

        let draw_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/draw")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "campaign_id": campaign_id,
                            "user_id": user_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(draw_response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(draw_response.into_body())
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let coupon = body["maybe_coupon"].as_object().unwrap();

        let coupon_id: i32 = coupon["id"].to_string().parse::<i32>().unwrap();
        assert!(coupon["redeem_code"].is_string());
        assert_eq!(coupon["redeemed"], false);

        // Request to /draw POST should fail if user doesn't exist

        let draw_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/draw")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "campaign_id": campaign_id,
                            "user_id": 999999
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(draw_response.status(), StatusCode::NOT_FOUND);

        // Check if /redeem GET endpoint works

        let redeem_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/redeem")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "coupon_id": coupon_id,
                            "user_id": user_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(redeem_response.status(), StatusCode::OK);

        // /redeem GET should fail because coupon has already been redeemed

        let redeem_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/redeem")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "coupon_id": coupon_id,
                            "user_id": user_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(redeem_response.status(), StatusCode::CONFLICT);

        // Check if the user-campaign mapping is cached

        let today_date = chrono::Utc::now().naive_utc().date();

        let enrolled_campaigns_cache_key =
            format!("user-{}:enrolled-campaigns:{}", user_id, today_date);

        let enrolled_campaigns_cache: Vec<String> = redis
            .lrange(enrolled_campaigns_cache_key.clone(), 0, -1)
            .await
            .unwrap();

        assert_eq!(enrolled_campaigns_cache.len(), 1);
        assert_eq!(enrolled_campaigns_cache[0], campaign_id.to_string());

        // Check if the campaign coupon types probability distribution is cached

        let coupon_types_cache_key = format!("campaign-{}:prob-dist", campaign_id);

        let coupon_types_cache: Vec<String> = redis
            .lrange(coupon_types_cache_key.clone(), 0, -1)
            .await
            .unwrap();

        assert_eq!(coupon_types_cache.len(), 3);
        coupon_types_cache.iter().for_each(|c| {
            let parts: Vec<&str> = c.split(":").collect();
            assert!(parts.len() == 2);
        });

        // Check if /campaign/:id GET endpoint works

        let get_campaign_details_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/campaign/{}", campaign_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(get_campaign_details_response.status(), StatusCode::OK);

        // Check if quota count is correct

        let body = hyper::body::to_bytes(get_campaign_details_response.into_body())
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let coupon_types = body["coupon_types"].as_array().unwrap();
        assert_eq!(coupon_types.len(), 3);
        let hundred_percent_coupon = coupon_types
            .iter()
            .find(|c| c["description"] == "100%")
            .unwrap();
        assert_eq!(hundred_percent_coupon["total_quota"], 50);
        assert_eq!(hundred_percent_coupon["daily_quota"], 10);
        assert_eq!(hundred_percent_coupon["current_quota"], 49);
        assert_eq!(hundred_percent_coupon["current_daily_quota"], 9);

        // /draw POST request should fail because user 1 already enrolled in the campaign today

        let draw_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/draw")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "campaign_id": campaign_id,
                            "user_id": user_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(draw_response.status(), StatusCode::CONFLICT);

        // User 2 draws should succeed because user 2 has not enrolled in the campaign yet

        let draw_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/draw")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "campaign_id": campaign_id,
                            "user_id": user_2_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(draw_response.status(), StatusCode::OK);

        // Check if quota count is correct

        let get_campaign_details_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/campaign/{}", campaign_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(get_campaign_details_response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(get_campaign_details_response.into_body())
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let coupon_types = body["coupon_types"].as_array().unwrap();
        let hundred_percent_coupon = coupon_types
            .iter()
            .find(|c| c["description"] == "100%")
            .unwrap();
        assert_eq!(hundred_percent_coupon["current_quota"], 48);
        assert_eq!(hundred_percent_coupon["current_daily_quota"], 8);

        // Clear user-campaign mapping cache, /draw POST should still fail

        let _: i32 = redis
            .del(enrolled_campaigns_cache_key.clone())
            .await
            .unwrap();

        let draw_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/draw")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "campaign_id": campaign_id,
                            "user_id": user_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(draw_response.status(), StatusCode::CONFLICT);

        let body = hyper::body::to_bytes(draw_response.into_body())
            .await
            .unwrap();
        // let body = String::from_utf8_lossy(body.as_ref());
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            body["Conflict"],
            "User has already drawn from this campaign. Come again tommorow".to_string()
        );

        // Check if the user-campaign mapping cache got repopulated

        let enrolled_campaigns_cache: Vec<String> = redis
            .lrange(enrolled_campaigns_cache_key.clone(), 0, -1)
            .await
            .unwrap();

        assert_eq!(enrolled_campaigns_cache.len(), 1);
        assert_eq!(enrolled_campaigns_cache[0], campaign_id.to_string());

        // Clear user-campaign mapping cache and remove draw entry from DB, /draw POST should succeed

        let _: i32 = redis
            .del(enrolled_campaigns_cache_key.clone())
            .await
            .unwrap();

        let query_result = sqlx::query!(
            "--sql
                delete from draws
                where campaign_id = $1 and user_id = $2 and date = CURRENT_DATE;
            ",
            campaign_id,
            user_id
        )
        .execute(&db_pool)
        .await
        .unwrap();

        assert_eq!(query_result.rows_affected(), 1);

        let draw_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/draw")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "campaign_id": campaign_id,
                            "user_id": user_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(draw_response.status(), StatusCode::OK);

        // Clear user-campaign mapping cache and remove draw entry from DB again

        let _: i32 = redis
            .del(enrolled_campaigns_cache_key.clone())
            .await
            .unwrap();

        sqlx::query!(
            "--sql
                delete from draws
                where campaign_id = $1 and user_id = $2 and date = CURRENT_DATE;
            ",
            campaign_id,
            user_id
        )
        .execute(&db_pool)
        .await
        .unwrap();

        // Change quota to 0 and check if /draw POST endpoint returns OK (no coupon)

        sqlx::query!(
            "--sql
                update campaign_coupon_types
                set current_quota = 0
                where campaign_id = $1 and description = '100%';
            ",
            campaign_id,
        )
        .execute(&db_pool)
        .await
        .unwrap();

        let draw_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/draw")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&json!({
                            "campaign_id": campaign_id,
                            "user_id": user_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(draw_response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(draw_response.into_body())
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(body.get("maybe_coupon").unwrap(), &serde_json::Value::Null);
    }
}
