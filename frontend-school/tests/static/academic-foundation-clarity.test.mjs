import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const read = (path) => readFileSync(new URL(`../../${path}`, import.meta.url), 'utf8');

test('academic core is a four-step planning path with focused shadcn controls', () => {
	const orchestrator = read('src/lib/components/academic-core/AcademicYearTermEditor.svelte');
	for (const component of [
		'AcademicYearSetupStep',
		'BellScheduleSetupStep',
		'BellSchedulePeriodsStep',
		'AcademicTermSetupStep'
	]) {
		assert.match(orchestrator, new RegExp(component));
	}
	assert.match(orchestrator, /ฉบับเตรียมการ/);
	assert.match(orchestrator, /การเปิดใช้.*การปิด.*การเลื่อนชั้น/s);

	const yearStep = read('src/lib/components/academic-core/setup/AcademicYearSetupStep.svelte');
	assert.match(yearStep, /ui\/checkbox/);
	assert.match(yearStep, /ui\/collapsible/);
	assert.match(yearStep, /standardAcademicYearName/);
	assert.match(yearStep, /startDate\s*>\s*endDate/);
	assert.doesNotMatch(yearStep, /<input[^>]+type=["']checkbox["']/);

	const periodStep = read('src/lib/components/academic-core/setup/BellSchedulePeriodsStep.svelte');
	assert.match(periodStep, /ui\/checkbox/);
	assert.match(periodStep, /ใช้ทุกวันเรียน/);
	assert.doesNotMatch(periodStep, /MON,TUE|split\(['"]?,['"]?\)/);

	const termStep = read('src/lib/components/academic-core/setup/AcademicTermSetupStep.svelte');
	assert.match(termStep, /ui\/collapsible/);
	assert.match(termStep, /standardTermName/);
	assert.match(termStep, /editing\?\.sequence\s*\?\?\s*nextSequence/);
	assert.match(termStep, /startDate\s*>\s*endDate/);
	assert.doesNotMatch(termStep, /term-(code|sequence)/);
});

test('homeroom and student-year registries own only human-readable fields', () => {
	const homeroomEditor = read('src/lib/components/academic-core/HomeroomEditor.svelte');
	assert.doesNotMatch(homeroomEditor, /homeroom-code|bind:value=\{draft\.code\}/);
	assert.doesNotMatch(homeroomEditor, /bind:value=\{advisorDraft\.role\}/);

	const studentPage = read('src/routes/(app)/staff/academic/student-years/+page.svelte');
	assert.match(studentPage, /listStudentYearCandidates/);
	assert.match(studentPage, /Dialog\.Root/);
	assert.match(studentPage, /Table\.Root/);
	assert.doesNotMatch(
		studentPage,
		/\?\?\s*(?:record\.)?(?:studentId|gradeLevelId|studyProgramId|homeroomId)/
	);
});

test('candidate lookup is lazy and generated API contracts remain authoritative', () => {
	const studentPage = read('src/routes/(app)/staff/academic/student-years/+page.svelte');
	const openDialog = studentPage.indexOf('openCreateDialog');
	const candidateCall = studentPage.indexOf('listStudentYearCandidates');
	assert.ok(openDialog >= 0, 'create dialog opener must exist');
	assert.ok(candidateCall >= 0, 'candidate API must be used');
	assert.doesNotMatch(studentPage, /onMount[\s\S]{0,900}listStudentYearCandidates/);

	const api = read('src/lib/api/academic-core.ts');
	assert.match(api, /operations\['listStudentYearCandidates'\]/);
	assert.match(api, /academicYearId:\s*requiredContextValue/);
});
