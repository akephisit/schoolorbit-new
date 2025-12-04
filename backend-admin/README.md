# SchoolOrbit Backend

Backend API server for SchoolOrbit built with Ohkami web framework.

## Tech Stack

- **Framework**: Ohkami (Rust web framework)
- **Runtime**: Tokio
- **Serialization**: Serde + Serde JSON

## Getting Started

### Prerequisites

- Rust 1.70+ and Cargo installed
- Run `rustup update` to ensure you have the latest stable Rust

### Installation

```bash
cd backend
cargo build
```

### Running the Server

```bash
cargo run
```

The server will start on `http://localhost:8080`

### Available Endpoints

- `GET /` - API information
- `GET /health` - Health check endpoint
- `GET /api/hello/:name` - Test endpoint with path parameter

### Testing the API

```bash
# Check API info
curl http://localhost:8080/

# Health check
curl http://localhost:8080/health

# Test hello endpoint
curl http://localhost:8080/api/hello/John
```

## Development

### Hot Reload

For development with hot reload, install cargo-watch:

```bash
cargo install cargo-watch
cargo watch -x run
```

## Project Structure

```
backend/
├── src/
│   └── main.rs       # Main application entry point
├── Cargo.toml        # Dependencies and project metadata
└── README.md         # This file
```

## Next Steps

- [ ] Add database connection (PostgreSQL/MySQL)
- [ ] Implement authentication & authorization
- [ ] Add CORS middleware
- [ ] Create API routes for school management
- [ ] Add environment configuration
- [ ] Implement logging
- [ ] Add API documentation (OpenAPI)
