use sqlx::pool::Pool;
use sqlx::postgres::Postgres;
use tokio::sync::Mutex;

pub struct StoreInternal {
    pub db_pool: Pool<Postgres>,
    pub redis: redis::Client,
}

pub type Store = Mutex<StoreInternal>;
