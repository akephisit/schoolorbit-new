# Shared API Contracts Pilot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish deterministic backend-first OpenAPI and generated TypeScript contracts for `GET /api/auth/me` without changing runtime payload or authentication behavior.

**Architecture:** Rust serde DTOs and handler metadata remain authoritative. `backend-school` exports OpenAPI 3.1 through an early CLI path, a repository script generates committed TypeScript wire types, and `frontend-school` maps the generated DTO into its existing auth-store domain model. Check mode regenerates in temporary storage and fails on drift.

**Tech Stack:** Rust 2021, Axum 0.8, serde, utoipa 5.5.0 (`chrono`, `uuid`), Node.js 22, openapi-typescript 7.13.0, TypeScript 5.9, SvelteKit 5, Node test runner, GitHub Actions

## Global Constraints

- Preserve the current route, cookie authentication, tenant checks, permission loading, status codes, envelope, and JSON payload.
- Rust DTOs and handler metadata are handwritten source. `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/school-api.ts` are generated; do not edit them manually.
- Generation must not require network access, runtime environment variables, secrets, tenant configuration, database pools, jobs, or an HTTP listener. Installing pinned dependencies is a separate setup action.
- Application builds consume committed generated files and never generate implicitly.
- Do not add Swagger UI, a public OpenAPI route, Zod response validation, a generated runtime client, or a database migration.
- Model serde exactly: required nullable fields differ from optional non-null fields.
- Never put real national IDs, credentials, cookies, tokens, URLs containing credentials, or personal data values in schemas, examples, fixtures, logs, or commits.
- Do not use `any`, `unknown`, or an untyped object for the known current-user payload.
- Keep frontend `User` as a domain model and map from the wire DTO explicitly.
- Use TDD, run focused verification after each task, and commit each reviewable task separately.

## File map

- Create `backend-school/src/api_contract.rs`: compose, sort, render, and test the School OpenAPI document.
- Modify `backend-school/src/main.rs`: expose `export-openapi` before application initialization.
- Modify `backend-school/src/api_response.rs`: add OpenAPI schemas for shared envelopes.
- Modify `backend-school/src/modules/auth/models.rs`: add the exact `UserResponse` wire schema.
- Modify `backend-school/src/modules/auth/handlers.rs`: describe `GET /api/auth/me`.
- Modify `backend-school/Cargo.toml` and `Cargo.lock`: pin utoipa.
- Create `scripts/generate-api-contracts.mjs` and its Node tests.
- Create `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/school-api.ts`.
- Modify `frontend-school/package.json` and lockfile: pin generator and add commands.
- Modify `frontend-school/src/lib/api/auth.ts`: consume the generated wire DTO.
- Modify the cross-stack static contract test, `.rules`, testing/API docs, and focused CI.

---

### Task 1: Backend OpenAPI document and offline exporter

**Files:**

- Create: `backend-school/src/api_contract.rs`
- Modify: `backend-school/src/main.rs:1-70`
- Modify: `backend-school/src/api_response.rs:1-91`
- Modify: `backend-school/src/modules/auth/models.rs:1-121`
- Modify: `backend-school/src/modules/auth/handlers.rs:118-169`
- Modify: `backend-school/Cargo.toml:1-55`
- Modify: `backend-school/Cargo.lock`
- Test: `backend-school/src/api_contract.rs`

**Interfaces:**

- Consumes: `ApiResponse<T>`, `ApiErrorResponse`, `UserResponse`, and `modules::auth::handlers::me`.
- Produces: `school_api_value() -> Result<serde_json::Value, serde_json::Error>` and `render_school_api() -> Result<String, serde_json::Error>`.
- Produces CLI: `cargo run --quiet --manifest-path backend-school/Cargo.toml -- export-openapi`; stdout contains JSON only.

- [ ] **Step 1: Declare the module and write failing tests**

Add `pub mod api_contract;` beside `pub mod api_response;` in `main.rs`. Create `api_contract.rs` with these tests before production functions:

