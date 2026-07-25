# Documentation Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 126 scattered Markdown documents with 11 current documents, make `.rules` the single development standard, and enforce the final documentation set in CI.

**Architecture:** Durable development rules live in `.rules`; detailed verification recipes live in `docs/TESTING.md`; runtime/deployment guidance lives in `docs/OPERATIONS.md`; root and service READMEs provide orientation only. Static tests follow code/configuration sources instead of historical narrative documents, and a repository-wide documentation policy test enforces the exact Markdown allowlist and validates local links.

**Tech Stack:** Markdown, Node.js 22 built-in test runner, Rust static architecture tests, GitHub Actions, existing SvelteKit npm scripts.

## Global Constraints

- The final repository contains exactly the 11 Markdown paths listed in the approved design.
- `.rules` is the only authoritative development standard; do not create `DEVELOPMENT.md`.
- Completed plans/specs are deleted, not archived.
- Files under `.worktrees/` are separate checkouts and must not be edited.
- Never copy legacy PostgreSQL `pgcrypto`, `ALTER ROLE`, database-setting encryption, plaintext-PII, legacy permission helper, or `roles.permissions` instructions into retained documents.
- Verify every documented command, path, workflow, port, and environment name against the current repository.
- Preserve unrelated changes and do not run broad write-formatters on unrelated files.
- Use `apply_patch` for every documentation edit and deletion.
- The approved design at `docs/superpowers/specs/2026-07-26-documentation-consolidation-design.md` and this plan are temporary and must be deleted before final verification.

---

## File Responsibility Map

**Canonical documents**

- Modify: `.rules` — mandatory development workflows and verification matrix.
- Modify: `README.md` — current repository orientation and quick start.
- Create: `docs/README.md` — short documentation index.
- Modify: `docs/TESTING.md` — detailed verification recipes.
- Create: `docs/OPERATIONS.md` — current deployment/runtime operations.
- Modify: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` — equivalent auto-loaded pointers.
- Modify: `backend-admin/README.md` — backend-admin setup/run reference.
- Modify: `backend-school/README.md` — backend-school setup/run reference.
- Modify: `frontend-admin/README.md` — frontend-admin setup/run reference.
- Modify: `frontend-school/README.md` — frontend-school setup/run reference.

**Guard and CI changes**

- Modify: `frontend-school/tests/static/api-response-contract.test.mjs` — stop testing transient documentation checkpoints.
- Delete: `frontend-school/tests/static/foundation-plan.test.mjs` — remove plan-as-runtime-contract test.
- Modify: `backend-school/tests/static_architecture.rs` — stop reading the deleted Podman guide.
- Modify: `.github/workflows/api-contract.yml` — remove deleted documentation paths.
- Create: `frontend-school/tests/static/documentation-policy.test.mjs` — exact allowlist, link, and `.rules` contract.
- Modify: `frontend-school/package.json` — add `check:docs`.
- Create: `.github/workflows/documentation.yml` — run documentation policy on documentation changes.

**Deletion set**

- Delete every path returned by the deterministic deletion-list command in Task 5.
- Delete the approved design and this implementation plan in Task 6.

---

### Task 1: Detach Static Guards from Historical Documents

**Files:**

- Modify: `frontend-school/tests/static/api-response-contract.test.mjs:426-471`
- Delete: `frontend-school/tests/static/foundation-plan.test.mjs`
- Modify: `backend-school/tests/static_architecture.rs:4349-4379`
- Modify: `.github/workflows/api-contract.yml:3-45`

**Interfaces:**

- Consumes: current OpenAPI generator tests, runtime code guards, Compose files, deployment workflows, and smoke scripts.
- Produces: static checks that remain valid after `API_DEVELOPMENT.md`, `IMPROVEMENT_PLAN.md`, `PODMAN_SETUP.md`, and completed plan documents are deleted.

- [ ] **Step 1: Prove the current tests depend on documents scheduled for deletion**

Run:

```bash
rg -n "API_DEVELOPMENT|IMPROVEMENT_PLAN|PODMAN_SETUP|SCHOOL_OPERATIONS_FOUNDATION_PLAN|178 unique operations" \
  backend-school/tests frontend-school/tests .github/workflows
