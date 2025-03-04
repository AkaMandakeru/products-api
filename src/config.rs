use mongodb::{Client, Database};
use std::env;
use dotenv::dotenv;

pub struct MongoConfig {
    pub database: Database,
}

impl MongoConfig {
    pub async fn init() -> Result<Self, mongodb::error::Error> {
        dotenv().ok();
        
        let mongo_uri = env::var("MONGODB_URI")
            .unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
        let database_name = env::var("DATABASE_NAME")
            .unwrap_or_else(|_| "products_db".to_string());

        let client = Client::with_uri_str(&mongo_uri).await?;
        let database = client.database(&database_name);

        Ok(MongoConfig { database })
    }
}
