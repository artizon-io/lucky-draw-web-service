#[cfg(test)]
mod tests {
    use crate::{create_app, user::CreateUserPayload};

    use axum::{
        body::Body,
        http::{self, Method, Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_list_delete_user() {
        let app = create_app().await;

        let phone = "+852 0000 0000";

        let create_user_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/user")
                    .method(Method::POST)
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_string(&CreateUserPayload {
                            phone: phone.to_string(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_user_response.status(), StatusCode::CREATED);

        let body = hyper::body::to_bytes(create_user_response.into_body())
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let user_id = body["id"].as_i64().unwrap();

        let list_users_response = app
            .clone()
            .oneshot(Request::builder().uri("/user").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(list_users_response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(list_users_response.into_body())
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(body
            .as_array()
            .unwrap()
            .iter()
            .any(|user| { user["phone"] == phone.to_string() }));

        let delete_user_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/user/{}", user_id))
                    .method(Method::DELETE)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(delete_user_response.status(), StatusCode::OK);
    }
}