```

Expected: matches in `api-response-contract.test.mjs`, `foundation-plan.test.mjs`, `static_architecture.rs`, and `api-contract.yml`.

- [ ] **Step 2: Replace duplicated API-document assertions with durable ownership assertions**

In `frontend-school/tests/static/api-response-contract.test.mjs`:

- keep `project rules document generated API contract ownership`;
- make it read only `.rules` and `docs/TESTING.md`;
- retain assertions for `generate:api-contracts`, `check:api-contracts`, `contracts/openapi/school-api.json`, and the generated-file no-edit rule;
- delete `project docs record the 178-operation manual timetable checkpoint`;
- delete `API docs record implemented reversible role and organization deactivation`.

The retained test must have this source set:

```js
const rules = await readRepoFile('.rules');
const testing = await readRepoFile('docs/TESTING.md');

for (const source of [rules, testing]) {
	assert.match(source, /generate:api-contracts/);
	assert.match(source, /check:api-contracts/);
	assert.match(source, /contracts\/openapi\/school-api\.json/);
	assert.match(source, /generated files?[^\n]*do not edit|do not edit[^\n]*generated files?/i);
}
```

- [ ] **Step 3: Delete the completed-plan-only test**

Delete `frontend-school/tests/static/foundation-plan.test.mjs` with `apply_patch`. Runtime organization terminology remains covered by backend/frontend permission and organization static guards.

- [ ] **Step 4: Make the readiness guard inspect executable sources only**

In `backend-school/tests/static_architecture.rs`, remove:

```rust
let podman_setup = read_source(repo_root().join("docs/PODMAN_SETUP.md"));
```

and remove the six `podman_setup` assertions. Keep assertions against:

- `docker-compose.yml`;
- `podman-compose.yml`;
- both backend deployment workflows;
- `scripts/smoke_test.sh`.

- [ ] **Step 5: Remove deleted documentation paths from API contract CI**

In both `pull_request.paths` and `push.paths` of `.github/workflows/api-contract.yml`, remove:

```yaml
- "docs/backend-school/API_DEVELOPMENT.md"
```

Keep `.rules` and `docs/TESTING.md`.

- [ ] **Step 6: Run focused guards**

Run:

```bash
cd frontend-school
node --test tests/static/api-response-contract.test.mjs

cd ../backend-school
cargo test deployment_and_smoke_checks_use_backend_readiness --test static_architecture

cd ..
rg -n "API_DEVELOPMENT|IMPROVEMENT_PLAN|PODMAN_SETUP|SCHOOL_OPERATIONS_FOUNDATION_PLAN|178 unique operations" \
  backend-school/tests frontend-school/tests .github/workflows
```

Expected:

- Node test passes.
- Rust focused test passes.
- final `rg` returns no matches.

- [ ] **Step 7: Commit the guard cleanup**

```bash
git add \
  frontend-school/tests/static/api-response-contract.test.mjs \
  frontend-school/tests/static/foundation-plan.test.mjs \
  backend-school/tests/static_architecture.rs \
  .github/workflows/api-contract.yml
git commit -m "test: detach guards from historical docs"
```

---

### Task 2: Make `.rules` the Single Development Standard

**Files:**

- Modify: `.rules`

**Interfaces:**

- Consumes: approved design, `contracts/permissions.schema.json`, current package scripts, backend module patterns, migration layout, request context/policy APIs, and existing UI/layout invariants.
- Produces: one durable development standard referenced by all retained entry-point and README documents.

- [ ] **Step 1: Capture required durable terms before rewriting**

Run:

```bash
rg -n \
  "Shared Page Layout UI|Sidebar Navigation IA|API Response Contract|Permission Constants|Migration Safety|generate:permissions|generate:api-contracts|field_encryption|static_architecture" \
  .rules frontend-school/tests/static
```

Expected: existing static tests show which durable headings/phrases must remain or be represented equivalently.

- [ ] **Step 2: Rewrite `.rules` around work workflows**

Replace transient feature checkpoints and obsolete example paths with these top-level sections:

```markdown
# SchoolOrbit Development Rules

