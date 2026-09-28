import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const root = new URL('../../../', import.meta.url);
const read = (path) => readFile(new URL(path, root), 'utf8');
const routePath = (name, file) =>
	`frontend-school/src/routes/(app)/staff/academic/admission/[id]/${name}/${file}`;

for (const [name, permission, collection] of [
	['enrollment', 'ADMISSION_ENROLL_ALL', 'listEnrollmentPending'],
	['student-ids', 'ADMISSION_MANAGE_ALL', 'listStudentIds'],
	['report', 'ADMISSION_READ_ALL', 'listApplications']
]) {
	test(`${name} starts independent authorized round and collection reads in its route`, async () => {
		const [route, page] = await Promise.all([
			read(routePath(name, '+page.ts')),
			read(routePath(name, '+page.svelte'))
		]);
		assert.match(route, new RegExp(`waitForAdmissionAccess\\(PERMISSIONS\\.${permission}\\)`));
		assert.match(route, /getRound\(id, \{ requestFetch: fetch \}\)/);
		assert.match(route, new RegExp(`${collection}\\(id,[\\s\\S]*?requestFetch: fetch`));
		assert.match(route, /captureRouteLoad/);
		assert.doesNotMatch(route, /Promise\.all\(\[getRound/);
		assert.doesNotMatch(page, /onMount/);
		assert.match(page, /\$effect\.pre/);
		assert.match(page, /LatestRequest/);
		assert.match(page, /PageSkeleton/);
		assert.match(page, /PageState/);
		assert.doesNotMatch(page, /backHref="\/staff\/academic\/admission\/\{id\}"/);
	});
}