```rust
#[cfg(test)]
mod tests {
    use super::{render_school_api, school_api_value};
    use serde_json::Value;

    fn required(schema: &Value) -> Vec<&str> {
        let mut fields = schema["required"]
            .as_array().expect("required must be an array")
            .iter().map(|value| value.as_str().expect("required entry must be a string"))
            .collect::<Vec<_>>();
        fields.sort_unstable();
        fields
    }

    fn contains_null(schema: &Value) -> bool {
        match schema {
            Value::String(value) => value == "null",
            Value::Array(values) => values.iter().any(contains_null),
            Value::Object(values) => values.values().any(contains_null),
            _ => false,
        }
    }

    #[test]
    fn documents_current_user_operation_and_envelopes() {
        let document = school_api_value().expect("document should serialize");
        let operation = &document["paths"]["/api/auth/me"]["get"];
        assert_eq!(operation["operationId"], "getCurrentUser");
        assert!(operation["responses"]["200"]["content"]["application/json"]["schema"].is_object());
        assert!(operation["responses"]["401"]["content"]["application/json"]["schema"].is_object());
    }

    #[test]
    fn current_user_schema_matches_serde() {
        let document = school_api_value().expect("document should serialize");
        let schema = &document["components"]["schemas"]["UserResponse"];
        assert_eq!(required(schema), vec![
            "createdAt", "email", "firstName", "id", "lastName", "nationalId",
            "phone", "profileImageUrl", "status", "userType", "username",
        ]);
        let properties = schema["properties"].as_object().expect("properties must exist");
        assert_eq!(properties["id"]["format"], "uuid");
        assert_eq!(properties["createdAt"]["format"], "date-time");
        for field in ["nationalId", "email", "phone", "profileImageUrl"] {
            assert!(contains_null(&properties[field]), "{field} must accept null");
        }
        for field in ["primaryRoleName", "permissions"] {
            assert!(!required(schema).contains(&field));
            assert!(!contains_null(&properties[field]), "{field} is omitted, not null");
        }
    }

    #[test]
    fn render_is_deterministic_and_newline_terminated() {
        let first = render_school_api().expect("first render");
        let second = render_school_api().expect("second render");
        assert_eq!(first, second);
        assert!(first.ends_with('\n'));
    }
}
```

- [ ] **Step 2: Run the test and verify RED**

Run `cd backend-school && cargo test api_contract::tests --bin backend-school`.

Expected: compilation FAILS because `school_api_value` and `render_school_api` do not exist.

- [ ] **Step 3: Add the pinned backend dependency and schema derives**

Add to `backend-school/Cargo.toml`:

```toml
utoipa = { version = "5.5.0", features = ["chrono", "uuid"] }
```

In `api_response.rs`, import `utoipa::ToSchema`, derive it for `ApiResponse<T>` and `ApiErrorResponse`, and keep omitted messages optional but non-null:

```rust
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub message: Option<String>,
}
```

Apply the same `ToSchema` derive and `#[schema(value_type = String)]` message annotation to `ApiErrorResponse`. Do not alter constructors or serialization.

In `auth/models.rs`, derive `ToSchema` for `UserResponse`. Add `#[schema(required = true)]` to `national_id`, `email`, `phone`, and `profile_image_url`. Add `#[schema(value_type = String)]` to skipped `primary_role_name` and `#[schema(value_type = Vec<String>)]` to skipped `permissions`. Retain `From<User>` unchanged.

- [ ] **Step 4: Describe the existing handler without changing it**

Import `ApiErrorResponse` and put this directly above `me`:

```rust
#[utoipa::path(
    get,
    path = "/api/auth/me",
    operation_id = "getCurrentUser",
    tag = "auth",
    responses(
        (status = 200, description = "Current authenticated user", body = ApiResponse<UserResponse>),
        (status = 401, description = "Authentication required or invalid", body = ApiErrorResponse)
    )
)]
```

Do not change the handler signature or body.

