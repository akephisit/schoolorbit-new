import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const root = new URL('../../../', import.meta.url);
const read = (path) => readFile(new URL(path, root), 'utf8');

for (const section of ['overview', 'cycles', 'templates']) {
	test(`supervision ${section} delegates primary regions to its route loader`, async () => {
		const base = `frontend-school/src/routes/(app)/staff/academic/supervision/${section}`;
		const [loader, page] = await Promise.all([
			read(`${base}/+page.ts`),
			read(`${base}/+page.svelte`)
		]);
		assert.match(loader, /captureRouteLoad/);
		assert.match(loader, /waitForSupervisionAccess/);
		assert.match(loader, /requestFetch: fetch/);
		assert.doesNotMatch(loader, /loadSupervisionManagementRegions/);
		assert.match(loader, new RegExp(`section: '${section}'`));
		assert.match(loader, /fetch, url/);
		assert.match(page, /routeData=\{data\}/);
	});
}

test('supervision management regions retain data, retry independently, and supersede old context', async () => {
	const workspace = await read(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);
	assert.match(workspace, /\$effect\.pre/);
	assert.match(workspace, /cyclesLoaded/);
	assert.match(workspace, /templatesLoaded/);
	assert.match(workspace, /teacherStatusLoaded/);
	assert.match(workspace, /RegionUpdatingState/);
	assert.match(workspace, /if \(routeData\) return/);
	assert.match(workspace, /retryManagementCycles/);
	assert.match(workspace, /retryManagementTemplates/);
	assert.match(workspace, /getSupervisionTemplate\(/);
});

test('cycle target hydration is bounded to the selected context and template summaries omit rubrics', async () => {
	const [cycles, templates, models] = await Promise.all([
		read('backend-school/crates/school-supervision/src/services/cycles.rs'),
		read('backend-school/crates/school-supervision/src/services/templates.rs'),
		read('backend-school/crates/school-supervision/src/models.rs')
	]);
	const targets = cycles.slice(cycles.indexOf('async fn load_cycle_targets_by_cycle'));
	assert.match(targets, /WHERE cycle_id = ANY\(\$1\)/);
	assert.match(templates, /pub async fn list_template_summaries/);
	const summary = models.slice(models.indexOf('pub struct SupervisionTemplateSummary'));
	assert.match(summary, /section_count/);
	assert.match(summary, /item_count/);
});
