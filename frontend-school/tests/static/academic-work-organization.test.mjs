import assert from 'node:assert/strict';
import { access, readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), '../..');

async function readProjectFile(relativePath) {
	return readFile(join(projectRoot, relativePath), 'utf8');
}

function menuGroup(source) {
	return source.match(/menu:\s*\{[\s\S]*?group:\s*'([^']+)'/)?.[1] ?? null;
}

test('staff academic services declare the approved school-work sections', async () => {
	const expected = new Map([
		['catalog/subject-groups', 'academic_curriculum'],
		['catalog/subjects', 'academic_curriculum'],
		['curricula', 'academic_curriculum'],
		['core', 'academic_delivery'],
		['delivery', 'academic_delivery'],
		['timetable/today', 'academic_delivery'],
		['timetable', 'academic_delivery'],
		['homerooms', 'academic_registry'],
		['student-years', 'academic_registry'],
		['assessments', 'academic_assessment'],
		['question-bank', 'academic_assessment'],
		['exam-schedules', 'academic_assessment'],
		['catalog/activities', 'academic_activities'],
		['supervision', 'academic_supervision'],
		['admission', 'academic_admission']
	]);

	for (const [route, expectedGroup] of expected) {
		const source = await readProjectFile(`src/routes/(app)/staff/academic/${route}/+page.ts`);
		assert.equal(menuGroup(source), expectedGroup, route);
		assert.match(source, /workspace:\s*'academic'/, route);
	}

	const students = await readProjectFile('src/routes/(app)/staff/students/+page.ts');
	assert.equal(menuGroup(students), 'academic_registry');
	const personalTimetable = await readProjectFile('src/routes/(app)/staff/timetable/+page.ts');
	const personalExams = await readProjectFile('src/routes/(app)/staff/exams/+page.ts');
	assert.equal(menuGroup(personalTimetable), 'main');
	assert.equal(menuGroup(personalExams), 'main');
});

test('period editing has one owner under academic core', async () => {
	const periodsMeta = await readProjectFile('src/routes/(app)/staff/academic/periods/+page.ts');
	const editor = await readProjectFile(
		'src/lib/components/academic-core/AcademicYearTermEditor.svelte'
	);

	assert.match(periodsMeta, /access:\s*\{/);
	assert.doesNotMatch(periodsMeta, /menu:\s*\{/);
	assert.match(periodsMeta, /redirect\(308,\s*'\/staff\/academic\/core#bell-schedules'\)/);
	assert.match(editor, /id="bell-schedules"/);
	await assert.rejects(
		access(join(projectRoot, 'src/routes/(app)/staff/academic/periods/+page.svelte'))
	);
});
