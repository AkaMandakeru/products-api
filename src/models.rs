use mongodb::bson::{oid::ObjectId, Document};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Electronics,
    Clothing,
    Food,
    Books,
    Other,
}

impl ToString for Category {
    fn to_string(&self) -> String {
        match self {
            Category::Electronics => "electronics".to_string(),
            Category::Clothing => "clothing".to_string(),
            Category::Food => "food".to_string(),
            Category::Books => "books".to_string(),
            Category::Other => "other".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Product {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub price: f64,
    pub category: Category,
    pub has_active_sale: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateProductRequest {
    pub name: String,
    pub price: f64,
    pub category: Category,
    pub has_active_sale: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateProductRequest {
    pub name: Option<String>,
    pub price: Option<f64>,
    pub category: Option<Category>,
    pub has_active_sale: Option<bool>,
}
