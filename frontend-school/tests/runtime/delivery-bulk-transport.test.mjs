import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const contract = JSON.parse(
	await readFile(new URL('../../../contracts/openapi/school-api.json', import.meta.url), 'utf8')
);
const source = await readFile(
	new URL('../../src/lib/api/learning-delivery.ts', import.meta.url),
	'utf8'
);
const compiled = ts.transpileModule(source, {
	compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
}).outputText;

function transportHarness() {
	const calls = [];
	const impact = Object.fromEntries(
		Object.keys(contract.components.schemas.OfferingDeletionImpact.properties).map((key) => [
			key,
			0
		])
	);
	const results = {
		previewBulkLearningGroupSetup: {
			targets: [],
			changedCount: 0,
			blockedCount: 0,
			skippedCount: 0,
			deliveryRowVersion: 1
		},
		applyBulkLearningGroupSetup: { groups: [], rosterTracking: [], deliveryRowVersion: 2 },
		previewLearningOfferingDeletion: {
			academicYearId: 'year-id',
			academicTermId: 'term-id',
			learningOfferingId: 'target-offering',
			learningGroupId: null,
			rowVersion: 7,
			sourceDigest: 'a'.repeat(64),
			impact: { ...impact }
		},
		previewLearningGroupDeletion: {
			academicYearId: 'year-id',
			academicTermId: 'term-id',
			learningOfferingId: 'target-offering',
			learningGroupId: 'target-group',
			rowVersion: 8,
			sourceDigest: 'b'.repeat(64),
			impact: { ...impact, learningGroups: 1 }
		},
		deleteLearningOffering: {
			academicYearId: 'year-id',
			academicTermId: 'term-id',
			learningOfferingId: 'target-offering',
			learningGroupId: null,
			deleted: true,
			deliveryRowVersion: 9
		},
		deleteLearningGroup: {
			academicYearId: 'year-id',
			academicTermId: 'term-id',
			learningOfferingId: 'target-offering',
			learningGroupId: 'target-group',
			deleted: true,
			deliveryRowVersion: 10
		}
	};
	class ApiClientError extends Error {
		constructor(message, status) {
			super(message);
			this.status = status;
		}
	}
	const send = async (method, path, body, options) => {
		const url = new URL(path, 'https://transport.invalid');
		const resourcePath = url.pathname.replace(
			/\/(offering-id|target-offering|target-group)(?=\/|$)/g,
			'/{id}'
		);
		const operation = contract.paths[resourcePath]?.[method.toLowerCase()];
		calls.push({
			method,
			path,
			body,
			options,
			resourcePath,
			query: options?.query,
			operationId: operation?.operationId
		});
		assert.equal(
			url.search,
			'',
			'query context belongs to API options separately from the registered path'
		);
		return operation
			? { success: true, status: 200, data: results[operation.operationId] }
			: { success: false, status: 405, error: `Method not registered: ${method} ${resourcePath}` };
	};
	const exports = {};
	new Function('require', 'exports', compiled)((specifier) => {
		assert.equal(specifier, '#lib/api/client.js');
		return {
			ApiClientError,
			apiClient: {
				get: (path, options) => send('GET', path, undefined, options),
				post: (...args) => send('POST', ...args),
				put: (...args) => send('PUT', ...args),
				deleteWithBody: (...args) => send('DELETE', ...args)
			},
			requireApiData(response, fallback) {
				if (!response.success)
					throw new ApiClientError(response.error ?? fallback, response.status);
				return response.data;
			}
		};
	}, exports);
	return { api: exports, calls, results, send };
}