## 0. Rule Ownership and Documentation Policy
## 1. Required Analysis Workflow
## 2. Adding or Changing a Feature
## 3. Backend: Rust, Axum, SQLx
## 4. Permissions and Resource Authorization
## 5. API Contracts
## 6. Database Migrations
## 7. Frontend: SvelteKit 5
## 8. Realtime and Events
## 9. Security, PDPA, and Logging
## 10. Deployment Constraints
## 11. Verification Matrix
```

The rewritten content must include all approved design requirements and retain these exact durable phrases needed by existing guards:

- `API Response Contract`
- `Shared Page Layout UI`
- `Sidebar Navigation IA`
- `collapsible workflow sections`
- `workspace icon rail`
- `full available app width`
- `do not put \`container\`, \`mx-auto\`, or \`max-w-*\` in \`contentClass\``
- `named contract`

It must explicitly document:

- `actor_tenant_context` and central tenant request-context helpers;
- thin handlers and the handler/service database boundary;
- policy-layer resource scopes and list-scope union behavior;
- typed API DTOs and the single success/error envelope;
- `contracts/permissions.json` as the only handwritten registry;
- generated permission/API artifacts as no-edit files;
- permission cache invalidation and `permission_changed`;
- new sequential migrations and immutable `001_baseline.sql`;
- `_meta.menu` versus `_meta.access`;
- `$can` and `/api/auth/me` as current-user permission sources;
- optional backend-enforced feature toggles;
- AES-256-GCM through `field_encryption.rs` and HMAC blind indexes;
- structured logging and forbidden sensitive log content;
- realtime identity, heartbeat, reconnect, and fail-closed behavior;
- exact verification commands by change type;
- the 11-file documentation policy and rule for adding a twelfth file.

Remove:

- the `178 unique operations` checkpoint;
- lists of individual migrated operations;
- implementation completion notes;
- obsolete `src/handlers/user.rs` and `src/repositories` examples;
- any claim that every feature needs a feature toggle.

- [ ] **Step 3: Verify required sections and forbidden transient text**

Run:

```bash
for heading in \
  "Rule Ownership and Documentation Policy" \
  "Required Analysis Workflow" \
  "Adding or Changing a Feature" \
  "Backend: Rust, Axum, SQLx" \
  "Permissions and Resource Authorization" \
  "API Contracts" \
  "Database Migrations" \
  "Frontend: SvelteKit 5" \
  "Realtime and Events" \
  "Security, PDPA, and Logging" \
  "Deployment Constraints" \
  "Verification Matrix"
do
  rg -F "## $heading" .rules
done

rg -n "178 unique operations|Current checkpoint|src/handlers/user.rs|src/repositories" .rules
```

Expected: every heading is found; the final `rg` returns no matches.

- [ ] **Step 4: Run static tests that treat `.rules` as a durable UI/API contract**

Run:

```bash
cd frontend-school
node --test \
  tests/static/frontend-layout-components.test.mjs \
  tests/static/sidebar-navigation.test.mjs \
  tests/static/api-response-contract.test.mjs \
  tests/static/api-global-contract.test.mjs
```

Expected: all tests pass.

- [ ] **Step 5: Commit the canonical rules**

```bash
git add .rules
git commit -m "docs: centralize development rules"
```

---

### Task 3: Build the Canonical Documentation Set

**Files:**

- Modify: `README.md`
- Create: `docs/README.md`
- Modify: `docs/TESTING.md`
- Create: `docs/OPERATIONS.md`

**Interfaces:**

- Consumes: `.rules`, current package scripts, Cargo manifests, Compose files, `.github/workflows/`, `scripts/`, current migration utilities, and current application-side encryption implementation.
- Produces: non-overlapping orientation, testing, and operational references linked by every retained README.

- [ ] **Step 1: Verify current executable names before documenting them**

Run:

```bash
node -e "for (const p of ['frontend-school/package.json','frontend-admin/package.json']) { const j=require('./'+p); console.log(p, j.scripts) }"
rg -n '8080|8081|/health|/ready|ENCRYPTION_KEY|BLIND_INDEX_KEY|INTERNAL_API_SECRET|DEPLOY_KEY' \
  docker-compose.yml podman-compose.yml .github/workflows scripts backend-admin/src backend-school/src \
  -g '*.yml' -g '*.yaml' -g '*.sh' -g '*.rs'
rg --files scripts backend-school/scripts | sort
```

