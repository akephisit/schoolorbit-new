import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const appRoutes = path.join(projectRoot, 'src/routes/(app)');
const inventoryPath = path.join(projectRoot, 'tests/fixtures/route-data-loading-inventory.json');

async function pageFiles(directory) {
	const files = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await pageFiles(fullPath)));
		else if (entry.name === '+page.svelte') files.push(fullPath);
	}
	return files;
}

async function sourceFiles(directory) {
	const files = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await sourceFiles(fullPath)));
		else if (/\.(?:svelte|ts)$/.test(entry.name)) files.push(fullPath);
	}
	return files;
}

test('legacy page-mount API reads only shrink during route migration', async () => {
	const inventory = JSON.parse(await readFile(inventoryPath, 'utf8'));
	const records = new Map(inventory.routes.map((record) => [record.route, record]));
	const candidates = [];
	for (const file of await pageFiles(appRoutes)) {
		const source = await readFile(file, 'utf8');
		if (/\bonMount\b/.test(source) && /\$lib\/api\//.test(source)) {
			const route = path.relative(appRoutes, path.dirname(file)).split(path.sep).join('/');
			const record = records.get(route);
			assert.equal(record?.legacyMountApiCandidate, true, `${route}: new page-mount API read`);
			assert.equal(record?.dataOwner, 'component-primary', `${route}: completed route regressed`);
			assert.equal(record?.status, 'planned', `${route}: completed route regressed`);
			candidates.push(route);
		}
	}
	assert.ok(!candidates.includes('staff/academic/delivery'));
	const delivery = records.get('staff/academic/delivery');
	assert.equal(delivery?.dataOwner, 'route');
	assert.equal(delivery?.status, 'complete');
	const loader = await readFile(path.join(appRoutes, 'staff/academic/delivery/+page.ts'), 'utf8');
	assert.match(loader, /\$lib\/api\/learning-delivery/);
});

test('API contract contains no route-wide page-view endpoint', async () => {
	const contract = JSON.parse(
		await readFile(path.join(projectRoot, '../contracts/openapi/school-api.json'), 'utf8')
	);
	assert.deepEqual(
		Object.keys(contract.paths).filter((apiPath) => apiPath.endsWith('/page-view')),
		[]
	);
});

test('the application keeps safe hover data preload enabled', async () => {
	const app = await readFile(path.join(projectRoot, 'src/app.html'), 'utf8');
	assert.match(app, /<body[^>]*data-sveltekit-preload-data="hover"/);
});

test('route and component code uses focused invalidation', async () => {
	const allowedInvalidateAllOwners = new Set([]);
	const offenders = [];
	for (const file of await sourceFiles(path.join(projectRoot, 'src'))) {
		const relative = path.relative(projectRoot, file);
		if (
			/\binvalidateAll\s*\(/.test(await readFile(file, 'utf8')) &&
			!allowedInvalidateAllOwners.has(relative)
		) {
			offenders.push(relative);
		}
	}
	assert.deepEqual(offenders.sort(), []);
});
