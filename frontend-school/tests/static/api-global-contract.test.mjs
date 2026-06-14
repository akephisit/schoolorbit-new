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

	for (const match of source.matchAll(/pub const ([A-Z0-9_]+): &str =\s*"([^"]+)";/g)) {
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

function extractConstObjectValues(source, objectName) {
	const match = new RegExp(
		`export const ${objectName}\\s*=\\s*\\{([\\s\\S]*?)\\}\\s*as const`
	).exec(source);
	if (!match) return new Map();

	const values = new Map();
	for (const valueMatch of match[1].matchAll(/\b([A-Z0-9_]+):\s*['"]([^'"]+)['"]/g)) {
		values.set(valueMatch[1], valueMatch[2]);
	}
	return values;
}

function appRouteIdFromFile(filePath, suffix) {
	const appRoutesDir = path.join(repoRoot, 'frontend-school/src/routes/(app)');
	const routePath = path.relative(appRoutesDir, filePath.replace(suffix, ''));
	const normalized = routePath.split(path.sep).filter(Boolean).join('/');
	return normalized ? `/(app)/${normalized}` : '/(app)';
}

function hasGuardedAncestor(routeId, guardedRouteIds) {
	let currentRouteId = routeId;
	while (currentRouteId.length > 0) {
		if (guardedRouteIds.has(currentRouteId)) return true;

		const lastSlash = currentRouteId.lastIndexOf('/');
		if (lastSlash <= 0) break;
		currentRouteId = currentRouteId.slice(0, lastSlash);
	}

	return false;
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
			violations.push(
				`${relative(file)}: raw Json(${identifier}) response must use { success, data }`
			);
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
					violations.push(
						`${relative(file)}: success response missing data in ${block.slice(0, 120)}`
					);
				}
				if (extraKeys.length > 0) {
					violations.push(
						`${relative(file)}: success response has non-envelope keys ${extraKeys.join(', ')}`
					);
				}
			} else if (hasSuccessFalse) {
				const extraKeys = keys.filter((key) => !allowedErrorKeys.has(key));
				if (!keySet.has('error')) {
					violations.push(
						`${relative(file)}: error response missing error in ${block.slice(0, 120)}`
					);
				}
				if (extraKeys.length > 0) {
					violations.push(
						`${relative(file)}: error response has non-envelope keys ${extraKeys.join(', ')}`
					);
				}
			} else {
				violations.push(
					`${relative(file)}: success is not statically true/false in ${block.slice(0, 120)}`
				);
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

	const frontendRegistrySource = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/permissions/registry.ts'),
		'utf8'
	);
	const frontendPermissions = extractConstObjectValues(frontendRegistrySource, 'PERMISSIONS');
	const frontendModules = extractConstObjectValues(frontendRegistrySource, 'PERMISSION_MODULES');

	for (const [name, permission] of frontendPermissions) {
		if (!allPermissionCodes.has(permission)) {
			violations.push(`frontend registry: PERMISSIONS.${name} is not in backend registry`);
		}
	}

	for (const [name, module] of frontendModules) {
		if (!modules.has(module)) {
			violations.push(`frontend registry: PERMISSION_MODULES.${name} is not in backend registry`);
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
				if (permission !== '*' && !allPermissionCodes.has(permission) && !modules.has(permission)) {
					violations.push(`${relative(file)}: unknown permission reference ${permission}`);
				}
			}
		}
	}

	assert.deepEqual(violations, []);
});

test('frontend app route metadata uses permission registry constants', async () => {
	const routeFiles = await listFiles(
		path.join(repoRoot, 'frontend-school/src/routes/(app)'),
		(file) => file.endsWith('+page.ts')
	);
	const violations = [];

	for (const file of routeFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		if (/\bpermission:\s*['"][^'"]+['"]/.test(source)) {
			violations.push(`${relative(file)}: use PERMISSIONS/PERMISSION_MODULES constants`);
		}
	}

	assert.deepEqual(violations, []);
});

