import assert from 'node:assert/strict';
import { access, readFile } from 'node:fs/promises';
import { test } from 'node:test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const readRepo = (file) => readFile(path.join(repoRoot, file), 'utf8');
const exists = async (file) => access(path.join(repoRoot, file)).then(() => true, () => false);

test('auto scheduler frontend and generated contract are absent', async () => {
	for (const removed of [
		'frontend-school/src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/timetable/scheduling-config/+page.ts',
		'frontend-school/src/routes/(app)/staff/academic/timetable/scheduling/jobs/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/timetable/scheduling/jobs/[jobId]/+page.svelte'
	]) {
		assert.equal(await exists(removed), false, removed);
	}

	const [api, timetable, templates, generated, contractText] = await Promise.all([
		readRepo('frontend-school/src/lib/api/scheduling.ts'),
		readRepo('frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte'),
		readRepo('frontend-school/src/routes/(app)/staff/academic/timetable/templates/+page.svelte'),
		readRepo('frontend-school/src/lib/api/generated/school-api.ts'),
		readRepo('contracts/openapi/school-api.json')
	]);
	const combined = `${api}\n${timetable}\n${templates}\n${generated}`;
	for (const removed of [
		'autoScheduleTimetable',
		'SchedulingJobResponse',
		'undoSchedulingJob',
		'/scheduling/jobs',
		'/scheduling/configuration',
		'/scheduling-config',
		'InstructorConstraintView',
		'SaveSchedulingConfigurationRequest',
		'จัดอัตโนมัติ'
	]) {
		assert.doesNotMatch(combined, new RegExp(removed.replaceAll('/', '\\/')));
	}

	const contract = JSON.parse(contractText);
	const operationIds = Object.values(contract.paths).flatMap((item) =>
		Object.values(item).flatMap((operation) => operation.operationId ?? [])
	);
	assert.equal(operationIds.length, 177);
	assert.equal(new Set(operationIds).size, 177);
	assert.equal(Object.keys(contract.paths).some((route) => route.includes('/scheduling/')), false);
});
