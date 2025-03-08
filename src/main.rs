use actix_cors::Cors;
use actix_web::{web, App, HttpServer, middleware::Logger};
use tracing_actix_web::TracingLogger;
use tracing::info;
use dotenv::dotenv;
use std::env;

mod config;
mod models;
mod handlers;
mod auth;

use config::MongoConfig;
use handlers::{
    create_product,
    get_product,
    list_products,
    update_product,
    delete_product,
    upload_products_csv,
};
use auth::{register, login, refresh_token};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    // Initialize logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    info!("Starting server...");

    let db = MongoConfig::init().await.expect("Failed to initialize MongoDB");
    let db_data = web::Data::new(db);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(Logger::default())
            .wrap(TracingLogger::default())
            .app_data(db_data.clone())
            // Public routes
            .service(
                web::scope("/api/auth")
                    .route("/register", web::post().to(register))
                    .route("/login", web::post().to(login))
                    .route("/refresh", web::post().to(refresh_token))
            )
            // Protected routes
            .service(
                web::scope("/api/products")
                    .wrap(auth::AuthMiddleware::default())
                    .route("", web::post().to(create_product))
                    .route("", web::get().to(list_products))
                    .route("/{id}", web::get().to(get_product))
                    .route("/{id}", web::put().to(update_product))
                    .route("/{id}", web::delete().to(delete_product))
                    .route("/import/csv", web::post().to(upload_products_csv))
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
