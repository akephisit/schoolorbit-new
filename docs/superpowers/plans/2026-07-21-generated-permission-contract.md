# Generated Permission Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the handwritten Rust and TypeScript permission registries with deterministic checked-in artifacts generated from one canonical JSON contract without changing any existing permission or runtime behavior.

**Architecture:** A Node.js built-in-only generator validates `contracts/permissions.json`, derives canonical codes/constants/modules, guards the accepted code set with a generated lock, and renders Rust and TypeScript registries. Thin handwritten wrapper files preserve every current import while a focused CI workflow verifies generation, compilation, and cross-stack guards.

**Tech Stack:** Node.js 22 ESM and `node:test`, JSON Schema 2020-12, Rust/Axum registry code, TypeScript/SvelteKit 5, GitHub Actions.

## Global Constraints

- Preserve every existing permission code, name, description, role grant, and authorization behavior byte-for-byte.
- Add, remove, or rename no permission in this change.
- Add or edit no file under `backend-school/migrations/`.
- Keep `crate::permissions::registry::{codes, ALL_PERMISSIONS}` and `$lib/permissions/registry` imports stable.
- Use only Node.js built-ins in the generator; generation requires no network and application builds do not run it implicitly.
- Commit the contract lock and both generated registries.
- `--check` never writes; validation/rendering failures leave existing outputs untouched.
- Normal generation refuses a code removal or rename; version 1 has no removal bypass for an existing lock.
- Keep frontend-only scope/action presentation helpers handwritten and behaviorally unchanged.
- Do not add OpenAPI generation, refactor feature handlers/services, or change permission UI in this plan.

---

## File map

### New files

- `contracts/permissions.json` — the only handwritten permission definitions and metadata.
- `contracts/permissions.schema.json` — editor-facing structural schema for the contract.
- `contracts/permissions.lock.json` — generated accepted code set and normalized-contract SHA-256.
- `scripts/generate-permissions.mjs` — pure validation/rendering functions plus CLI.
- `scripts/tests/generate-permissions.test.mjs` — generator, lock, check-mode, and write-safety unit tests.
- `backend-school/src/permissions/registry_generated.rs` — generated `codes` and `ALL_PERMISSIONS`.
- `frontend-school/src/lib/permissions/registry.generated.ts` — generated constants, module values, and types.
- `.github/workflows/permission-contract.yml` — focused drift/compile/static CI.

### Modified files

- `backend-school/src/permissions/registry.rs` — retain `PermissionDef`; include generated Rust data.
- `backend-school/tests/static_architecture.rs` — replace handwritten-registry parsing assumptions with generated-registry/wrapper guards.
- `frontend-school/src/lib/permissions/registry.ts` — re-export generated contract; retain presentation helpers.
- `frontend-school/tests/static/api-global-contract.test.mjs` — make canonical JSON the cross-stack reference while retaining usage guards.
- `frontend-school/package.json` — add ergonomic generation/check/test scripts.
- `.rules` — document the one-source permission workflow.
- `docs/TESTING.md` — document local and CI permission checks.
- `docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md` — record completion of this developer-flexibility slice without marking general API generation complete.

---

### Task 1: Build and test the deterministic generator

**Files:**
- Create: `contracts/permissions.schema.json`
- Create: `scripts/generate-permissions.mjs`
- Create: `scripts/tests/generate-permissions.test.mjs`

**Interfaces:**
- Consumes: JSON objects with `{ schema_version: 1, permissions: PermissionInput[] }`.
- Produces: `validateAndNormalizeContract(value)`, `renderPermissionArtifacts(normalized)`, and `generatePermissions(options)` named exports.
- `generatePermissions(options)` accepts exact paths plus `check` and `initializeLock` booleans and returns `{ changedPaths, digest, permissionCount }`.
- CLI accepts no flag, `--check`, or `--initialize-lock`; other flags fail before file access.

- [ ] **Step 1: Write failing generator tests**

Create tests that import the three named functions and exercise a small valid contract containing one wildcard and two normal permissions. Use `mkdtemp`, `tmpdir`, and explicit temporary output paths. Include these exact behavioral cases:

