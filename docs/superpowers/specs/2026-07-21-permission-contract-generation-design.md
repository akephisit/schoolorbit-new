# Permission Contract Generation Design

**Date:** 2026-07-21  
**Status:** Approved design  
**Scope:** Permission registry source-of-truth and generation only

## Context

SchoolOrbit currently maintains permission definitions in two handwritten registries:

- `backend-school/src/permissions/registry.rs` defines Rust constants, permission metadata, and `ALL_PERMISSIONS` for database synchronization.
- `frontend-school/src/lib/permissions/registry.ts` repeats permission codes and module names for route and UI authorization checks.

Static tests detect some drift after both files have been edited, but developers still need to update the same contract in multiple places. The backend registry also repeats each permission as a constant and as a `PermissionDef`. This design replaces those handwritten copies with a language-neutral canonical contract and deterministic generated outputs.

## Goals

- Make one JSON document the source of truth for permission codes and metadata.
- Generate the Rust and TypeScript permission registries from that document.
- Keep existing backend and frontend imports stable.
- Preserve every existing permission code and all runtime authorization behavior.
- Commit generated outputs so application builds do not depend on running the generator.
- Detect stale or manually edited generated files locally and in CI.
- Prevent accidental permission deletion or rename because runtime permission sync deletes database permissions absent from the registry.

## Non-goals

- Add, remove, rename, or otherwise change any permission code.
- Change role grants, authorization policies, permission resolution, cache behavior, or database synchronization behavior.
- Add or edit a database migration.
- Change an HTTP request, response, route, or frontend permission call site.
- Generate OpenAPI contracts or API clients.
- Refactor feature handlers, services, stores, or UI components.
- Redesign permission-management UI.

## Architecture

The canonical and generated flow is:

```text
contracts/permissions.json
            |
            v
scripts/generate-permissions.mjs
        +---+-----------------------+
        |                           |
        v                           v
Rust generated registry      TypeScript generated registry
        |                           |
        v                           v
Permission DB sync           Route and UI permission checks
Backend policies             PermissionCode types
```

The files are:

```text
contracts/
├── permissions.json
├── permissions.schema.json
└── permissions.lock.json

scripts/
├── generate-permissions.mjs
└── tests/generate-permissions.test.mjs

backend-school/src/permissions/
├── registry.rs
└── registry_generated.rs

frontend-school/src/lib/permissions/
├── registry.ts
└── registry.generated.ts
```

`contracts/permissions.json` is the only handwritten permission data. `permissions.schema.json` documents the structure for editors. `permissions.lock.json` is generated and records the accepted code set and contract digest. The Rust and TypeScript generated files are committed artifacts.

## Canonical contract

The document contains a schema version and an ordered permission list. A normal permission contains only the fields needed to describe its semantics and display metadata:

```json
{
  "schema_version": 1,
  "permissions": [
    {
      "module": "staff",
      "action": "read",
      "scope": "all",
      "name": "ดูบุคลากรทั้งหมด",
      "description": "ดูข้อมูลบุคลากรทั้งหมด"
    }
  ]
}
```

The generator derives the redundant identifiers:

```text
constant = STAFF_READ_ALL
code     = staff.read.all
```

This prevents `constant`, `code`, `module`, `action`, and `scope` from disagreeing. Values are generated with ASCII uppercase and underscore rules; contract components must already use canonical lowercase snake case.

Normal records omit `kind`. The wildcard is the only special record and uses the explicit shape:

```json
{
  "kind": "wildcard",
  "module": "system",
  "action": "all",
  "scope": "global",
  "name": "Super Admin Access",
  "description": "สิทธิ์ระดับสูงสุด (เข้าถึงทุกส่วน)"
}
```

It must resolve to exactly:

```text
constant = WILDCARD
code     = *
module   = system
action   = all
scope    = global
```

Permission names and descriptions remain UTF-8 strings and retain their current values byte-for-byte during the initial conversion.

## Generated Rust registry

`backend-school/src/permissions/registry.rs` remains the public module and keeps the handwritten `PermissionDef` type. It includes `registry_generated.rs`, which exports:

- `codes::*` string constants;
- `ALL_PERMISSIONS: &[PermissionDef]` with the full metadata.

Existing imports such as `use crate::permissions::registry::codes` and consumers of `ALL_PERMISSIONS` do not change. `permission_sync` continues to upsert the same rows and delete codes not present in the same logical registry.

The generated file begins with a do-not-edit header and the source contract digest. It is formatted deterministically and must pass `cargo fmt --check`.

## Generated TypeScript registry

`frontend-school/src/lib/permissions/registry.generated.ts` exports:

- `WILDCARD_PERMISSION`;
- `PERMISSION_MODULES` derived from the permission modules;
- `PERMISSIONS` derived from normal permission constants and codes;
- `PermissionCode`, `PermissionModule`, and `RoutePermission` types.

The existing `registry.ts` re-exports these generated values and types. It retains handwritten frontend-only presentation helpers such as scope labels, action labels, and tone-to-class mapping. Existing imports from `$lib/permissions/registry` remain unchanged.

The generated file begins with a do-not-edit header and the same source contract digest. Object keys are ordered deterministically so repeated generation produces no diff.

## Generator behavior

The generator uses Node.js built-ins and requires no network access. Its public operations are structured as pure load, validate, render, and compare functions so unit tests can use temporary directories without modifying repository files.

