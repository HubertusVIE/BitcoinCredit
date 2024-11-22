use super::Result;
use surrealdb::{
    engine::any::{connect, Any},
    Surreal,
};

pub mod contact;

#[allow(dead_code)]
pub async fn get_surreal_db(
    connection_string: &str,
    namespace: &str,
    database: &str,
) -> Result<Surreal<Any>> {
    let db = connect(connection_string).await?;
    db.use_ns(namespace).use_db(database).await?;
    Ok(db)
}

#[cfg(test)]
pub async fn get_memory_db(namespace: &str, database: &str) -> Result<Surreal<Any>> {
    let db = connect("mem://").await?;
    db.use_ns(namespace).use_db(database).await?;
    Ok(db)
}
