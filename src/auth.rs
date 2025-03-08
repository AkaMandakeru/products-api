use actix_web::{web, HttpResponse, Error, error::ErrorUnauthorized, dev::{Service, Transform, ServiceRequest, ServiceResponse}};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation, errors::Error as JwtError};
use mongodb::{Collection, bson::{doc, oid::ObjectId}};
use serde::{Deserialize, Serialize};
use validator::Validate;
use tracing::{debug, error, info};
use std::{
    future::{Ready, Future},
    pin::Pin,
    task::{Context, Poll},
};
use futures_util::future::{ok, Ready as FutureReady};

use crate::config::MongoConfig;

const JWT_SECRET: &[u8] = b"your-secret-key"; // In production, use environment variable
const REFRESH_SECRET: &[u8] = b"your-refresh-secret-key"; // In production, use environment variable

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub password_hash: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 2))]
    pub first_name: String,
    #[validate(length(min = 2))]
    pub last_name: String,
    #[validate(length(min = 6))]
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub refresh_token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,     // User ID
    pub exp: i64,        // Expiration time
    pub iat: i64,        // Issued at
}

pub async fn register(
    db: web::Data<MongoConfig>,
    user_data: web::Json<RegisterRequest>,
) -> Result<HttpResponse, Error> {
    // Validate request
    if let Err(errors) = user_data.validate() {
        return Ok(HttpResponse::BadRequest().json(errors));
    }

    let collection: Collection<User> = db.database.collection("users");

    // Check if email already exists
    if let Ok(Some(_)) = collection
        .find_one(doc! { "email": &user_data.email }, None)
        .await
    {
        return Ok(HttpResponse::BadRequest().json(doc! {
            "message": "Email already registered"
        }));
    }

    // Hash password
    let password_hash = hash(user_data.password.as_bytes(), DEFAULT_COST).map_err(|e| {
        error!("Failed to hash password: {}", e);
        actix_web::error::ErrorInternalServerError("Password hashing failed")
    })?;

    let user = User {
        id: None,
        email: user_data.email.clone(),
        first_name: user_data.first_name.clone(),
        last_name: user_data.last_name.clone(),
        password_hash,
    };

    // Insert user
    let result = collection.insert_one(&user, None).await.map_err(|e| {
        error!("Failed to insert user: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to create user")
    })?;

    let user_id = result.inserted_id.as_object_id().unwrap();

    info!("Created new user with ID: {}", user_id);
    Ok(HttpResponse::Created().json(doc! {
        "message": "User registered successfully",
        "id": user_id.to_string()
    }))
}

pub async fn login(
    db: web::Data<MongoConfig>,
    credentials: web::Json<LoginRequest>,
) -> Result<HttpResponse, Error> {
    let collection: Collection<User> = db.database.collection("users");

    // Find user by email
    let user = match collection
        .find_one(doc! { "email": &credentials.email }, None)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })? {
        Some(user) => user,
        None => return Ok(HttpResponse::Unauthorized().json(doc! {
            "message": "Invalid credentials"
        })),
    };

    // Verify password
    if !verify(&credentials.password, &user.password_hash).map_err(|e| {
        error!("Password verification error: {}", e);
        actix_web::error::ErrorInternalServerError("Password verification failed")
    })? {
        return Ok(HttpResponse::Unauthorized().json(doc! {
            "message": "Invalid credentials"
        }));
    }

    // Generate tokens
    let user_id = user.id.as_ref().unwrap();
    let (token, refresh_token) = generate_tokens(user_id).await?;

    let user_response = UserResponse {
        id: user_id.to_string(),
        email: user.email,
        first_name: user.first_name,
        last_name: user.last_name,
    };

    Ok(HttpResponse::Ok().json(AuthResponse {
        token,
        refresh_token,
        user: user_response,
    }))
}

pub async fn refresh_token(
    req: web::Json<RefreshTokenRequest>,
) -> Result<HttpResponse, Error> {
    // Verify refresh token
    let claims = match decode::<Claims>(
        &req.refresh_token,
        &DecodingKey::from_secret(REFRESH_SECRET),
        &Validation::default(),
    ) {
        Ok(token_data) => token_data.claims,
        Err(e) => {
            error!("Token verification error: {}", e);
            return Ok(HttpResponse::Unauthorized().json(doc! {
                "message": "Invalid refresh token"
            }));
        }
    };

    // Generate new tokens
    let user_id = ObjectId::parse_str(&claims.sub).map_err(|e| {
        error!("Failed to parse ObjectId: {}", e);
        actix_web::error::ErrorInternalServerError("Invalid user ID format")
    })?;

    let (token, refresh_token) = generate_tokens(&user_id).await?;

    Ok(HttpResponse::Ok().json(doc! {
        "token": token,
        "refresh_token": refresh_token
    }))
}

pub async fn generate_tokens(user_id: &ObjectId) -> Result<(String, String), Error> {
    let now = Utc::now();

    // Access token (2 hours)
    let access_claims = Claims {
        sub: user_id.to_string(),
        exp: (now + Duration::hours(2)).timestamp(),
        iat: now.timestamp(),
    };

    // Refresh token (7 days)
    let refresh_claims = Claims {
        sub: user_id.to_string(),
        exp: (now + Duration::days(7)).timestamp(),
        iat: now.timestamp(),
    };

    let token = encode(
        &Header::default(),
        &access_claims,
        &EncodingKey::from_secret(JWT_SECRET),
    ).map_err(|e| {
        error!("Token generation error: {}", e);
        actix_web::error::ErrorInternalServerError("Token generation failed")
    })?;

    let refresh_token = encode(
        &Header::default(),
        &refresh_claims,
        &EncodingKey::from_secret(REFRESH_SECRET),
    ).map_err(|e| {
        error!("Refresh token generation error: {}", e);
        actix_web::error::ErrorInternalServerError("Refresh token generation failed")
    })?;

    Ok((token, refresh_token))
}

pub fn verify_token(token: &str) -> Result<Claims, JwtError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

// Auth middleware implementation
pub struct AuthMiddleware;

impl Default for AuthMiddleware {
    fn default() -> Self {
        AuthMiddleware
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareMiddleware<S>;
    type Future = FutureReady<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthMiddlewareMiddleware { service })
    }
}

pub struct AuthMiddlewareMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let auth_header = req.headers().get("Authorization");

        let auth_header = match auth_header {
            Some(header) => header,
            None => {
                return Box::pin(async move {
                    Err(ErrorUnauthorized("No authorization header"))
                });
            }
        };

        let auth_str = match auth_header.to_str() {
            Ok(str) => str,
            Err(_) => {
                return Box::pin(async move {
                    Err(ErrorUnauthorized("Invalid authorization header"))
                });
            }
        };

        if !auth_str.starts_with("Bearer ") {
            return Box::pin(async move {
                Err(ErrorUnauthorized("Invalid authorization header format"))
            });
        }

        let token = &auth_str[7..];

        match verify_token(token) {
            Ok(claims) => {
                // Store claims in request extensions if needed
                // req.extensions_mut().insert(claims);
                let fut = self.service.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    Ok(res)
                })
            }
            Err(_) => Box::pin(async move {
                Err(ErrorUnauthorized("Invalid token"))
            }),
        }
    }
}

// Factory for AuthMiddleware
pub fn auth_middleware<S>(
    service: S,
) -> AuthMiddlewareMiddleware<S> {
    AuthMiddlewareMiddleware { service }
}
