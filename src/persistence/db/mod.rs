use super::Result;
use surrealdb::{
    engine::any::{connect, Any},
    Surreal,
};

pub mod contact;

pub async fn get_surreal_db(connection_string: &str, namespace: &str) -> Result<Surreal<Any>> {
    let db = connect(connection_string).await?;
    db.use_ns(namespace).await?;
    Ok(db)
}