- [ ] **Step 5: Implement deterministic document rendering**

Add above the tests in `api_contract.rs`:

```rust
use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::modules::auth::models::UserResponse;
use serde_json::Value;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(crate::modules::auth::handlers::me),
    components(schemas(UserResponse, ApiResponse<UserResponse>, ApiErrorResponse)),
    tags((name = "auth", description = "Authentication and current-user operations"))
)]
struct SchoolApiDoc;

fn sort_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries = std::mem::take(map).into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (_, child) in &mut entries { sort_json(child); }
            map.extend(entries);
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json),
        _ => {}
    }
}

pub fn school_api_value() -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(SchoolApiDoc::openapi())?;
    sort_json(&mut value);
    Ok(value)
}

pub fn render_school_api() -> Result<String, serde_json::Error> {
    let mut output = serde_json::to_string_pretty(&school_api_value()?)?;
    output.push('\n');
    Ok(output)
}
```

If utoipa chooses an unexpected generic envelope component name, keep the derived `ApiResponse<UserResponse>` and make the test follow the response `$ref`; do not duplicate the envelope to control a cosmetic name.

- [ ] **Step 6: Add the early CLI branch**

Before `dotenv().ok()` in `main`:

```rust
let command_args = env::args().skip(1).collect::<Vec<_>>();
if command_args.first().map(String::as_str) == Some("export-openapi") {
    if command_args.len() != 1 {
        eprintln!("usage: backend-school export-openapi");
        std::process::exit(2);
    }
    match api_contract::render_school_api() {
        Ok(document) => {
            use std::io::Write;
            if let Err(error) = std::io::stdout().write_all(document.as_bytes()) {
                eprintln!("failed to write OpenAPI document: {error}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("failed to render OpenAPI document: {error}");
            std::process::exit(1);
        }
    }
    return;
}
```

This intentional CLI stdout/stderr is permitted by `.rules`. No dotenv, logging, pool, scheduler, or router code may run before return.

- [ ] **Step 7: Verify GREEN and offline export**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test api_contract::tests --bin backend-school
env -i PATH="$PATH" cargo run --quiet -- export-openapi > /tmp/schoolorbit-openapi.json
jq -e '.openapi == "3.1.0" and .paths["/api/auth/me"].get.operationId == "getCurrentUser"' /tmp/schoolorbit-openapi.json
cargo check --bin backend-school
```

Expected: tests PASS, `jq` prints `true`, and check succeeds.

- [ ] **Step 8: Commit**

```bash
git add backend-school/Cargo.toml backend-school/Cargo.lock backend-school/src/main.rs \
  backend-school/src/api_contract.rs backend-school/src/api_response.rs \
  backend-school/src/modules/auth/models.rs backend-school/src/modules/auth/handlers.rs
git commit -m "feat(api): export current-user OpenAPI contract"
```

---

### Task 2: Deterministic OpenAPI and TypeScript artifact generator

**Files:**

- Create: `scripts/generate-api-contracts.mjs`
- Create: `scripts/tests/generate-api-contracts.test.mjs`
- Create: `contracts/openapi/school-api.json`
- Create: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/package.json:5-20`
- Modify: `frontend-school/package-lock.json`

**Interfaces:**

- Consumes the backend `export-openapi` CLI.
- Produces `generateApiContracts({ repositoryRoot, check, exportSchoolDocument, generateSchoolTypes }) -> Promise<void>` for dependency-injected tests.
- Produces `npm run generate:api-contracts`, `npm run check:api-contracts`, and `npm run test:api-contracts` from `frontend-school`.

- [ ] **Step 1: Pin the generator and add package commands**

Run:

```bash
cd frontend-school
npm install --save-dev openapi-typescript@7.13.0
```

Add these scripts to `package.json`:

```json
"generate:api-contracts": "node ../scripts/generate-api-contracts.mjs",
"check:api-contracts": "node ../scripts/generate-api-contracts.mjs --check",
"test:api-contracts": "node --test ../scripts/tests/generate-api-contracts.test.mjs"
```

