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
	const results = {
		previewBulkLearningGroupSetup: { targets: [], changedCount: 1 },
		applyBulkLearningGroupSetup: { groups: [] }
	};
	class ApiClientError extends Error {
		constructor(message, status) {
			super(message);
			this.status = status;
		}
	}
	const send = async (method, path, body, options) => {
		calls.push({ method, path, body, options });
		const resourcePath = path.replace('/offering-id/', '/{id}/');
		const operation = contract.paths[resourcePath]?.[method.toLowerCase()];
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
				post: (...args) => send('POST', ...args),
				put: (...args) => send('PUT', ...args)
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
		deliveryVersionId: 'version',
		deliveryVersionRowVersion: 1,
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
	assert.equal(calls[0].options, options);
});

test('unregistered POST to the bulk apply path is refused instead of serving a preview', async () => {
	const { send } = transportHarness();
	const response = await send('POST', '/api/academic/offerings/offering-id/groups/bulk-setup', {});
	assert.equal(response.status, 405);
	assert.equal(response.success, false);
});