```js
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
	generatePermissions,
	renderPermissionArtifacts,
	validateAndNormalizeContract
} from '../generate-permissions.mjs';

const validContract = {
	schema_version: 1,
	permissions: [
		{
			kind: 'wildcard',
			module: 'system',
			action: 'all',
			scope: 'global',
			name: 'Super Admin Access',
			description: 'สิทธิ์ระดับสูงสุด'
		},
		{
			module: 'staff',
			action: 'read',
			scope: 'all',
			name: 'ดูบุคลากร',
			description: 'ดูข้อมูลบุคลากร'
		},
		{
			module: 'staff_profile',
			action: 'read',
			scope: 'own',
			name: 'ดูโปรไฟล์ตนเอง',
			description: 'ดูโปรไฟล์ของตนเอง'
		}
	]
};

test('derives canonical constants codes and modules', () => {
	const normalized = validateAndNormalizeContract(validContract);
	assert.deepEqual(
		normalized.permissions.map(({ constant, code }) => [constant, code]),
		[
			['WILDCARD', '*'],
			['STAFF_READ_ALL', 'staff.read.all'],
			['STAFF_PROFILE_READ_OWN', 'staff_profile.read.own']
		]
	);
	assert.deepEqual(normalized.modules, ['staff', 'staff_profile', 'system']);
});

test('renders byte-identical artifacts for identical input', () => {
	const normalized = validateAndNormalizeContract(validContract);
	assert.deepEqual(renderPermissionArtifacts(normalized), renderPermissionArtifacts(normalized));
});

for (const [label, mutate, message] of [
	['duplicate tuple', (value) => value.permissions.push({ ...value.permissions[1] }), /duplicate/],
	['invalid module', (value) => (value.permissions[1].module = 'Staff'), /snake_case/],
	['unsupported action', (value) => (value.permissions[1].action = 'download'), /unsupported action/],
	['unsupported scope', (value) => (value.permissions[1].scope = 'department'), /unsupported scope/],
	['invalid wildcard', (value) => (value.permissions[0].scope = 'school'), /wildcard/],
	['unknown field', (value) => (value.permissions[1].code = 'staff.read.all'), /unknown field/]
]) {
	test(`rejects ${label}`, () => {
		const value = structuredClone(validContract);
		mutate(value);
		assert.throws(() => validateAndNormalizeContract(value), message);
	});
}
```

Add filesystem coverage using explicit paths and content snapshots:

```js
async function temporaryPaths(contract = validContract) {
	const root = await mkdtemp(path.join(tmpdir(), 'permission-generator-'));
	const paths = {
		contractPath: path.join(root, 'permissions.json'),
		lockPath: path.join(root, 'permissions.lock.json'),
		rustOutputPath: path.join(root, 'registry_generated.rs'),
		typeScriptOutputPath: path.join(root, 'registry.generated.ts')
	};
	await writeFile(paths.contractPath, `${JSON.stringify(contract, null, 2)}\n`, 'utf8');
	return paths;
}

async function outputContents(paths) {
	return Promise.all([
		readFile(paths.lockPath, 'utf8'),
		readFile(paths.rustOutputPath, 'utf8'),
		readFile(paths.typeScriptOutputPath, 'utf8')
	]);
}

test('initializes a missing lock exactly once', async () => {
	const paths = await temporaryPaths();
	await assert.rejects(generatePermissions(paths), /initialize-lock/);
	await generatePermissions({ ...paths, initializeLock: true });
	await assert.rejects(
		generatePermissions({ ...paths, initializeLock: true }),
		/already exists/
	);
});

test('allows additions and metadata changes without allowing removals', async () => {
	const paths = await temporaryPaths();
	await generatePermissions({ ...paths, initializeLock: true });
	const expanded = structuredClone(validContract);
	expanded.permissions[1].description = 'คำอธิบายใหม่';
	expanded.permissions.push({
		module: 'staff', action: 'update', scope: 'all',
		name: 'แก้ไขบุคลากร', description: 'แก้ไขข้อมูลบุคลากร'
	});
	await writeFile(paths.contractPath, `${JSON.stringify(expanded, null, 2)}\n`, 'utf8');
	await generatePermissions(paths);
	const reduced = structuredClone(expanded);
	reduced.permissions.splice(1, 1);
	await writeFile(paths.contractPath, `${JSON.stringify(reduced, null, 2)}\n`, 'utf8');
	await assert.rejects(generatePermissions(paths), /refusing to remove.*staff\.read\.all/s);
});

test('check mode reports but never rewrites stale output', async () => {
	const paths = await temporaryPaths();
	await generatePermissions({ ...paths, initializeLock: true });
	const edited = `${await readFile(paths.typeScriptOutputPath, 'utf8')}// manual edit\n`;
	await writeFile(paths.typeScriptOutputPath, edited, 'utf8');
	await assert.rejects(generatePermissions({ ...paths, check: true }), /stale/);
	assert.equal(await readFile(paths.typeScriptOutputPath, 'utf8'), edited);
});

