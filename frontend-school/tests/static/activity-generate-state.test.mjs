import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

function extractFunction(source, name) {
	const asyncMarker = `async function ${name}(`;
	const syncMarker = `function ${name}(`;
	const asyncStart = source.indexOf(asyncMarker);
	const syncStart = source.indexOf(syncMarker);
	const start = asyncStart === -1 ? syncStart : asyncStart;
	assert.notEqual(start, -1, `${name} should exist`);

	const openBrace = source.indexOf('{', start);
	assert.notEqual(openBrace, -1, `${name} should have a body`);

	let depth = 0;
	for (let cursor = openBrace; cursor < source.length; cursor += 1) {
		const char = source[cursor];
		if (char === '{') depth += 1;
		if (char === '}') {
			depth -= 1;
			if (depth === 0) return source.slice(start, cursor + 1);
		}
	}

	assert.fail(`${name} body should close`);
}

test('activity generate patches returned slots and groups without reloading the full page data', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/activities/+page.svelte');
	const api = await readProjectFile('src/lib/api/academic.ts');
	const generated = await readProjectFile('src/lib/api/generated/school-api.ts');
	const handleGenerate = extractFunction(page, 'handleGenerate');
	const mergeGeneratedActivities = extractFunction(page, 'mergeGeneratedActivities');

	assert.match(page, /const canGenerateFromPlan = \$derived\(canManageActivity && canReadCurriculumForGeneration\)/);
	assert.match(handleGenerate, /if \(!canGenerateFromPlan\)/);
	assert.match(page, /\{#if canGenerateFromPlan\}[\s\S]{0,500}onclick=\{openGenerateDialog\}/);
	assert.doesNotMatch(
		handleGenerate,
		/\bloadData\s*\(/,
		'handleGenerate should patch generated slots/groups locally instead of loadData()'
	);
	assert.match(handleGenerate, /\bmergeGeneratedActivities\s*\(\s*res\s*\)/);
	assert.match(mergeGeneratedActivities, /\bslots\s*=\s*result\.slots/);
	assert.match(mergeGeneratedActivities, /\bgroups\s*=\s*result\.groups/);
	assert.match(mergeGeneratedActivities, /\bslotInstructorsMap\s*=/);
	assert.match(mergeGeneratedActivities, /\bslotClassroomAssignmentsMap\s*=/);
	assert.match(
		api,
		/export\s+type\s+GenerateActivitiesFromPlanResponse\s*=\s*Schemas\['GenerateActivitiesFromPlanOutcome'\]/
	);
	assert.match(
		generated,
		/GenerateActivitiesFromPlanOutcome:\s*\{[\s\S]*slots:\s*components\['schemas'\]\['ActivitySlot'\]\[\]/
	);
	assert.match(
		generated,
		/GenerateActivitiesFromPlanOutcome:\s*\{[\s\S]*groups:\s*components\['schemas'\]\['ActivityGroup'\]\[\]/
	);
	assert.match(
		generated,
		/GenerateActivitiesFromPlanOutcome:\s*\{[\s\S]*slot_instructors:\s*\{[\s\S]*components\['schemas'\]\['SlotInstructorInfo'\]\[\]/
	);
	assert.match(
		generated,
		/GenerateActivitiesFromPlanOutcome:\s*\{[\s\S]*slot_classroom_assignments:\s*\{[\s\S]*components\['schemas'\]\['SlotClassroomAssignment'\]\[\]/
	);
});
