mod config;
mod models;
mod handlers;

use actix_web::{web, App, HttpServer, middleware::Logger};
use config::MongoConfig;
use handlers::{create_product, get_product, list_products, update_product, delete_product};
use tracing::{info, Level};
use tracing_subscriber::{self, EnvFilter};
use tracing_actix_web::TracingLogger;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(Level::INFO.into())
                .add_directive("actix_web=info".parse().unwrap())
                .add_directive("products_json_api=debug".parse().unwrap())
        )
        .init();

    info!("Starting products API server");

    let mongo_config = MongoConfig::init()
        .await
        .expect("Failed to initialize MongoDB");

    info!("MongoDB connection established");
    let db_data = web::Data::new(mongo_config);

    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(db_data.clone())
            .service(
                web::scope("/api/products")
                    .route("", web::post().to(create_product))
                    .route("", web::get().to(list_products))
                    .route("/{id}", web::get().to(get_product))
                    .route("/{id}", web::put().to(update_product))
                    .route("/{id}", web::delete().to(delete_product))
            )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