test('validation failure leaves every generated output unchanged', async () => {
	const paths = await temporaryPaths();
	await generatePermissions({ ...paths, initializeLock: true });
	const before = await outputContents(paths);
	const invalid = structuredClone(validContract);
	invalid.permissions[1].scope = 'department';
	await writeFile(paths.contractPath, `${JSON.stringify(invalid, null, 2)}\n`, 'utf8');
	await assert.rejects(generatePermissions(paths), /unsupported scope/);
	assert.deepEqual(await outputContents(paths), before);
});
```

- [ ] **Step 2: Run the tests to verify the missing module failure**

Run:

```bash
node --test scripts/tests/generate-permissions.test.mjs
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND` for `scripts/generate-permissions.mjs`.

- [ ] **Step 3: Add the schema and minimal complete generator**

Use JSON Schema draft 2020-12. Require exactly `schema_version` and `permissions` at the root, reject additional properties, and define normal and wildcard records as separate `oneOf` branches. Use these exact vocabularies:

```js
const ALLOWED_ACTIONS = new Set([
	'all', 'approve', 'assign', 'create', 'delete', 'enroll', 'evaluate', 'execute',
	'manage', 'manage_members', 'publish', 'read', 'remove', 'request', 'scores', 'update',
	'verify'
]);
const ALLOWED_SCOPES = new Set([
	'all', 'assigned', 'global', 'organization_tree', 'organization_unit', 'own', 'school'
]);
const COMPONENT_PATTERN = /^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$/;
```

Define the exported API with JSDoc matching these exact interface contracts and no hidden global paths:

```ts
type NormalizedPermission = {
	kind: 'wildcard' | 'permission';
	constant: string;
	code: string;
	module: string;
	action: string;
	scope: string;
	name: string;
	description: string;
};

type NormalizedContract = {
	schema_version: 1;
	permissions: NormalizedPermission[];
	modules: string[];
};

type RenderedArtifacts = {
	digest: string;
	lockContent: string;
	rustContent: string;
	typeScriptContent: string;
};

export declare function validateAndNormalizeContract(value: unknown): NormalizedContract;
export declare function renderPermissionArtifacts(normalized: NormalizedContract): RenderedArtifacts;
export declare function generatePermissions(options: {
	contractPath: string;
	lockPath: string;
	rustOutputPath: string;
	typeScriptOutputPath: string;
	check?: boolean;
	initializeLock?: boolean;
}): Promise<{ changedPaths: string[]; digest: string; permissionCount: number }>;
```

Implementation requirements:

- Derive `code` as `${module}.${action}.${scope}` and `constant` as the uppercase underscore form.
- Preserve wildcard first and sort all normal permissions by derived constant.
- Derive unique modules from every record and sort their uppercase keys.
- Compute SHA-256 from `JSON.stringify(normalizedContract)` rather than raw whitespace.
- Render a lock with `schema_version`, `contract_sha256`, and sorted `permission_codes`.
- Render Rust literals with a dedicated escape function for backslash, quote, newline, carriage return, tab, and control characters using Rust `\u{...}` syntax; preserve ordinary UTF-8 text directly.
- Render TypeScript string literals with `JSON.stringify` and trailing commas/prettier-compatible tabs.
- Put `// @generated by scripts/generate-permissions.mjs; DO NOT EDIT.` and the same digest in both generated source headers.
- Read and validate every input before writing; compute every output in memory before the first write.
- In check mode, compare all three generated outputs byte-for-byte and report every stale path.
- If the lock is missing, reject unless `initializeLock` is true.
- Reject `initializeLock` when the lock already exists.
- When the lock exists, calculate `oldCodes - newCodes` and reject any non-empty difference before rendering writes.
- Resolve CLI paths from the repository root derived from `import.meta.url`, not the process working directory.
- Reject simultaneous `--check` and `--initialize-lock` and any unknown flag.

- [ ] **Step 4: Run generator unit tests**

