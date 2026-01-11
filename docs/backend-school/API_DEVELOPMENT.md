# Backend School - API Development Guide

## Workflow: Adding a New Endpoint

To add a new API endpoint, follow these steps to ensure consistency:

### 1. Define the Protocol
Decide on the HTTP Method (GET, POST, PUT, DELETE) and the URL path.
*   *Example:* `GET /api/staff/:id`

### 2. Create the Request/Response Models
If the endpoint expects a JSON body or returns a JSON response, define the structs.
*   **Location:** `src/models/` or inside the handler file if specific to that handler.
*   **Derive Macros:** `#[derive(Serialize, Deserialize, Validate)]`

### 3. Implement the Repository Method
If you need new data, write the SQL query in the repository layer.
*   **File:** e.g., `src/repositories/staff_repo.rs`

### 4. Implement the Service Logic
Call the repository and apply any business rules.
*   **File:** e.g., `src/services/staff_service.rs`

### 5. Create the Handler
Extract data from the Request, call the Service, and handle errors.
*   **File:** e.g., `src/handlers/staff.rs`
*   **Return Type:** Use `AxumResult<Json<YourResponse>>` (or standard `Result`).

### 6. Register the Route
Add the new handler to the router configuration.
*   **File:** `src/routes.rs` (or `main.rs` depending on setup).
*   **Middleware:** Don't forget to wrap it in `check_permission` if needed.

## Error Handling
*   Use the custom `AppError` type (if available) to map errors to HTTP status codes.
*   Internal DB errors should generally result in `500 Internal Server Error`, while validation failures result in `400 Bad Request`.

## Authentication & Permissions

### System Overview
The system uses a **Permission-Based Access Control (PBAC)** model. Users are assigned Roles, and Roles have Permissions.
*   **Authentication:** Handled by `auth_middleware` (validates JWT/Cookie).
*   **Authorization:** Handled explicitly within each Handler using `check_permission`.

### Implementing Permission Checks
To enforce that a user must have a specific permission (e.g., `staff.create.all`) to use an endpoint, follow this pattern inside your handler function:

```rust
use crate::middleware::permission::check_permission;
// or use the local helper if available in your handler module
// use check_user_permission; 

pub async fn my_protected_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    // 1. Get Database Pool (Standard flow)
    let subdomain = match extract_subdomain_from_request(&headers) { ... };
    let db_url = match get_school_database_url(...).await { ... };
    let pool = match state.pool_manager.get_pool(...).await { ... };

    // 2. Enforce Permission
    // Returns the User object if allowed, or an Error Response immediately if denied.
    let user = match check_permission(&headers, &pool, "my.feature.read").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    // 3. Proceed with business logic
    // You now have access to the authenticated 'user' object
}
```

### Defining New Permissions
1.  **Database:** Permissions are stored in the `permissions` table. Use a migration or a seed script to add new rows if you are creating a new system module.
2.  **Code Registry:** Add the permission constant to `src/permissions/registry.rs` (recommended) to avoid magic strings.
    ```rust
    // src/permissions/registry.rs
    pub const MY_FEATURE_READ: &str = "my.feature.read";
    ```
3.  **Usage:** `check_permission(&headers, &pool, codes::MY_FEATURE_READ).await`
