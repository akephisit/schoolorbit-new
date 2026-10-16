import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import * as generated from '../../src/lib/permissions/registry.generated.ts';

async function load(sourcePath, dependencies) {
	const source = await readFile(new URL(sourcePath, import.meta.url), 'utf8');
	const compiled = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
	}).outputText;
	const exports = {};
	new Function('require', 'exports', compiled)((id) => {
		assert.ok(id in dependencies, id);
		return dependencies[id];
	}, exports);
	return exports;
}
const registry = await load('../../src/lib/permissions/registry.ts', {
	'./registry.generated.js': generated,
	'./registry.generated': generated
});
const { filterOrganizationPermissions: filter } = await load(
	'../../src/lib/features/staff/organization-permission-search.ts',
	{ '#lib/permissions/registry.js': registry }
);
const modules = {
	staff_profile: [
		{
			id: 'own',
			code: 'staff_profile.read.own',
			name: 'ดูข้อมูลตนเอง',
			description: 'เปิดประวัติของผู้ใช้',
			action: 'read',
			scope: 'own'
		},
		{
			id: 'school',
			code: 'staff_profile.update.school',
			name: 'แก้ไขข้อมูล',
			description: null,
			action: 'update',
			scope: 'school'
		}
	],
	academic_assessment: [
		{
			id: 'assessment',
			code: 'academic_assessment.manage.assigned',
			name: 'จัดการโครงสร้างคะแนน',
			description: null,
			action: 'manage',
			scope: 'assigned'
		}
	]
};
test('every contracted module has a Thai label and an unknown module keeps its identity', () => {
	for (const module of Object.values(generated.PERMISSION_MODULES))
		assert.match(registry.permissionModuleLabel(module), /[ก-๙]/, module);
	assert.equal(registry.permissionModuleLabel('future_module'), 'future_module');
});
test('search combines Thai module, name, description, action, scope and technical code terms', () => {
	assert.deepEqual(
		filter(modules, 'ข้อมูลบุคลากร ทั้งโรงเรียน').staff_profile.map((p) => p.id),
		['school']
	);
	assert.deepEqual(
		filter(modules, 'การวัดผล').academic_assessment.map((p) => p.id),
		['assessment']
	);
	assert.deepEqual(
		filter(modules, 'เปิดประวัติ').staff_profile.map((p) => p.id),
		['own']
	);
	assert.deepEqual(
		filter(modules, 'STAFF_PROFILE.UPDATE').staff_profile.map((p) => p.id),
		['school']
	);
	assert.deepEqual(
		filter(modules, 'แก้ไข ทั้งโรงเรียน').staff_profile.map((p) => p.id),
		['school']
	);
});
test('empty and unmatched searches preserve the catalog and do not mutate hidden permissions', () => {
	const before = structuredClone(modules);
	assert.deepEqual(filter(modules, '  '), modules);
	assert.deepEqual(filter(modules, 'ไม่ตรงกับสิทธิ์ใด'), {});
	filter(modules, 'ตนเอง');
	assert.deepEqual(modules, before);
});
