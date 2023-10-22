use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{routing, Router, Server};
use dotenv::dotenv;
use hyper::Error;
use utoipa::OpenApi;
use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;

use crate::store::{Store, StoreInternal};
use crate::types::{CampaignCoupon, CampaignCouponType, Draw, User};

use campaign::{
    CampaignError, CreateCampaignPayload, CreateCampaignPayloadCouponType, GetCampaignResult,
    GetCampaignResultCouponType,
};
use draw::{DrawError, DrawPayload, DrawResult};
use redeem::{RedeemError, RedeemPayload};
use user::{CreateUserPayload, UserError};

mod campaign;
mod draw;
mod redeem;
mod user;

mod store;
mod types;

#[tokio::main]
async fn main() -> Result<(), Error> {
    #[derive(OpenApi)]
    #[openapi(
        paths(
            user::list_users,
            user::create_user,
            user::delete_user,
            campaign::create_campaign,
            campaign::get_campaign,
            draw::draw,
            redeem::redeem_coupon,
        ),
        components(
            schemas(CampaignCouponType, CampaignCoupon, Draw, User),
            schemas(UserError, CreateUserPayload),
            schemas(RedeemError, RedeemPayload),
            schemas(CampaignError, CampaignError, CreateCampaignPayload, CreateCampaignPayloadCouponType, GetCampaignResult, GetCampaignResultCouponType),
            schemas(DrawError, DrawError, DrawPayload, DrawResult),
        ),
        tags(
            (name = "user", description = "User management API"),
            (name = "campaign", description = "Campaign management API"),
            (name = "draw", description = "Draw API"),
            (name = "redeem", description = "Redeem API")
        )
    )]
    struct ApiDoc;

    let app = create_app()
        .await
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
        .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"));

    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 8080));
    print!(
        r#"
-----------------------
API playgrounds available on:
Swagger: {address}/swagger-ui
Redoc: {address}/redoc
Rapidoc: {address}/rapidoc
------------------------
    "#
    );
    Server::bind(&address).serve(app.into_make_service()).await
}

pub async fn create_app() -> Router {
    let store = create_store().await;

    Router::new()
        .route(
            "/user",
            routing::get(user::list_users).post(user::create_user),
        )
        .route("/user/:id", routing::delete(user::delete_user))
        .route("/redeem", routing::post(redeem::redeem_coupon))
        .route("/campaign", routing::post(campaign::create_campaign))
        .route("/campaign/:id", routing::get(campaign::get_campaign))
        .route("/draw", routing::post(draw::draw))
        .with_state(store)
}

pub async fn create_store() -> Arc<Store> {
    dotenv().ok();

    let redis_client = redis::Client::open("redis://localhost/").expect("Redis connection failed");

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing in .env");
    let db_pool = sqlx::postgres::PgPool::connect(&db_url)
        .await
        .expect("Failed to connect to DB");

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to migrate DB");

    Arc::new(Store::new(StoreInternal {
        db_pool,
        redis: redis_client,
    }))
}
