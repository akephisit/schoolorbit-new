import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const source = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');
test('staff home regions use route and shared layout owners without duplicate startup reads', async () => {
	const [layout, loader, page, sidebar] = await Promise.all([
		source('src/routes/(app)/+layout.ts'),
		source('src/routes/(app)/staff/+page.ts'),
		source('src/routes/(app)/staff/+page.svelte'),
		source('src/lib/components/layout/Sidebar.svelte')
	]);
	assert.match(layout, /getUserMenu/);
	assert.match(layout, /getMyWorkCounts/);
	assert.match(layout, /requestFetch: fetch/);
	assert.match(loader, /getStaffDashboard/);
	assert.doesNotMatch(page, /onMount\(|getUserMenu/);
	assert.doesNotMatch(sidebar, /getUserMenu|fetchCounts/);
});
for (const route of ['staff/profile', 'staff/view/[id]']) {
	test(`${route} owns visible profile regions and rejects stale results`, async () => {
		const [loader, page] = await Promise.all([
			source(`src/routes/(app)/${route}/+page.ts`),
			source(`src/routes/(app)/${route}/+page.svelte`)
		]);
		assert.match(loader, /captureRouteLoad/);
		assert.match(loader, /requestFetch: fetch/);
		assert.match(page, /\$effect.pre/);
		assert.match(page, /LatestRequest/);
		assert.doesNotMatch(page, /onMount\(/);
	});
}
test('profile image compression and cropper stay action-lazy', async () => {
	const page = await source('src/lib/components/forms/ProfileImageUpload.svelte');
	assert.doesNotMatch(page, /import Compressor from|import ImageCropper from/);
	assert.match(page, /import\('compressorjs'\)/);
	assert.match(page, /import\('\.\/ImageCropper\.svelte'\)/);
});

test('dashboard and Sidebar use the same preload policy and contextual destinations', async () => {
	for (const path of [
		'src/routes/(app)/staff/+page.svelte',
		'src/lib/components/layout/Sidebar.svelte'
	]) {
		const page = await source(path);
		assert.match(page, /menuPreloadPolicy/);
		assert.match(page, /academicContextualMenuPath/);
	}
});
