# Backend build scope

Date: 2026-11-11

Status: implementation design for Senior and independent review before source changes.

## Goal and preserved contracts

Reduce Academic HTTP rebuild scope, remove repeated generic query-observation completion code, and remove redundant release-job work. Preserve the production release profile, runtime behavior, SQL statements and operation labels, transactions, permissions, tenant/session/CSRF boundaries, realtime protocol and generated API contracts. The baseline is `2dd8a55675de48d7a24dfad2419b246238ccf6e2` on `perf/academic-http-build-scope`.

This change does not alter applied migrations, UI behavior, schema, permission registries, dependency versions, production optimization settings, or codegen-unit settings. It does not introduce boxed futures, type erasure, dynamic handler dispatch, or a runtime workaround for compiler costs.

## Academic transport ownership

Retire `school-academic-http` entirely. Six cohesive transport crates receive the existing handlers and API annotations, each with its own concrete `Clone` state and nongeneric `#[inline(never)]` route factories. Each factory compiles handler extraction, futures and response encoding in its transport owner. The executable binds each state before merging routers and applying existing middleware.

| New crate | Source owned after the move | Concrete state fields |
| --- | --- | --- |
| `school-academic-core-http` | Existing `core.rs`, `core/handlers.rs`; Core management, public context options and personal context options | `auth_runtime`, `placement_rosters`, `websocket_manager` |
| `school-academic-delivery-http` | Existing `delivery.rs`, `delivery/**`; personal activity registration routes | `auth_runtime`, `assessment_publication`, `publication_history`, `websocket_manager` |
| `school-academic-assessment-http` | Existing assessment and exam handlers, `gradebook.rs`/`gradebook/**`, `learner_evaluation.rs`/`learner_evaluation/**`; personal and staff exam routes | `auth_runtime`, `result_locks`, `websocket_manager` |
| `school-academic-timetable-http` | Existing timetable handlers, teacher handoff handlers, `mutation.rs`, `websockets.rs` and `websockets/**`; personal timetable and socket routes | HTTP: `auth_runtime`, `collaboration`, `staff_directory`, `publication_history`, `websocket_manager`; socket: `auth_runtime`, `collaboration`, `staff_directory`, concrete `websocket_manager` |
| `school-academic-results-http` | Existing `results.rs`, `results/**` | `auth_runtime`, `websocket_manager` |
| `school-academic-lifecycle-http` | Existing `lifecycle.rs`, `lifecycle/**` | `auth_runtime`, `lifecycle_providers`, `retention`, `websocket_manager` |

State names follow the owner: `AcademicCoreHttpState`, `AcademicDeliveryHttpState`, `AcademicAssessmentHttpState`, `AcademicTimetableHttpState`, `AcademicResultsHttpState`, `AcademicLifecycleHttpState`; Timetable also owns `AcademicWebSocketState`. There is no aggregate Academic state or HTTP facade.

`websocket_manager` in HTTP states retains the existing `Arc<dyn AcademicRealtimePort>`. Timetable's socket state retains `Arc<WebSocketManager>`. Existing provider interfaces remain unchanged. Domain service dependencies are intentional; a transport crate must not depend on another transport crate, including indirectly through a convenience facade, or on the executable. The existing shared session/context owner `school-auth-http` remains the sole transport-context dependency and is not an Academic sibling.

### Realtime contract admission

Create `school-academic-realtime` containing exactly the existing `realtime.rs` DTOs, mutation classification and `AcademicRealtimePort`. Its production dependencies are only `serde`, `uuid`, and `school-errors`. These stable protocol types and the narrow broadcast/tenant-registration port have six independent transport consumers and an application adapter; their ownership cannot remain with Timetable HTTP without creating sibling HTTP dependencies. This is a bounded protocol contract owner, not a common utility crate. It has no Axum, database, session runtime, collaboration implementation, or business logic dependency.

The Senior explicitly accepts a bounded exception to the literal public-interface-narrower-than-implementation admission criterion: a protocol-only crate intentionally exposes almost all of its small implementation. Its cohesive existing wire contract, independent consumers, acyclic dependency direction and separation from the concrete socket implementation justify this exception. Add meaningful serialization/round-trip and mutation-classification tests and a narrowly scoped durable ownership statement in `.rules`; this decision does not waive admission criteria for other small crates. Expected benefit is isolation from socket implementation edits, while protocol edits intentionally invalidate all consumers; record the actual before/after rebuilt packages and timings before claiming a measured benefit.

`WebSocketManager` and all socket connection/heartbeat machinery move to Timetable HTTP. The application constructs one manager and one `TimetableCollaborationRuntime`, preserving the current `manager.collaboration()` wiring. All HTTP states receive that same manager as the existing port, and Timetable HTTP mutations and sockets receive that same collaboration runtime. The application-owned `ApplicationFacilityChanges` adapter retains its concrete `Arc<WebSocketManager>` from Timetable HTTP: it invalidates through `.collaboration().invalidate_term(tenant, None)` before broadcasting through the existing event contract. Do not widen the realtime port or remove that invalidation. Permission listeners retain that same manager and runtime. No second manager is constructed by any `FromRef` implementation.

### Policy ownership

Move `academic_catalog_access_policy`, `academic_curriculum_access_policy`, and `homeroom_roster_access_policy` from HTTP to `school-academic-core::policies`. Move `learning_offering_access_policy` to `school-academic-delivery::policies`, adding only the workspace permission dependency its current implementation requires. Their queries, safe operation labels, permission codes, authorization order and errors remain unchanged.

