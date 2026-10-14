import assert from 'node:assert/strict';
import http from 'node:http';
import test from 'node:test';

import {
	pruneGhcrVersions,
	selectDeletionCandidates,
	selectCacheDeletionCandidates
} from '../prune_ghcr_versions.mjs';
import { SCHOOL_BUILD_CACHE_PACKAGE } from '../lib/pipeline-build-cache.mjs';

const OWNER = 'akephisit';
const PACKAGE = 'schoolorbit-backend-admin';

function releaseVersion(index) {
	return {
		id: 1000 + index,
		created_at: new Date(Date.UTC(2026, 7, 31 - index)).toISOString(),
		metadata: {
			package_type: 'container',
			container: {
				tags: [index.toString(16).padStart(40, '0')]
			}
		}
	};
}

function releaseInventory(count) {
	return Array.from({ length: count }, (_, index) => releaseVersion(index));
}

async function startPackageServer({
	versions,
	listStatus = 200,
	linkHeader,
	readVersion,
	packageName = PACKAGE
}) {
	const requests = [];
	const versionReads = new Map();
	const packagePath = `/users/${OWNER}/packages/container/${packageName}/versions`;
	const server = http.createServer((request, response) => {
		const url = new URL(request.url, 'http://127.0.0.1');
		requests.push(`${request.method} ${url.pathname}${url.search}`);

		if (request.method === 'GET' && url.pathname === packagePath) {
			const page = Number(url.searchParams.get('page'));
			const start = (page - 1) * 100;
			const pageVersions = versions.slice(start, start + 100);
			response.statusCode = listStatus;
			response.setHeader('content-type', 'application/json');
			const resolvedLinkHeader =
				typeof linkHeader === 'function'
					? linkHeader({ page, request })
					: linkHeader;
			if (resolvedLinkHeader !== undefined) response.setHeader('link', resolvedLinkHeader);
			response.end(listStatus === 200 ? JSON.stringify(pageVersions) : '{"message":"denied"}');
			return;
		}

		const match = url.pathname.match(new RegExp(`^${packagePath}/([0-9]+)$`));
		if (!match) {
			response.statusCode = 404;
			response.end();
			return;
		}

		const id = Number(match[1]);
		const version = versions.find((candidate) => candidate.id === id);
		if (!version) {
			response.statusCode = 404;
			response.end();
			return;
		}

		if (request.method === 'GET') {
			const count = (versionReads.get(id) ?? 0) + 1;
			versionReads.set(id, count);
			const current = readVersion?.(structuredClone(version), count) ?? version;
			response.setHeader('content-type', 'application/json');
			response.end(JSON.stringify(current));
			return;
		}

		if (request.method === 'DELETE') {
			response.statusCode = 204;
			response.end();
			return;
		}

		response.statusCode = 405;
		response.end();
	});

	await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
	const address = server.address();
	return {
		apiBase: `http://127.0.0.1:${address.port}`,
		requests,
		close: () => new Promise((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())))
	};
}

test('selector keeps latest and the 30 newest releases while protecting unknown versions', () => {
	const versions = releaseInventory(35);
	versions[0].metadata.container.tags.push('latest');
	versions.push({
		id: 2001,
		created_at: '2026-01-01T00:00:00.000Z',
		metadata: { package_type: 'container', container: { tags: [] } }
	});
	versions.push({
		id: 2002,
		created_at: '2025-01-01T00:00:00.000Z',
		metadata: { package_type: 'container', container: { tags: ['attestation'] } }
	});

	const candidates = selectDeletionCandidates(versions, 30);

	assert.deepEqual(
		candidates.map(({ id }) => id),
		[1034, 1033, 1032, 1031, 1030]
	);
	assert.ok(candidates.every(({ safeTags }) => safeTags.every((tag) => /^[0-9a-f]{40}$/.test(tag))));
});

test('dry run lists candidates without rereading or deleting package versions', async (t) => {
	const server = await startPackageServer({ versions: releaseInventory(32) });
	t.after(server.close);
	const lines = [];

	const result = await pruneGhcrVersions({
		owner: OWNER,
		packageName: PACKAGE,
		keep: 30,
		execute: false,
		token: 'test-token',
		apiBase: server.apiBase,
		logger: (line) => lines.push(line)
	});

	assert.equal(result.candidates, 2);
	assert.equal(result.deleted, 0);
	assert.equal(server.requests.filter((request) => request.startsWith('DELETE ')).length, 0);
	assert.equal(server.requests.filter((request) => /\/versions\/[0-9]+$/.test(request)).length, 0);
	assert.ok(lines.every((line) => !line.includes('test-token')));
});

