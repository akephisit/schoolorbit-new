import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, stat, utimes, writeFile } from 'node:fs/promises';
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
const permissionContractPath = fileURLToPath(
	new URL('../../contracts/permissions.json', import.meta.url)
);
const permissionSchemaPath = fileURLToPath(
	new URL('../../contracts/permissions.schema.json', import.meta.url)
);

function validatePermissionAction(action) {
	const contract = structuredClone(validContract);
	contract.permissions.push({
		module: 'academic_result',
		action,
		scope: 'school',
		name: `Academic result ${action}`,
		description: `Academic result ${action} capability`
	});

	try {
		validateAndNormalizeContract(contract);
		return true;
	} catch {
		return false;
	}
}

test('lifecycle transitions have distinct canonical capabilities', () => {
	for (const action of ['close', 'reopen', 'activate']) {
		assert.equal(validatePermissionAction(action), true, `${action} must be grantable independently`);
	}
	assert.equal(validatePermissionAction('force_close'), false);
});

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

test('accepts certificate workflow actions as canonical permission actions', () => {
	const contract = structuredClone(validContract);
	for (const action of ['submit', 'issue', 'revoke', 'download']) {
		contract.permissions.push({
			module: 'certificate',
			action,
			scope: action === 'download' ? 'organization_unit' : 'school',
			name: `Certificate ${action}`,
			description: `Certificate ${action} capability`
		});
	}

	const normalized = validateAndNormalizeContract(contract);
	assert.deepEqual(
		normalized.permissions
			.filter(({ module }) => module === 'certificate')
			.map(({ code }) => code),
		[
			'certificate.download.organization_unit',
			'certificate.issue.school',
			'certificate.revoke.school',
			'certificate.submit.school'
		]
	);
});

test('accepts lock and correct as canonical permission actions', async () => {
	assert.equal(validatePermissionAction('lock'), true);
	assert.equal(validatePermissionAction('correct'), true);

	const schema = JSON.parse(await readFile(permissionSchemaPath, 'utf8'));
	assert.equal(schema.$defs.action.enum.includes('lock'), true);
	assert.equal(schema.$defs.action.enum.includes('correct'), true);
});

test('generates the exact approved gradebook and result permission codes', async () => {
	const permissionContract = JSON.parse(await readFile(permissionContractPath, 'utf8'));
	const generatedCodes = validateAndNormalizeContract(permissionContract).permissions.map(
		({ code }) => code
	);

	assert.deepEqual(
		generatedCodes.filter((code) => code.startsWith('academic_gradebook.')),
		[
			'academic_gradebook.manage.assigned',
			'academic_gradebook.manage.school',
			'academic_gradebook.read.assigned',
			'academic_gradebook.read.organization_unit',
			'academic_gradebook.read.school'
		]
	);
	assert.deepEqual(
		generatedCodes.filter((code) => code.startsWith('academic_result.')),
		[
			'academic_result.correct.school',
			'academic_result.lock.school',
			'academic_result.manage.assigned',
			'academic_result.manage.school',
			'academic_result.read.assigned',
			'academic_result.read.organization_unit',
			'academic_result.read.school'
		]
	);
	assert.deepEqual(
		generatedCodes.filter((code) => code.startsWith('academic_learner_evaluation.')),
		[
			'academic_learner_evaluation.correct.school',
			'academic_learner_evaluation.lock.school',
			'academic_learner_evaluation.manage.assigned',
			'academic_learner_evaluation.manage.school',
			'academic_learner_evaluation.read.assigned',
			'academic_learner_evaluation.read.organization_unit',
			'academic_learner_evaluation.read.school'
		]
	);
});

