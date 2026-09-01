import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const readProjectFile = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

const timetableCodes = [
	'academic_timetable.read.assigned',
	'academic_timetable.read.organization_unit',
	'academic_timetable.read.organization_tree',
	'academic_timetable.read.school',
	'academic_timetable.manage.assigned',
	'academic_timetable.manage.organization_unit',
	'academic_timetable.manage.organization_tree',
	'academic_timetable.manage.school',
	'academic_timetable.publish.school'
];

test('timetable owns a generated permission boundary independent from Delivery', async () => {
	const permissionContract = JSON.parse(await readProjectFile('../contracts/permissions.json'));
	const generatedRegistry = await readProjectFile('src/lib/permissions/registry.generated.ts');
	const policy = await readProjectFile('../backend-school/src/policies/timetable_access_policy.rs');
	const policyModules = await readProjectFile('../backend-school/src/policies.rs');
	const route = await readProjectFile('src/routes/(app)/staff/academic/timetable/+page.ts');
	const page = await readProjectFile('src/routes/(app)/staff/academic/timetable/+page.svelte');
	const handlers = await readProjectFile('../backend-school/src/modules/academic/handlers/timetable.rs');

	const contractCodes = permissionContract.permissions.map(
		(permission) => `${permission.module}.${permission.action}.${permission.scope}`
	);
	for (const code of timetableCodes) {
		assert.ok(contractCodes.includes(code), `${code} must be defined by the permission contract`);
		assert.match(generatedRegistry, new RegExp(code.replaceAll('.', '\\.')));
	}

	assert.match(policyModules, /pub mod timetable_access_policy;/);
	assert.match(policy, /enum TimetableAction[\s\S]*Read[\s\S]*Manage[\s\S]*Publish/);
	assert.match(policy, /struct TimetableResourceSet/);
	assert.match(policy, /require_timetable_resources/);
	assert.match(route, /permission:\s*PERMISSION_MODULES\.ACADEMIC_TIMETABLE/);
	assert.doesNotMatch(route, /PERMISSION_MODULES\.LEARNING_OFFERING/);
	assert.doesNotMatch(page, /PERMISSIONS\.LEARNING_OFFERING_(?:READ|MANAGE)_/);
	assert.doesNotMatch(handlers, /codes::LEARNING_OFFERING_(?:READ|MANAGE)_/);
});