Normal generation is:

```bash
node scripts/generate-permissions.mjs
```

It performs these steps:

1. Read and parse the canonical JSON.
2. Validate its schema-equivalent structure and semantic rules.
3. Derive constants, codes, modules, and the contract SHA-256 digest.
4. Compare the proposed code set with the existing generated lock.
5. Render the lock, Rust registry, and TypeScript registry entirely in memory.
6. Write outputs only after all validation and rendering succeeds.

Verification without writes is:

```bash
node scripts/generate-permissions.mjs --check
```

Check mode renders expected content in memory, compares every generated artifact byte-for-byte, and exits non-zero if an output is absent, stale, or manually edited.

Normal generation and check mode fail when the lock is absent. The initial repository conversion establishes the unchanged baseline with the one-time command:

```bash
node scripts/generate-permissions.mjs --initialize-lock
```

Initialization fails if a lock already exists. Once committed, normal generation refuses any removal from the locked code set. A rename is treated as removal of the old code plus addition of a new code and is therefore refused. A future intentional removal requires a separate reviewed design with migration and role-grant handling; version 1 of this generator does not provide a removal bypass for an existing lock.

## Validation and errors

Validation rejects:

- missing, empty, or unknown fields;
- unsupported `schema_version`;
- components that are not canonical lowercase snake case;
- actions or scopes outside the current supported vocabulary;
- duplicate `(module, action, scope)` tuples;
- duplicate derived codes or constants;
- any wildcard record that does not match the reserved wildcard contract;
- deletion or rename of an existing locked code;
- output paths outside the explicitly configured repository files.

Errors identify the JSON path and the rejected value. For example:

```text
permissions[42].scope: unsupported scope "department";
expected one of: all, assigned, global, organization_tree,
organization_unit, own, school
```

The generator parses and renders before writing. A validation or rendering error therefore leaves all existing files untouched. Check mode never writes. Every failure exits non-zero so local scripts and CI stop immediately.

## Developer workflow

Adding a future permission uses one edit and one generation command:

```text
1. Edit contracts/permissions.json.
2. Run node scripts/generate-permissions.mjs.
3. Review the canonical and generated diffs.
4. Run focused backend/frontend checks.
5. Commit the contract, lock, and both generated registries together.
```

Application builds consume committed Rust and TypeScript files. They do not read the JSON at runtime and do not run the generator implicitly. This keeps local, container, and deployment builds reproducible and avoids adding runtime file I/O.

## Testing

Generator unit tests cover:

- valid contract rendering;
- deterministic byte-for-byte output;
- duplicate tuples, codes, and constants;
- invalid module, action, and scope values;
- invalid wildcard definitions;
- missing and unknown fields;
- stale and manually edited outputs in check mode;
- blocked deletion and rename against the generated lock;
- no partial writes after validation failure.

Cross-stack static tests use the canonical contract and generated outputs instead of treating a regular-expression parse of handwritten Rust as the source of truth. Existing guards continue to ensure:

- backend production code uses `codes::*` for known permissions;
- frontend production code uses `PERMISSIONS` or `PERMISSION_MODULES`;
- route metadata does not hardcode permission strings;
- permission codes use canonical action and scope vocabulary;
- current-user UI checks use the shared `$can` store.

Verification for the initial conversion includes:

- exact equality between the pre-conversion and canonical permission code sets;
- exact equality of existing names and descriptions;
- generator unit tests and `--check` from a clean worktree;
- `cargo fmt --all -- --check`;
- `cargo check --bin backend-school`;
- backend static architecture tests;
- frontend static tests;
- frontend `npm run check`;
- repository diff and migration-path checks.

## CI

A focused `.github/workflows/permission-contract.yml` runs when the contract, generator, registries, or their tests change. It sets up Node.js 22 and the Rust toolchain, then runs:

1. `node scripts/generate-permissions.mjs --check`;
2. generator unit tests;
3. Rust formatting and backend compile checks;
4. backend permission architecture tests;
5. frontend static tests and Svelte/TypeScript checks.

The workflow does not deploy or mutate a database. Because generated backend and frontend files are committed, their existing deployment path filters continue to trigger both application deployments when a real permission change is generated.

## Rollout and rollback

The first rollout converts only representation. It does not change the code set, metadata, role grants, or database schema. Backend startup therefore synchronizes the same permission rows it synchronized before the refactor.

Rollout evidence must show no path under `backend-school/migrations/` changed and no permission code was added or removed. Environment-dependent database, smoke, or browser tests are not required to prove representation parity, but existing available checks may still be run as additional evidence.

Rollback reverts the contract, generator, wrapper, and generated files together. Since the database contract and runtime behavior remain unchanged, rollback does not require a database migration or data repair.

## Acceptance criteria

- One canonical JSON file contains all permission semantics and display metadata.
- Every pre-existing permission code, name, and description is preserved.
- Rust and TypeScript registries are reproducible generated artifacts.
- Existing backend and frontend permission imports compile without call-site changes.
- `--check` fails for stale or hand-edited generated files.
- Default generation refuses permission removal and rename.
- Permission database synchronization behaves exactly as before.
- No migration, role grant, authorization policy, API contract, or UI behavior changes.
- Focused CI and all affected local checks pass.
