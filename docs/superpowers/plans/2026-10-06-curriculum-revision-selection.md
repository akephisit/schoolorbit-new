# Curriculum revision selection implementation

1. Add a forward migration preserving explicit edition years and legacy year metadata before retiring curriculum-version calendar bounds.
2. Update the Academic Core models, edition services, overview and program options. Remove calendar checks from homeroom, student, promotion, activation and delivery consumers while preserving their other guards.
3. Correct the verified legacy hierarchy by level with explicit source reconciliation and stable program/requirement identities.
4. Update curriculum presentation and creation, and show/filter edition-qualified program choices during homeroom arrangement. Generate OpenAPI and TypeScript from Rust owners.
5. Update meaningful migration and lifecycle fixtures and browser coverage. Run `cargo fmt --all -- --check`, focused tests, `cargo test --test static_architecture`, `cargo check --workspace --all-targets`, API contract generation/check/test, frontend lint/check/static and focused Playwright checks.
6. Integrate only when required checks pass. Release matched backend/frontend through the existing maintenance and centralized migration workflow; report any unavailable deployment credential or failed gate without touching production schema.

## Verification checkpoint

- Implementation and generated OpenAPI/TypeScript are present on `fix/curriculum-revision-selection`; production hierarchy and schema are unchanged.
- Workspace/all-target Rust compilation, the focused published-edition selection unit test, Svelte type checks, API generator tests and ten focused curriculum static tests passed.
- Twelve curriculum/detail browser scenarios and the room-selection scenario in desktop/mobile light/dark passed. The latter verifies choosing revision 2569 in academic year 2572, refusing unrelated grade options and preventing dialog overflow.
- The full frontend static run exposed one retired year-label assertion, now corrected, and eight environment failures caused by missing `envsubst` and `podman-compose`. The focused curriculum tests pass after the correction.
- The database runner refuses to start because native Podman is unavailable. Migration 091/092 fixture execution, real-data rehearsal and recovery evidence remain required before integration or release.
- Full lint exposed an unused create-options helper, now removed; the focused lint rerun passed. All 191 backend architecture tests passed. The API artifact recheck was started; confirm its result before recording it as passed. Full lint and the applicable checks must pass for the final integration tree.
