import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

async function listFiles(dir, predicate) {
	const entries = await readdir(dir, { withFileTypes: true });
	const files = [];

	for (const entry of entries) {
		const fullPath = path.join(dir, entry.name);
		if (entry.isDirectory()) {
			files.push(...(await listFiles(fullPath, predicate)));
		} else if (predicate(fullPath)) {
			files.push(fullPath);
		}
	}

	return files;
}

function relative(filePath) {
	return path.relative(repoRoot, filePath);
}

function stripComments(source) {
	return source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '');
}

function extractJsonResponseBlocks(source) {
	const markers = ['Json(json!', 'Json(serde_json::json!', 'JsonResponse(serde_json::json!'];
	const blocks = [];

	for (const marker of markers) {
		let index = 0;
		while ((index = source.indexOf(marker, index)) !== -1) {
			const openBrace = source.indexOf('{', index + marker.length);
			if (openBrace === -1) break;

			let depth = 0;
			let inString = false;
			let escaped = false;
			for (let cursor = openBrace; cursor < source.length; cursor += 1) {
				const char = source[cursor];

				if (inString) {
					if (escaped) {
						escaped = false;
					} else if (char === '\\') {
						escaped = true;
					} else if (char === '"') {
						inString = false;
					}
					continue;
				}

				if (char === '"') {
					inString = true;
				} else if (char === '{') {
					depth += 1;
				} else if (char === '}') {
					depth -= 1;
					if (depth === 0) {
						blocks.push(source.slice(openBrace, cursor + 1));
						index = cursor + 1;
						break;
					}
				}
			}
		}
	}

	return blocks;
}

function topLevelKeys(jsonMacroObject) {
	const keys = [];
	let depth = 0;
	let inString = false;
	let escaped = false;
	let stringStart = 0;
	let lastString = null;

	for (let cursor = 0; cursor < jsonMacroObject.length; cursor += 1) {
		const char = jsonMacroObject[cursor];

		if (inString) {
			if (escaped) {
				escaped = false;
			} else if (char === '\\') {
				escaped = true;
			} else if (char === '"') {
				inString = false;
				lastString = jsonMacroObject.slice(stringStart, cursor);
			}
			continue;
		}

		if (char === '"') {
			inString = true;
			stringStart = cursor + 1;
		} else if (char === '{') {
			depth += 1;
		} else if (char === '}') {
			depth -= 1;
		} else if (char === ':' && depth === 1 && lastString) {
			keys.push(lastString);
			lastString = null;
		} else if (char === ',' && depth === 1) {
			lastString = null;
		}
	}

	return keys;
}

function rawJsonResponseIdentifiers(source) {
	const identifiers = [];

	for (const line of source.split('\n')) {
		if (!/\bOk\s*\(/.test(line)) continue;

		const match = /\bJson\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*\)/.exec(line);
		if (match) {
			identifiers.push(match[1]);
		}
	}

	return identifiers;
}

function extractPermissionRegistry(source) {
	const constants = new Map();
	const allPermissionConstantNames = new Set();
	const modules = new Set();

	for (const match of source.matchAll(/pub const ([A-Z0-9_]+): &str = "([^"]+)";/g)) {
		constants.set(match[1], match[2]);
	}

	for (const match of source.matchAll(/code:\s*codes::([A-Z0-9_]+)/g)) {
		allPermissionConstantNames.add(match[1]);
	}

	for (const match of source.matchAll(/module:\s*"([^"]+)"/g)) {
		modules.add(match[1]);
	}

	const allPermissionCodes = new Set(
		[...allPermissionConstantNames].map((name) => constants.get(name)).filter(Boolean)
	);

	return {
		constants,
		allPermissionConstantNames,
		allPermissionCodes,
		modules
	};
}

