use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{routing, Router, Server};
use hyper::Error;
use utoipa::OpenApi;
use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;

use crate::store::Store;

mod store;
mod user;

#[tokio::main]
async fn main() -> Result<(), Error> {
    #[derive(OpenApi)]
    #[openapi(
        paths(
            user::list_users,
            user::create_user,
            user::delete_user,
        ),
        components(
            schemas(user::User, user::UserError)
        ),
        tags(
            (name = "user", description = "User management API")
        )
    )]
    struct ApiDoc;

    let db_url = "postgres://user:password@localhost:5432/pg";
    let db_pool = sqlx::postgres::PgPool::connect(db_url).await.unwrap();

    sqlx::migrate!("./migrations").run(&db_pool).await.unwrap();

    let store = Arc::new(Store::new(db_pool));
    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
        .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
        .route(
            "/user",
            routing::get(user::list_users).post(user::create_user),
        )
        .route("/user/:id", routing::delete(user::delete_user))
        .with_state(store);

    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 8080));
    println!("Swagger: {}/swagger-ui", address);
    println!("Redoc: {}/redoc", address);
    println!("Rapidoc: {}/rapidoc", address);
    Server::bind(&address).serve(app.into_make_service()).await
}