Expected: use only commands, ports, environment names, scripts, and health endpoints confirmed by these sources.

- [ ] **Step 2: Rewrite the root README**

Use this concise structure:

```markdown
# SchoolOrbit

## Services
## Repository Map
## Quick Start
## Development Rules
## Verification
## Operations
```

Requirements:

- describe Rust/Axum/PostgreSQL backends and SvelteKit 5 frontends accurately;
- identify ports `8080` and `8081` only where confirmed;
- link `.rules`, `docs/README.md`, `docs/TESTING.md`, and `docs/OPERATIONS.md`;
- link all four service READMEs;
- remove feature marketing lists, old R2 walkthrough duplication, placeholder license/support text, and stale response examples.

- [ ] **Step 3: Create the short documentation index**

Create `docs/README.md` with:

```markdown
# Documentation

- [Development rules](../.rules)
- [Testing](./TESTING.md)
- [Operations](./OPERATIONS.md)

Service setup:

- [Backend admin](../backend-admin/README.md)
- [Backend school](../backend-school/README.md)
- [Frontend admin](../frontend-admin/README.md)
- [Frontend school](../frontend-school/README.md)
```

Add one sentence stating that implementation history belongs in Git history and issue/PR discussions, not permanent plan documents.

- [ ] **Step 4: Rewrite `docs/TESTING.md` as command recipes**

Use these sections:

```markdown
# Testing

## Reporting Verification
## Every Change
## Backend School
## Backend Admin
## Frontend School
## Frontend Admin
## Permission Contract
## API Contract
## Database and Migration Tests
## Encryption and PII
## Smoke Tests
## Browser E2E
## Realtime Rollout Checks
## Ubuntu 26.04 Playwright
```

Preserve current, verified commands and environment rules, including:

- `git diff --check` and `git status --short`;
- focused tests before broad tests;
- `cargo fmt --all -- --check`, `cargo check`, and backend static architecture tests;
- frontend `lint`, `check`, and focused static tests;
- permission and API generator/check/test commands;
- `TEST_DATABASE_URL` isolation and direct Neon endpoint caveat;
- `scripts/smoke_test.sh` and secret-only credentials;
- Playwright variables and browser-context cookie checks;
- `PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64`;
- the realtime legacy query-identity checklist naming `user_id`, `name`, and `school_key`;
- explicit reporting when an environment-dependent check cannot run.

Remove operation-count checkpoints and completed rollout histories.

- [ ] **Step 5: Create `docs/OPERATIONS.md` from current executable sources**

Use these sections:

```markdown
# Operations

## Runtime Topology
## Required Environment and Secrets
## Health and Readiness
## Deployment Workflows
## Reverse Proxy and Realtime
## Tenant Migration and Cutover
## Permission and Menu Synchronization
## Encryption and Key Rotation
## File Storage
## Focused Troubleshooting
```

Required current rules:

- bind container services to `0.0.0.0`;
- use environment variables for all secrets;
- list `ENCRYPTION_KEY`, `BLIND_INDEX_KEY`, `JWT_SECRET`, `INTERNAL_API_SECRET`, and `DEPLOY_KEY`;
- document caller-specific `INTERNAL_API_SECRET_<CALLER>` fallback behavior;
- distinguish `/health` from `/ready`;
- point to current Compose, Nginx config, workflows, and scripts;
- explain that route/menu sync requires `VITE_DEPLOY_KEY` and `SUBDOMAIN`;
- describe `prepare_clean_tenant_db.sh` and `cutover_tenant_data.sh` safety at a high level;
- require stable encryption/blind-index keys and a dedicated re-encryption/reindex job for rotation;
- state that national IDs use app-side AES-256-GCM plus keyed HMAC blind indexes;
- reject `pgcrypto`, `ALTER ROLE`, or database session settings for application fields.

- [ ] **Step 6: Scan retained core documents for stale instructions**

Run:

```bash
rg -n -i \
  "pgcrypto|ALTER ROLE|app\\.encryption_key|roles\\.permissions|check_user_permission|users\\.view|roles\\.manage|src/repositories|178 unique operations" \
  README.md docs/README.md docs/TESTING.md docs/OPERATIONS.md .rules
```