Run:

```bash
node --test scripts/tests/generate-permissions.test.mjs
```

Expected: PASS with zero failed tests and explicit coverage of validation, lock removal, deterministic rendering, stale check, and no-write-on-failure behavior.

- [ ] **Step 5: Commit the generator foundation**

```bash
git add contracts/permissions.schema.json scripts/generate-permissions.mjs scripts/tests/generate-permissions.test.mjs
git commit -m "feat(permissions): add deterministic contract generator"
```

---

### Task 2: Convert the current registries with auditable parity

**Files:**
- Create: `contracts/permissions.json`
- Create: `contracts/permissions.lock.json`
- Create: `backend-school/src/permissions/registry_generated.rs`
- Create: `frontend-school/src/lib/permissions/registry.generated.ts`
- Modify temporarily for parity test: `frontend-school/tests/static/api-global-contract.test.mjs`

**Interfaces:**
- Consumes: Task 1 generator and the still-handwritten legacy registries.
- Produces: a locked canonical contract and generated artifacts whose code/metadata sets match the legacy Rust registry exactly.

- [ ] **Step 1: Add a failing legacy-parity test before changing either registry wrapper**

Extend the existing static test helpers to read `contracts/permissions.json`, derive wildcard or `${module}.${action}.${scope}`, and compare it against the still-handwritten Rust `codes`/`ALL_PERMISSIONS` and TypeScript `PERMISSIONS`/`PERMISSION_MODULES`.

The test must compare exact sets and backend metadata, not only a frontend subset:

```js
assert.deepEqual(new Set(contractCodes), legacyBackend.allPermissionCodes);
assert.deepEqual(new Set(contractModules), legacyBackend.modules);
assert.deepEqual(new Set(frontendPermissions.values()), new Set(contractCodes.filter((code) => code !== '*')));
assert.deepEqual(new Set(frontendModules.values()), new Set(contractModules));
assert.deepEqual(contractMetadataByCode, legacyBackend.metadataByCode);
```

- [ ] **Step 2: Run the parity test to verify the missing contract failure**

Run:

```bash
cd frontend-school
node --test --test-name-pattern="permission contract preserves legacy registries" tests/static/api-global-contract.test.mjs
```

Expected: FAIL because `contracts/permissions.json` does not exist.

- [ ] **Step 3: Transcribe the legacy registry into the canonical contract**

Create one contract record for every current `PermissionDef`. Preserve the exact `module`, `action`, `scope`, `name`, and `description` strings. Mark only `codes::WILDCARD` with `"kind": "wildcard"`; omit `kind` for every normal record. Do not hand-author `constant` or `code` fields because the generator derives them.

Before initialization, verify the contract-derived count and legacy count are equal and the test reports no missing/extra code. Then initialize the immutable baseline:

```bash
node scripts/generate-permissions.mjs --initialize-lock
```

Expected: creates the lock plus both generated registries and reports the current permission count without removing or renaming any code.

- [ ] **Step 4: Run parity, generator, and check-mode tests**

Run:

```bash
node --test scripts/tests/generate-permissions.test.mjs
node scripts/generate-permissions.mjs --check
cd frontend-school
node --test --test-name-pattern="permission contract preserves legacy registries" tests/static/api-global-contract.test.mjs
```

Expected: all commands PASS; backend codes/metadata and frontend constants/modules exactly match the contract.

- [ ] **Step 5: Inspect for accidental permission or migration changes**

Run:

```bash
git diff --check
git diff --name-only -- backend-school/migrations
git diff -- contracts/permissions.json contracts/permissions.lock.json backend-school/src/permissions/registry_generated.rs frontend-school/src/lib/permissions/registry.generated.ts
```

Expected: no migration path; generated code contains every current code and metadata value exactly once.

- [ ] **Step 6: Commit the parity-preserving contract and artifacts**

```bash
git add contracts/permissions.json contracts/permissions.lock.json \
  backend-school/src/permissions/registry_generated.rs \
  frontend-school/src/lib/permissions/registry.generated.ts \
  frontend-school/tests/static/api-global-contract.test.mjs
git commit -m "refactor(permissions): canonicalize existing registry"
```

---

### Task 3: Switch production wrappers and architecture guards to generated data

**Files:**
- Modify: `backend-school/src/permissions/registry.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `frontend-school/src/lib/permissions/registry.ts`
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`

