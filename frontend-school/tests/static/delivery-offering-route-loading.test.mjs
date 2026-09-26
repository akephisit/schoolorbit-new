import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const routeDirectory = path.resolve(
	import.meta.dirname,
	'../../src/routes/(app)/staff/academic/delivery/[offeringId]'
);
const rosterComponent = path.resolve(
	import.meta.dirname,
	'../../src/lib/components/learning-delivery/DatedRosterMemberships.svelte'
);

async function routeSource(filename) {
	return readFile(path.join(routeDirectory, filename), 'utf8');
}

test('offering, groups, versions, and selected group start as independent route regions', async () => {
	const [page, component, roster] = await Promise.all([
		routeSource('+page.ts'),
		routeSource('+page.svelte'),
		readFile(rosterComponent, 'utf8')
	]);
	assert.match(page, /getLearningOffering/);
	assert.match(page, /listLearningGroups/);
	assert.match(page, /listTimetableVersions/);
	assert.match(page, /getLearningGroup/);
	assert.match(page, /listDatedRosterMemberships/);
	assert.match(page, /groupId/);
	assert.match(page, /requestFetch:\s*fetch/);
	assert.match(component, /data\.offering/);
	assert.match(component, /data\.groups/);
	assert.match(component, /data\.selectedGroup/);
	assert.doesNotMatch(component, /\bonMount\s*\(/);
	assert.match(roster, /initialMemberships/);
	assert.doesNotMatch(roster, /\bonMount\s*\(/);
	assert.doesNotMatch(page, /getLearningDeliveryManagementOptions/);
});

test('heavy offering detail links preload on tap instead of hover', async () => {
	for (const [filename, expected] of [
		['HomeroomDeliveryWorkspace.svelte', 2],
		['OfferingOverviewTable.svelte', 2],
		['AcademicChangeSetPanel.svelte', 1]
	]) {
		const source = await readFile(
			path.resolve(import.meta.dirname, '../../src/lib/components/learning-delivery', filename),
			'utf8'
		);
		const links = [...source.matchAll(/href=\{`\/staff\/academic\/delivery\/\$\{[^}]+\}[^`]*`\}/g)];
		const tapLinks = [
			...source.matchAll(
				/href=\{`\/staff\/academic\/delivery\/\$\{[^}]+\}[^`]*`\}[\s\S]{0,200}data-sveltekit-preload-data="tap"/g
			)
		];
		assert.equal(links.length, expected, filename);
		assert.equal(tapLinks.length, expected, `${filename}: every detail link must use tap preload`);
	}
});
