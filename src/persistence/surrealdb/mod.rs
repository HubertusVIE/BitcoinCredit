use surrealdb::{
    engine::any::{connect, Any},
    Surreal,
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("SurrealDB connection error {0}")]
    Connection(#[from] surrealdb::Error),
}

pub async fn get_db() -> Result<Surreal<Any>> {
    let db = connect("rocksdb://data/surrealdb").await?;
    db.use_ns("ebills").await?;
    Ok(db)
}