test('inventory pagination is complete and deletion selection is capped at 100', async (t) => {
	const server = await startPackageServer({ versions: releaseInventory(135) });
	t.after(server.close);

	const result = await pruneGhcrVersions({
		owner: OWNER,
		packageName: PACKAGE,
		keep: 30,
		execute: false,
		token: 'test-token',
		apiBase: server.apiBase,
		logger: () => {}
	});

	assert.deepEqual(result, { candidates: 105, selected: 100, deleted: 0 });
	assert.deepEqual(
		server.requests.filter((request) => request.includes('?per_page=')),
		[
			`GET /users/${OWNER}/packages/container/${PACKAGE}/versions?per_page=100&page=1`,
			`GET /users/${OWNER}/packages/container/${PACKAGE}/versions?per_page=100&page=2`
		]
	);
});

test('canonicalized GitHub pagination URLs do not block complete inventory', async (t) => {
	const server = await startPackageServer({
		versions: releaseInventory(135),
		linkHeader({ page, request }) {
			if (page !== 1) return undefined;
			return `<http://${request.headers.host}/users/123456/packages/container/${PACKAGE}/versions?state=active&per_page=100&page=2>; rel="next", <http://${request.headers.host}/users/123456/packages/container/${PACKAGE}/versions?state=active&per_page=100&page=2>; rel="last"`;
		}
	});
	t.after(server.close);

	const result = await pruneGhcrVersions({
		owner: OWNER,
		packageName: PACKAGE,
		keep: 30,
		execute: false,
		token: 'test-token',
		apiBase: server.apiBase,
		logger: () => {}
	});

	assert.deepEqual(result, { candidates: 105, selected: 100, deleted: 0 });
	assert.equal(
		server.requests.filter((request) => request.includes('?per_page=')).length,
		2
	);
});

test('execute mode revalidates and deletes only candidates from oldest to newest', async (t) => {
	const server = await startPackageServer({ versions: releaseInventory(32) });
	t.after(server.close);

	const result = await pruneGhcrVersions({
		owner: OWNER,
		packageName: PACKAGE,
		keep: 30,
		execute: true,
		token: 'test-token',
		apiBase: server.apiBase,
		logger: () => {}
	});

	assert.equal(result.deleted, 2);
	assert.deepEqual(
		server.requests.filter((request) => request.startsWith('DELETE ')),
		[
			`DELETE /users/${OWNER}/packages/container/${PACKAGE}/versions/1031`,
			`DELETE /users/${OWNER}/packages/container/${PACKAGE}/versions/1030`
		]
	);
	for (const id of [1031, 1030]) {
		assert.equal(
			server.requests.filter(
				(request) => request === `GET /users/${OWNER}/packages/container/${PACKAGE}/versions/${id}`
			).length,
			2
		);
	}
});

test('changed protected metadata fails closed before any deletion', async (t) => {
	const versions = releaseInventory(31);
	const server = await startPackageServer({
		versions,
		readVersion(version) {
			version.metadata.container.tags.push('latest');
			return version;
		}
	});
	t.after(server.close);

	await assert.rejects(
		pruneGhcrVersions({
			owner: OWNER,
			packageName: PACKAGE,
			keep: 30,
			execute: true,
			token: 'test-token',
			apiBase: server.apiBase,
			logger: () => {}
		}),
		/changed during revalidation/
	);
	assert.equal(server.requests.filter((request) => request.startsWith('DELETE ')).length, 0);
});

test('malformed pagination and unauthorized inventories fail before deletion', async (t) => {
	const malformed = await startPackageServer({
		versions: releaseInventory(31),
		linkHeader: 'not-a-link-header'
	});
	const unauthorized = await startPackageServer({
		versions: releaseInventory(31),
		listStatus: 401
	});
	t.after(async () => {
		await malformed.close();
		await unauthorized.close();
	});

	for (const server of [malformed, unauthorized]) {
		await assert.rejects(
			pruneGhcrVersions({
				owner: OWNER,
				packageName: PACKAGE,
				keep: 30,
				execute: true,
				token: 'test-token',
				apiBase: server.apiBase,
				logger: () => {}
			})
		);
		assert.equal(server.requests.filter((request) => request.startsWith('DELETE ')).length, 0);
	}
});

test('unsupported packages fail before making a request', async () => {
	await assert.rejects(
		pruneGhcrVersions({
			owner: OWNER,
			packageName: 'unrelated-package',
			keep: 30,
			execute: false,
			token: 'test-token',
			apiBase: 'http://127.0.0.1:1',
			logger: () => {}
		}),
		/Unsupported GHCR package/
	);
});

