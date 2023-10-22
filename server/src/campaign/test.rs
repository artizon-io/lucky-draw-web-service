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
    }
}
