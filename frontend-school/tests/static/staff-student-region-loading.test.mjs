import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');
for (const route of ['staff/students', 'staff/students/[id]', 'staff/students/[id]/edit']) {
	test(`${route} owns selected-year primary reads and supersedes stale results`, async () => {
		const loader = await read(`src/routes/(app)/${route}/+page.ts`);
		const page = await read(`src/routes/(app)/${route}/+page.svelte`);
		assert.match(loader, /requestFetch: fetch/);
		assert.match(loader, /school:app-identity/);
		assert.match(loader, /STUDENT_READ_ASSIGNED/);
		assert.match(loader, /url.searchParams.get\('academicYearId'\)/);
		assert.doesNotMatch(page, /onMount\(/);
		assert.match(page, /\$effect.pre/);
		assert.match(page, /LatestRequest/);
	});
}
test('student directory navigation context includes filters and paging', async () => {
	const loader = await read('src/routes/(app)/staff/students/+page.ts');
	for (const key of ['search', 'status', 'page'])
		assert.ok(loader.includes(`url.searchParams.get('${key}')`));
});
