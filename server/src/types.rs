use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Clone, FromRow)]
pub struct User {
    pub id: i32,
    #[schema(example = r"+852 1234 5678")]
    pub phone: String,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, FromRow)]
pub struct Campaign {
    pub id: i32
}

#[derive(ToSchema, Clone, FromRow)]
pub struct CampaignCouponType {
    pub id: i32,
    pub campaign_id: i32,
    pub description: String,
    #[schema(example = "0.1")]
    pub probability: f32,
    pub total_quota: Option<i32>,
    pub daily_quota: Option<i32>,
    pub current_quota: Option<i32>,
    pub current_daily_quota: Option<i32>,
    pub last_drawn_date: Option<chrono::NaiveDate>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, FromRow)]
pub struct CampaignCoupon {
    pub id: i32,
    #[schema(example = "BK81-DNFJ")]
    pub redeem_code: String,
    pub campaign_coupon_type_id: i32,
    pub redeemed: bool,
}

#[derive(ToSchema, Clone, FromRow)]
pub struct Draw {
    pub id: i32,
    pub user_id: i32,
    pub campaign_id: i32,
    pub campaign_coupon_id: Option<i32>,
    pub date: chrono::NaiveDate,
}
