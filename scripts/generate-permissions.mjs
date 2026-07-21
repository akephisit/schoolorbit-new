import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const ALLOWED_ACTIONS = new Set([
	'all',
	'approve',
	'assign',
	'create',
	'delete',
	'enroll',
	'evaluate',
	'execute',
	'manage',
	'manage_members',
	'publish',
	'read',
	'remove',
	'request',
	'scores',
	'update',
	'verify'
]);
const ALLOWED_SCOPES = new Set([
	'all',
	'assigned',
	'global',
	'organization_tree',
	'organization_unit',
	'own',
	'school'
]);
const COMPONENT_PATTERN = /^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$/;
const NORMAL_FIELDS = new Set(['module', 'action', 'scope', 'name', 'description']);
const WILDCARD_FIELDS = new Set(['kind', ...NORMAL_FIELDS]);
const ROOT_FIELDS = new Set(['schema_version', 'permissions']);

function isRecord(value) {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function assertKnownFields(value, allowed, location, label = 'field') {
	for (const field of Object.keys(value)) {
		if (!allowed.has(field)) {
			throw new Error(`${location}: unknown ${label} "${field}"`);
		}
	}
}

function requiredString(value, field, location) {
	if (!(field in value)) {
		throw new Error(`${location}: missing field "${field}"`);
	}
	if (typeof value[field] !== 'string') {
		throw new Error(`${location}.${field}: must be a string`);
	}
	if (value[field].length === 0) {
		throw new Error(`${location}.${field}: must not be empty`);
	}
	return value[field];
}

function permissionConstant(moduleName, action, scope) {
	return `${moduleName}_${action}_${scope}`.toUpperCase();
}

function rustStringLiteral(value) {
	let escaped = '';
	for (const character of value) {
		switch (character) {
			case '\\':
				escaped += '\\\\';
				break;
			case '"':
				escaped += '\\"';
				break;
			case '\n':
				escaped += '\\n';
				break;
			case '\r':
				escaped += '\\r';
				break;
			case '\t':
				escaped += '\\t';
				break;
			default: {
				const codePoint = character.codePointAt(0);
				escaped += codePoint < 0x20 || codePoint === 0x7f
					? `\\u{${codePoint.toString(16)}}`
					: character;
			}
		}
	}
	return `"${escaped}"`;
}

function typeScriptStringLiteral(value) {
	return `'${value
		.replaceAll('\\', '\\\\')
		.replaceAll("'", "\\'")
		.replaceAll('\n', '\\n')
		.replaceAll('\r', '\\r')
		.replaceAll('\t', '\\t')}'`;
}

function normalizePermission(value, index) {
	const location = `permissions[${index}]`;
	if (!isRecord(value)) {
		throw new Error(`${location}: must be an object`);
	}
	const wildcard = value.kind === 'wildcard';
	assertKnownFields(value, wildcard ? WILDCARD_FIELDS : NORMAL_FIELDS, location);

	if ('kind' in value && !wildcard) {
		throw new Error(`${location}.kind: only "wildcard" is supported`);
	}

	const moduleName = requiredString(value, 'module', location);
	const action = requiredString(value, 'action', location);
	const scope = requiredString(value, 'scope', location);
	const name = requiredString(value, 'name', location);
	const description = requiredString(value, 'description', location);

	for (const [field, component] of [
		['module', moduleName],
		['action', action],
		['scope', scope]
	]) {
		if (!COMPONENT_PATTERN.test(component)) {
			throw new Error(`${location}.${field}: must use canonical lowercase snake_case`);
		}
	}

	if (!ALLOWED_ACTIONS.has(action)) {
		throw new Error(`${location}.action: unsupported action "${action}"`);
	}
	if (!ALLOWED_SCOPES.has(scope)) {
		throw new Error(`${location}.scope: unsupported scope "${scope}"`);
	}

	if (wildcard && (moduleName !== 'system' || action !== 'all' || scope !== 'global')) {
		throw new Error(`${location}: wildcard must use system/all/global`);
	}

	return {
		kind: wildcard ? 'wildcard' : 'permission',
		constant: wildcard ? 'WILDCARD' : permissionConstant(moduleName, action, scope),
		code: wildcard ? '*' : `${moduleName}.${action}.${scope}`,
		module: moduleName,
		action,
		scope,
		name,
		description
	};
}

export function validateAndNormalizeContract(value) {
	if (!isRecord(value)) {
		throw new Error('contract: must be an object');
	}
	assertKnownFields(value, ROOT_FIELDS, 'contract', 'root field');
	if (!('schema_version' in value)) {
		throw new Error('contract: missing field "schema_version"');
	}
	if (value.schema_version !== 1) {
		throw new Error(`contract: unsupported schema_version "${value.schema_version}"`);
	}
	if (!Array.isArray(value.permissions) || value.permissions.length === 0) {
		throw new Error('contract.permissions: must be a non-empty array');
	}

	const permissions = value.permissions.map(normalizePermission);
	const wildcardPermissions = permissions.filter(({ kind }) => kind === 'wildcard');
	if (wildcardPermissions.length !== 1) {
		throw new Error(`contract.permissions: expected exactly one wildcard, found ${wildcardPermissions.length}`);
	}

	const seenTuples = new Set();
	const seenCodes = new Set();
	const seenConstants = new Set();
	for (const permission of permissions) {
		const tuple = `${permission.module}\0${permission.action}\0${permission.scope}`;
		if (seenTuples.has(tuple)) {
			throw new Error(`duplicate permission tuple ${permission.module}/${permission.action}/${permission.scope}`);
		}
		if (seenCodes.has(permission.code)) {
			throw new Error(`duplicate permission code ${permission.code}`);
		}
		if (seenConstants.has(permission.constant)) {
			throw new Error(`duplicate permission constant ${permission.constant}`);
		}
		seenTuples.add(tuple);
		seenCodes.add(permission.code);
		seenConstants.add(permission.constant);
	}

	const wildcard = wildcardPermissions[0];
	const normalPermissions = permissions
		.filter(({ kind }) => kind === 'permission')
		.sort((left, right) => left.constant.localeCompare(right.constant, 'en'));
	const modules = [...new Set(permissions.map(({ module }) => module))].sort((left, right) =>
		left.localeCompare(right, 'en')
	);

	return {
		schema_version: 1,
		permissions: [wildcard, ...normalPermissions],
		modules
	};
}

function renderRust(normalized, digest) {
	const constants = normalized.permissions
		.map(({ constant, code }) => {
			const value = rustStringLiteral(code);
			const line = `    pub const ${constant}: &str = ${value};`;
			return line.length > 100 ? `    pub const ${constant}: &str =\n        ${value};` : line;
		})
		.join('\n');
	const definitions = normalized.permissions
		.map(
			(permission) => `    PermissionDef {
        code: codes::${permission.constant},
        name: ${rustStringLiteral(permission.name)},
        module: ${rustStringLiteral(permission.module)},
        action: ${rustStringLiteral(permission.action)},
        scope: ${rustStringLiteral(permission.scope)},
        description: ${rustStringLiteral(permission.description)},
    },`
		)
		.join('\n');

	return `// @generated by scripts/generate-permissions.mjs; DO NOT EDIT.
// contract-sha256: ${digest}

pub mod codes {
${constants}
}

pub const ALL_PERMISSIONS: &[PermissionDef] = &[
${definitions}
];
`;
}

function renderTypeScriptObjectEntry(key, value, index, entries) {
	const suffix = index === entries.length - 1 ? '' : ',';
	const literal = typeScriptStringLiteral(value);
	const line = `\t${key}: ${literal}${suffix}`;
	const visualWidth = line.replace(/^\t/, '  ').length;
	return visualWidth > 100 ? `\t${key}:\n\t\t${literal}${suffix}` : line;
}

function renderTypeScript(normalized, digest) {
	const modules = normalized.modules
		.map((moduleName) => [moduleName.toUpperCase(), moduleName])
		.map(([key, value], index, entries) => renderTypeScriptObjectEntry(key, value, index, entries))
		.join('\n');
	const permissions = normalized.permissions
		.filter(({ kind }) => kind === 'permission')
		.map(({ constant, code }) => [constant, code])
		.map(([key, value], index, entries) => renderTypeScriptObjectEntry(key, value, index, entries))
		.join('\n');

	return `// @generated by scripts/generate-permissions.mjs; DO NOT EDIT.
// contract-sha256: ${digest}

export const WILDCARD_PERMISSION = '*' as const;

export const PERMISSION_MODULES = {
${modules}
} as const;

export const PERMISSIONS = {
${permissions}
} as const;

export type PermissionCode = (typeof PERMISSIONS)[keyof typeof PERMISSIONS];
export type PermissionModule = (typeof PERMISSION_MODULES)[keyof typeof PERMISSION_MODULES];
export type RoutePermission = PermissionCode | PermissionModule;
`;
}

export function renderPermissionArtifacts(normalized) {
	const digest = createHash('sha256').update(JSON.stringify(normalized)).digest('hex');
	const permissionCodes = normalized.permissions.map(({ code }) => code).sort();
	const lockContent = `${JSON.stringify(
		{
			schema_version: 1,
			contract_sha256: digest,
			permission_codes: permissionCodes
		},
		null,
		2
	)}\n`;
	return {
		digest,
		lockContent,
		rustContent: renderRust(normalized, digest),
		typeScriptContent: renderTypeScript(normalized, digest)
	};
}

async function readOptional(filePath) {
	try {
		return await readFile(filePath, 'utf8');
	} catch (error) {
		if (error?.code === 'ENOENT') return undefined;
		throw error;
	}
}

function parseLock(content, lockPath) {
	let value;
	try {
		value = JSON.parse(content);
	} catch (error) {
		throw new Error(`${lockPath}: invalid JSON: ${error.message}`);
	}
	if (
		!isRecord(value) ||
		value.schema_version !== 1 ||
		!Array.isArray(value.permission_codes) ||
		!value.permission_codes.every((code) => typeof code === 'string')
	) {
		throw new Error(`${lockPath}: invalid permission lock`);
	}
	return value;
}

export async function generatePermissions({
	contractPath,
	lockPath,
	rustOutputPath,
	typeScriptOutputPath,
	check = false,
	initializeLock = false
}) {
	if (check && initializeLock) {
		throw new Error('--check and --initialize-lock cannot be combined');
	}

	let source;
	try {
		source = JSON.parse(await readFile(contractPath, 'utf8'));
	} catch (error) {
		throw new Error(`${contractPath}: unable to read permission contract: ${error.message}`);
	}
	const normalized = validateAndNormalizeContract(source);
	const artifacts = renderPermissionArtifacts(normalized);
	const existingLockContent = await readOptional(lockPath);

	if (existingLockContent === undefined && !initializeLock) {
		throw new Error(`${lockPath}: lock is missing; use --initialize-lock only for the initial baseline`);
	}
	if (existingLockContent !== undefined && initializeLock) {
		throw new Error(`${lockPath}: lock already exists; --initialize-lock is not allowed`);
	}
	if (existingLockContent !== undefined) {
		const existingLock = parseLock(existingLockContent, lockPath);
		const currentCodes = new Set(normalized.permissions.map(({ code }) => code));
		const removedCodes = existingLock.permission_codes.filter((code) => !currentCodes.has(code));
		if (removedCodes.length > 0) {
			throw new Error(`refusing to remove permission codes: ${removedCodes.sort().join(', ')}`);
		}
	}

	const expectedFiles = [
		[lockPath, artifacts.lockContent],
		[rustOutputPath, artifacts.rustContent],
		[typeScriptOutputPath, artifacts.typeScriptContent]
	];
	const existingFiles = await Promise.all(expectedFiles.map(([filePath]) => readOptional(filePath)));
	const changedPaths = expectedFiles
		.filter(([, expected], index) => existingFiles[index] !== expected)
		.map(([filePath]) => filePath);

	if (check) {
		if (changedPaths.length > 0) {
			throw new Error(`stale generated files: ${changedPaths.join(', ')}`);
		}
	} else {
		await Promise.all(
			expectedFiles.map(async ([filePath, expected], index) => {
				if (existingFiles[index] !== expected) await writeFile(filePath, expected, 'utf8');
			})
		);
	}

	return {
		changedPaths,
		digest: artifacts.digest,
		permissionCount: normalized.permissions.length
	};
}

function cliOptions(arguments_) {
	const supported = new Set(['--check', '--initialize-lock']);
	for (const argument of arguments_) {
		if (!supported.has(argument)) throw new Error(`unknown argument: ${argument}`);
	}
	if (new Set(arguments_).size !== arguments_.length) {
		throw new Error('duplicate command-line argument');
	}
	return {
		check: arguments_.includes('--check'),
		initializeLock: arguments_.includes('--initialize-lock')
	};
}

async function main() {
	const options = cliOptions(process.argv.slice(2));
	const scriptPath = fileURLToPath(import.meta.url);
	const repositoryRoot = path.resolve(path.dirname(scriptPath), '..');
	const result = await generatePermissions({
		contractPath: path.join(repositoryRoot, 'contracts/permissions.json'),
		lockPath: path.join(repositoryRoot, 'contracts/permissions.lock.json'),
		rustOutputPath: path.join(
			repositoryRoot,
			'backend-school/src/permissions/registry_generated.rs'
		),
		typeScriptOutputPath: path.join(
			repositoryRoot,
			'frontend-school/src/lib/permissions/registry.generated.ts'
		),
		...options
	});
	const action = options.check ? 'verified' : 'generated';
	console.log(`${action} ${result.permissionCount} permissions (${result.digest})`);
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : undefined;
if (invokedPath === fileURLToPath(import.meta.url)) {
	main().catch((error) => {
		console.error(error.message);
		process.exitCode = 1;
	});
}