**Interfaces:**
- Consumes: Task 2 generated `codes`, `ALL_PERMISSIONS`, `PERMISSIONS`, and `PERMISSION_MODULES`.
- Produces: the same public backend/frontend imports backed only by generated data.

- [ ] **Step 1: Add failing wrapper-source guards**

Add focused guards requiring the thin wrappers and banning handwritten copies:

```rust
#[test]
fn permission_registry_wraps_generated_contract() {
    let wrapper = read_source(manifest_dir().join("src/permissions/registry.rs"));
    assert!(wrapper.contains("include!(\"registry_generated.rs\")"));
    assert!(!wrapper.contains("pub const STAFF_READ_ALL"));
    assert!(!wrapper.contains("pub const ALL_PERMISSIONS"));
}
```

In the frontend static test, require `registry.ts` to re-export `registry.generated` and ensure its handwritten portion no longer declares `PERMISSIONS` or `PERMISSION_MODULES`.

- [ ] **Step 2: Run the focused guards to verify they fail on handwritten wrappers**

Run:

```bash
cd backend-school
cargo test --test static_architecture permission_registry_wraps_generated_contract
cd ../frontend-school
node --test --test-name-pattern="permission registry wrappers use generated contract" tests/static/api-global-contract.test.mjs
```

Expected: both focused guards FAIL because production wrappers still contain handwritten registries.

- [ ] **Step 3: Replace backend registry data with the include**

Keep the `PermissionDef` definition and serde derives exactly as they are, delete the handwritten `codes` module and `ALL_PERMISSIONS`, and end the wrapper with:

```rust
include!("registry_generated.rs");
```

Update architecture tests that previously parsed `registry.rs` for permission definitions to use `registry_generated.rs` where source parsing is still relevant. Remove redundant regex validation of code/module/action/scope consistency only when an equivalent generator unit test exists. Retain all production-usage bans and known-constant reference checks.

- [ ] **Step 4: Replace frontend registry data with re-exports**

Delete only the handwritten wildcard, module object, permission object, and the three types. Keep `PermissionMeta`, `SCOPE_META`, `ACTION_LABELS`, and helper functions unchanged. Add:

```ts
export {
	PERMISSIONS,
	PERMISSION_MODULES,
	WILDCARD_PERMISSION
} from './registry.generated';
export type {
	PermissionCode,
	PermissionModule,
	RoutePermission
} from './registry.generated';
```

Update the cross-stack static test to load canonical JSON as the source and generated files as artifacts. Preserve the existing scans for raw permission literals, unknown `codes::*`, route metadata constants, and module rollout coverage.

- [ ] **Step 5: Run focused and affected suites**

Run:

```bash
node scripts/generate-permissions.mjs --check
cd backend-school
cargo fmt --all -- --check
cargo check --bin backend-school
cargo test --test static_architecture
cd ../frontend-school
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: generator clean; backend static suite passes; frontend static suite passes; Svelte check reports 0 errors and 0 warnings.

- [ ] **Step 6: Commit production integration**

```bash
git add backend-school/src/permissions/registry.rs \
  backend-school/tests/static_architecture.rs \
  frontend-school/src/lib/permissions/registry.ts \
  frontend-school/tests/static/api-global-contract.test.mjs
