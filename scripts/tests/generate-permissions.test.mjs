import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

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

const generatorPath = fileURLToPath(new URL('../generate-permissions.mjs', import.meta.url));

async function temporaryPaths(t, contract = validContract) {
	const root = await mkdtemp(path.join(tmpdir(), 'schoolorbit-permission-generator-'));
	t.after(() => rm(root, { recursive: true, force: true }));
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

test('derives canonical constants, codes, and modules', () => {
	const normalized = validateAndNormalizeContract(validContract);
	assert.deepEqual(
		normalized.permissions.map(({ constant, code }) => [constant, code]),
		[
			['WILDCARD', '*'],
			['STAFF_PROFILE_READ_OWN', 'staff_profile.read.own'],
			['STAFF_READ_ALL', 'staff.read.all']
		]
	);
	assert.deepEqual(normalized.modules, ['staff', 'staff_profile', 'system']);
});

test('renders deterministic Rust, TypeScript, and lock artifacts', () => {
	const normalized = validateAndNormalizeContract(validContract);
	const first = renderPermissionArtifacts(normalized);
	const second = renderPermissionArtifacts(normalized);
	assert.deepEqual(first, second);
	assert.match(first.rustContent, /pub const STAFF_READ_ALL: &str = "staff\.read\.all";/);
	assert.match(first.rustContent, /pub const ALL_PERMISSIONS: &\[PermissionDef\]/);
	assert.match(first.typeScriptContent, /STAFF_READ_ALL: 'staff\.read\.all'/);
	assert.match(first.typeScriptContent, /export type PermissionCode/);
	assert.deepEqual(JSON.parse(first.lockContent).permission_codes, [
		'*',
		'staff.read.all',
		'staff_profile.read.own'
	]);
});

test('renders long constants in rustfmt and Prettier compatible form', () => {
	const contract = structuredClone(validContract);
	contract.permissions.push({
		module: 'academic_question_bank',
		action: 'manage',
		scope: 'organization_unit',
		name: 'จัดการคลังข้อสอบในกลุ่มสาระ',
		description: 'สร้างและแก้ไขข้อสอบของรายวิชาในกลุ่มสาระเดียวกัน'
	});
	const artifacts = renderPermissionArtifacts(validateAndNormalizeContract(contract));
	assert.match(
		artifacts.rustContent,
		/pub const ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT: &str =\n        "academic_question_bank\.manage\.organization_unit";/
	);
	assert.match(
		artifacts.typeScriptContent,
		/\tACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT:\n\t\t'academic_question_bank\.manage\.organization_unit',/
	);
	assert.doesNotMatch(artifacts.typeScriptContent, /SYSTEM: 'system',\n} as const/);
	assert.doesNotMatch(artifacts.typeScriptContent, /STAFF_READ_ALL: 'staff\.read\.all',\n} as const/);
});

for (const [label, mutate, message] of [
	['duplicate tuple', (value) => value.permissions.push({ ...value.permissions[1] }), /duplicate/],
	['invalid module', (value) => (value.permissions[1].module = 'Staff'), /snake_case/],
	['unsupported action', (value) => (value.permissions[1].action = 'download'), /unsupported action/],
	['unsupported scope', (value) => (value.permissions[1].scope = 'department'), /unsupported scope/],
	['invalid wildcard', (value) => (value.permissions[0].scope = 'school'), /wildcard/],
	['unknown field', (value) => (value.permissions[1].code = 'staff.read.all'), /unknown field/],
	['missing name', (value) => delete value.permissions[1].name, /missing field "name"/],
	['empty description', (value) => (value.permissions[1].description = ''), /must not be empty/]
]) {
	test(`rejects ${label}`, () => {
		const value = structuredClone(validContract);
		mutate(value);
		assert.throws(() => validateAndNormalizeContract(value), message);
	});
}

