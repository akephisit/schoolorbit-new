import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(__dirname, '../..');

async function readFrontendFile(relativePath) {
	return readFile(path.join(frontendRoot, relativePath), 'utf8');
}

test('captures CSRF in module memory and injects only backend unsafe methods', async () => {
	const security = await import('../../src/lib/api/session-security.ts');
	security.clearSessionSecurity();
	security.captureSessionSecurityHeaders(new Headers({ 'X-CSRF-Token': 'csrf-one' }));
	security.captureSessionSecurityHeaders(new Headers());
	const callerHeaders = new Headers({ 'X-CSRF-Token': 'caller-controlled' });
	const postHeaders = security.withSessionSecurityHeaders('POST', callerHeaders);
	const getHeaders = security.withSessionSecurityHeaders('GET', callerHeaders);
	assert.equal(postHeaders.get('X-CSRF-Token'), 'csrf-one');
	assert.equal(getHeaders.has('X-CSRF-Token'), false);
	security.clearSessionSecurity();
	assert.equal(
		security.withSessionSecurityHeaders('DELETE', new Headers()).has('X-CSRF-Token'),
		false
	);
});

test('parses delta-seconds Retry-After and rejects dates or invalid values', async () => {
	const { retryAfterSeconds } = await import('../../src/lib/api/session-security.ts');
	assert.equal(retryAfterSeconds(new Headers({ 'Retry-After': '30' })), 30);
	assert.equal(retryAfterSeconds(new Headers({ 'Retry-After': '31' })), undefined);
	assert.equal(retryAfterSeconds(new Headers({ 'Retry-After': '0' })), undefined);
	assert.equal(retryAfterSeconds(new Headers({ 'Retry-After': '-1' })), undefined);
	assert.equal(
		retryAfterSeconds(new Headers({ 'Retry-After': 'Wed, 21 Oct 2030 07:28:00 GMT' })),
		undefined
	);
});

test('session security state is memory-only and never exposes the raw token', async () => {
	const source = await readFrontendFile('src/lib/api/session-security.ts');

	assert.doesNotMatch(source, /\b(?:localStorage|sessionStorage)\b/);
	assert.doesNotMatch(source, /document\.cookie|cookieStore/);
	assert.doesNotMatch(source, /export\s+(?:const|let|var)\s+csrfToken\b/);
	assert.doesNotMatch(source, /export\s+function\s+(?:get|read)CsrfToken\b/i);
});

test('all backend fetches share capture and feature modules cannot set security headers', async () => {
	const client = await readFrontendFile('src/lib/api/client.ts');
	const apiDir = path.join(frontendRoot, 'src/lib/api');
	const featureFiles = (await readdir(apiDir, { withFileTypes: true }))
		.filter(
			(entry) =>
				entry.isFile() &&
				entry.name.endsWith('.ts') &&
				!['client.ts', 'session-security.ts'].includes(entry.name)
		)
		.map((entry) => entry.name);

	assert.match(client, /private\s+async\s+fetchBackend\s*\(/);
	assert.match(
		client,
		/fetchBackend[\s\S]*?await\s+fetch\s*\([\s\S]*?captureSessionSecurityHeaders\(response\.headers\)/
	);
	assert.equal(
		[...client.matchAll(/\bfetch\s*\(/g)].length,
		2,
		'only fetchBackend and isolated getExternalBlob may call fetch directly'
	);
	for (const method of ['request', 'getBlob', 'postBlob', 'postBlobWithBody', 'postMultipart']) {
		assert.match(client, new RegExp(`${method}[\\s\\S]*?this\\.fetchBackend\\(`));
	}

	for (const file of featureFiles) {
		const source = await readFrontendFile(`src/lib/api/${file}`);
		assert.doesNotMatch(source, /X-(?:CSRF-Token|School-Subdomain)/i, file);
	}
});

test('generated auth contract exposes minimal current user and session operations', async () => {
	const generated = await readFrontendFile('src/lib/api/generated/school-api.ts');
	const auth = await readFrontendFile('src/lib/api/auth.ts');

	for (const schema of ['CurrentUserResponse', 'SessionResponse', 'SessionListData']) {
		assert.match(generated, new RegExp(`^[\\t ]*${schema}:\\s*\\{`, 'm'));
	}
	assert.match(auth, /Schemas\['CurrentUserResponse'\]/);
	assert.match(auth, /Schemas\['SessionResponse'\]/);
	assert.match(auth, /Schemas\['SessionListData'\]/);
	assert.match(auth, /apiClient\.get<SessionListData>\(['"]\/api\/auth\/sessions['"]\)/);
	assert.match(auth, /apiClient\.delete<EmptyData>\(`\/api\/auth\/sessions\/\$\{sessionId\}`\)/);
	assert.match(auth, /apiClient\.post<EmptyData>\(['"]\/api\/auth\/logout-all['"]\)/);
});
