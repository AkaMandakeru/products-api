use actix_web::{web, HttpResponse, Error};
use mongodb::{bson::{doc, oid::ObjectId}, Collection};
use futures::TryStreamExt;
use tracing::{info, error, debug};
use crate::{config::MongoConfig, models::{Product, CreateProductRequest, UpdateProductRequest}};

pub async fn create_product(
    db: web::Data<MongoConfig>,
    product: web::Json<CreateProductRequest>,
) -> Result<HttpResponse, Error> {
    let collection: Collection<Product> = db.database.collection("products");

    debug!("Creating new product: {:?}", product);

    let new_product = Product {
        id: None,
        name: product.name.clone(),
        price: product.price,
        category: product.category.clone(),
        has_active_sale: product.has_active_sale,
    };

    let result = collection.insert_one(new_product, None).await.map_err(|e| {
        error!("Failed to create product: {}", e);
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    info!("Product created successfully with ID: {}", result.inserted_id);
    Ok(HttpResponse::Created().json(doc! { "id": result.inserted_id }))
}

pub async fn get_product(
    db: web::Data<MongoConfig>,
    id: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let collection: Collection<Product> = db.database.collection("products");

    debug!("Fetching product with ID: {}", id);

    let object_id = ObjectId::parse_str(id.as_str()).map_err(|_| {
        error!("Invalid product ID format: {}", id);
        actix_web::error::ErrorBadRequest("Invalid ID format")
    })?;

    let filter = doc! { "_id": object_id };
    let product = collection.find_one(filter, None).await.map_err(|e| {
        error!("Failed to fetch product {}: {}", id, e);
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    match product {
        Some(product) => {
            info!("Product found: {}", id);
            Ok(HttpResponse::Ok().json(product))
        },
        None => {
            debug!("Product not found: {}", id);
            Ok(HttpResponse::NotFound().finish())
        },
    }
}

pub async fn list_products(
    db: web::Data<MongoConfig>,
) -> Result<HttpResponse, Error> {
    let collection: Collection<Product> = db.database.collection("products");

    debug!("Fetching products");

    let mut products = Vec::new();
    let mut cursor = collection.find(None, None).await.map_err(|e| {
        error!("Failed to fetch products: {}", e);
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    while let Some(result) = cursor.try_next().await.map_err(|e| {
        error!("Error while iterating products: {}", e);
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })? {
        products.push(result);
    }

    info!("Retrieved {} products", products.len());
    Ok(HttpResponse::Ok().json(products))
}

pub async fn update_product(
    db: web::Data<MongoConfig>,
    id: web::Path<String>,
    update: web::Json<UpdateProductRequest>,
) -> Result<HttpResponse, Error> {
    let collection: Collection<Product> = db.database.collection("products");

    debug!("Updating product {}: {:?}", id, update);

    let object_id = ObjectId::parse_str(id.as_str()).map_err(|_| {
        error!("Invalid product ID format: {}", id);
        actix_web::error::ErrorBadRequest("Invalid ID format")
    })?;

    let mut update_doc = doc! {};

    if let Some(name) = &update.name {
        update_doc.insert("name", name);
    }
    if let Some(price) = update.price {
        update_doc.insert("price", price);
    }
    if let Some(category) = &update.category {
        update_doc.insert("category", category.to_string());
    }
    if let Some(has_active_sale) = update.has_active_sale {
        update_doc.insert("has_active_sale", has_active_sale);
    }

    let filter = doc! { "_id": object_id };
    let update_doc = doc! { "$set": update_doc };

    let result = collection.update_one(filter, update_doc, None).await.map_err(|e| {
        error!("Failed to update product {}: {}", id, e);
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    if result.matched_count == 0 {
        debug!("Product not found for update: {}", id);
        Ok(HttpResponse::NotFound().finish())
    } else {
        info!("Product updated successfully: {}", id);
        Ok(HttpResponse::Ok().finish())
    }
}

pub async fn delete_product(
    db: web::Data<MongoConfig>,
    id: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let collection: Collection<Product> = db.database.collection("products");

    debug!("Deleting product: {}", id);

    let object_id = ObjectId::parse_str(id.as_str()).map_err(|_| {
        error!("Invalid product ID format: {}", id);
        actix_web::error::ErrorBadRequest("Invalid ID format")
    })?;

    let filter = doc! { "_id": object_id };
    let result = collection.delete_one(filter, None).await.map_err(|e| {
        error!("Failed to delete product {}: {}", id, e);
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    if result.deleted_count == 0 {
        debug!("Product not found for deletion: {}", id);
        Ok(HttpResponse::NotFound().finish())
    } else {
        info!("Product deleted successfully: {}", id);
        Ok(HttpResponse::Ok().finish())
    }
}
