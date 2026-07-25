# SchoolOrbit

SchoolOrbit is a multi-tenant school management system. Its backends use Rust, Axum, SQLx, and PostgreSQL; its web applications use SvelteKit 5 and TypeScript.

## Services

- `backend-admin` manages schools, tenant database metadata, and deployment coordination. It listens on port `8080`.
- `backend-school` serves tenant school APIs, resolves each school database through backend-admin, and listens on port `8081`.
- `frontend-admin` is the administration web application.
- `frontend-school` is the tenant-facing school web application.

## Repository Map

- [Backend admin](./backend-admin/README.md)
- [Backend school](./backend-school/README.md)
- [Frontend admin](./frontend-admin/README.md)
- [Frontend school](./frontend-school/README.md)
- `contracts/` — permission and generated OpenAPI contracts
- `scripts/` — generators, smoke tests, and guarded tenant migration utilities
- `backend-school/migrations/` — active tenant schema history

## Quick Start

Create a local environment file and start the backend stack:

```bash
cp .env.example .env
docker compose up --build
```

The local Compose stack exposes backend-admin at `http://localhost:8080` and backend-school at `http://localhost:8081`. Configure and run either frontend from its own directory as described in its README.

Do not use the example secret values in a shared or production environment.

## Development Rules

[`.rules`](./.rules) is the single authoritative development standard. Read it before changing code, permissions, API contracts, migrations, security-sensitive fields, or deployment behavior.

Use the [documentation index](./docs/README.md) to find the small set of maintained references.

## Verification

[Testing](./docs/TESTING.md) contains the command matrix for backend, frontend, permission, API, database, smoke, and browser checks.

At minimum, every change ends with:

```bash
git diff --check
git status --short
```

## Operations

[Operations](./docs/OPERATIONS.md) covers topology, environment variables, health/readiness, deployment workflows, tenant cutover, encryption keys, and file storage.
