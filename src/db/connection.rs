use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use std::env;
use std::time::Duration;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type DbConnection = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

pub fn establish_connection() -> DbPool {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    
    r2d2::Pool::builder()
        .max_size(50)
        .min_idle(Some(10))
        .connection_timeout(Duration::from_secs(30))
        .build(manager)
        .expect("Failed to create connection pool")
}