- [ ] **Step 2: Write failing generator tests**

Create `scripts/tests/generate-api-contracts.test.mjs`:

```javascript
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { generateApiContracts } from '../generate-api-contracts.mjs';

const document = {
  openapi: '3.1.0',
  info: { title: 'School API', version: '0.1.0' },
  paths: {},
  components: { schemas: {} }
};
const exportSchoolDocument = async () => document;
const generateSchoolTypes = async () => '// generated\nexport interface paths {}\n';

async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-api-contract-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(path.join(root, 'contracts/openapi'), { recursive: true });
  await mkdir(path.join(root, 'frontend-school/src/lib/api/generated'), { recursive: true });
  return root;
}

test('generation is deterministic', async (t) => {
  const repositoryRoot = await fixture(t);
  const options = { repositoryRoot, exportSchoolDocument, generateSchoolTypes };
  await generateApiContracts(options);
  const openApiPath = path.join(repositoryRoot, 'contracts/openapi/school-api.json');
  const typesPath = path.join(repositoryRoot, 'frontend-school/src/lib/api/generated/school-api.ts');
  const first = [await readFile(openApiPath, 'utf8'), await readFile(typesPath, 'utf8')];
  await generateApiContracts(options);
  assert.deepEqual(
    [await readFile(openApiPath, 'utf8'), await readFile(typesPath, 'utf8')],
    first
  );
  assert.ok(first[0].endsWith('\n'));
  assert.ok(first[1].startsWith('// @generated'));
});

test('check mode reports stale output without writing', async (t) => {
  const repositoryRoot = await fixture(t);
  const openApiPath = path.join(repositoryRoot, 'contracts/openapi/school-api.json');
  const typesPath = path.join(repositoryRoot, 'frontend-school/src/lib/api/generated/school-api.ts');
  await writeFile(openApiPath, 'old openapi\n');
  await writeFile(typesPath, 'old types\n');
  await assert.rejects(
    generateApiContracts({
      repositoryRoot,
      check: true,
      exportSchoolDocument,
      generateSchoolTypes
    }),
    /stale generated API contract artifacts/
  );
  assert.equal(await readFile(openApiPath, 'utf8'), 'old openapi\n');
  assert.equal(await readFile(typesPath, 'utf8'), 'old types\n');
});

test('type-generation failure leaves both tracked outputs untouched', async (t) => {
  const repositoryRoot = await fixture(t);
  const openApiPath = path.join(repositoryRoot, 'contracts/openapi/school-api.json');
  const typesPath = path.join(repositoryRoot, 'frontend-school/src/lib/api/generated/school-api.ts');
  await writeFile(openApiPath, 'old openapi\n');
  await writeFile(typesPath, 'old types\n');
  await assert.rejects(generateApiContracts({
    repositoryRoot,
    exportSchoolDocument,
    generateSchoolTypes: async () => { throw new Error('type generation failed'); }
  }), /type generation failed/);
  assert.equal(await readFile(openApiPath, 'utf8'), 'old openapi\n');
  assert.equal(await readFile(typesPath, 'utf8'), 'old types\n');
});
```

- [ ] **Step 3: Run tests and verify RED**

Run `cd frontend-school && npm run test:api-contracts`.

Expected: FAIL with `ERR_MODULE_NOT_FOUND` for `scripts/generate-api-contracts.mjs`.

- [ ] **Step 4: Implement render, temporary generation, check, and write**

Create `scripts/generate-api-contracts.mjs`. Use these exact public and default boundaries:

```javascript
import { execFile } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);
const scriptPath = fileURLToPath(import.meta.url);
const defaultRepositoryRoot = path.resolve(path.dirname(scriptPath), '..');

function sortValue(value) {
  if (Array.isArray(value)) return value.map(sortValue);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right, 'en'))
      .map(([key, child]) => [key, sortValue(child)]));
  }
  return value;
}

function renderOpenApi(document) {
  if (!document || document.openapi !== '3.1.0') {
    throw new Error('backend-school exporter must return OpenAPI 3.1.0');
  }
  return JSON.stringify(sortValue(document), null, 2) + '\n';
}

async function readIfPresent(filePath) {
  try { return await readFile(filePath, 'utf8'); }
  catch (error) {
    if (error?.code === 'ENOENT') return null;
    throw error;
  }
}
```

