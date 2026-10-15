# Opt-in School OpenAPI compilation

Approved scope: retain Rust-owned OpenAPI and generated TypeScript while excluding documentation
metadata from production compilation. The user approved this approach in chat on 2026-10-15.

The existing application continues composing the document in `src/api_contract.rs`. Each existing
crate that owns schemas or path metadata exposes an opt-in `api-docs` Cargo feature, forwarding
that feature only through its existing dependency edges. Utoipa becomes optional. Runtime DTOs,
Serde attributes, handlers, authorization, errors, persistence and routes retain their owners and
behavior. No new crate, migration, dependency version or frontend contract is introduced.

The application feature enables composition and the offline `export-openapi` CLI. Without the
feature, that command fails explicitly before runtime configuration or network startup. The local
generator enables the feature automatically. Production uses the default feature set. Generated
JSON and TypeScript must remain byte-identical to the accepted baseline.

Verification covers default and documentation-enabled workspace targets, existing schema tests,
offline export, generated artifacts, default-mode HTTP/database regressions and the full local
pipeline. A paired build benchmark must measure the same runner, CPU quota, toolchain and release
profile; dependency priming and warm application/domain changes are reported separately. Removing
metadata is a hypothesis, not evidence that OpenAPI was the main bottleneck. Integrate only after
checks and a useful measured result. Rollback is a normal revert of the coherent change.
