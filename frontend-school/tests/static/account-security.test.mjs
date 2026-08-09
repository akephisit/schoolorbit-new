import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(__dirname, '../..');

function frontendPath(relativePath) {
	return path.join(frontendRoot, relativePath);
}

async function readFrontendFile(relativePath) {
	return readFile(frontendPath(relativePath), 'utf8');
}

async function listSourceFiles(relativeDirectory) {
	const directory = frontendPath(relativeDirectory);
	const entries = await readdir(directory, { withFileTypes: true });
	const files = [];

	for (const entry of entries) {
		const relativePath = path.join(relativeDirectory, entry.name);
		if (entry.isDirectory()) {
			files.push(...(await listSourceFiles(relativePath)));
		} else if (/\.(?:svelte|ts)$/.test(entry.name)) {
			files.push(relativePath);
		}
	}

	return files;
}

test('account security is guard-only and available to every authenticated user type', async () => {
	const pageMeta = await readFrontendFile('src/routes/(app)/account/security/+page.ts');
	const page = await readFrontendFile('src/routes/(app)/account/security/+page.svelte');
	const routeAccess = await readFrontendFile('src/lib/auth/route-access.ts');

	assert.match(pageMeta, /access:\s*\{\s*authenticated:\s*true\s*\}/s);
	assert.doesNotMatch(pageMeta, /\bmenu\s*:/);
	assert.match(page, /<PageShell[^>]*title="ความปลอดภัยของบัญชี"/);
	assert.match(page, /<SessionSecurityPanel\s*\/>/);
	assert.match(routeAccess, /authenticated\?:\s*boolean/);
	assert.match(routeAccess, /access\.authenticated/);
	assert.match(routeAccess, /if\s*\(!user\)\s*return\s+false/);
});

test('session state patches only the revoked row and validates password byte limits', async () => {
	const { keepCurrentSession, passwordValidation, removeRevokedSession } =
		await import('../../src/lib/features/session-security/session-state.ts');
	const sessions = [
		{ id: 'current', isCurrent: true },
		{ id: 'other', isCurrent: false }
	];

	assert.deepEqual(removeRevokedSession(sessions, 'other'), [{ id: 'current', isCurrent: true }]);
	assert.deepEqual(keepCurrentSession(sessions), [{ id: 'current', isCurrent: true }]);
	assert.equal(passwordValidation('', 'password', 'password'), 'กรุณากรอกข้อมูลให้ครบถ้วน');
	assert.equal(
		passwordValidation('current-pass', 'password', 'different'),
		'รหัสผ่านใหม่ไม่ตรงกัน'
	);
	assert.match(passwordValidation('current-pass', 'short', 'short') ?? '', /8–128/);
	assert.equal(passwordValidation('current-pass', 'ก'.repeat(23), 'ก'.repeat(23)), null);
	assert.match(passwordValidation('current-pass', 'ก'.repeat(24), 'ก'.repeat(24)) ?? '', /ยาวเกิน/);
});

test('session security panel owns loading, retry, session mutations, and password state', async () => {
	const panel = await readFrontendFile(
		'src/lib/features/session-security/SessionSecurityPanel.svelte'
	);

	assert.match(panel, /\$state\.raw<SessionDto\[\]>\(\[\]\)/);
	assert.match(panel, /onMount\(loadSessions\)/);
	assert.match(panel, /\{#each\s+sessions\s+as\s+session\s+\(session\.id\)\}/);
	assert.match(panel, /<PageSkeleton/);
	assert.match(panel, /<PageState[\s\S]*variant="error"[\s\S]*onaction=\{loadSessions\}/);
	assert.match(panel, /<PageState[\s\S]*ยังไม่มีอุปกรณ์ที่เข้าสู่ระบบ/);
	assert.match(panel, /อุปกรณ์นี้/);
	assert.match(panel, /session\.rememberMe/);
	assert.match(panel, /revokingSessionId/);
	assert.match(panel, /isLoggingOutAll/);
	assert.match(panel, /isChangingPassword/);
	assert.match(panel, /removeRevokedSession\(sessions,\s*session\.id\)/);
	assert.match(panel, /revokeSession\(session\.id,\s*\{\s*current:\s*true\s*\}\)/);
	assert.match(panel, /authAPI\.logoutAll\(\)/);
	assert.match(panel, /keepCurrentSession\(sessions\)/);
	assert.match(panel, /passwordValidation\(currentPassword,\s*newPassword,\s*confirmPassword\)/);
	assert.match(
		panel,
		/currentPassword\s*=\s*''[\s\S]*newPassword\s*=\s*''[\s\S]*confirmPassword\s*=\s*''/
	);
	assert.ok((panel.match(/<AlertDialog\.Root/g) ?? []).length >= 2);
	assert.ok((panel.match(/<LoadingButton/g) ?? []).length >= 4);
});

test('settings and profile menu point to one shared account-security page', async () => {
	const profileMenu = await readFrontendFile('src/lib/components/layout/ProfileMenu.svelte');
	const staffSettings = await readFrontendFile('src/routes/(app)/staff/settings/+page.svelte');
	const studentSettings = await readFrontendFile('src/routes/(app)/student/settings/+page.svelte');

	assert.match(profileMenu, /ความปลอดภัยของบัญชี/);
	assert.match(profileMenu, /\/account\/security/);
	assert.doesNotMatch(profileMenu, /user\.email/);

	for (const settings of [staffSettings, studentSettings]) {
		assert.match(settings, /href="\/account\/security"/);
		assert.doesNotMatch(settings, /type=\{?['"]password['"]\}?/);
		assert.doesNotMatch(settings, /authAPI\.changePassword/);
	}
});

test('default auth and menu state contain no minimized PII or duplicated permissions', async () => {
	const authStore = await readFrontendFile('src/lib/stores/auth.ts');
	const authApi = await readFrontendFile('src/lib/api/auth.ts');
	const profileMenu = await readFrontendFile('src/lib/components/layout/ProfileMenu.svelte');
	const staffProfile = await readFrontendFile('src/routes/(app)/staff/profile/+page.svelte');
	const debugPage = await readFrontendFile('src/routes/(app)/debug/+page.svelte');

	assert.doesNotMatch(authStore, /nationalId|email\?:|phone\?:|createdAt/);
	assert.doesNotMatch(authStore, /permissions\?:/);
	assert.match(authStore, /setUser:\s*\(user:\s*User,\s*permissions:\s*string\[\]\)/);
	assert.match(authStore, /setPermissions\(permissions\)/);
	assert.doesNotMatch(authApi, /authStore\.user\.permissions/);
	assert.doesNotMatch(profileMenu, /user\.email/);
	assert.doesNotMatch(staffProfile, /user\?\.(nationalId|createdAt)/);
	assert.doesNotMatch(debugPage, /authState\.user\??\.permissions/);

	const violations = [];
	for (const relativePath of await listSourceFiles('src')) {
		if (relativePath.startsWith('src/lib/api/generated/')) continue;
		const source = await readFrontendFile(relativePath);
		if (/authStore\.user\.permissions|\$authStore\.user\??\.permissions/.test(source)) {
			violations.push(relativePath);
		}
	}
	assert.deepEqual(violations, []);
});