Export the Rust document using `execFileAsync('cargo', ['run', '--quiet', '--manifest-path', path.join(repositoryRoot, 'backend-school/Cargo.toml'), '--', 'export-openapi'])`, `cwd: repositoryRoot`, and a 16 MiB max buffer. Parse stdout as JSON.

Generate TypeScript only in `mkdtemp()` using the pinned executable at `frontend-school/node_modules/.bin/openapi-typescript` (append `.cmd` on Windows), with arguments `[inputPath, '--output', outputPath, '--alphabetize']`. Always remove the temporary directory in `finally`.

Implement the exported function exactly at this boundary:

```javascript
export async function generateApiContracts({
  repositoryRoot = defaultRepositoryRoot,
  check = false,
  exportSchoolDocument = defaultExportSchoolDocument,
  generateSchoolTypes = defaultGenerateSchoolTypes
} = {}) {
  const document = await exportSchoolDocument(repositoryRoot);
  const openApiOutput = renderOpenApi(document);
  const rawTypes = await generateSchoolTypes(repositoryRoot, document);
  const typeOutput = '// @generated by scripts/generate-api-contracts.mjs; DO NOT EDIT.\n'
    + rawTypes.replace(/^\/\/ This file was auto-generated by openapi-typescript\.\s*/, '');
  const outputs = [
    [path.join(repositoryRoot, 'contracts/openapi/school-api.json'), openApiOutput],
    [path.join(repositoryRoot, 'frontend-school/src/lib/api/generated/school-api.ts'), typeOutput]
  ];

  if (check) {
    const stale = [];
    for (const [filePath, expected] of outputs) {
      if ((await readIfPresent(filePath)) !== expected) {
        stale.push(path.relative(repositoryRoot, filePath));
      }
    }
    if (stale.length) {
      throw new Error('stale generated API contract artifacts: ' + stale.join(', '));
    }
    return;
  }

  for (const [filePath] of outputs) await mkdir(path.dirname(filePath), { recursive: true });
  for (const [filePath, output] of outputs) await writeFile(filePath, output);
}
```

For direct invocation, accept no argument or one `--check`; otherwise print usage and exit 2. Catch generation errors, print only the error message, and exit 1. All outputs must render before the first tracked write.

- [ ] **Step 5: Verify GREEN and generate real artifacts**

Run:

```bash
cd frontend-school
npm run test:api-contracts
npm run generate:api-contracts
npm run check:api-contracts
cp ../contracts/openapi/school-api.json /tmp/school-api.first.json
cp src/lib/api/generated/school-api.ts /tmp/school-api.first.ts
npm run generate:api-contracts
diff -u /tmp/school-api.first.json ../contracts/openapi/school-api.json
diff -u /tmp/school-api.first.ts src/lib/api/generated/school-api.ts
```

Expected: every command exits 0 and both diffs are empty.

- [ ] **Step 6: Verify current-user types are concrete**

Run:

```bash
rg -n "UserResponse|nationalId|primaryRoleName|permissions" src/lib/api/generated/school-api.ts
rg -n "UserResponse[^}]*(any|unknown)" src/lib/api/generated/school-api.ts
```

Expected: the first finds the schema/fields; the second exits 1 with no match.

- [ ] **Step 7: Commit**

```bash
git add scripts/generate-api-contracts.mjs scripts/tests/generate-api-contracts.test.mjs \
  contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/package.json frontend-school/package-lock.json
git commit -m "feat(api): generate school contract types"
```

---

### Task 3: Adopt the generated current-user wire DTO

**Files:**

