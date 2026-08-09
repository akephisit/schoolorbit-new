import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(__dirname, '../..');

function frontendPath(relativePath) {
	return path.join(frontendRoot, relativePath);
}

test('auth refresh policy preserves users only for unavailable transport', async () => {
	const { authRefreshDecision } = await import('../../src/lib/auth/auth-refresh-policy.ts');
	assert.deepEqual(authRefreshDecision(200), { result: 'authenticated', clear: false });
	assert.deepEqual(authRefreshDecision(401), { result: 'unauthenticated', clear: true });
	assert.deepEqual(authRefreshDecision(403), { result: 'unavailable', clear: false });
	assert.deepEqual(authRefreshDecision(503), { result: 'unavailable', clear: false });
});

test('auth store distinguishes unavailability without discarding identity or permissions', async () => {
	const source = await readFile(frontendPath('src/lib/stores/auth.ts'), 'utf8');

	assert.match(source, /isUnavailable:\s*boolean/);
	assert.equal(
		[...source.matchAll(/isUnavailable:\s*false/g)].length,
		3,
		'initial, authenticated, and cleared states must reset availability'
	);
	assert.match(
		source,
		/setUnavailable:\s*\(\)\s*=>\s*\{[\s\S]*?update\(\(state\)\s*=>\s*\(\{[\s\S]*?\.\.\.state,[\s\S]*?isLoading:\s*false,[\s\S]*?isUnavailable:\s*true/
	);
	assert.match(source, /clearUser:[\s\S]*?clearPermissions\(\)/);
});

test('auth refresh maps explicit outcomes and preserves state on malformed or unavailable responses', async () => {
	const source = await readFile(frontendPath('src/lib/api/auth.ts'), 'utf8');

	assert.doesNotMatch(source, /async\s+checkAuth\s*\(/);
	assert.match(source, /refreshCurrentUser[\s\S]*?Promise<AuthRefreshResult>/);
	assert.match(source, /authRefreshDecision\(response\.status\)/);
	assert.match(source, /response\.data\s*===\s*undefined[\s\S]*?authStore\.setUnavailable\(\)/);
	assert.match(source, /catch[\s\S]*?authStore\.setUnavailable\(\)[\s\S]*?return\s+'unavailable'/);
});

test('all auth bootstrap call sites branch on explicit refresh states', async () => {
	const files = {
		appLayout: frontendPath('src/routes/(app)/+layout.svelte'),
		loginPage: frontendPath('src/routes/login/+page.svelte'),
		debugPage: frontendPath('src/routes/(app)/debug/+page.svelte'),
		staffProfile: frontendPath('src/routes/(app)/staff/profile/+page.svelte')
	};
	const sources = Object.fromEntries(
		await Promise.all(
			Object.entries(files).map(async ([name, file]) => [name, await readFile(file, 'utf8')])
		)
	);

	for (const [name, source] of Object.entries(sources)) {
		assert.doesNotMatch(source, /\bcheckAuth\s*\(/, name);
	}
	assert.match(sources.appLayout, /authStatus\s*=\s*'unavailable'/);
	assert.match(sources.appLayout, /retryAuthentication/);
	assert.match(sources.appLayout, /ระบบยืนยันตัวตนไม่พร้อมใช้งาน/);
	assert.match(
		sources.appLayout,
		/\$authStore\.isAuthenticated[\s\S]*?authStatus\s*=\s*'authenticated'/
	);
	assert.match(sources.loginPage, /result\s*===\s*'authenticated'/);
	assert.match(sources.loginPage, /result\s*===\s*'unavailable'/);
});
