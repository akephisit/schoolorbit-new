import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');

async function read(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

test('supervision wrapper uses generated operations and schemas as its wire owner', async () => {
	const [wrapper, generated] = await Promise.all([
		read('src/lib/api/supervision.ts'),
		read('src/lib/api/generated/school-api.ts')
	]);

	assert.match(wrapper, /type \{ components, operations \}/);
	assert.match(wrapper, /type Schemas = components\['schemas'\]/);
	assert.match(wrapper, /type SupervisionCycle = Schemas\['SupervisionCycle'\]/);
	assert.match(wrapper, /type SupervisionObservation = Schemas\['SupervisionObservation'\]/);
	assert.match(wrapper, /type SupervisionTemplate = Schemas\['SupervisionTemplate'\]/);
	assert.match(
		wrapper,
		/RequestSupervisionObservationRequest =[\s\S]*operations\['requestSupervisionObservation'\]/
	);
	assert.match(wrapper, /operations\['listSupervisionCycles'\]/);
	assert.match(wrapper, /operations\['listSupervisionObservations'\]/);
	assert.doesNotMatch(wrapper, /export\s+interface\s+(?:Supervision|CreateSupervision|Evaluator)/);
	assert.doesNotMatch(wrapper, /export\s+type\s+Supervision\w+\s*=\s*['"]|\bas any\b|unknown as/);

	for (const operation of [
		'listSupervisionCycles',
		'createSupervisionCycle',
		'listSupervisionTemplates',
		'listSupervisionObservations',
		'getSupervisionObservation',
		'getSupervisionObservationReview',
		'getSupervisionEvaluatorAvailability',
		'getSupervisionObservationTimetableOptions',
		'getSupervisionCycleProgress',
		'getSupervisionTeacherStatusOverview'
	]) {
		assert.match(generated, new RegExp(`\\b${operation}:`), `missing generated ${operation}`);
	}
});

test('all supervision reads accept and forward cancellable request options', async () => {
	const wrapper = await read('src/lib/api/supervision.ts');

	assert.match(wrapper, /type ApiRequestOptions/);
	for (const functionName of [
		'listSupervisionCycles',
		'listSupervisionTemplates',
		'getSupervisionTemplate',
		'listSupervisionObservations',
		'getSupervisionObservation',
		'getSupervisionObservationReview',
		'getSupervisionEvaluatorAvailability',
		'getSupervisionObservationTimetableOptions',
		'getSupervisionCycleProgress',
		'getSupervisionTeacherStatusOverview'
	]) {
		const start = wrapper.indexOf(`function ${functionName}`);
		assert.notEqual(start, -1, `missing ${functionName}`);
		const body = wrapper.slice(start, wrapper.indexOf('\n}', start) + 2);
		assert.match(body, /ApiRequestOptions\s*=\s*\{\}/, `${functionName} must accept options`);
		assert.match(
			body,
			/apiClient\.(?:get|post)<[^>]+>\([\s\S]*options/,
			`${functionName} must forward options`
		);
	}
});

test('supervision workspace owns and cleans up cancellable refreshes', async () => {
	const workspace = await read('src/lib/components/supervision/SupervisionWorkspace.svelte');

	assert.match(workspace, /LatestRequest/);
	assert.match(workspace, /isAbortError/);
	assert.match(workspace, /\.begin\(\)/);
	assert.match(workspace, /\.isCurrent\(revision\)/);
	assert.match(workspace, /\{ signal \}/);
	assert.match(workspace, /\.abort\(\)/);
	assert.match(workspace, /if \(isAbortError\(error\)\) return/);
	assert.doesNotMatch(workspace, /let refreshRevision = 0/);
});