- Modify: `frontend-school/src/lib/api/auth.ts:1-125`
- Test: `frontend-school/tests/static/api-response-contract.test.mjs:45-65`

**Interfaces:**

- Consumes `components['schemas']['UserResponse']` from `$lib/api/generated/school-api`.
- Produces exported `CurrentUserDto` and private `normalizeCurrentUser(dto: CurrentUserDto): User`.
- Preserves `AuthAPI.login`, `checkAuth`, and `refreshCurrentUser` behavior.

- [ ] **Step 1: Tighten the static contract test first**

Extend `frontend auth consumes the shared envelope through apiClient`:

```javascript
assert.match(source,
  /import\s+type\s+\{\s*components\s*\}\s+from\s+['"]\$lib\/api\/generated\/school-api['"]/);
assert.match(source,
  /export\s+type\s+CurrentUserDto\s*=\s*components\['schemas'\]\['UserResponse'\]/);
assert.match(source, /function\s+normalizeCurrentUser\(userData:\s*CurrentUserDto\):\s*User/);
assert.doesNotMatch(source, /interface\s+BackendUser/);
assert.doesNotMatch(source, /userData\.user_type/);
assert.doesNotMatch(source, /\.\.\.userData/);
assert.match(source, /nationalId:\s*userData\.nationalId\s*\?\?\s*undefined/);
assert.match(source, /profileImageUrl:\s*userData\.profileImageUrl\s*\?\?\s*undefined/);
```

- [ ] **Step 2: Run test and verify RED**

Run `cd frontend-school && node --test tests/static/api-response-contract.test.mjs`.

Expected: FAIL because `auth.ts` still contains `BackendUser`, snake-case fallback, and object spread.

- [ ] **Step 3: Implement the explicit wire-to-domain mapper**

Replace the old compatibility DTO/mapping block in `auth.ts`:

```typescript
import type { components } from '$lib/api/generated/school-api';

export type CurrentUserDto = components['schemas']['UserResponse'];

interface LoginData {
  user: CurrentUserDto;
}

function normalizeCurrentUser(userData: CurrentUserDto): User {
  return {
    id: userData.id,
    username: userData.username,
    nationalId: userData.nationalId ?? undefined,
    email: userData.email ?? undefined,
    firstName: userData.firstName,
    lastName: userData.lastName,
    role: userData.primaryRoleName ?? userData.userType,
    user_type: userData.userType,
    phone: userData.phone ?? undefined,
    status: userData.status,
    createdAt: userData.createdAt,
    primaryRoleName: userData.primaryRoleName,
    profileImageUrl: userData.profileImageUrl ?? undefined,
    permissions: userData.permissions
  };
}
```

Remove `BackendUser` and `normalizeUser`. Change login to `normalizeCurrentUser(response.data.user)`. Change the self request and mapping to:

```typescript
const response = await apiClient.get<CurrentUserDto>('/api/auth/me');
const userData = requireApiData(response, 'Failed to check auth');
const user = normalizeCurrentUser(userData);
```

Do not alter loading, catch, toast, cookie, or store behavior.

- [ ] **Step 4: Verify GREEN**

Run:

```bash
cd frontend-school
node --test tests/static/api-response-contract.test.mjs
npm run check
npm run test:static
```

Expected: all PASS, with zero Svelte errors and warnings.

- [ ] **Step 5: Commit**

```bash
git add frontend-school/src/lib/api/auth.ts frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "refactor(auth): consume generated current-user DTO"
```

---

### Task 4: Developer workflow and focused drift CI

**Files:**

- Create: `.github/workflows/api-contract.yml`
- Modify: `.rules:115-125,173-180`
- Modify: `docs/TESTING.md:45-75`
- Modify: `docs/backend-school/API_DEVELOPMENT.md:1-75`
- Test: `frontend-school/tests/static/api-response-contract.test.mjs`

**Interfaces:**

- Consumes generator/check/test commands from Task 2.
- Produces a documented source-of-truth workflow and a path-filtered GitHub Actions gate.