// Prepared input tags are release inventory; protect the accepted component.
test('retention covers prepared input tags and preserves the accepted older component', () => {
    const inventory = releaseInventory(35);
    const accepted = inventory[34].metadata.container.tags[0];
    inventory[33].metadata.container.tags = ['input-' + 'b'.repeat(64)];
    const candidates = selectDeletionCandidates(inventory, 30, [accepted]);
    assert.ok(candidates.some(x => x.id === inventory[33].id));
    assert.ok(!candidates.some(x => x.id === inventory[34].id));
});

function cacheInventory(count) {
	return releaseInventory(count).map((version) => ({ ...version, metadata: { package_type: 'container', container: { tags: [] } } }));
}

test('cache selector protects all tags, two newest versions and the 24-hour grace period', () => {
	const now = Date.UTC(2026, 9, 14);
	const versions = cacheInventory(8);
	versions[0].metadata.container.tags = ['buildcache'];
	versions[1].metadata.container.tags = ['unknown-tag'];
	versions[5].created_at = new Date(now - 1000).toISOString();
	versions[6].created_at = new Date(now - 2000).toISOString();
	versions[7].created_at = new Date(now - 3600000).toISOString();
	assert.deepEqual(selectCacheDeletionCandidates(versions, 2, now).map(({ id }) => id), [1004, 1003, 1002]);
	assert.throws(() => selectCacheDeletionCandidates([versions[0], versions[0]], 2, now), /duplicate/);
	assert.throws(() => selectCacheDeletionCandidates(versions, 0, now), /retention count/);
	assert.throws(() => selectCacheDeletionCandidates(versions, 2, NaN), /retention clock/);
});

test('cache execution deletes only old untagged versions from its separate package', async (t) => {
	const versions = cacheInventory(8);
	versions[6].metadata.container.tags = ['buildcache'];
	versions[7].metadata.container.tags = ['unknown-tag'];
	const api = await startPackageServer({ versions, packageName: SCHOOL_BUILD_CACHE_PACKAGE });
	t.after(api.close);
	const result = await pruneGhcrVersions({ owner: OWNER, packageName: SCHOOL_BUILD_CACHE_PACKAGE, keep: 2, execute: true, token: 'test-token', apiBase: api.apiBase, logger() {} });
	assert.equal(result.deleted, 4);
	assert.deepEqual(api.requests.filter((request) => request.startsWith('DELETE ')), [1005, 1004, 1003, 1002].map((id) => `DELETE /users/${OWNER}/packages/container/${SCHOOL_BUILD_CACHE_PACKAGE}/versions/${id}`));
	assert.ok(api.requests.every((request) => request.includes(`/container/${SCHOOL_BUILD_CACHE_PACKAGE}/`)));
});

test('a cache version becoming tagged during immediate revalidation prevents every deletion', async (t) => {
	const api = await startPackageServer({ versions: cacheInventory(5), packageName: SCHOOL_BUILD_CACHE_PACKAGE, readVersion(version, count) {
		if (version.id === 1004 && count === 2) version.metadata.container.tags = ['buildcache'];
		return version;
	} });
	t.after(api.close);
	await assert.rejects(pruneGhcrVersions({ owner: OWNER, packageName: SCHOOL_BUILD_CACHE_PACKAGE, keep: 2, execute: true, token: 'test-token', apiBase: api.apiBase, logger() {} }), /changed during revalidation/);
	assert.equal(api.requests.filter((request) => request.startsWith('DELETE ')).length, 0);
});

for (const status of [404, 403]) test(`new cache package inventory status ${status} is handled explicitly`, async (t) => {
	const api = await startPackageServer({ versions: [], packageName: SCHOOL_BUILD_CACHE_PACKAGE, listStatus: status });
	t.after(api.close);
	const operation = pruneGhcrVersions({ owner: OWNER, packageName: SCHOOL_BUILD_CACHE_PACKAGE, keep: 2, execute: true, token: 'test-token', apiBase: api.apiBase, logger() {} });
	if (status === 404) assert.deepEqual(await operation, { candidates: 0, selected: 0, deleted: 0 });
	else await assert.rejects(operation, /status 403/);
	assert.equal(api.requests.filter((request) => request.startsWith('DELETE ')).length, 0);
});

test('runtime packages retain their missing-inventory failure and cache packages reject release-protection arguments', async (t) => {
	const api = await startPackageServer({ versions: [], listStatus: 404 });
	t.after(api.close);
	await assert.rejects(pruneGhcrVersions({ owner: OWNER, packageName: PACKAGE, keep: 30, execute: true, token: 'test-token', apiBase: api.apiBase, logger() {} }), /status 404/);
	let requests = 0;
	await assert.rejects(pruneGhcrVersions({ owner: OWNER, packageName: SCHOOL_BUILD_CACHE_PACKAGE, keep: 2, protectedTags: ['a'.repeat(40)], token: 'test-token', fetchImpl() { requests++; }, logger() {} }), /does not accept release protection/);
	assert.equal(requests, 0);
});