test('bulk preview and apply wrappers use the separately registered OpenAPI operations', async () => {
	const { api, calls, results } = transportHarness();
	const request = {
		deliveryRowVersion: 1,
		groups: [{ learningGroupId: 'group', rowVersion: 1 }],
		primaryTeacher: { teacherId: 'teacher', replaceExistingPrimary: false }
	};
	const options = { signal: new AbortController().signal };
	assert.equal(
		await api.previewBulkGroupSetup('offering-id', request, options),
		results.previewBulkLearningGroupSetup
	);
	assert.equal(
		await api.applyBulkGroupSetup('offering-id', request),
		results.applyBulkLearningGroupSetup
	);
	assert.deepEqual(
		calls.map(({ method, path }) => [method, path]),
		[
			['POST', '/api/academic/offerings/offering-id/groups/bulk-setup/preview'],
			['PUT', '/api/academic/offerings/offering-id/groups/bulk-setup']
		]
	);
	assert.equal(calls[0].body, request);
	assert.equal(calls[1].body, request);
	assert.deepEqual(request, {
		deliveryRowVersion: 1,
		groups: [{ learningGroupId: 'group', rowVersion: 1 }],
		primaryTeacher: { teacherId: 'teacher', replaceExistingPrimary: false }
	});
	assert.deepEqual(
		calls.map((call) => call.operationId),
		['previewBulkLearningGroupSetup', 'applyBulkLearningGroupSetup']
	);
	assert.ok(
		Object.keys(request).every((key) =>
			Object.hasOwn(contract.components.schemas.BulkGroupSetupRequest.properties, key)
		)
	);
	assert.equal(calls[0].options, options);
});

test('unregistered POST to the bulk apply path is refused instead of serving a preview', async () => {
	const { send } = transportHarness();
	const response = await send('POST', '/api/academic/offerings/offering-id/groups/bulk-setup', {});
	assert.equal(response.status, 405);
	assert.equal(response.success, false);
});

test('current resource previews and deletion use registered operations and exact accepted context, revisions and digests', async () => {
	const { api, calls, results } = transportHarness();
	const context = { academicYearId: 'year-id', academicTermId: 'term-id' };
	const options = { signal: new AbortController().signal };
	const offering = await api.previewOfferingDeletion('target-offering', context, options);
	const group = await api.previewGroupDeletion('target-group', context, options);
	assert.equal(offering, results.previewLearningOfferingDeletion);
	assert.equal(group, results.previewLearningGroupDeletion);
	const offeringBody = {
		...context,
		rowVersion: offering.rowVersion,
		sourceDigest: offering.sourceDigest
	};
	const groupBody = { ...context, rowVersion: group.rowVersion, sourceDigest: group.sourceDigest };
	assert.equal(
		await api.deleteLearningOffering('target-offering', offeringBody),
		results.deleteLearningOffering
	);
	assert.equal(
		await api.deleteLearningGroup('target-group', groupBody),
		results.deleteLearningGroup
	);
	assert.deepEqual(
		calls.map(({ method, path, operationId }) => [method, path, operationId]),
		[
			[
				'GET',
				'/api/academic/offerings/target-offering/deletion-preview',
				'previewLearningOfferingDeletion'
			],
			[
				'GET',
				'/api/academic/learning-groups/target-group/deletion-preview',
				'previewLearningGroupDeletion'
			],
			['DELETE', '/api/academic/offerings/target-offering', 'deleteLearningOffering'],
			['DELETE', '/api/academic/learning-groups/target-group', 'deleteLearningGroup']
		]
	);
	for (const call of calls.slice(0, 2)) {
		assert.equal(call.body, undefined);
		assert.equal(call.query, context);
		assert.equal(call.options.signal, options.signal);
		assert.deepEqual(
			contract.paths[call.resourcePath].get.parameters
				.filter((parameter) => parameter.in === 'query')
				.map((parameter) => parameter.name)
				.sort(),
			Object.keys(context).sort()
		);
	}
	assert.equal(calls[2].body, offeringBody);
	assert.equal(calls[3].body, groupBody);
	assert.deepEqual(offeringBody, { ...context, rowVersion: 7, sourceDigest: 'a'.repeat(64) });
	assert.deepEqual(groupBody, { ...context, rowVersion: 8, sourceDigest: 'b'.repeat(64) });
	for (const body of [offeringBody, groupBody]) {
		assert.deepEqual(
			Object.keys(body).sort(),
			[...contract.components.schemas.DeleteDeliveryResourceRequest.required].sort()
		);
	}
});