The teacher-handoff handler currently imports `delivery::handlers::require_term_change_set_access`. Replace that HTTP-to-HTTP dependency with the Delivery-owned public operation:

```rust
pub async fn require_term_change_set_access(
    pool: &sqlx::PgPool,
    actor: &school_authorization::ActorContext,
    change_set_id: uuid::Uuid,
    action: OfferingAction,
) -> Result<Vec<uuid::Uuid>, school_errors::AppError>;
```

Place it in `school-academic-delivery::policies::learning_offering_access_policy`. It calls the existing `offerings::operational_change_offering_ids` followed by the existing batch-access policy, returning the original IDs. Delivery and Timetable handlers pass their already-resolved pool and actor. Preserve ID selection/order, database labels, empty-batch handling, not-found/forbidden behavior, and the caller's existing transaction boundaries. No SQL or business rule is added to handlers.

## Application composition and contracts

Update workspace members, workspace dependencies, root dependencies and `api-docs` feature forwarding for the six transport crates and the protocol crate. New manifests include only dependencies actually used by their owned implementation; domain crate edges are retained where legitimate. Remove the old crate directory and all active package references. Lockfile generation is serialized with the designated Cargo owner and must not upgrade versions.

`src/http_state.rs` owns six independent `FromRef<AppState>` implementations and the existing socket projection. `src/main.rs`, application adapters and test fixtures import the new manager/protocol owners. Application routing preserves this layout:

| Boundary | Composition |
| --- | --- |
| Public HTTP | Core's `/public/academic-context/options` under existing `/api`, outside the session middleware |
| Protected personal HTTP | Core context, Delivery activity registration, Assessment personal/staff exams, and Timetable personal timetable under existing `/api` |
| Protected management HTTP | Six concrete routers merged under existing `/api/academic` |
| Question bank | Existing independent router remains nested at `/api/academic/question-bank` |
| WebSocket | Timetable socket factory remains under `/ws`, authenticating its own upgrade using the existing session and tenant-origin policy |

Each `.with_state(OwnerState::from_ref(state))` runs at the composition boundary before protected routers receive the shared session layer. Global body limits, timing, CSRF/session behavior, public/protected separation and route methods remain unchanged.

`src/api_contract.rs` continues owning the complete OpenAPI document and schema registry, with handler references changed to the new transport owners. Existing wire types and operation declarations retain their names and payloads. Canonical OpenAPI must equal the baseline exactly; generated frontend API and permission files must remain byte-identical. Source-path guards in frontend static tests change only to follow actual ownership and retain their assertions.

## Nongeneric query-observation completion

Change the private `school-database::QueryObservation::complete<T>(&Result<T, Error>)` to `complete(Option<&Error>)`; `observed<T>` calls it with `result.as_ref().err()`. The typed query APIs and `observed<T>` remain generic because callers need their concrete SQLx results. The completion boundary is nongeneric; use `#[inline(never)]` to keep its status/error classification outside per-result code generation.

`None` marks success. `Some(error)` sets `status = "error"` and uses the existing fixed safe `error_class`. Drop still emits the same static labels and elapsed duration and retains `"cancelled"` when the future is dropped before completion. No error payload, bind, SQL text, tenant credential or query result is formatted. Borrow/transaction ownership and cancellation semantics remain unchanged. Preserve the meaningful success/error/cancellation and secret-exclusion tests.

## Release work ownership

The Pipeline writer owns `scripts/**`, `.github/**` and `docs/OPERATIONS.md`. It removes only evidenced redundant build/release work and updates source/package probes for the new owners. Build provenance, immutable inputs, required production checks, image/cache behavior and deployment gates remain intact. Its concrete script/workflow changes and focused checks are reviewed with this implementation; Backend does not edit those paths.

## Evidence and acceptance

One designated owner serializes all Cargo/compiler work against the same compatible target. Before and after comparisons use the same source path, toolchain, machine, profile, dependency versions and concurrency. Prime dependency/feature graphs separately, distinguish priming/no-op/cache reuse from source-changing samples, and record actual rebuilt packages and durations.

Measure the old all-Academic-HTTP invalidation case against source edits in each new domain HTTP owner. Also measure no-op, application-only, domain-service and shared-protocol edits to expose remaining and intentional fan-out. Preserve the production profile and report compile-scope tradeoffs without claiming runtime improvement from compile data. The protocol owner intentionally fans out when its shared wire contract changes.

Acceptance requires workspace compilation for all targets with default and `api-docs` features; production panic-policy Clippy; affected HTTP, Core/Delivery policy and database unit tests; architecture guards proving complete route coverage, concrete nongeneric factories, state binding, allowed dependency directions and old-owner retirement; protected transport/session/tenant/CSRF tests; socket security and collaboration fixtures; query result/rollback/cancellation observation fixtures; focused existing affected service fixtures selected independently by QA; and exact baseline OpenAPI plus unchanged generated-contract checks. The implementation plan records commands and execution ownership. Failures are repaired at their owning source, with no snapshot blessing or weakened assertions.

## Tradeoffs and rollback

Six transport crates increase manifest and composition entries but contain independent handler code generation and follow the six established academic domain owners. Shared auth, domain-service and protocol changes still legitimately fan out. Narrow independent states prevent unused provider edges from reconnecting the whole transport graph. Link cost may remain; measurements determine the actual benefit.

No data migration or runtime cutover is needed. Reverting the source change restores the previous package graph; accepted database state and all runtime contracts remain compatible throughout.
