import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const routesRoot = new URL('../src/routes/api/migration/', import.meta.url);
const cases = [
	{ route: 'status', method: 'GET', endpoint: 'migration-status' },
	{ route: 'migrate-all', method: 'POST', endpoint: 'migrate-all' }
] as const;

async function handlerFor(
	route: string,
	method: string,
	backendUrl: string,
	internalSecret: string
): Promise<() => Promise<Response>> {
	const source = await readFile(new URL(`${route}/+server.ts`, routesRoot), 'utf8');
	const envSource = `export const BACKEND_SCHOOL_URL = ${JSON.stringify(backendUrl)};
export const INTERNAL_API_SECRET = ${JSON.stringify(internalSecret)};`;
	const envUrl = `data:text/javascript;base64,${Buffer.from(envSource).toString('base64')}`;
	const compiled = ts
		.transpileModule(source, {
			compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ESNext }
		})
		.outputText.replace(/(["'])\$app\/env\/private\1/g, () => JSON.stringify(envUrl));
	const module = (await import(
		`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
	)) as Record<string, () => Promise<Response>>;
	const handler = module[method];
	assert.equal(typeof handler, 'function');
	return handler;
}

for (const { route, method, endpoint } of cases) {
	for (const missing of ['BACKEND_SCHOOL_URL', 'INTERNAL_API_SECRET'] as const) {
		test(`${route} refuses a missing ${missing} before contacting the backend`, async () => {
			const handler = await handlerFor(
				route,
				method,
				missing === 'BACKEND_SCHOOL_URL' ? '' : 'https://school-api.example.test',
				missing === 'INTERNAL_API_SECRET' ? '' : 'synthetic-fixture-secret'
			);
			const originalFetch = globalThis.fetch;
			let requests = 0;
			globalThis.fetch = async () => {
				requests++;
				throw new Error('A missing configuration must not contact the backend');
			};
			try {
				const response = await handler();
				assert.equal(response.status, 503);
				assert.deepEqual(await response.json(), {
					error: 'Migration service not configured',
					details: `${missing} environment variable is missing`
				});
				assert.equal(requests, 0);
			} finally {
				globalThis.fetch = originalFetch;
			}
		});
	}

	test(`${route} forwards the configured request and preserves the backend response`, async () => {
		const handler = await handlerFor(
			route,
			method,
			'https://school-api.example.test',
			'synthetic-fixture-secret'
		);
		const originalFetch = globalThis.fetch;
		const requests: { url: string; init?: RequestInit }[] = [];
		const payload = { success: true, data: { schools: [] } };
		globalThis.fetch = async (input, init) => {
			requests.push({ url: String(input), init });
			return Response.json(payload, { status: 207 });
		};
		try {
			const response = await handler();
			assert.equal(response.status, 207);
			assert.deepEqual(await response.json(), payload);
			assert.equal(requests.length, 1);
			const request = requests[0];
			assert.ok(request);
			assert.equal(request.url, `https://school-api.example.test/internal/${endpoint}`);
			assert.equal(request.init?.method ?? 'GET', method);
			assert.equal(
				new Headers(request.init?.headers).get('X-Internal-Secret'),
				'synthetic-fixture-secret'
			);
		} finally {
			globalThis.fetch = originalFetch;
		}
	});
}
