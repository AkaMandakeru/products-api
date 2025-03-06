use actix_web::{web, HttpResponse, Error};
use actix_multipart::Multipart;
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::FindOptions,
    Collection,
};
use futures::TryStreamExt;
use tracing::{info, error, debug};
use serde::{Deserialize, Serialize};
use csv::ReaderBuilder;
use tempfile::NamedTempFile;
use regex::escape;
use futures_util::StreamExt;
use std::io::Write;
use crate::{config::MongoConfig, models::{Product, CreateProductRequest, UpdateProductRequest, Category}};

#[derive(Debug, Deserialize)]
pub struct ListProductsQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    price: Option<f64>,
    sort: Option<String>,
    direction: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListProductsResponse {
    products: Vec<Product>,
    total_pages: i64,
}

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
    query: web::Query<ListProductsQuery>,
) -> Result<HttpResponse, Error> {
    let collection: Collection<Product> = db.database.collection("products");

    // Set up pagination
    let per_page = query.per_page.unwrap_or(15);
    let page = query.page.unwrap_or(1).max(1);
    let skip = (page - 1) * per_page;

    // Build filter
    let mut filter = Document::new();
    if let Some(name_filter) = &query.filter {
        filter.insert("name", doc! {
            "$regex": format!("(?i){}", escape(name_filter))
        });
    }
    if let Some(price) = query.price {
        filter.insert("price", price);
    }

    // Build sort
    let allowed_sort_columns = ["name", "price"];
    let sort_column = query.sort
        .as_deref()
        .filter(|&s| allowed_sort_columns.contains(&s))
        .unwrap_or("name");

    let sort_direction = match query.direction.as_deref() {
        Some("desc") => -1,
        _ => 1,
    };

    let sort_doc = doc! { sort_column: sort_direction };

    // Set up options with sort and pagination
    let find_options = FindOptions::builder()
        .sort(sort_doc)
        .skip(skip as u64)
        .limit(per_page as i64)
        .build();

    // Get total count for pagination
    let total_count = collection.count_documents(filter.clone(), None).await.map_err(|e| {
        error!("Failed to count products: {}", e);
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    let total_pages = ((total_count as f64) / (per_page as f64)).ceil() as i64;

    // Fetch products
    let mut products = Vec::new();
    let mut cursor = collection.find(filter, find_options).await.map_err(|e| {
        error!("Failed to fetch products: {}", e);
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    while let Some(result) = cursor.try_next().await.map_err(|e| {
        error!("Error while iterating products: {}", e);
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })? {
        products.push(result);
    }

    info!("Retrieved {} products (page {} of {})", products.len(), page, total_pages);

    Ok(HttpResponse::Ok().json(ListProductsResponse {
        products,
        total_pages,
    }))
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

pub async fn upload_products_csv(
    db: web::Data<MongoConfig>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    let collection: Collection<Product> = db.database.collection("products");
    let mut errors = Vec::new();
    let mut success_count = 0;

    // Process the multipart form data
    while let Some(item) = payload.next().await {
        let mut field = item.map_err(|e| {
            error!("Error getting multipart field: {}", e);
            actix_web::error::ErrorBadRequest(format!("Multipart error: {}", e))
        })?;

        if field.name() == "file" {
            // Create a temporary file to store the CSV data
            let mut temp_file = NamedTempFile::new().map_err(|e| {
                error!("Failed to create temp file: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to process file")
            })?;

            // Write the field data to the temp file
            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|e| {
                    error!("Error reading multipart chunk: {}", e);
                    actix_web::error::ErrorBadRequest("Failed to read uploaded file")
                })?;
                temp_file.write_all(&data).map_err(|e| {
                    error!("Failed to write to temp file: {}", e);
                    actix_web::error::ErrorInternalServerError("Failed to process file")
                })?;
            }

            // Create a CSV reader
            let mut rdr = ReaderBuilder::new()
                .flexible(true)
                .trim(csv::Trim::All)
                .from_reader(temp_file.reopen().map_err(|e| {
                    error!("Failed to reopen temp file: {}", e);
                    actix_web::error::ErrorInternalServerError("Failed to process file")
                })?);

            let mut line_number = 2; // Start from 2 to account for header row
            for result in rdr.records() {
                match result {
                    Ok(record) => {
                        let mut has_error = false;

                        if record.len() < 4 {
                            errors.push(doc! {
                                "line": line_number,
                                "error": "Invalid number of columns",
                                "data": record.iter().collect::<Vec<_>>()
                            });
                            has_error = true;
                        }

                        // Parse the CSV record - safely get values or use empty strings
                        let raw_name = record.get(0).unwrap_or("").trim();

                        // Split the name into product name and ID parts
                        let (product_name, product_id) = if let Some((name, id)) = raw_name.split_once("#") {
                            (name.trim(), id.trim())
                        } else {
                            (raw_name, "")
                        };

                        // Validate and sanitize product ID
                        let sanitized_id = if !product_id.is_empty() {
                            // Remove any surrounding parentheses
                            let id_content = product_id.trim_start_matches('(').trim_end_matches(')');

                            // Only allow alphanumeric and basic symbols in ID
                            let clean_id = id_content
                                .chars()
                                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                                .collect::<String>();

                            if clean_id.is_empty() {
                                String::new()
                            } else {
                                format!("#{}", clean_id)
                            }
                        } else {
                            String::new()
                        };

                        // Sanitize product name
                        let clean_name = product_name
                            .chars()
                            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-')
                            .collect::<String>()
                            .trim()
                            .to_string();

                        let price_str = record.get(1).unwrap_or("").trim();
                        let category_str = record.get(2).unwrap_or("").trim().to_lowercase();
                        let has_active_sale = record.get(3).unwrap_or("false").trim().parse::<bool>();

                        // Validate name
                        if clean_name.is_empty() {
                            errors.push(doc! {
                                "line": line_number,
                                "error": "Name is required",
                                "data": record.iter().collect::<Vec<_>>()
                            });
                            has_error = true;
                        }

                        // Parse and validate price
                        let price = if price_str.is_empty() {
                            errors.push(doc! {
                                "line": line_number,
                                "error": "Price is required",
                                "data": record.iter().collect::<Vec<_>>()
                            });
                            has_error = true;
                            0.0
                        } else {
                            // Remove '$', whitespace, and any hidden characters
                            let cleaned_price = price_str
                                .trim_start_matches('$')
                                .trim()
                                .replace(['\u{200B}', '\u{FEFF}', '\r', '\n'], ""); // Remove zero-width spaces, BOM, and line endings

                            match cleaned_price.parse::<f64>() {
                                Ok(p) if p >= 0.0 => p,
                                Ok(p) => {
                                    errors.push(doc! {
                                        "line": line_number,
                                        "error": format!("Invalid price: must be non-negative, got: '{}'", p),
                                        "data": record.iter().collect::<Vec<_>>()
                                    });
                                    has_error = true;
                                    0.0
                                },
                                Err(e) => {
                                    errors.push(doc! {
                                        "line": line_number,
                                        "error": format!("Invalid price format. Expected format: $X.XX, got: '{}'. Parse error: {}", price_str, e),
                                        "data": record.iter().collect::<Vec<_>>()
                                    });
                                    has_error = true;
                                    0.0
                                }
                            }
                        };

                        let category = match category_str.as_str() {
                            "electronics" => Category::Electronics,
                            "clothing" => Category::Clothing,
                            "food" => Category::Food,
                            "books" => Category::Books,
                            _ => Category::Other,
                        };

                        let has_active_sale = has_active_sale.unwrap_or(false);

                        // Only proceed with insertion if there are no errors for this record
                        if !has_error {
                            let product = Product {
                                id: None,
                                name: format!("{} {}", clean_name.clone(), sanitized_id).to_string(),
                                price,
                                category,
                                has_active_sale,
                            };

                            // Insert the product into the database
                            match collection.insert_one(product, None).await {
                                Ok(_) => success_count += 1,
                                Err(e) => {
                                    error!("Failed to insert product at line {}: {}", line_number, e);
                                    errors.push(doc! {
                                        "line": line_number,
                                        "error": format!("Database error: {}", e),
                                        "data": record.iter().collect::<Vec<_>>()
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading CSV record at line {}: {}", line_number, e);
                        errors.push(doc! {
                            "line": line_number,
                            "error": format!("Failed to parse CSV record: {}", e),
                        });
                    }
                }
                line_number += 1;
            }
        }
    }

    // Return response with results
    if errors.is_empty() {
        debug!("Successfully imported {} products", success_count);
        Ok(HttpResponse::Ok().json(doc! {
            "message": format!("Successfully imported {} products", success_count)
        }))
    } else {
        debug!("Found {} errors while importing products", errors.len());
        Ok(HttpResponse::UnprocessableEntity().json(doc! {
            "errors": errors
        }))
    }
}