test('frontend app pages have route guard metadata or guarded ancestor fallback', async () => {
	const appRoutesDir = path.join(repoRoot, 'frontend-school/src/routes/(app)');
	const pageSvelteFiles = await listFiles(appRoutesDir, (file) => file.endsWith('+page.svelte'));
	const pageTsFiles = await listFiles(appRoutesDir, (file) => file.endsWith('+page.ts'));
	const guardedRouteIds = new Set();
	const allowedOpenRoutes = new Set(['/(app)/403', '/(app)/debug', '/(app)/settings/consent']);
	const violations = [];

	for (const file of pageTsFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		if (/\b_meta\s*=/.test(source) && /\b(?:menu|access)\s*:/.test(source)) {
			guardedRouteIds.add(appRouteIdFromFile(file, '/+page.ts'));
		}
	}

	for (const file of pageSvelteFiles) {
		const routeId = appRouteIdFromFile(file, '/+page.svelte');
		if (allowedOpenRoutes.has(routeId)) continue;
		if (!hasGuardedAncestor(routeId, guardedRouteIds)) {
			violations.push(`${relative(file)}: missing _meta.menu guard metadata or guarded ancestor`);
		}
	}

	assert.deepEqual(violations, []);
});

test('frontend route access supports guard-only metadata without creating menu items', async () => {
	const routeAccess = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/auth/route-access.ts'),
		'utf8'
	);
	const workManageRoute = await readFile(
		path.join(repoRoot, 'frontend-school/src/routes/(app)/staff/work/manage/+page.ts'),
		'utf8'
	);

	assert.match(routeAccess, /\baccess\?:/);
	assert.match(routeAccess, /\bworkflowManage\?:\s*boolean/);
	assert.match(routeAccess, /module\._meta\?\.access\s*\?\?/);
	assert.match(routeAccess, /hasWorkflowManagePermission\(permissions\)/);

	assert.match(workManageRoute, /\baccess:\s*\{/);
	assert.match(workManageRoute, /\buser_type:\s*'staff'/);
	assert.match(workManageRoute, /\bworkflowManage:\s*true/);
	assert.doesNotMatch(workManageRoute, /\bmenu:\s*\{/);
});

test('frontend menu route metadata is complete for deployment sync', async () => {
	const routeFiles = await listFiles(
		path.join(repoRoot, 'frontend-school/src/routes/(app)'),
		(file) => file.endsWith('+page.ts')
	);
	const violations = [];
	const allowedWorkspaces = new Set([
		'home',
		'teaching',
		'academic',
		'student_affairs',
		'personnel',
		'operations',
		'settings'
	]);

	for (const file of routeFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		if (!/\b_meta\s*=/.test(source) || !/\bmenu\s*:/.test(source)) continue;

		for (const requiredField of ['title', 'icon', 'group', 'workspace', 'order', 'user_type']) {
			if (!new RegExp(`\\b${requiredField}:`).test(source)) {
				violations.push(`${relative(file)}: _meta.menu is missing ${requiredField}`);
			}
		}

		const workspaceMatch = /\bworkspace:\s*['"]([^'"]+)['"]/.exec(source);
		if (workspaceMatch && !allowedWorkspaces.has(workspaceMatch[1])) {
			violations.push(`${relative(file)}: unknown _meta.menu.workspace ${workspaceMatch[1]}`);
		}
	}

	assert.deepEqual(violations, []);
});

test('frontend permission updates use SSE-triggered silent auth refresh', async () => {
	const notificationStore = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/stores/notification.ts'),
		'utf8'
	);
	const authApi = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/api/auth.ts'),
		'utf8'
	);

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
			/(?:authState|authStore|\$authStore|user)\.user\??\.permissions\??\.includes\(/.test(source)
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

test('frontend runtime uses organization units instead of legacy department endpoints', async () => {
	const frontendFiles = await listFiles(path.join(repoRoot, 'frontend-school/src'), (file) =>
		/\.(svelte|ts)$/.test(file)
	);
	const legacyOrganizationPatterns =
		/\/staff\/departments|\/api\/departments|\/api\/lookup\/departments|\bdepartment_assignments\b|\bdepartment_id\b|\bparent_department_id\b|\bis_primary_department\b|\borg_type\b|\bstaff\.departments\b|\blistDepartments\b|\bgetDepartment\b|\bcreateDepartment\b|\bupdateDepartment\b|\bdeleteDepartment\b/;
	const violations = [];

	for (const file of frontendFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		if (legacyOrganizationPatterns.test(source)) {
			violations.push(relative(file));
		}
	}

	assert.deepEqual(violations, []);
});

test('frontend permission contracts use organization units instead of department names', async () => {
	const frontendFiles = await listFiles(path.join(repoRoot, 'frontend-school/src'), (file) =>
		/\.(svelte|ts)$/.test(file)
	);
	const legacyPermissionPatterns =
		/(['"`])(?:(?:(?!\1).)*)(?:dept_work|\.department)(?:(?:(?!\1).)*)\1|\bDEPT_WORK_[A-Z0-9_]*\b|\bACADEMIC_CURRICULUM_MANAGE_DEPARTMENT\b/;
	const violations = [];

	for (const file of frontendFiles) {
		const source = stripComments(await readFile(file, 'utf8'));
		if (legacyPermissionPatterns.test(source)) {
			violations.push(relative(file));
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
	const nginxConfig = await readFile(
		path.join(repoRoot, 'nginx-configs/school-api.schoolorbit.app.conf'),
		'utf8'
	);
	const smokeTest = await readFile(path.join(repoRoot, 'scripts/smoke_test.sh'), 'utf8');

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

	assert.match(nginxConfig, /Access-Control-Allow-Headers[\s\S]*X-School-Subdomain/);
	assert.match(smokeTest, /expect_header_contains_ci/);
	assert.match(smokeTest, /access-control-allow-headers[\s\S]*x-school-subdomain/);
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

test('frontend API contract avoids unknown endpoint generics and blind envelope casts', async () => {
	const frontendFiles = await listFiles(path.join(repoRoot, 'frontend-school/src'), (file) =>
		/\.(svelte|ts)$/.test(file)
	);
	const apiFiles = await listFiles(path.join(repoRoot, 'frontend-school/src/lib/api'), (file) =>
		file.endsWith('.ts')
	);
	const violations = [];

	for (const file of frontendFiles) {
		const rel = relative(file);
		const source = stripComments(await readFile(file, 'utf8'));
		if (
			/\bapiClient\.(?:get|post|put|patch|delete|deleteWithBody|postMultipart)<\s*unknown\s*>/.test(
				source
			)
		) {
			violations.push(`${rel}: use a concrete apiClient<T> response type instead of unknown`);
		}
		if (/\bApiResponse<\s*unknown\s*>/.test(source)) {
			violations.push(`${rel}: use a concrete ApiResponse<T> type instead of unknown`);
		}
		if (
			/\b(?:ApiResponse|LoadedApiResponse)<\s*void\s*>/.test(source) ||
			/\bapiClient\.(?:get|post|put|patch|delete|deleteWithBody|postMultipart)<\s*void\s*>/.test(
				source
			) ||
			/\bfetchApi<\s*void\s*>/.test(source)
		) {
			violations.push(
				`${rel}: empty mutation responses must use Record<string, never> instead of void`
			);
		}
	}

	for (const file of apiFiles) {
		const rel = relative(file);
		const source = stripComments(await readFile(file, 'utf8'));
		if (/fetchApi<\s*T\s*=\s*unknown\s*>/.test(source)) {
			violations.push(`${rel}: fetchApi default generic must be a concrete empty response type`);
		}
		if (/return\s+(?:response|res)\s+as\b/.test(source)) {
			violations.push(`${rel}: API helpers must not return blind response casts`);
		}
		if (/\b(?:response|res)\.data\s+as\b/.test(source)) {
			violations.push(`${rel}: API helpers must not cast envelope data in endpoint wrappers`);
		}
		if (/apiClient\.(?:get|post|put|patch|delete|deleteWithBody|postMultipart)\s*\(/.test(source)) {
			violations.push(`${rel}: endpoint wrappers must call apiClient with a concrete generic`);
		}
	}

	assert.deepEqual(violations, []);
});

test('frontend apiClient validates the backend envelope before returning typed responses', async () => {
	const source = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/api/client.ts'),
		'utf8'
	);

	assert.match(source, /function\s+normalizeApiResponse<T>/);
	assert.match(source, /typeof\s+payload\.success\s*!==\s*'boolean'/);
	assert.match(source, /!\('data'\s+in\s+payload\)/);
	assert.doesNotMatch(source, /return\s+data\s+as\s+ApiResponse<T>/);
});

test('backend services do not return raw serde_json::Value for API contracts', async () => {
	const serviceFiles = await listFiles(
		path.join(repoRoot, 'backend-school/src/modules'),
		(file) => file.endsWith('.rs') && /\/services(?:\/|\.rs$)/.test(file)
	);
	const violations = [];

	for (const file of serviceFiles) {
		const rel = relative(file);
		const source = stripComments(await readFile(file, 'utf8'));
		if (/Result\s*<\s*serde_json::Value\s*,\s*AppError\s*>/.test(source)) {
			violations.push(`${rel}: return a typed DTO/outcome instead of serde_json::Value`);
		}
		if (/Result\s*<\s*Vec\s*<\s*serde_json::Value\s*>\s*,\s*AppError\s*>/.test(source)) {
			violations.push(`${rel}: return a typed DTO vector instead of Vec<serde_json::Value>`);
		}
	}

	assert.deepEqual(violations, []);
});