git commit -m "refactor(permissions): use generated registries"
```

---

### Task 4: Add developer commands, focused CI, and documentation

**Files:**
- Create: `.github/workflows/permission-contract.yml`
- Modify: `frontend-school/package.json`
- Modify: `.rules`
- Modify: `docs/TESTING.md`
- Modify: `docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md`

**Interfaces:**
- Consumes: Task 1 CLI and Task 3 wrapper/test commands.
- Produces: discoverable local commands and a non-deploying CI gate.

- [ ] **Step 1: Add script-contract tests that fail before package/workflow wiring**

Extend generator tests or the cross-stack static test to assert these exact package scripts:

```json
{
  "generate:permissions": "node ../scripts/generate-permissions.mjs",
  "check:permissions": "node ../scripts/generate-permissions.mjs --check",
  "test:permissions": "node --test ../scripts/tests/generate-permissions.test.mjs"
}
```

Add a static assertion that `.github/workflows/permission-contract.yml` runs check mode, generator tests, backend format/check/static tests, frontend static tests, and frontend check without any deploy, push, SSH, or database command.

- [ ] **Step 2: Run focused tests to verify missing workflow/scripts**

Run:

```bash
node --test scripts/tests/generate-permissions.test.mjs
cd frontend-school
node --test --test-name-pattern="permission contract developer workflow" tests/static/api-global-contract.test.mjs
```

Expected: FAIL because package scripts and the focused workflow are absent.

- [ ] **Step 3: Add package scripts and focused GitHub Actions workflow**

Add the three scripts without changing dependencies. The workflow must:

- trigger on pull requests and pushes to `main` when contract/generator/registry/test/workflow paths change;
- use `actions/checkout@v6`, `actions/setup-node@v6` with Node 22, and the repository Rust toolchain;
- run generator check and unit tests from the repository root;
- run `cargo fmt --all -- --check`, `cargo check --bin backend-school`, and `cargo test --test static_architecture` from `backend-school`;
- run `npm ci`, `npm run test:static`, and `npm run check` from `frontend-school` with test public environment values;
- request only `contents: read` and contain no deployment or database mutation step.

- [ ] **Step 4: Update project instructions and testing docs**

Replace the `.rules` instruction that says to update backend and frontend registries manually with this workflow:

```text
Edit contracts/permissions.json, run npm run generate:permissions from frontend-school,
commit the contract lock and both generated registries, and run npm run check:permissions.
Never edit registry_generated.rs or registry.generated.ts directly.
```

Add the exact local commands and removal-safety explanation to `docs/TESTING.md`. In the improvement analysis, mark only permission registry generation complete and explicitly leave general Rust/TypeScript API contract generation as future work.

- [ ] **Step 5: Run developer workflow and static tests**

Run:

```bash
cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: generation produces no diff, every command passes, and frontend check reports 0 errors and 0 warnings.

- [ ] **Step 6: Commit CI and documentation**

```bash
git add .github/workflows/permission-contract.yml frontend-school/package.json \
  .rules docs/TESTING.md docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md \
  scripts/tests/generate-permissions.test.mjs \
  frontend-school/tests/static/api-global-contract.test.mjs
git commit -m "ci(permissions): verify generated contract"
```

---

### Task 5: Final parity and regression verification

**Files:**
- Verify only; modify documentation only if a command or result recorded in Task 4 is inaccurate.

**Interfaces:**
- Consumes: all prior task commits.
- Produces: merge-ready evidence that representation changed but behavior and permission data did not.

- [ ] **Step 1: Run generator and repository integrity checks**

```bash
node scripts/generate-permissions.mjs --check
node --test scripts/tests/generate-permissions.test.mjs
git diff --check
git diff --name-only main...HEAD -- backend-school/migrations
```

Expected: all commands pass and the migration command prints nothing.

- [ ] **Step 2: Run full affected backend verification**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --bin backend-school
cargo test --bin backend-school -- \
  --skip modules::auth::tests::auth_tests::test_login_success \
  --skip modules::auth::tests::auth_tests::test_login_invalid_credentials
cargo test --test static_architecture
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: format/check/Clippy pass, non-DB unit tests have zero failures with only the two known DB login tests filtered, and all architecture tests pass.

- [ ] **Step 3: Run full affected frontend verification**

```bash
cd frontend-school
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
npm run check:permissions
npm run test:permissions
npx prettier --check src/lib/permissions/registry.ts src/lib/permissions/registry.generated.ts \
  tests/static/api-global-contract.test.mjs package.json
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run build
```

Expected: 0 Svelte errors/warnings, zero failed tests, Prettier clean, and production build succeeds.

- [ ] **Step 4: Prove unchanged permission and runtime boundaries**

Use a read-only comparison between the design-base registry at `b5671e77` and the canonical/generated outputs. Report exact counts and empty missing/extra/metadata-difference sets. Also run:

```bash
rg -n "permissions/registry_generated\.rs|permissions/registry\.generated\.ts" backend-school/migrations frontend-school/src/routes || true
git status --short
git log --oneline --decorate b5671e77..HEAD
```

Expected: no migration or route directly depends on generated file paths, the worktree is clean, and only the planned task commits appear.

- [ ] **Step 5: Request final review before integration**

Review against the design acceptance criteria with emphasis on accidental code removal, lock bypasses, manual registry leftovers, cross-stack import compatibility, and CI mutation safety. Address findings with focused tests and commits, then rerun the affected verification command before claiming completion.
