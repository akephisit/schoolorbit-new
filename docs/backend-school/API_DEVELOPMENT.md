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
*   **Permission:** Load the actor context in the handler and call `actor.require_*` if the endpoint is protected.

## Generated API contracts

Rust request/response DTOs and OpenAPI handler metadata are the source of truth.
`contracts/openapi/school-api.json` and files under
`frontend-school/src/lib/api/generated/` are generated files; do not edit them
directly.

After changing a documented DTO or endpoint:

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Commit Rust source, OpenAPI, generated TypeScript, and focused tests together.
Frontend API modules import generated wire DTOs and may map them to separate
domain/view models. Generation must not require database credentials or start
the backend server.

## Error Handling
*   Use the custom `AppError` type (if available) to map errors to HTTP status codes.
*   Internal DB errors should generally result in `500 Internal Server Error`, while validation failures result in `400 Bad Request`.

## Authentication & Permissions

### System Overview
The system uses a **Permission-Based Access Control (PBAC)** model. Users are assigned Roles, and Roles have Permissions.
*   **Authentication:** Handled by `auth_middleware` (validates JWT/Cookie).
*   **Authorization:** Handled explicitly within each handler using `utils::request_context` helpers and `ActorContext` methods.

### Implementing Permission Checks
To enforce that a user must have a specific permission (e.g., `staff.create.all`) to use an endpoint, follow this pattern inside your handler function:

```rust
use axum::{extract::State, http::HeaderMap, Json};
use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

pub async fn my_protected_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<MyResponse>>, AppError> {
    // 1. Resolve tenant + actor through the central request context helper.
    let context = actor_tenant_context(&state, &headers).await?;

    // 2. Enforce permission through ActorContext.
    context.actor.require_permission(codes::MY_FEATURE_READ_ALL)?;

    // 3. Proceed with business logic
    // Use context.tenant.pool for service calls and context.actor.user_id when needed.
    let result = my_service::load(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(result)))
}
```

Use `tenant_context(&state, &headers).await?` or `tenant_pool(&state, &headers).await?` for public tenant routes that do not need an actor. Do not call `resolve_tenant_pool`, `load_actor_context`, `pool_manager.get_pool`, or local `get_pool` helpers from feature handlers; those lower-level APIs are wrapped by `utils::request_context`.

### Defining New Permissions
1.  **Source Registry:** Add the permission to `contracts/permissions.json` using the canonical `module.action.scope` shape.
2.  **Generate Registries:** From `frontend-school`, run `npm run generate:permissions`, `npm run check:permissions`, and `npm run test:permissions`.
3.  **Commit Generated Artifacts:** Commit the permission contract, lock file, backend registry, and frontend registry together. Never edit `backend-school/src/permissions/registry_generated.rs` or `frontend-school/src/lib/permissions/registry.generated.ts` directly.
4.  **Database Migration:** Add new database permission rows through a new sequential migration after `001_baseline.sql`; do not edit an already-applied migration.
5.  **Usage:** load `ActorContext` once through `actor_tenant_context(...)`, then call `actor.require_permission(codes::MY_FEATURE_READ_ALL)` or `actor.require_any_permission(&[...])`.

## Logging

Runtime code should use `tracing::debug!`, `tracing::info!`, `tracing::warn!`, or `tracing::error!`. Avoid `println!` and `eprintln!` outside intentional CLI/bin output. Do not log plaintext PII, national IDs, credentials, tokens, database URLs, or raw request bodies.