test('backend JSON handler responses use the standard envelope shape', async () => {
	const backendFiles = await listFiles(path.join(repoRoot, 'backend-school/src/modules'), (file) =>
		file.endsWith('.rs')
	);
	const violations = [];

	for (const file of backendFiles) {
		const source = await readFile(file, 'utf8');
		const blocks = extractJsonResponseBlocks(source);
		const rawIdentifiers = rawJsonResponseIdentifiers(source);

		for (const identifier of rawIdentifiers) {
			violations.push(`${relative(file)}: raw Json(${identifier}) response must use { success, data }`);
		}

		for (const block of blocks) {
			const keys = topLevelKeys(block);
			const keySet = new Set(keys);
			const hasSuccess = keySet.has('success');
			const hasSuccessTrue = /"success"\s*:\s*true/.test(block);
			const hasSuccessFalse = /"success"\s*:\s*false/.test(block);
			const allowedSuccessKeys = new Set(['success', 'data', 'message']);
			const allowedErrorKeys = new Set(['success', 'error', 'message', 'data']);

			if (!hasSuccess) {
				violations.push(`${relative(file)}: missing top-level success in ${block.slice(0, 120)}`);
				continue;
			}

			if (hasSuccessTrue) {
				const extraKeys = keys.filter((key) => !allowedSuccessKeys.has(key));
				if (!keySet.has('data')) {
					violations.push(`${relative(file)}: success response missing data in ${block.slice(0, 120)}`);
				}
				if (extraKeys.length > 0) {
					violations.push(
						`${relative(file)}: success response has non-envelope keys ${extraKeys.join(', ')}`
					);
				}
			} else if (hasSuccessFalse) {
				const extraKeys = keys.filter((key) => !allowedErrorKeys.has(key));
				if (!keySet.has('error')) {
					violations.push(`${relative(file)}: error response missing error in ${block.slice(0, 120)}`);
				}
				if (extraKeys.length > 0) {
					violations.push(
						`${relative(file)}: error response has non-envelope keys ${extraKeys.join(', ')}`
					);
				}
			} else {
				violations.push(`${relative(file)}: success is not statically true/false in ${block.slice(0, 120)}`);
			}
		}
	}

	assert.deepEqual(violations, []);
});

test('backend consent type filter uses the user_type query parameter contract', async () => {
	const file = path.join(repoRoot, 'backend-school/src/modules/consent/handlers.rs');
	const source = await readFile(file, 'utf8');

	assert.match(source, /\.get\("user_type"\)/);
	assert.equal(source.includes('headers.get("user-type")'), false);
});

