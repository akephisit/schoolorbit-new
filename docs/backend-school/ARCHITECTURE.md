# Backend School - System Architecture

## Overview
This document outlines the architectural structure of the `backend-school` service. It is built using **Rust** with the **Axum** web framework.

## Layered Architecture
The application follows a standard layered architecture to separate concerns:

1.  **Transport Layer (Handlers)**
    *   **Location:** `src/handlers/`
    *   **Responsibility:** Handles HTTP requests, extracts parameters/body, validates input, calls the appropriate Service, and maps the result to an HTTP response.
    *   **Authentication:** Middleware is applied here (or at the router level) to ensure permissions.

2.  **Business Logic Layer (Services)**
    *   **Location:** `src/services/`
    *   **Responsibility:** Contains the core business logic. It orchestrates specific workflows, performs validations that require data access, and talks to the Repository layer.
    *   **Note:** Keep logic here, not in handlers.

3.  **Data Access Layer (Repositories)**
    *   **Location:** `src/repositories/` (or within `src/db/`)
    *   **Responsibility:** Direct interaction with the database (SQLx queries). No business logic should reside here, only CRUD operations and complex queries.

4.  **Models/Entities**
    *   **Location:** `src/models/`
    *   **Responsibility:** Structs representing database tables and domain objects.

## Database
*   **Technology:** PostgreSQL
*   **Interaction:** SQLx for compile-time checked queries.
*   **Migrations:** Managed via `sqlx-cli`.
