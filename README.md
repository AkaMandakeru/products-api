# Products JSON API

A RESTful API built with Rust, Actix-web, and MongoDB for managing product information.

## Features

- CRUD operations for products
- MongoDB integration
- Structured logging
- JSON request/response format
- Category enumeration
- Input validation

## Prerequisites

- Rust (latest stable version)
- MongoDB (running locally or accessible via URL)
- Cargo (Rust's package manager)

## Configuration

The application uses environment variables for configuration. Create a `.env` file in the root directory with the following variables:

```env
MONGODB_URI=mongodb://localhost:27017
DATABASE_NAME=products_db
```

## Building and Running

1. Clone the repository
2. Install dependencies:
```bash
cargo build
```

3. Run the server:
```bash
# Run with default logging (info level)
cargo run

# Run with debug logging
RUST_LOG=debug cargo run
```

The server will start at `http://localhost:8080`

## API Endpoints

### Products

- **GET** `/api/products` - List all products
- **GET** `/api/products/{id}` - Get a specific product
- **POST** `/api/products` - Create a new product
- **PUT** `/api/products/{id}` - Update a product
- **DELETE** `/api/products/{id}` - Delete a product

### Request/Response Examples

#### Create Product
```bash
curl -X POST http://localhost:8080/api/products \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Laptop",
    "price": 999.99,
    "category": "electronics",
    "has_active_sale": false
  }'
```

#### Product Schema
```json
{
  "name": "string",
  "price": "float",
  "category": "string (electronics|clothing|food|books|other)",
  "has_active_sale": "boolean"
}
```

## Logging

The application uses the `tracing` framework for structured logging. Log levels can be controlled via the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug   # All logs
RUST_LOG=info    # Info and above
RUST_LOG=error   # Only errors
```

## Error Handling

The API returns appropriate HTTP status codes:
- 200: Success
- 201: Created
- 404: Not Found
- 400: Bad Request
- 500: Internal Server Error

## Development

The project structure:
```
.
├── src/
│   ├── main.rs        # Application entry point and server setup
│   ├── config.rs      # MongoDB configuration
│   ├── models.rs      # Data models and schemas
│   └── handlers.rs    # Request handlers
├── Cargo.toml         # Dependencies and project metadata
├── .env              # Environment variables
└── README.md         # This file
```

## License

MIT