Expected: no legacy positive instruction. If a forbidden term appears only in an explicit prohibition, read the line and retain it only when the prohibition is unambiguous.

- [ ] **Step 7: Commit the canonical documents**

```bash
git add README.md docs/README.md docs/TESTING.md docs/OPERATIONS.md
git commit -m "docs: add canonical testing and operations guides"
```

---

### Task 4: Rewrite Tool Entry Points and Service READMEs

**Files:**

- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `GEMINI.md`
- Modify: `backend-admin/README.md`
- Modify: `backend-school/README.md`
- Modify: `frontend-admin/README.md`
- Modify: `frontend-school/README.md`

**Interfaces:**

- Consumes: `.rules`, `docs/README.md`, `docs/TESTING.md`, `docs/OPERATIONS.md`, current Cargo manifests, package scripts, service routes, and environment examples.
- Produces: concise equivalent tool entry points and accurate per-service setup references.

- [ ] **Step 1: Rewrite the three tool entry points from one shared body**

Each of `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` must contain the same content:

```markdown
# SchoolOrbit project instructions

Read [`.rules`](./.rules) before analysis or changes. It is the single authoritative development standard.

Active references:

- [Documentation index](./docs/README.md)
- [Testing](./docs/TESTING.md)
- [Operations](./docs/OPERATIONS.md)

High-risk invariants:

- never edit an applied migration;
- never store or log plaintext national IDs;
- use generated permission and API contracts;
- verify claims with the change-type matrix in `.rules`.
```

- [ ] **Step 2: Verify current service commands and ports**

Run:

```bash
rg -n '^name\\s*=|^\\[package\\]' backend-admin/Cargo.toml backend-school/Cargo.toml
node -e "for (const p of ['frontend-admin/package.json','frontend-school/package.json']) { const j=require('./'+p); console.log(p, j.scripts) }"
rg -n 'PORT|8080|8081|/health|/ready' backend-admin/src backend-school/src docker-compose.yml podman-compose.yml -g '*.rs' -g '*.yml'
```

Expected: README commands and ports are derived from this output, not old README text.

- [ ] **Step 3: Rewrite both backend READMEs**

Use the same concise shape:

```markdown
# <Service name>

## Purpose
## Stack
## Local Setup
## Run
## Check and Test
## Environment
## Health
## Project Documentation
```

Backend-specific requirements:

- describe Axum, Tokio, SQLx, and PostgreSQL accurately;
- use the correct service directory and Cargo commands;
- distinguish `/health` and `/ready`;
- backend-school must describe tenant database resolution without duplicating the permission tutorial;
- backend-admin must remove the obsolete Ohkami claim;
- link `../.rules`, `../docs/TESTING.md`, and `../docs/OPERATIONS.md`.

- [ ] **Step 4: Rewrite both frontend READMEs**

Use:

```markdown
# <Service name>

## Purpose
## Stack
## Local Setup
## Development
## Check and Build
## Environment
## Project Documentation
```

Requirements:

- describe SvelteKit 5, TypeScript, and the actual adapter/build setup;
- use only scripts present in the service package;
- remove generated `sv` starter text;
- frontend-school must mention generated API/permission contracts by linking to `.rules`, not by duplicating commands;
- link canonical testing and operations guides.

- [ ] **Step 5: Verify entry-point equivalence and README links**

Run:

```bash
cmp -s AGENTS.md CLAUDE.md
cmp -s AGENTS.md GEMINI.md
rg -n "Ohkami|MODULE_CREATION_GUIDE|FILE_STORAGE_PHASE|sv create|Your License Here|docs/backend-school/API_DEVELOPMENT" \
  AGENTS.md CLAUDE.md GEMINI.md \
  backend-admin/README.md backend-school/README.md \
  frontend-admin/README.md frontend-school/README.md
```

Expected: both `cmp` commands exit `0`; `rg` returns no matches.

- [ ] **Step 6: Commit the entry points and service READMEs**

```bash
git add \
  AGENTS.md CLAUDE.md GEMINI.md \
  backend-admin/README.md backend-school/README.md \
  frontend-admin/README.md frontend-school/README.md
git commit -m "docs: refresh service entry points"
```

---

### Task 5: Delete Every Superseded Markdown Document

**Files:**

