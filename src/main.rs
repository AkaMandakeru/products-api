mod config;
mod models;
mod handlers;

use actix_web::{web, App, HttpServer, middleware::Logger};
use actix_cors::Cors;
use config::MongoConfig;
use handlers::{create_product, get_product, list_products, update_product, delete_product, upload_products_csv};
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
        // Configure CORS
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")  // Add your frontend URL here
            .allowed_origin("http://localhost:5173")  // Add Vite's default URL
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::ACCEPT,
                actix_web::http::header::CONTENT_TYPE,
            ])
            .max_age(3600);

        App::new()
            .wrap(cors)  // Add CORS middleware
            .wrap(TracingLogger::default())
            .app_data(db_data.clone())
            .service(
                web::scope("/api/products")
                    .route("", web::post().to(create_product))
                    .route("", web::get().to(list_products))
                    .route("/{id}", web::get().to(get_product))
                    .route("/{id}", web::put().to(update_product))
                    .route("/{id}", web::delete().to(delete_product))
                    .route("/import/csv", web::post().to(upload_products_csv))
            )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
