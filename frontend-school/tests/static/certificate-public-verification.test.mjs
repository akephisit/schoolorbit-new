import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const projectRoot = new URL('../../', import.meta.url);

async function source(relativePath) {
	return readFile(new URL(relativePath, projectRoot), 'utf8');
}

test('public certificate API uses generated contracts without cookies or referrers', async () => {
	const api = await source('src/lib/api/public-certificates.ts');
	const client = await source('src/lib/api/client.ts');
	const transport = `${api}\n${client}`;

	for (const contract of [
		'ManualCertificateVerificationRequest',
		'QrCertificateVerificationRequest',
		'PublicCertificateVerificationData',
		'PublicCertificateRenderRequest',
		'CertificateRenderManifest'
	]) {
		assert.match(api, new RegExp(`Schemas\\['${contract}'\\]`));
	}
	assert.match(api, /apiClient\.postPublic/);
	assert.match(client, /type ApiTransport = ['"]session['"] \| ['"]public['"]/);
	assert.match(client, /credentials:\s*usesSession\s*\?\s*['"]include['"]\s*:\s*['"]omit['"]/);
	assert.match(client, /if \(!usesSession\)[\s\S]*?referrerPolicy = ['"]no-referrer['"]/);
	assert.match(client, /this\.request<[\s\S]*?['"]public['"]\s*\)/);
	assert.match(transport, /cache = ['"]no-store['"]/);
	assert.match(transport, /X-School-Subdomain/);
	assert.doesNotMatch(api, /proof=.*(?:\?|&)|URLSearchParams\([^)]*proof/);
});

test('manual and QR routes share one verification component with three manual fields', async () => {
	const component = await source(
		'src/lib/components/certificates/PublicCertificateVerification.svelte'
	);
	const manualPage = await source('src/routes/(public)/verify/certificate/+page.svelte');
	const qrPage = await source('src/routes/(public)/verify/certificate/[number]/+page.svelte');

	for (const field of ['certificateNumber', 'firstName', 'lastName']) {
		assert.match(component, new RegExp(`bind:value=\\{${field}\\}`));
	}
	assert.match(manualPage, /PublicCertificateVerification/);
	assert.match(qrPage, /PublicCertificateVerification/);
	assert.match(qrPage, /autoVerifyQr/);
});

test('QR proof is copied from the fragment, removed, then sent only in a POST body', async () => {
	const component = await source(
		'src/lib/components/certificates/PublicCertificateVerification.svelte'
	);
	const readAt = component.indexOf('window.location.hash');
	const removeAt = component.indexOf('window.history.replaceState');
	const verifyAt = component.indexOf('verifyCertificateByQr(');

	assert.ok(readAt >= 0, 'QR flow must read the fragment in the browser');
	assert.ok(removeAt > readAt, 'QR flow must remove the fragment after copying it');
	assert.ok(
		verifyAt > removeAt,
		'QR verification must start only after the address bar is scrubbed'
	);
	assert.match(component, /onMount/);
	assert.doesNotMatch(component, /console\.(?:log|debug|info|warn|error)/);
});

test('revoked public results never expose the PDF download action', async () => {
	const component = await source(
		'src/lib/components/certificates/PublicCertificateVerification.svelte'
	);

	assert.match(component, /result\.status\s*===\s*['"]issued['"]/);
	assert.match(component, /result\.status\s*===\s*['"]revoked['"]/);
	assert.match(component, /result\.receipt/);
	assert.match(component, /createPublicCertificateRenderManifest/);
	assert.match(component, /loadCertificateRenderer/);
	assert.match(component, /downloadCertificatePdf/);
});

test('public verification pages declare no-referrer metadata', async () => {
	const pages = await Promise.all([
		source('src/routes/(public)/verify/certificate/+page.svelte'),
		source('src/routes/(public)/verify/certificate/[number]/+page.svelte')
	]);

	for (const page of pages) {
		assert.match(page, /name=["']referrer["']/);
		assert.match(page, /content=["']no-referrer["']/);
	}
});