- Delete: every tracked Markdown path outside the 11-file allowlist, except the temporary approved design and this plan until Task 6.

**Interfaces:**

- Consumes: completed canonical documents and exact Git-tracked Markdown inventory.
- Produces: a worktree with only the 11 permanent Markdown files plus the two temporary implementation artifacts.

- [ ] **Step 1: Generate and inspect the exact permanent deletion list**

Run from the repository root:

```bash
docs_allowlist_file="$(mktemp)"
printf '%s\n' \
  AGENTS.md \
  CLAUDE.md \
  GEMINI.md \
  README.md \
  docs/README.md \
  docs/TESTING.md \
  docs/OPERATIONS.md \
  backend-admin/README.md \
  backend-school/README.md \
  frontend-admin/README.md \
  frontend-school/README.md \
  docs/superpowers/specs/2026-07-26-documentation-consolidation-design.md \
  docs/superpowers/plans/2026-07-26-documentation-consolidation.md \
  | sort > "$docs_allowlist_file"
comm -23 <(git ls-files '*.md' | sort) "$docs_allowlist_file"
```

Expected:

- every output path is a historical, duplicated, completed, or stale document approved for deletion;
- no allowlisted path appears;
- no path under `.worktrees/` appears.

- [ ] **Step 2: Delete the printed paths with `apply_patch`**

Create one or more `apply_patch` deletion patches containing an explicit:

```text
*** Delete File: <exact path from Step 1>
```

for every printed path. Do not use `rm`, a glob, or recursive deletion.

- [ ] **Step 3: Find residual references to deleted Markdown paths**

Run:

```bash
git diff --name-status -- '*.md'
rg -n \
  "MODULE_CREATION_GUIDE|API_DEVELOPMENT|docs/PERMISSIONS|PODMAN_SETUP|PROJECT_PLAN|IMPROVEMENT_PLAN|TODO_ENCRYPTION|docs/superpowers|docs/plans/|FILE_STORAGE_PHASE|FIX-ENCRYPTION-KEY-PRODUCTION|CLOUDFLARE_DEPLOYMENT|README-UPDATE" \
  . \
  -g '!.git/**' -g '!.worktrees/**' -g '!target/**' -g '!node_modules/**'
```

Expected: remaining matches are only the temporary design/plan describing the cleanup. Update any non-temporary source, test, workflow, or retained document reference before continuing.

- [ ] **Step 4: Verify the interim Markdown inventory**

Run:

```bash
git ls-files '*.md' | while IFS= read -r file; do
  if [ -f "$file" ]; then printf '%s\n' "$file"; fi
done | sort
```

Expected: the 11 permanent allowlist paths plus exactly:

```text
docs/superpowers/plans/2026-07-26-documentation-consolidation.md
docs/superpowers/specs/2026-07-26-documentation-consolidation-design.md
```

- [ ] **Step 5: Run the existing static suites after deletion**

Run:

```bash
cd frontend-school
npm run test:static

cd ../backend-school
cargo test --test static_architecture
```

Expected: both suites pass without reading a deleted document.

- [ ] **Step 6: Commit superseded-document removal**

```bash
git diff --name-only --diff-filter=D -- '*.md' -z | xargs -0 git add -u --
git commit -m "docs: remove superseded documentation"
```

---

### Task 6: Enforce the Documentation Allowlist and Remove Temporary Artifacts

**Files:**

- Create: `frontend-school/tests/static/documentation-policy.test.mjs`
- Modify: `frontend-school/package.json`
- Create: `.github/workflows/documentation.yml`
- Delete: `docs/superpowers/specs/2026-07-26-documentation-consolidation-design.md`
- Delete: `docs/superpowers/plans/2026-07-26-documentation-consolidation.md`

**Interfaces:**

- Consumes: the final permanent documentation paths and `.rules` section/command contract.
- Produces: `npm run check:docs`, CI enforcement, exact final Markdown inventory, and no completed implementation artifacts.

- [ ] **Step 1: Add the documentation policy test**

Create `frontend-school/tests/static/documentation-policy.test.mjs` with this implementation:

