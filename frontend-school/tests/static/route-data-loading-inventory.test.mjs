import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const routesRoot = path.join(projectRoot, 'src/routes/(app)');
const inventoryPath = path.join(projectRoot, 'tests/fixtures/route-data-loading-inventory.json');
const dataOwners = new Set([
	'component-primary',
	'route',
	'shared-layout',
	'interaction-only',
	'no-data'
]);
const preloads = new Set(['hover', 'tap', 'none']);
const statuses = new Set(['planned', 'complete']);

async function discoverPages(directory = routesRoot) {
	const pages = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) pages.push(...(await discoverPages(fullPath)));
		else if (entry.name === '+page.svelte') {
			pages.push(path.relative(routesRoot, directory).split(path.sep).join('/'));
		}
	}
	return pages.sort();
}

async function readInventory() {
	let source;
	try {
		source = await readFile(inventoryPath, 'utf8');
	} catch (error) {
		if (error?.code === 'ENOENT') {
			assert.fail('authenticated route data inventory is missing');
		}
		throw error;
	}
	return JSON.parse(source);
}

test('every authenticated page has one reviewed route data owner', async () => {
	const inventory = await readInventory();
	assert.equal(inventory.version, 1);
	assert.ok(Array.isArray(inventory.routes));
	const names = [];
	for (const record of inventory.routes) {
		assert.equal(typeof record.route, 'string');
		assert.ok(record.route.length > 0, 'route name must not be empty');
		assert.ok(dataOwners.has(record.dataOwner), `${record.route}: invalid data owner`);
		assert.ok(preloads.has(record.preload), `${record.route}: invalid preload policy`);
		assert.ok(statuses.has(record.status), `${record.route}: unreviewed status`);
		for (const key of ['wave', 'context', 'backendOwner', 'notes']) {
			assert.ok(
				typeof record[key] === 'string' && record[key].trim().length > 0,
				`${record.route}: missing ${key}`
			);
		}
		if (record.dataOwner === 'no-data') {
			assert.equal(record.status, 'complete', `${record.route}: static route is incomplete`);
		}
		if (record.dataOwner === 'route' && record.status === 'complete') {
			const routeDirectory = path.join(routesRoot, record.route);
			const loaderFiles = ['+page.ts', '+page.server.ts'];
			const loaders = await Promise.all(
				loaderFiles.map(async (file) => {
					try {
						return await readFile(path.join(routeDirectory, file), 'utf8');
					} catch (error) {
						if (error?.code === 'ENOENT') return '';
						throw error;
					}
				})
			);
			assert.ok(
				loaders.some((source) => /export\s+(?:const|function)\s+load\b/.test(source)),
				`${record.route}: completed route has no load function`
			);
		}
		names.push(record.route);
	}
	assert.equal(new Set(names).size, names.length, 'duplicate route inventory entries');
	assert.deepEqual(names.sort(), await discoverPages());
});
