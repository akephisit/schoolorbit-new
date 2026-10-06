# Curriculum edition hierarchy

The user approved all eight changes in chat and authorized implementation, preservation, verification and coordinated production deployment. No additional approval is needed for those changes.

## Canonical ownership

School curriculum editions own their educational levels; levels own study programs, term slots and course/activity requirements. Edition selection precedes level and program selection. Revision year identifies an edition and never limits its use in later academic years.

Use `curriculum_editions` for the shared school edition and `curriculum_levels` for its educational-level structure. Level UUIDs map from existing curriculum-version UUIDs; program, slot and requirement UUIDs stay stable. Edition metadata has one database owner. Level metadata and covered grades are copied from the existing level root into each existing edition-level structure. Retire the old root/version runtime contracts after the coordinated cutover.

Published editions are immutable. Publication validates every included level/program and publishes the complete edition atomically. New editions and copied programs start as drafts. Room, student and offering references remain pinned to their existing study-program UUIDs. Copying creates new child identities and never changes source data or placements.

School-scoped curriculum read/manage permissions govern the entire feature. Remove curriculum and program organization ownership; unrelated catalog and organization permissions remain with their existing owners. Preserve existing curriculum access through explicit migration of retired curriculum grants to the corresponding school permission. Reader pages must not request management-only options.

## User flow

The main route lists all editions, including drafts, with revision year/name/status and an add-edition action. An edition detail shows only its levels and an add-level action. A level detail shows only its programs, covered grades, terms and requirements, with add-program and copy-program-from-existing-edition actions. Remove curriculum-code, English-name and owner inputs; generate internal codes server-side. Room selection chooses a published edition and a program covering the selected grade.

Program copying explicitly selects a published source edition, level and program. Preserve existing term semantics and map them to matching destination slots; reject incompatible grades or slot mappings. Copy actual course/activity requirements and ordering; do not invent missing term-2 data. Validate stale source/destination revisions and published destination refusal.

## Migration and recovery

Forward-only migration from schema 92. Group known edition members only by explicit normalized edition identity and publication state; unknown revision years remain unknown and isolated. Preserve a root with no version as a named draft requiring its revision year before publication. Preserve populated drafts and unassigned levels during migration. The user subsequently identified the draft 2570 and empty TEST/ต้นยาก root as experimental and authorized their cleanup before migration; an exact-identity/reference-guarded transaction removed only those two records and their copied draft children, retaining the five published real plans.

Snapshot complete requirement content, program identities and live room/student/offering references before transformation; verify exact equality afterward. Preserve original level metadata in migration provenance. Rebind progression scope to the corresponding edition-level identity without losing configured rules. Ambiguous mapping refuses atomically. Replace all schema-dependent trigger functions and restore publication guards before committing.

After this migration, pre-cutover binaries are prohibited. Rehearse the central migration runner on disposable copies of every active tenant. Retain a fresh protected recovery point for at least seven days, using the documented encrypted-archive/no-compute-branch fallback if snapshot quota is full. Deploy backend, generated contracts, migrations and both tenant frontends together under maintenance; open traffic only after migration equality, authenticated read-only smoke and frontend mount acceptance. Recover forward after cutover and account for subsequent writes.

## Verification

Cover migration identity/content/reference preservation, populated drafts, unknown years, ambiguity refusal, permission allow/deny, immutable published editions, atomic publication, program copy isolation/grade/term/revision guards and later-year room selection. Run Rust format, architecture and workspace checks, generated permission/API contract verification, frontend lint/type/static suites and production-build desktop/mobile light/dark browser flows. Native rootless Podman CI owns routine database tests when local Podman is unavailable; provider-copy rehearsal uses only the centralized runner.

## Crate evaluation

Keep the implementation in the existing academic-core crate: editions, levels, programs and their validation share its current persistence and authorization boundary. A new crate would add a split without reducing affected consumer compilation or introducing an independent cohesive owner.
