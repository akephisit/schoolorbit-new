import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const root = new URL('../../', import.meta.url);
const read = (path) => readFile(new URL(path, root), 'utf8');
test('certificate preparation routes own primary reads and editor preload cannot POST', async () => {
	for (const route of ['recipients', 'templates', 'templates/[templateId]/editor']) {
		const code = await read(`src/routes/(app)/staff/certificates/[campaignId]/${route}/+page.ts`);
		assert.match(code, /captureRouteLoad/);
		assert.match(code, /school:app-identity/);
		assert.match(code, /requestFetch: fetch/);
		assert.doesNotMatch(code, /createCertificateTemplatePreviewManifest/);
	}
});
test('certificate preparation consumes captured data without lifecycle primary reads', async () => {
	for (const path of [
		'src/lib/components/certificates/CertificateRecipientWorkspace.svelte',
		'src/routes/(app)/staff/certificates/[campaignId]/templates/+page.svelte',
		'src/routes/(app)/staff/certificates/[campaignId]/templates/[templateId]/editor/+page.svelte'
	]) {
		const code = await read(path);
		assert.doesNotMatch(code, /afterNavigate|onMount|loadWorkspace/);
		assert.match(code, /\$effect\.pre/);
		assert.match(code, /onDestroy/);
		assert.match(code, /LatestRequest/);
	}
});
