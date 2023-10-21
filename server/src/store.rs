use sqlx::pool::Pool;
use sqlx::postgres::Postgres;
use tokio::sync::Mutex;

pub type Store = Mutex<Pool<Postgres>>;