for (const [label, mutate, message] of [
	['duplicate tuple', (value) => value.permissions.push({ ...value.permissions[1] }), /duplicate/],
	['invalid module', (value) => (value.permissions[1].module = 'Staff'), /snake_case/],
	['unsupported action', (value) => (value.permissions[1].action = 'archive'), /unsupported action/],
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

test('allows an explicitly authorized contract cutover to remove permissions', async (t) => {
	const paths = await temporaryPaths(t);
	await generatePermissions({ ...paths, initializeLock: true });

	const reduced = structuredClone(validContract);
	reduced.permissions.splice(1, 1);
	await writeFile(paths.contractPath, `${JSON.stringify(reduced, null, 2)}\n`, 'utf8');

	const result = await generatePermissions({ ...paths, allowRemovals: true });
	const lock = JSON.parse(await readFile(paths.lockPath, 'utf8'));

	assert.equal(result.permissionCount, 2);
	assert.ok(!lock.permission_codes.includes('staff.read.all'));
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

function workflowManagementRows(source) {
	return [...source.matchAll(/code: "([a-z0-9_.]+)",\s*scope: "([a-z_]+)"/g)]
		.map(([, code, scope]) => [code, scope]);
}

test('Workflow management projection matches the existing canonical action and scope filter', async () => {
	const contract = JSON.parse(await readFile(permissionContractPath, 'utf8'));
	const normalized = validateAndNormalizeContract(contract);
	const expected = normalized.permissions.filter(({ action, scope }) =>
		action === 'manage' && ['school', 'all', 'organization_unit', 'organization_tree'].includes(scope)
	).map(({ code, scope }) => [code, scope]);
	const { workflowManagementRustContent } = renderPermissionArtifacts(normalized);
	assert.deepEqual(workflowManagementRows(workflowManagementRustContent), expected);
});

test('Workflow management projection leaves unrelated codes and metadata outside its output', async (t) => {
	const contract = structuredClone(validContract);
	const scopes = ['school', 'all', 'organization_unit', 'organization_tree', 'own', 'assigned', 'global'];
	for (const scope of scopes) {
		contract.permissions.push({
			module: 'workflow_fixture',
			action: 'manage',
			scope,
			name: `Manage ${scope}`,
			description: `Manage fixture for ${scope}`
		});
	}
	const paths = await temporaryPaths(t, contract);
	const workflowManagementOutputPath = path.join(
		path.dirname(paths.rustOutputPath), 'workflow_management_permissions_generated.rs'
	);
	const options = { ...paths, workflowManagementOutputPath };
	await generatePermissions({ ...options, initializeLock: true });
	const before = await readFile(workflowManagementOutputPath, 'utf8');
	assert.deepEqual(workflowManagementRows(before), [
		['workflow_fixture.manage.all', 'all'],
		['workflow_fixture.manage.organization_tree', 'organization_tree'],
		['workflow_fixture.manage.organization_unit', 'organization_unit'],
		['workflow_fixture.manage.school', 'school']
	]);
	assert.doesNotMatch(before, /contract-sha256|description:|name:/);
	const oldTimestamp = new Date('2000-01-01T00:00:00Z');
	await utimes(workflowManagementOutputPath, oldTimestamp, oldTimestamp);
	const originalMtime = (await stat(workflowManagementOutputPath)).mtimeMs;

	const eligible = contract.permissions.find(({ module, action, scope }) =>
		module === 'workflow_fixture' && action === 'manage' && scope === 'school'
	);
	eligible.description = 'Updated school management description';
	contract.permissions.push({
		module: 'workflow_fixture',
		action: 'read',
		scope: 'school',
		name: 'Read fixtures',
		description: 'Read fixtures for school'
	});
	await writeFile(paths.contractPath, JSON.stringify(contract));
	const result = await generatePermissions(options);
	assert.deepEqual(result.changedPaths, [
		paths.lockPath, paths.rustOutputPath, paths.typeScriptOutputPath
	]);
	assert.equal(await readFile(workflowManagementOutputPath, 'utf8'), before);
	assert.equal((await stat(workflowManagementOutputPath)).mtimeMs, originalMtime);

	contract.permissions.push({
		module: 'new_workflow_fixture',
		action: 'manage',
		scope: 'school',
		name: 'Manage new fixture',
		description: 'Manage a new fixture at school scope'
	});
	await writeFile(paths.contractPath, JSON.stringify(contract));
	await assert.rejects(
		generatePermissions({ ...options, check: true }),
		/stale generated files:.*workflow_management_permissions_generated\.rs/
	);
	const update = await generatePermissions(options);
	assert.ok(update.changedPaths.includes(workflowManagementOutputPath));
	assert.match(await readFile(workflowManagementOutputPath, 'utf8'), /new_workflow_fixture\.manage\.school/);
});

test('check reports missing and stale Workflow metadata without writing it', async (t) => {
	const paths = await temporaryPaths(t);
	const workflowManagementOutputPath = path.join(
		path.dirname(paths.rustOutputPath), 'workflow_management_permissions_generated.rs'
	);
	const options = { ...paths, workflowManagementOutputPath };
	await generatePermissions({ ...paths, initializeLock: true });
	const expected = { message: `stale generated files: ${workflowManagementOutputPath}` };
	await assert.rejects(generatePermissions({ ...options, check: true }), expected);
	await assert.rejects(readFile(workflowManagementOutputPath), /ENOENT/);
	await writeFile(workflowManagementOutputPath, '// stale\n');
	await assert.rejects(generatePermissions({ ...options, check: true }), expected);
	assert.equal(await readFile(workflowManagementOutputPath, 'utf8'), '// stale\n');
	assert.deepEqual((await generatePermissions(options)).changedPaths, [workflowManagementOutputPath]);
	assert.deepEqual((await generatePermissions({ ...options, check: true })).changedPaths, []);
});

async function scopedRustPaths(t) {
	const paths = await temporaryPaths(t);
	const backendRoot = path.join(path.dirname(paths.rustOutputPath), 'backend-school');
	const authorization = path.join(backendRoot, 'crates/school-authorization');
	const attendance = path.join(backendRoot, 'crates/school-attendance');
	for (const owner of [authorization, attendance]) {
		await mkdir(owner, { recursive: true });
		await writeFile(path.join(owner, 'Cargo.toml'), '[package]\n');
	}
	await mkdir(path.join(attendance, 'tests'));
	await writeFile(path.join(attendance, 'tests/score.rs'), '');
	return {
		paths,
		backendRoot,
		consumerSelections: {
			'school-authorization': {
				production: ['WILDCARD'],
				unitTests: ['WILDCARD', 'STAFF_READ_ALL']
			},
			'school-attendance': {
				production: ['STAFF_PROFILE_READ_OWN'],
				integrationTests: { score: ['STAFF_PROFILE_READ_OWN'] }
			}
		},
		authorizationCodes: path.join(authorization, 'src/permission_codes_generated.rs'),
		authorizationFixtures: path.join(authorization, 'src/test_support/permission_codes_generated.rs'),
		attendanceCodes: path.join(attendance, 'src/permission_codes_generated.rs'),
		attendanceScore: path.join(attendance, 'tests/score/permission_codes_generated.rs')
	};
}

test('keeps local Rust code outputs byte-stable for unrelated permission changes', async (t) => {
	const {
		paths, backendRoot, consumerSelections, authorizationCodes, authorizationFixtures,
		attendanceCodes, attendanceScore
	} = await scopedRustPaths(t);
	await generatePermissions({ ...paths, initializeLock: true });
	const globalOutputs = await outputContents(paths);
	const options = { ...paths, backendRoot, consumerSelections };
	const initial = await generatePermissions(options);
	const localOutputs = [attendanceCodes, attendanceScore, authorizationCodes, authorizationFixtures];
	assert.deepEqual(initial.changedPaths, localOutputs);
	assert.deepEqual(await outputContents(paths), globalOutputs);

	const wildcard = await readFile(authorizationCodes, 'utf8');
	assert.equal(
		wildcard,
		'// @generated by scripts/generate-permissions.mjs; DO NOT EDIT.\n\npub const WILDCARD: &str = "*";\n'
	);
	const fixtures = await readFile(authorizationFixtures, 'utf8');
	assert.match(fixtures, /pub use super::permission_codes_generated::WILDCARD;/);
	assert.match(fixtures, /pub const STAFF_READ_ALL: &str = "staff\.read\.all";/);
	assert.doesNotMatch(wildcard, /STAFF_READ_ALL/);
	assert.match(await readFile(attendanceCodes, 'utf8'), /STAFF_PROFILE_READ_OWN/);
	assert.equal(await readFile(attendanceScore, 'utf8'), await readFile(attendanceCodes, 'utf8'));
	const oldTimestamp = new Date('2000-01-01T00:00:00Z');
	const outputs = await Promise.all(localOutputs.map(async (file) => {
		await utimes(file, oldTimestamp, oldTimestamp);
		return [await readFile(file, 'utf8'), (await stat(file)).mtimeMs];
	}));

	const changed = structuredClone(validContract);
	changed.permissions.push({
		module: 'budget',
		action: 'read',
		scope: 'school',
		name: 'Read budget',
		description: 'Read school budget'
	});
	await writeFile(paths.contractPath, JSON.stringify(changed), 'utf8');
	const update = await generatePermissions(options);
	assert.deepEqual(update.changedPaths, [paths.lockPath, paths.rustOutputPath, paths.typeScriptOutputPath]);
	assert.deepEqual(await Promise.all(localOutputs.map(async (file) => [
		await readFile(file, 'utf8'), (await stat(file)).mtimeMs
	])), outputs);
	assert.deepEqual((await generatePermissions({ ...options, check: true })).changedPaths, []);
});

test('updates every consumer of a changed cross-domain Rust constant', async (t) => {
	const { paths, backendRoot, consumerSelections, authorizationCodes, attendanceCodes, attendanceScore } =
		await scopedRustPaths(t);
	await generatePermissions({ ...paths, backendRoot, consumerSelections, initializeLock: true });
	const changed = structuredClone(validContract);
	changed.permissions[2].scope = 'assigned';
	await writeFile(paths.contractPath, JSON.stringify(changed));
	await assert.rejects(
		generatePermissions({ ...paths, backendRoot, consumerSelections, allowRemovals: true }),
		/unknown permission constant STAFF_PROFILE_READ_OWN/
	);
	consumerSelections['school-attendance'].production = ['STAFF_PROFILE_READ_ASSIGNED'];
	consumerSelections['school-attendance'].integrationTests.score = ['STAFF_PROFILE_READ_ASSIGNED'];
	const result = await generatePermissions({ ...paths, backendRoot, consumerSelections, allowRemovals: true });
	assert.ok(!result.changedPaths.includes(authorizationCodes));
	assert.deepEqual(result.changedPaths.slice(-2), [attendanceCodes, attendanceScore]);
	assert.match(await readFile(attendanceCodes, 'utf8'), /STAFF_PROFILE_READ_ASSIGNED/);
	assert.equal(await readFile(attendanceScore, 'utf8'), await readFile(attendanceCodes, 'utf8'));
});

test('generates feature-only permissions outside the default Rust projection', async (t) => {
	const { paths, backendRoot, consumerSelections, authorizationCodes } = await scopedRustPaths(t);
	consumerSelections['school-authorization'].integrationTestSupport = ['STAFF_READ_ALL'];
	const featureCodes = path.join(
		path.dirname(authorizationCodes), 'test_support/integration/permission_codes_generated.rs'
	);
	const options = { ...paths, backendRoot, consumerSelections };
	await generatePermissions({ ...options, initializeLock: true });
	const defaultCodes = await readFile(authorizationCodes, 'utf8');
	assert.doesNotMatch(defaultCodes, /STAFF_READ_ALL/);
	assert.match(await readFile(featureCodes, 'utf8'), /STAFF_READ_ALL: &str = "staff\.read\.all";/);
	assert.deepEqual((await generatePermissions({ ...options, check: true })).changedPaths, []);

	delete consumerSelections['school-authorization'].integrationTestSupport;
	await assert.rejects(generatePermissions({ ...options, check: true }), {
		message: `orphaned generated files: ${featureCodes}`
	});
});

test('check detects stale or missing consumer outputs without rewriting them', async (t) => {
	const { paths, backendRoot, consumerSelections, authorizationCodes } = await scopedRustPaths(t);
	const options = { ...paths, backendRoot, consumerSelections };
	await generatePermissions({ ...options, initializeLock: true });
	const expectedError = { message: `stale generated files: ${authorizationCodes}` };
	await writeFile(authorizationCodes, '// stale\n', 'utf8');
	await assert.rejects(generatePermissions({ ...options, check: true }), expectedError);
	assert.equal(await readFile(authorizationCodes, 'utf8'), '// stale\n');

	await rm(authorizationCodes);
	await assert.rejects(generatePermissions({ ...options, check: true }), expectedError);
	await assert.rejects(readFile(authorizationCodes), /ENOENT/);
	assert.deepEqual((await generatePermissions(options)).changedPaths, [authorizationCodes]);
	assert.deepEqual((await generatePermissions({ ...options, check: true })).changedPaths, []);
});

test('rejects duplicate selections and stale owner targets before writing artifacts', async (t) => {
	const { paths, backendRoot, consumerSelections } = await scopedRustPaths(t);
	const options = { ...paths, backendRoot, consumerSelections, initializeLock: true };
	consumerSelections['school-authorization'].production.push('WILDCARD');
	await assert.rejects(generatePermissions(options), /duplicate permission constant/);
	consumerSelections['school-authorization'].production.pop();
	consumerSelections['school-attendance'].integrationTests.retired = ['WILDCARD'];
	await assert.rejects(generatePermissions(options), /missing permission consumer.*retired\.rs/);
	await assert.rejects(readFile(paths.lockPath), /ENOENT/);
});

test('check detects generated outputs orphaned by removed selections', async (t) => {
	const { paths, backendRoot, consumerSelections, authorizationCodes, attendanceScore } =
		await scopedRustPaths(t);
	const options = { ...paths, backendRoot, consumerSelections };
	await generatePermissions({ ...options, initializeLock: true });
	delete consumerSelections['school-authorization'].production;
	delete consumerSelections['school-attendance'].integrationTests;
	const orphans = [authorizationCodes, attendanceScore].sort();
	await assert.rejects(generatePermissions({ ...options, check: true }), {
		message: `orphaned generated files: ${orphans.join(', ')}`
	});
	assert.match(await readFile(authorizationCodes, 'utf8'), /WILDCARD/);
	assert.match(await readFile(attendanceScore, 'utf8'), /STAFF_PROFILE_READ_OWN/);
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

test('rejects removal authorization in check mode', async (t) => {
	const paths = await temporaryPaths(t);
	await assert.rejects(
		generatePermissions({ ...paths, check: true, allowRemovals: true }),
		/--check and --allow-removals cannot be combined/
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

test('admits explicit owner-scoped electronic signature capability', () => {
 const contract=structuredClone(validContract);
 contract.permissions.push({module:'correspondence',action:'sign',scope:'own',name:'ลงนาม',description:'Explicit owner electronic signature'});
 const normalized=validateAndNormalizeContract(contract);
 assert.ok(normalized.permissions.some(({code})=>code==='correspondence.sign.own'));
});