- [ ] **Step 1: Add a failing documentation guard**

Add to `api-response-contract.test.mjs`:

```javascript
test('project rules document generated API contract ownership', async () => {
  const rules = await readRepoFile('.rules');
  const testing = await readRepoFile('docs/TESTING.md');
  const guide = await readRepoFile('docs/backend-school/API_DEVELOPMENT.md');
  for (const source of [rules, testing, guide]) {
    assert.match(source, /generate:api-contracts/);
    assert.match(source, /check:api-contracts/);
    assert.match(source, /contracts\/openapi\/school-api\.json/);
    assert.match(source, /generated files?[^\n]*do not edit|do not edit[^\n]*generated files?/i);
  }
});
```

- [ ] **Step 2: Run test and verify RED**

Run `cd frontend-school && node --test tests/static/api-response-contract.test.mjs`.

Expected: FAIL because all three documents do not yet contain the workflow.

- [ ] **Step 3: Document the exact workflow**

Add a section named `Generated API contracts` to `.rules`, `docs/TESTING.md`, and `docs/backend-school/API_DEVELOPMENT.md` using the exact body below:

````markdown
### Generated API contracts

Rust request/response DTOs and OpenAPI handler metadata are the source of truth.
`contracts/openapi/school-api.json` and files under
`frontend-school/src/lib/api/generated/` are generated files; do not edit them
directly.

After changing a documented DTO or endpoint:

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Commit Rust source, OpenAPI, generated TypeScript, and focused tests together.
Frontend API modules import generated wire DTOs and may map them to separate
domain/view models. Generation must not require database credentials or start
the backend server.
````

Also correct the stale permission section in `API_DEVELOPMENT.md`: permissions are added in `contracts/permissions.json` and generated with `npm run generate:permissions`; never instruct developers to edit generated registries.

- [ ] **Step 4: Add focused CI**

Create `.github/workflows/api-contract.yml` with `pull_request` and `push` on `main`, filtered to:

```yaml
paths:
  - "contracts/openapi/**"
  - "scripts/generate-api-contracts.mjs"
  - "scripts/tests/generate-api-contracts.test.mjs"
  - "backend-school/Cargo.toml"
  - "backend-school/Cargo.lock"
  - "backend-school/src/api_contract.rs"
  - "backend-school/src/api_response.rs"
  - "backend-school/src/modules/**"
  - "frontend-school/src/lib/api/**"
  - "frontend-school/tests/static/api-response-contract.test.mjs"
  - "frontend-school/package.json"
  - "frontend-school/package-lock.json"
  - ".rules"
  - "docs/TESTING.md"
  - "docs/backend-school/API_DEVELOPMENT.md"
  - ".github/workflows/api-contract.yml"
```

Use Ubuntu 24.04, Node 22, stable Rust with rustfmt, and the same action versions as `permission-contract.yml`. Run these steps in order:

```yaml
- run: npm ci
  working-directory: frontend-school
- run: npm run test:api-contracts
  working-directory: frontend-school
- run: npm run check:api-contracts
  working-directory: frontend-school
- run: cargo fmt --all -- --check
  working-directory: backend-school
- run: cargo test api_contract::tests --bin backend-school
  working-directory: backend-school
- run: cargo check --bin backend-school
  working-directory: backend-school
- run: node --test tests/static/api-response-contract.test.mjs
  working-directory: frontend-school
- run: npm run check
  working-directory: frontend-school
```

Set the same non-secret `PUBLIC_BACKEND_URL`, `PUBLIC_VAPID_KEY`, and `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` values as the permission workflow.

- [ ] **Step 5: Verify GREEN and commit**

Run:

```bash
cd frontend-school
node --test tests/static/api-response-contract.test.mjs
npm run test:api-contracts
npm run check:api-contracts
cd ..
git diff --check
```

Expected: all PASS and no whitespace errors. Commit:

```bash
git add .github/workflows/api-contract.yml .rules docs/TESTING.md \
  docs/backend-school/API_DEVELOPMENT.md frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "ci(api): verify generated contracts"
```

---

### Task 5: Full pilot verification and rollout checkpoint

**Files:**

- Verify only; modify implementation files only to correct a failure proven by these commands.
- Reference: `docs/superpowers/specs/2026-07-22-shared-api-contracts-design.md`

**Interfaces:**

- Consumes all pilot outputs.
- Produces a reviewed baseline for the separate auth/authorization rollout plan.

- [ ] **Step 1: Prove generated artifacts are current**

Run `cd frontend-school && npm run test:api-contracts && npm run check:api-contracts`.

Expected: PASS with no stale artifacts.

- [ ] **Step 2: Run full backend verification**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --bin backend-school
cargo test --bin backend-school
cargo test --test static_architecture
cargo clippy --bin backend-school -- -D warnings
```

Expected: all PASS. Database-gated tests may be explicitly skipped only when documented environment variables are absent.

- [ ] **Step 3: Run full frontend verification**

```bash
cd frontend-school
npm run check
npm run test:static
npm run build
```

Expected: zero Svelte errors/warnings, all tests PASS, and production build succeeds.

- [ ] **Step 4: Check repository integrity**

```bash
git diff --check
git status --short
git log --oneline -5
```

Expected: no whitespace errors, only intentional files, and Tasks 1-4 are separate commits.

- [ ] **Step 5: Run smoke verification only with approved credentials**

When `SMOKE_BASE_URL`, `SMOKE_SCHOOL_KEY`, `SMOKE_USERNAME`, and `SMOKE_PASSWORD` are set, run `./scripts/smoke_test.sh` and expect login plus authenticated `/api/auth/me` to PASS. Otherwise report `NOT RUN — approved sandbox credentials unavailable`; never invent or persist credentials.

- [ ] **Step 6: Request code review**

Invoke `superpowers:requesting-code-review` on the full pilot diff. Fix only verified in-scope findings, rerun affected focused commands, then repeat Steps 1-4 before completion.

- [ ] **Step 7: Write the next bounded plan**

After pilot review, use `superpowers:writing-plans` to create `docs/superpowers/plans/2026-07-22-shared-api-contracts-auth-authorization.md`. It must inventory every remaining backend-school auth, role, permission, user-role assignment, and organization authorization route/DTO/frontend consumer explicitly and reuse the proven exporter/generator interfaces.

---

## Follow-on plan boundaries

The approved design covers all application JSON HTTP APIs. This plan intentionally stops after proving the toolchain with one operation: combining both backends and roughly 120 registered route declarations into one execution plan would prevent independent review and safe rollback.

Continue through separately executable plans:

1. `shared-api-contracts-auth-authorization` — remaining backend-school auth and authorization administration.
2. `shared-api-contracts-school-read` — lookup, menu, parents, dashboards, calendar, notifications, and other read APIs.
3. `shared-api-contracts-school-workflows-a` — academic and timetable APIs, split further if route inventory requires it.
4. `shared-api-contracts-school-workflows-b` — admission, supervision, exams, facilities, work, consent, files metadata, and remaining modules in bounded batches.
5. `shared-api-contracts-admin` — backend-admin, frontend-admin, and SvelteKit proxy boundaries.
6. `shared-api-contracts-coverage` — route inventory, justified protocol exclusions, missing-contract gate, and generated-client/runtime-validation decision.

Each begins with direct route/DTO/frontend-consumer inspection and ends with TDD, drift checks, behavior verification, and review.

## Primary references

- `docs/superpowers/specs/2026-07-22-shared-api-contracts-design.md`
- `.rules`
- `docs/backend-school/API_DEVELOPMENT.md`
- `docs/TESTING.md`
- https://docs.rs/utoipa/5.5.0/utoipa/derive.ToSchema.html
- https://docs.rs/utoipa/5.5.0/utoipa/openapi/
- https://openapi-ts.dev/cli
- https://openapi-ts.dev/node
