import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const routesRoot = path.resolve(import.meta.dirname, '../../src/routes/(app)/staff/academic');

const cases = [
	{
		route: 'core',
		read: 'getAcademicSetupWorkspace',
		result: 'workspace',
		dependency: 'ACADEMIC_SETUP_WORKSPACE_DEPENDENCY',
		lazy: 'listBellSchedulePeriods'
	},
	{
		route: 'curricula',
		read: 'getCurriculumOverview',
		result: 'overview',
		dependency: 'CURRICULUM_OVERVIEW_DEPENDENCY',
		lazy: 'getCurriculumCreateOptions'
	}
];

test('academic setup and curriculum list own first-visible reads in their route loaders', async () => {
	for (const entry of cases) {
		const directory = path.join(routesRoot, entry.route);
		const loader = await readFile(path.join(directory, '+page.ts'), 'utf8');
		const page = await readFile(path.join(directory, '+page.svelte'), 'utf8');
		assert.match(loader, new RegExp(`\\b${entry.read}\\b`), entry.route);
		assert.match(loader, /captureRouteLoad\(/, entry.route);
		assert.match(loader, /requestFetch:\s*fetch/, entry.route);
		assert.match(loader, new RegExp(`depends\\(${entry.dependency}\\)`), entry.route);
		assert.match(loader, new RegExp(`${entry.result}\\s*:`), entry.route);
		assert.doesNotMatch(page, new RegExp(`\\b${entry.read}\\s*\\(`), entry.route);
		assert.doesNotMatch(page, /\bonMount\s*\(/, entry.route);
		if (entry.route === 'core') assert.match(page, /\blistBellSchedulePeriods\b/);
		else {
			const dialog = await readFile(
				path.resolve(
					import.meta.dirname,
					'../../src/lib/components/academic-core/CurriculumCreateDialog.svelte'
				),
				'utf8'
			);
			assert.match(dialog, /showDialog\(\)[^]*?getCurriculumCreateOptions\(/);
		}
	}
});

test('loaded empty regions remain visible with focused retry during failed refresh', async () => {
	const setup = await readFile(path.join(routesRoot, 'core/+page.svelte'), 'utf8');
	const curricula = await readFile(path.join(routesRoot, 'curricula/+page.svelte'), 'utf8');
	assert.match(setup, /loading\s*&&\s*!workspaceReady/);
	assert.match(setup, /errorMessage\s*&&\s*!workspaceReady/);
	assert.match(setup, /RegionUpdatingState/);
	assert.match(setup, /role="alert"[^]*?onclick=\{loadWorkspace\}[^]*?ลองอีกครั้ง/);
	assert.match(curricula, /loading\s*&&\s*!overview/);
	assert.match(curricula, /errorMessage\s*&&\s*!overview/);
	assert.match(curricula, /RegionUpdatingState/);
	assert.match(curricula, /role="alert"[^]*?onclick=\{loadOverview\}[^]*?ลองอีกครั้ง/);
});

test('older setup workspace results cannot overwrite a completed local mutation', async () => {
	const setup = await readFile(path.join(routesRoot, 'core/+page.svelte'), 'utf8');
	assert.match(setup, /const initialMutationRevision = mutationRevision/);
	assert.match(setup, /mutationRevision === initialMutationRevision/);
	assert.match(setup, /mutationRevision \+= 1/);
});