```js
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { access, readFile } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const execFileAsync = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

const MARKDOWN_ALLOWLIST = [
	'AGENTS.md',
	'CLAUDE.md',
	'GEMINI.md',
	'README.md',
	'backend-admin/README.md',
	'backend-school/README.md',
	'docs/OPERATIONS.md',
	'docs/README.md',
	'docs/TESTING.md',
	'frontend-admin/README.md',
	'frontend-school/README.md'
].sort();

async function existingTrackedMarkdown() {
	const { stdout } = await execFileAsync('git', ['ls-files', '*.md'], { cwd: repoRoot });
	const paths = stdout
		.split(/\r?\n/)
		.map((value) => value.trim())
		.filter(Boolean);
	const existing = [];

	for (const relativePath of paths) {
		try {
			await access(path.join(repoRoot, relativePath));
			existing.push(relativePath);
		} catch {
			// A tracked file deleted in the current worktree is not active documentation.
		}
	}

	return existing.sort();
}

function localLinkTargets(source) {
	const targets = [];
	const pattern = /!?\[[^\]]*]\(([^)]+)\)/g;

	for (const match of source.matchAll(pattern)) {
		const rawTarget = match[1].trim().replace(/^<|>$/g, '');
		const target = rawTarget.split(/\s+["']/)[0];
		if (
			!target ||
			target.startsWith('#') ||
			/^(?:https?:|mailto:|tel:)/i.test(target)
		) {
			continue;
		}
		targets.push(decodeURIComponent(target.split('#')[0]));
	}

	return targets;
}

test('tracked Markdown is limited to the canonical documentation set', async () => {
	assert.deepEqual(await existingTrackedMarkdown(), MARKDOWN_ALLOWLIST);
});

test('canonical Markdown local links resolve', async () => {
	const broken = [];

	for (const relativePath of MARKDOWN_ALLOWLIST) {
		const source = await readFile(path.join(repoRoot, relativePath), 'utf8');
		for (const target of localLinkTargets(source)) {
			const resolved = path.resolve(repoRoot, path.dirname(relativePath), target);
			try {
				await access(resolved);
			} catch {
				broken.push(`${relativePath} -> ${target}`);
			}
		}
	}

	assert.deepEqual(broken, []);
});

test('project rules own durable development and verification workflows', async () => {
	const rules = await readFile(path.join(repoRoot, '.rules'), 'utf8');
	const required = [
		'## 0. Rule Ownership and Documentation Policy',
		'## 1. Required Analysis Workflow',
		'## 2. Adding or Changing a Feature',
		'## 4. Permissions and Resource Authorization',
		'## 5. API Contracts',
		'## 6. Database Migrations',
		'## 7. Frontend: SvelteKit 5',
		'## 9. Security, PDPA, and Logging',
		'## 11. Verification Matrix',
		'contracts/permissions.json',
		'npm run generate:permissions',
		'npm run check:permissions',
		'npm run test:permissions',
		'npm run generate:api-contracts',
		'npm run check:api-contracts',
		'npm run test:api-contracts',
		'cargo test --test static_architecture',
		'git diff --check'
	];

	for (const value of required) {
		assert.ok(rules.includes(value), `.rules must contain: ${value}`);
	}
});
```

- [ ] **Step 2: Add a local documentation check command**

Add to `frontend-school/package.json` scripts, adjacent to `test:static`:

```json
"check:docs": "node --test tests/static/documentation-policy.test.mjs"
```

- [ ] **Step 3: Verify the allowlist test fails only for temporary artifacts**

Run:

```bash
cd frontend-school
npm run check:docs
```

Expected: the allowlist test fails and the actual list differs only by:

- `docs/superpowers/plans/2026-07-26-documentation-consolidation.md`
- `docs/superpowers/specs/2026-07-26-documentation-consolidation-design.md`

The link and `.rules` tests must pass.

- [ ] **Step 4: Add documentation CI**

Create `.github/workflows/documentation.yml`:

```yaml
name: Documentation

on:
  pull_request:
    paths:
      - "**/*.md"
      - ".rules"
      - "frontend-school/tests/static/documentation-policy.test.mjs"
      - "frontend-school/package.json"
      - ".github/workflows/documentation.yml"
  push:
    branches:
      - main
    paths:
      - "**/*.md"
      - ".rules"
      - "frontend-school/tests/static/documentation-policy.test.mjs"
      - "frontend-school/package.json"
      - ".github/workflows/documentation.yml"

permissions:
  contents: read

jobs:
  verify:
    runs-on: ubuntu-24.04
    steps:
      - name: Checkout repository
        uses: actions/checkout@v6
      - name: Setup Node.js
        uses: actions/setup-node@v6
        with:
          node-version: "22"
      - name: Check canonical documentation
        run: node --test frontend-school/tests/static/documentation-policy.test.mjs
```