test('backend auth middleware and login validation errors use the response envelope', async () => {
	const middleware = await readFile(
		path.join(repoRoot, 'backend-school/src/middleware/auth.rs'),
		'utf8'
	);
	const loginHandler = await readFile(
		path.join(repoRoot, 'backend-school/src/modules/auth/handlers.rs'),
		'utf8'
	);

	assert.match(middleware, /ApiErrorResponse::new\("No authentication token found"\)/);
	assert.match(middleware, /ApiErrorResponse::new\(format!\("Invalid token:/);
	assert.equal(middleware.includes('Json(json!'), false);

	assert.match(loginHandler, /Result<Json<LoginRequest>, JsonRejection>/);
	assert.match(loginHandler, /AppError::ValidationError\(rejection\.body_text\(\)\)/);
});

test('backend permission checks use registry constants instead of string literals', async () => {
	const backendFiles = await listFiles(path.join(repoRoot, 'backend-school/src'), (file) =>
		file.endsWith('.rs')
	);
	const callWithPermissionLiteral =
		/\b(?:has_permission|has_any_permission|has_all_permissions|require_permission|require_any_permission|require_all_permissions)\s*\((?:(?!;).)*?"[a-z_]+(?:\.[a-z_]+){0,2}"/gs;
	const violations = [];

	for (const file of backendFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		const matches = source.matchAll(callWithPermissionLiteral);

		for (const match of matches) {
			if (match[0].includes('codes::')) continue;
			violations.push(`${relative(file)}: ${match[0].replace(/\s+/g, ' ').slice(0, 140)}`);
		}
	}

	assert.deepEqual(violations, []);
});

test('backend permission handlers use ActorContext loader APIs only', async () => {
	const backendFiles = await listFiles(path.join(repoRoot, 'backend-school/src'), (file) =>
		file.endsWith('.rs')
	);
	const legacyPermissionHelpers =
		/\b(?:check_permission|check_any_permission|check_all_permissions|check_user_permission|get_actor_context|get_actor_context_or_error)\b/;
	const violations = [];

	for (const file of backendFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		if (legacyPermissionHelpers.test(source)) {
			violations.push(
				`${relative(file)}: use load_actor_context/load_actor_context_or_error and actor.require_* helpers`
			);
		}
	}

	assert.deepEqual(violations, []);
});

test('permission registry covers backend and frontend permission references', async () => {
	const registrySource = await readFile(
		path.join(repoRoot, 'backend-school/src/permissions/registry.rs'),
		'utf8'
	);
	const { constants, allPermissionConstantNames, allPermissionCodes, modules } =
		extractPermissionRegistry(registrySource);
	const violations = [];

	for (const name of allPermissionConstantNames) {
		if (!constants.has(name)) {
			violations.push(`registry: ALL_PERMISSIONS references missing codes::${name}`);
		}
	}

	for (const [name, code] of constants) {
		if (!allPermissionConstantNames.has(name)) {
			violations.push(`registry: codes::${name} (${code}) is not included in ALL_PERMISSIONS`);
		}
	}

	const duplicateCodes = [...constants.values()].filter(
		(code, index, codes) => codes.indexOf(code) !== index
	);
	for (const code of new Set(duplicateCodes)) {
		violations.push(`registry: duplicate permission code ${code}`);
	}

	const backendFiles = await listFiles(path.join(repoRoot, 'backend-school/src'), (file) =>
		file.endsWith('.rs')
	);
	for (const file of backendFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		for (const match of source.matchAll(/\bcodes::([A-Z0-9_]+)/g)) {
			const name = match[1];
			if (!constants.has(name)) {
				violations.push(`${relative(file)}: unknown permission constant codes::${name}`);
			}
		}
	}

	const frontendFiles = await listFiles(path.join(repoRoot, 'frontend-school/src'), (file) =>
		/\.(svelte|ts)$/.test(file)
	);
	const frontendPermissionPatterns = [
		/\bpermission:\s*['"]([^'"]+)['"]/g,
		/\$?can\.(?:has|hasModule|hasAny|hasAll)\(\s*['"]([^'"]+)['"]/g,
		/\bpermissions\.has\(\s*['"]([^'"]+)['"]\s*\)/g,
		/\bpermissions(?:\?\.)?\.includes\(\s*['"]([^'"]+)['"]\s*\)/g
	];

	for (const file of frontendFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		for (const pattern of frontendPermissionPatterns) {
			for (const match of source.matchAll(pattern)) {
				const permission = match[1];
				if (
					permission !== '*' &&
					!allPermissionCodes.has(permission) &&
					!modules.has(permission)
				) {
					violations.push(`${relative(file)}: unknown permission reference ${permission}`);
				}
			}
		}
	}

	assert.deepEqual(violations, []);
});

test('backend permissions do not use the legacy UserPermissions resolver', async () => {
	const backendFiles = await listFiles(path.join(repoRoot, 'backend-school/src'), (file) =>
		file.endsWith('.rs')
	);
	const violations = [];

	for (const file of backendFiles) {
		const source = await readFile(file, 'utf8');
		if (/\bUserPermissions\b|\bget_user_with_permissions\b/.test(source)) {
			violations.push(relative(file));
		}
	}

	assert.deepEqual(violations, []);
});

test('backend module handlers use ActorContext instead of raw permission lists', async () => {
	const backendFiles = await listFiles(
		path.join(repoRoot, 'backend-school/src/modules'),
		(file) => file.endsWith('.rs')
	);
	const violations = [];

	for (const file of backendFiles) {
		if (relative(file) === 'backend-school/src/modules/auth/handlers.rs') continue;

		const source = await readFile(file, 'utf8');
		if (/\bget_cached_user_permissions\b|\bpermission_matches\s*\(/.test(source)) {
			violations.push(relative(file));
		}
	}

	assert.deepEqual(violations, []);
});

test('backend auth responses use the shared effective permission resolver', async () => {
	const authHandler = await readFile(
		path.join(repoRoot, 'backend-school/src/modules/auth/handlers.rs'),
		'utf8'
	);

	assert.match(authHandler, /\bget_cached_user_permissions\b/);
	assert.equal(authHandler.includes('permission_delegations'), false);
	assert.equal(authHandler.includes('department_permissions dp'), false);
	assert.equal(authHandler.includes('JOIN role_permissions'), false);
});

test('backend menu and feature handlers do not parse auth or query permissions directly', async () => {
	const checkedFiles = [
		'backend-school/src/modules/menu/handlers/admin.rs',
		'backend-school/src/modules/menu/services/menu_service.rs',
		'backend-school/src/modules/system/handlers/feature_toggles.rs'
	];
	const violations = [];

	for (const relativePath of checkedFiles) {
		const source = await readFile(path.join(repoRoot, relativePath), 'utf8');
		if (/\bJwtService\b|\bfield_encryption\b|JOIN role_permissions|permission_delegations/.test(source)) {
			violations.push(relativePath);
		}
	}

	assert.deepEqual(violations, []);
});

test('backend permission cache invalidations notify active clients', async () => {
	const backendFiles = await listFiles(path.join(repoRoot, 'backend-school/src'), (file) =>
		file.endsWith('.rs')
	);
	const violations = [];

	for (const file of backendFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		const lines = source.split('\n');
		for (let index = 0; index < lines.length; index += 1) {
			const line = lines[index];
			const nextLines = lines.slice(index + 1, index + 4).join('\n');
			if (
				line.includes('permission_cache.clear_all()') &&
				!nextLines.includes('notify_all_permissions_changed()')
			) {
				violations.push(`${relative(file)}:${index + 1}: clear_all must emit permission_changed`);
			}
			if (
				line.includes('permission_cache.invalidate(') &&
				!nextLines.includes('notify_permission_changed(')
			) {
				violations.push(`${relative(file)}:${index + 1}: invalidate must emit permission_changed`);
			}
		}
	}

	assert.deepEqual(violations, []);
});

test('frontend permission updates use SSE-triggered silent auth refresh', async () => {
	const notificationStore = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/stores/notification.ts'),
		'utf8'
	);
	const authApi = await readFile(path.join(repoRoot, 'frontend-school/src/lib/api/auth.ts'), 'utf8');

	assert.match(notificationStore, /addEventListener\('permission_changed'/);
	assert.match(notificationStore, /refreshCurrentUser\(\{\s*silent:\s*true\s*\}\)/);
	assert.match(authApi, /async refreshCurrentUser\(/);
	assert.match(authApi, /if \(!silent\) authStore\.setLoading\(true\)/);
});

test('frontend current-user permission checks go through the can store', async () => {
	const frontendFiles = await listFiles(path.join(repoRoot, 'frontend-school/src'), (file) =>
		/\.(svelte|ts)$/.test(file)
	);
	const allowedFiles = new Set([
		'frontend-school/src/routes/(app)/debug/+page.svelte',
		'frontend-school/src/lib/stores/permissions.ts',
		'frontend-school/src/lib/auth/route-access.ts'
	]);
	const violations = [];

	for (const file of frontendFiles) {
		const rel = relative(file);
		if (allowedFiles.has(rel)) continue;

		const source = stripComments(await readFile(file, 'utf8'));
		if (
			/(?:authState|authStore|\$authStore|user)\.user\??\.permissions\??\.includes\(/.test(
				source
			)
		) {
			violations.push(`${rel}: use $can.has/$can.hasAny instead of direct current-user includes`);
		}
	}

	assert.deepEqual(violations, []);
});

test('frontend does not use legacy separate current-user permission loading', async () => {
	const frontendFiles = await listFiles(path.join(repoRoot, 'frontend-school/src'), (file) =>
		/\.(svelte|ts)$/.test(file)
	);
	const violations = [];

	for (const file of frontendFiles) {
		const rel = relative(file);
		const source = stripComments(await readFile(file, 'utf8'));
		if (/\bloadUserPermissions\b|\bpermissionsLoading\b/.test(source)) {
			violations.push(`${rel}: current-user permissions must come from /api/auth/me`);
		}
	}

	assert.deepEqual(violations, []);
});

test('internal API secrets use constant-time comparison and caller headers', async () => {
	const checkedFiles = [
		'backend-school/src/middleware/internal_auth.rs',
		'backend-admin/src/handlers/internal.rs'
	];

	for (const relativePath of checkedFiles) {
		const source = await readFile(path.join(repoRoot, relativePath), 'utf8');
		assert.match(source, /ConstantTimeEq/);
		assert.match(source, /X-Internal-Caller/);
		assert.match(source, /INTERNAL_API_SECRET_/);
		assert.equal(source.includes('!= internal_secret'), false);
		assert.equal(source.includes('== internal_secret'), false);
	}

	const backendSchoolClient = await readFile(
		path.join(repoRoot, 'backend-school/src/db/admin_client.rs'),
		'utf8'
	);
	const backendAdminClient = await readFile(
		path.join(repoRoot, 'backend-admin/src/clients/backend_school_client.rs'),
		'utf8'
	);

	assert.match(backendSchoolClient, /X-Internal-Caller/);
	assert.match(backendSchoolClient, /backend-school/);
	assert.match(backendAdminClient, /X-Internal-Caller/);
	assert.match(backendAdminClient, /backend-admin/);
});

test('backend module handlers resolve tenant pools through the central resolver', async () => {
	const backendFiles = await listFiles(path.join(repoRoot, 'backend-school/src/modules'), (file) =>
		file.endsWith('.rs')
	);
	const directPoolAllowed = new Set(['backend-school/src/modules/system/handlers/migration.rs']);
	const violations = [];

	for (const file of backendFiles) {
		const source = await readFile(file, 'utf8');
		const fileName = relative(file);

		if (source.includes('get_school_database_url')) {
			violations.push(`${fileName}: use utils::tenant resolver instead of get_school_database_url`);
		}

		if (source.includes('PgPool::connect(')) {
			violations.push(`${fileName}: use AppState PoolManager via utils::tenant resolver`);
		}

		if (!directPoolAllowed.has(fileName) && /\.pool_manager\s*\.get_pool\s*\(/.test(source)) {
			violations.push(`${fileName}: use utils::tenant resolver instead of pool_manager.get_pool`);
		}
	}

	assert.deepEqual(violations, []);
});

test('tenant routing uses Origin by default with explicit X-School-Subdomain override', async () => {
	const subdomainResolver = await readFile(
		path.join(repoRoot, 'backend-school/src/utils/subdomain.rs'),
		'utf8'
	);
	const apiClient = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/api/client.ts'),
		'utf8'
	);

	assert.match(subdomainResolver, /SCHOOL_SUBDOMAIN_HEADER/);
	assert.match(subdomainResolver, /normalize_subdomain/);
	assert.match(subdomainResolver, /headers\.get\(SCHOOL_SUBDOMAIN_HEADER\)/);
	assert.match(subdomainResolver, /Subdomain header does not match origin/);
	assert.match(subdomainResolver, /\.get\("origin"\)/);
	assert.match(subdomainResolver, /\.get\("referer"\)/);

	assert.match(apiClient, /X-School-Subdomain/);
	assert.match(apiClient, /PUBLIC_SCHOOL_SUBDOMAIN/);
	assert.equal(apiClient.includes('window.location.hostname'), false);
	assert.match(apiClient, /applyTenantHeader\(headers\)/);
});

test('frontend application code routes backend API calls through apiClient', async () => {
	const frontendFiles = await listFiles(path.join(repoRoot, 'frontend-school/src'), (file) =>
		/\.(svelte|ts)$/.test(file)
	);
	const allowedFetchFiles = new Set([
		'frontend-school/src/lib/api/client.ts',
		'frontend-school/src/lib/utils/pdf.ts',
		'frontend-school/src/service-worker.ts'
	]);
	const violations = [];

	for (const file of frontendFiles) {
		const rel = relative(file);
		if (allowedFetchFiles.has(rel)) continue;

		const source = await readFile(file, 'utf8');
		if (/\bfetch\s*\(/.test(source)) {
			violations.push(rel);
		}
	}

	assert.deepEqual(violations, []);
});
