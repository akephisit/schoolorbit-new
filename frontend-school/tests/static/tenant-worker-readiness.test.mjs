import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import path from 'node:path';
import test from 'node:test';
import { pathToFileURL } from 'node:url';

const repoRoot = path.resolve(import.meta.dirname, '../../..');
const verifierUrl = pathToFileURL(
	path.join(repoRoot, 'frontend-school/scripts/verify-tenant-worker-readiness.mjs')
).href;

const loadVerifier = async () => {
	try {
		return await import(verifierUrl);
	} catch (error) {
		assert.fail(`tenant Worker readiness verifier must be importable: ${error.message}`);
	}
};

const listen = async (handler) => {
	const server = createServer(handler);
	await new Promise((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', resolve);
	});
	const address = server.address();
	assert.ok(address && typeof address === 'object');
	return {
		origin: `http://127.0.0.1:${address.port}`,
		close: () => new Promise((resolve) => server.close(resolve))
	};
};

test('tenant readiness retries the complete document and asset condition after a transient 404', async (t) => {
	let stylesheetRequests = 0;
	const fixture = await listen((request, response) => {
		if (request.url === '/') {
			response.writeHead(200, { 'content-type': 'text/html' });
			response.end(
				'<link rel="stylesheet" href="/_app/immutable/assets/app.css"><script src="/_app/immutable/entry/app.js"></script>'
			);
			return;
		}
		if (request.url === '/_app/immutable/assets/app.css') {
			stylesheetRequests += 1;
			response.writeHead(stylesheetRequests < 3 ? 404 : 200, {
				'content-type': 'text/css'
			});
			response.end('body {}');
			return;
		}
		if (request.url === '/_app/immutable/entry/app.js') {
			response.writeHead(200, { 'content-type': 'text/javascript' });
			response.end('export {};');
			return;
		}
		response.writeHead(404).end();
	});
	t.after(fixture.close);

	const { verifyTenantWorkerReadiness } = await loadVerifier();
	const result = await verifyTenantWorkerReadiness({
		origin: fixture.origin,
		assetMaxAttempts: 3,
		assetRetryDelayMs: 1,
		requestTimeoutMs: 1_000,
		mountMaxAttempts: 1,
		mountRetryDelayMs: 1,
		mountCheck: async () => undefined,
		onAttemptFailure: () => undefined
	});

	assert.deepEqual(result, { assetAttempts: 3, mountAttempts: 1 });
	assert.equal(stylesheetRequests, 3);
});

test('tenant readiness retries a delayed application mount with a fresh check', async (t) => {
	const fixture = await listen((request, response) => {
		if (request.url === '/') {
			response.writeHead(200, { 'content-type': 'text/html' });
			response.end('<script src="/_app/immutable/entry/app.js"></script>');
			return;
		}
		response.writeHead(200, { 'content-type': 'text/javascript' });
		response.end('export {};');
	});
	t.after(fixture.close);

	let mountChecks = 0;
	const { verifyTenantWorkerReadiness } = await loadVerifier();
	const result = await verifyTenantWorkerReadiness({
		origin: fixture.origin,
		assetMaxAttempts: 1,
		assetRetryDelayMs: 1,
		requestTimeoutMs: 1_000,
		mountMaxAttempts: 3,
		mountRetryDelayMs: 1,
		mountCheck: async () => {
			mountChecks += 1;
			if (mountChecks < 3) throw new Error('mount_failed');
		},
		onAttemptFailure: () => undefined
	});

	assert.deepEqual(result, { assetAttempts: 1, mountAttempts: 3 });
});

test('tenant readiness fails after the bounded asset retry budget is exhausted', async (t) => {
	let stylesheetRequests = 0;
	const fixture = await listen((request, response) => {
		if (request.url === '/') {
			response.writeHead(200, { 'content-type': 'text/html' });
			response.end('<link rel="stylesheet" href="/_app/immutable/assets/app.css">');
			return;
		}
		stylesheetRequests += 1;
		response.writeHead(404).end();
	});
	t.after(fixture.close);

	const { verifyTenantWorkerReadiness } = await loadVerifier();
	await assert.rejects(
		verifyTenantWorkerReadiness({
			origin: fixture.origin,
			assetMaxAttempts: 2,
			assetRetryDelayMs: 1,
			requestTimeoutMs: 1_000,
			mountMaxAttempts: 1,
			mountRetryDelayMs: 1,
			mountCheck: async () => undefined,
			onAttemptFailure: () => undefined
		}),
		/asset_readiness_failed_after_2_attempts_http_404/
	);
	assert.equal(stylesheetRequests, 2);
});

test('tenant readiness rejects an asset whose response body is truncated', async (t) => {
	const fixture = await listen((request, response) => {
		if (request.url === '/') {
			response.writeHead(200, { 'content-type': 'text/html' });
			response.end('<script src="/_app/immutable/entry/app.js"></script>');
			return;
		}
		response.writeHead(200, {
			'content-length': '20',
			'content-type': 'text/javascript'
		});
		response.flushHeaders();
		response.write('partial');
		response.destroy();
	});
	t.after(fixture.close);

	const { verifyTenantWorkerReadiness } = await loadVerifier();
	await assert.rejects(
		verifyTenantWorkerReadiness({
			origin: fixture.origin,
			assetMaxAttempts: 1,
			assetRetryDelayMs: 1,
			requestTimeoutMs: 1_000,
			mountMaxAttempts: 1,
			mountRetryDelayMs: 1,
			mountCheck: async () => undefined,
			onAttemptFailure: () => undefined
		}),
		/asset_readiness_failed_after_1_attempts_asset_body_incomplete/
	);
});