- [ ] **Step 5: Delete the temporary design and implementation plan**

Use `apply_patch` to delete exactly:

```text
docs/superpowers/specs/2026-07-26-documentation-consolidation-design.md
docs/superpowers/plans/2026-07-26-documentation-consolidation.md
```

Do not delete other files in this step.

- [ ] **Step 6: Verify the final documentation policy**

Run:

```bash
cd frontend-school
npm run check:docs

cd ..
git ls-files '*.md' | while IFS= read -r file; do
  if [ -f "$file" ]; then printf '%s\n' "$file"; fi
done | sort
```

Expected:

- all three documentation-policy tests pass;
- the inventory contains exactly the 11 allowlisted paths.

- [ ] **Step 7: Commit enforcement and temporary-artifact removal**

```bash
git add \
  frontend-school/tests/static/documentation-policy.test.mjs \
  frontend-school/package.json \
  .github/workflows/documentation.yml
git add -u \
  docs/superpowers/specs/2026-07-26-documentation-consolidation-design.md \
  docs/superpowers/plans/2026-07-26-documentation-consolidation.md
git commit -m "ci: enforce canonical documentation"
```

---

### Task 7: Run Full Verification

**Files:**

- Verify: `.rules`
- Verify: all 11 retained Markdown files
- Verify: modified static tests and workflows

**Interfaces:**

- Consumes: all completed tasks.
- Produces: fresh evidence for every acceptance criterion and a clean completion report.

- [ ] **Step 1: Verify documentation inventory and links**

Run:

```bash
cd frontend-school
npm run check:docs
```

Expected: 3 tests pass, 0 fail.

- [ ] **Step 2: Verify frontend-school formatting, lint, types, and static behavior**

Run:

```bash
cd frontend-school
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: every command exits `0`.

- [ ] **Step 3: Verify permission and API generators remain current**

Run:

```bash
cd frontend-school
npm run check:permissions
npm run test:permissions
npm run check:api-contracts
npm run test:api-contracts
```

Expected: all generated artifacts are current and all generator tests pass.

- [ ] **Step 4: Verify backend-school formatting, architecture, API contracts, and compilation**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo test api_contract::tests --bin backend-school
cargo check --bin backend-school
```

Expected: every command exits `0`.

- [ ] **Step 5: Verify no stale retained-document instructions remain**

Run from the repository root:

```bash
rg -n -i \
  "178 unique operations|Current checkpoint|roles\\.permissions|check_user_permission|users\\.view|roles\\.manage|src/repositories|app\\.encryption_key" \
  .rules \
  AGENTS.md CLAUDE.md GEMINI.md README.md \
  docs/README.md docs/TESTING.md docs/OPERATIONS.md \
  backend-admin/README.md backend-school/README.md \
  frontend-admin/README.md frontend-school/README.md
```

Expected: no matches.

For `pgcrypto` and `ALTER ROLE`, run:

```bash
rg -n -i "pgcrypto|ALTER ROLE" \
  .rules docs/OPERATIONS.md
```

Expected: matches are allowed only in explicit prohibitions stating that these paths must not be used.

- [ ] **Step 6: Verify repository diff hygiene**

Run:

```bash
git diff --check
git status --short
git log -7 --oneline
```

Expected:

- `git diff --check` exits `0`;
- worktree is clean after the implementation commits;
- recent commits show the documentation consolidation sequence.

- [ ] **Step 7: Prepare the completion report**

Report:

- Markdown count before and after: `126 -> 11`;
- `.rules` as the single authoritative development standard;
- the new `docs/README.md`, `docs/TESTING.md`, and `docs/OPERATIONS.md` ownership;
- historical and stale documents removed with Git history as recovery;
- documentation policy and CI added;
- every verification command and its result;
- any environment-dependent smoke/Playwright test not run, with the missing credentials/environment and remaining risk.
