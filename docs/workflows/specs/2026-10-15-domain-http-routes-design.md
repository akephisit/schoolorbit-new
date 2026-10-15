# School HTTP route ownership

The user approved organizing HTTP by domain even when the organizational change does not improve compilation time. This design applies the existing workspace boundaries; it does not introduce new crates or change API behavior.

## Owners and composition

- Existing application-owned capabilities own their registrations in `src/modules/<feature>/routes.rs`. Existing domain factories already colocated with their feature remain with that owner.
- `app.rs` owns global prefixes, public/protected partitioning, the single shared session middleware, internal authentication, platform health routes and application-wide layers. Feature factories use paths relative to the application-owned `/api` prefix.
- Academic public context and personal/staff routes join the existing `school-academic-http` crate as concrete `Router<AcademicHttpState>` factories. Root retains its websocket adapter and Question Bank composition.
- Certificates owns its public and protected factories in `school-certificates-http`, returning `Router<CertificateHttpState>`. Public certificate body limits and response security headers travel with those public routes. Root binds the existing narrow state once before shared authentication.
- Auth route registrations stay with the root Auth module: its profile orchestration still needs application adapters. `school-auth-http` continues to own request context/cookies; this change does not move root handlers into it.

## Preserved behavior and tradeoffs

Every method/URL/handler association, public/protected classification, tenant/session rule, route-specific body limit, status, wire DTO, service, permission and database operation must remain unchanged. OpenAPI and generated TypeScript must be byte-identical. No migrations, dependency versions, production profiles or frontend changes are needed.

Moving registration within the executable improves ownership without changing its Cargo invalidation boundary. Moving concrete factories into existing HTTP crates changes codegen ownership and can shift compile cost to those crates. This task makes no deployment speed promise; any later speed claim requires a paired benchmark of application and affected domain edits. Rollback is the previous binary with the same database and API contracts.

## Verification

Review every registration against the baseline, including limits and public certificate headers. Adapt architecture guards to follow actual route owners without weakening their inventories. Exercise the real composed router for representative protected domains and public certificate boundaries using disposable fixtures. Run both Cargo feature modes, owned HTTP tests, offline byte-equal OpenAPI export and `./scripts/pipeline verify --scope auto`. Publish exact results in the PR, then follow the existing Pipeline integration and production acceptance gates.