test('rejects unsupported schema versions and invalid root fields', () => {
	assert.throws(
		() => validateAndNormalizeContract({ ...validContract, schema_version: 2 }),
		/unsupported schema_version/
	);
	assert.throws(
		() => validateAndNormalizeContract({ ...validContract, extra: true }),
		/unknown root field/
	);
});

test('escapes Rust literals without changing ordinary UTF-8 text', () => {
	const contract = structuredClone(validContract);
	contract.permissions[1].name = 'ดู "บุคลากร"\\ทั้งหมด\nบรรทัดใหม่';
	const { rustContent } = renderPermissionArtifacts(validateAndNormalizeContract(contract));
	assert.match(rustContent, /name: "ดู \\"บุคลากร\\"\\\\ทั้งหมด\\nบรรทัดใหม่"/);
});

test('initializes a missing lock exactly once', async (t) => {
	const paths = await temporaryPaths(t);
	await assert.rejects(generatePermissions(paths), /--initialize-lock/);
	const result = await generatePermissions({ ...paths, initializeLock: true });
	assert.equal(result.permissionCount, 3);
	assert.equal(result.changedPaths.length, 3);
	await assert.rejects(
		generatePermissions({ ...paths, initializeLock: true }),
		/already exists/
	);
});

test('allows additions and metadata changes without allowing removals', async (t) => {
	const paths = await temporaryPaths(t);
	await generatePermissions({ ...paths, initializeLock: true });

	const expanded = structuredClone(validContract);
	expanded.permissions[1].description = 'คำอธิบายใหม่';
	expanded.permissions.push({
		module: 'staff',
		action: 'update',
		scope: 'all',
		name: 'แก้ไขบุคลากร',
		description: 'แก้ไขข้อมูลบุคลากร'
	});
	await writeFile(paths.contractPath, `${JSON.stringify(expanded, null, 2)}\n`, 'utf8');
	await generatePermissions(paths);

	const reduced = structuredClone(expanded);
	reduced.permissions.splice(1, 1);
	await writeFile(paths.contractPath, `${JSON.stringify(reduced, null, 2)}\n`, 'utf8');
	await assert.rejects(generatePermissions(paths), /refusing to remove.*staff\.read\.all/s);
});

test('check mode reports but never rewrites stale output', async (t) => {
	const paths = await temporaryPaths(t);
	await generatePermissions({ ...paths, initializeLock: true });
	const clean = await generatePermissions({ ...paths, check: true });
	assert.deepEqual(clean.changedPaths, []);

	const edited = `${await readFile(paths.typeScriptOutputPath, 'utf8')}// manual edit\n`;
	await writeFile(paths.typeScriptOutputPath, edited, 'utf8');
	await assert.rejects(generatePermissions({ ...paths, check: true }), /stale generated file/);
	assert.equal(await readFile(paths.typeScriptOutputPath, 'utf8'), edited);
});

test('validation failure leaves every generated output unchanged', async (t) => {
	const paths = await temporaryPaths(t);
	await generatePermissions({ ...paths, initializeLock: true });
	const before = await outputContents(paths);

	const invalid = structuredClone(validContract);
	invalid.permissions[1].scope = 'department';
	await writeFile(paths.contractPath, `${JSON.stringify(invalid, null, 2)}\n`, 'utf8');
	await assert.rejects(generatePermissions(paths), /unsupported scope/);
	assert.deepEqual(await outputContents(paths), before);
});

test('rejects incompatible generation modes before writing', async (t) => {
	const paths = await temporaryPaths(t);
	await assert.rejects(
		generatePermissions({ ...paths, check: true, initializeLock: true }),
		/cannot be combined/
	);
	await assert.rejects(readFile(paths.lockPath), /ENOENT/);
});

test('CLI rejects unknown flags before reading repository files', () => {
	const result = spawnSync(process.execPath, [generatorPath, '--unknown'], {
		cwd: tmpdir(),
		encoding: 'utf8'
	});
	assert.notEqual(result.status, 0);
	assert.match(result.stderr, /unknown argument: --unknown/);
	assert.doesNotMatch(result.stderr, /permission contract/);
});
