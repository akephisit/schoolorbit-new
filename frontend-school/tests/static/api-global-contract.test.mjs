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
		/\b(?:check_permission|check_any_permission|check_all_permissions|check_user_permission|has_permission)\s*\((?:(?!;).)*?"[a-z_]+(?:\.[a-z_]+){0,2}"/gs;
	const violations = [];

	for (const file of backendFiles) {
		const source = await readFile(file, 'utf8');
		const matches = source.matchAll(callWithPermissionLiteral);

		for (const match of matches) {
			violations.push(`${relative(file)}: ${match[0].replace(/\s+/g, ' ').slice(0, 140)}`);
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
		if (/\bUserPermissions\b|\bhas_permission\s*\(/.test(source)) {
			violations.push(relative(file));
		}
	}

	assert.deepEqual(violations, []);
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
