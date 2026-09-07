import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const projectRoot = path.resolve(import.meta.dirname, '../..');

async function read(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

let wrapperRevision = 0;

async function importWrapper(relativePath) {
	globalThis.__academicContractCalls = [];
	const clientModule = `
		export const apiClient = new Proxy({}, {
			get(_target, method) {
				return (endpoint, bodyOrOptions, options) => {
					globalThis.__academicContractCalls.push({ method, endpoint, bodyOrOptions, options });
					return Promise.resolve({ success: true, data: {}, status: 200 });
				};
			}
		});
		export function requireApiData(response) { return response.data; }
	`;
	const clientUrl = `data:text/javascript;base64,${Buffer.from(clientModule).toString('base64')}`;
	const source = (await read(relativePath)).replace(
		/(['"])\$lib\/api\/client\1/g,
		`'${clientUrl}'`
	);
	const compiled = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
		fileName: relativePath
	}).outputText;
	wrapperRevision += 1;
	return import(
		`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}#${wrapperRevision}`
	);
}

test('gradebook, learner-evaluation, and result operations own canonical context and envelopes', async () => {
	const contract = JSON.parse(await read('../contracts/openapi/school-api.json'));
	const expected = [
		['/api/academic/gradebook/subjects', 'get', 'listGradebookSubjects'],
		['/api/academic/gradebook/controls', 'get', 'listGradebookControls'],
		['/api/academic/gradebook/controls/{control_id}', 'put', 'updateGradebookControl'],
		[
			'/api/academic/gradebook/groups/{group_id}/phases/{phase_code}',
			'get',
			'getGradebookGroupPhaseWorkspace'
		],
		[
			'/api/academic/gradebook/groups/{group_id}/phases/{phase_code}/items',
			'post',
			'createGradebookItem'
		],
		[
			'/api/academic/gradebook/groups/{group_id}/phases/{phase_code}/items/{item_id}',
			'put',
			'updateGradebookItem'
		],
		[
			'/api/academic/gradebook/groups/{group_id}/phases/{phase_code}/items/{item_id}',
			'delete',
			'removeGradebookItem'
		],
		[
			'/api/academic/gradebook/groups/{group_id}/phases/{phase_code}/scores',
			'put',
			'saveGradebookScoresBatch'
		],
		[
			'/api/academic/gradebook/groups/{group_id}/phases/{phase_code}/confirm',
			'post',
			'confirmGradebookPhase'
		],
		[
			'/api/academic/learner-evaluations/groups/{group_id}/domains/{domain}',
			'get',
			'getLearnerEvaluationWorkspace'
		],
		[
			'/api/academic/learner-evaluations/groups/{group_id}/domains/{domain}/responses',
			'put',
			'saveLearnerEvaluationResponses'
		],
		[
			'/api/academic/learner-evaluations/groups/{group_id}/domains/{domain}/confirm',
			'post',
			'confirmLearnerEvaluationGroup'
		],
		[
			'/api/academic/learner-evaluations/subjects/{subject_id}/domains/{domain}/lock',
			'post',
			'lockLearnerEvaluationSubject'
		],
		['/api/academic/results/groups/{group_id}/course', 'get', 'getCourseResultPreparation'],
		[
			'/api/academic/results/groups/{group_id}/course/selection',
			'put',
			'saveCourseResultSelection'
		],
		['/api/academic/results/groups/{group_id}/course/confirm', 'post', 'confirmCourseGroupResults'],
		['/api/academic/results/groups/{group_id}/activity', 'get', 'getActivityResultPreparation'],
		[
			'/api/academic/results/groups/{group_id}/activity/outcomes',
			'put',
			'saveActivityResultOutcomes'
		],
		[
			'/api/academic/results/groups/{group_id}/activity/confirm',
			'post',
			'confirmActivityGroupResults'
		],
		['/api/academic/results/readiness', 'get', 'getAcademicResultReadiness'],
		['/api/academic/results/effective', 'get', 'searchEffectiveAcademicResults'],
		['/api/academic/results/corrections', 'post', 'correctEffectiveAcademicResult']
	];

	for (const [route, method, operationId] of expected) {
		const operation = contract.paths?.[route]?.[method];
		assert.equal(operation?.operationId, operationId, `${method} ${route}`);
		const query = new Map(
			(operation.parameters ?? [])
				.filter((parameter) => parameter.in === 'query')
				.map((parameter) => [parameter.name, parameter.required])
		);
		assert.equal(query.get('academicYearId'), true, `${operationId} academicYearId`);
		assert.equal(query.get('academicTermId'), true, `${operationId} academicTermId`);
		assert.match(
			operation.responses?.['200']?.content?.['application/json']?.schema?.$ref ?? '',
			/^#\/components\/schemas\/ApiResponse_/
		);
	}

	const schemas = contract.components.schemas;
	assert.equal(schemas.AssessmentPhaseControl.properties.scoreEntryEnabled, undefined);
	assert.equal(schemas.CourseGradingPolicy, undefined);
	assert.equal(schemas.ActivityResult, undefined);
});

test('academic result wrappers are generated-contract consumers only', async () => {
	for (const relativePath of [
		'src/lib/api/academicGradebook.ts',
		'src/lib/api/academicLearnerEvaluations.ts',
		'src/lib/api/academicResults.ts'
	]) {
		const source = await read(relativePath);
		assert.match(source, /import type \{ components, operations \}/);
		assert.match(source, /components\['schemas'\]/);
		assert.match(source, /operations\['/);
		assert.match(source, /satisfies\s+\w+(?:Query|Path|Body)/);
		assert.doesNotMatch(source, /\b(?:unknown|Record<string, unknown>)\b/);
		assert.doesNotMatch(source, /\bacademic_year_id\b|\bacademic_term_id\b/);
		assert.doesNotMatch(source, /export\s+interface\s+/);
	}
});

test('academic result wrappers forward canonical context through reads and mutations', async () => {
	const context = { academicYearId: 'year-1', academicTermId: 'term-1' };
	const gradebook = await importWrapper('src/lib/api/academicGradebook.ts');
	await gradebook.listGradebookSubjects(context);
	assert.deepEqual(globalThis.__academicContractCalls.pop(), {
		method: 'get',
		endpoint: '/api/academic/gradebook/subjects',
		bodyOrOptions: { query: context },
		options: undefined
	});
	await gradebook.updateGradebookControl('control/1', context, {
		scoreEntryEnabled: true,
		rowVersion: 1
	});
	assert.deepEqual(globalThis.__academicContractCalls.pop(), {
		method: 'put',
		endpoint: '/api/academic/gradebook/controls/control%2F1',
		bodyOrOptions: { scoreEntryEnabled: true, rowVersion: 1 },
		options: { query: context }
	});

	const evaluations = await importWrapper('src/lib/api/academicLearnerEvaluations.ts');
	await evaluations.removeSubjectEvaluationCriterion(
		'subject/1',
		'desirable_characteristic',
		'criterion/1',
		context,
		{ rowVersion: 2 }
	);
	assert.deepEqual(globalThis.__academicContractCalls.pop(), {
		method: 'deleteWithBody',
		endpoint:
			'/api/academic/learner-evaluations/subjects/subject%2F1/domains/desirable_characteristic/criteria/criterion%2F1',
		bodyOrOptions: { rowVersion: 2 },
		options: { query: context }
	});

	const results = await importWrapper('src/lib/api/academicResults.ts');
	const correction = {
		kind: 'activity',
		activityResultId: 'result-1',
		outcome: 'pass',
		expectedEffectiveVersion: 1
	};
	await results.correctEffectiveAcademicResult(context, correction);
	assert.deepEqual(globalThis.__academicContractCalls.pop(), {
		method: 'post',
		endpoint: '/api/academic/results/corrections',
		bodyOrOptions: correction,
		options: { query: context }
	});
});
