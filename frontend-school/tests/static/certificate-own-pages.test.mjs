import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

async function source(relativePath) {
	try {
		return await readFile(path.join(projectRoot, relativePath), 'utf8');
	} catch (error) {
		if (error && typeof error === 'object' && error.code === 'ENOENT') return '';
		throw error;
	}
}

test('own certificate API exposes only current-session list detail and render operations', async () => {
	const api = await source('src/lib/api/certificates.ts');

	assert.match(api, /listOwnCertificates[\s\S]*?['"]\/api\/me\/certificates['"]/);
	assert.match(
		api,
		/getOwnCertificate[\s\S]*?`\/api\/me\/certificates\/\$\{encodeURIComponent\(certificateId\)\}`/
	);
	assert.match(
		api,
		/createOwnCertificateRenderManifest[\s\S]*?`\/api\/me\/certificates\/\$\{encodeURIComponent\(certificateId\)\}\/render-manifest`/
	);
	for (const operation of [
		'listOwnCertificates',
		'getOwnCertificate',
		'createOwnCertificateRenderManifest'
	]) {
		assert.doesNotMatch(
			api,
			new RegExp(`function\\s+${operation}\\s*\\([^)]*(?:targetUserId|target_user_id|userId)`)
		);
	}
});

test('staff achievements use route-backed permission-isolated issued and self-recorded tabs', async () => {
	const [root, layout, issued, selfRecorded] = await Promise.all([
		source('src/routes/(app)/staff/achievements/+page.ts'),
		source('src/routes/(app)/staff/achievements/+layout.svelte'),
		source('src/routes/(app)/staff/achievements/issued/+page.ts'),
		source('src/routes/(app)/staff/achievements/self-recorded/+page.ts')
	]);

	assert.match(
		root,
		/permission:\s*\[\s*PERMISSION_MODULES\.ACHIEVEMENT,\s*PERMISSIONS\.CERTIFICATE_READ_OWN\s*\]/
	);
	assert.match(layout, /PERMISSIONS\.CERTIFICATE_READ_OWN/);
	assert.match(layout, /PERMISSION_MODULES\.ACHIEVEMENT/);
	assert.match(layout, /\/staff\/achievements\/issued/);
	assert.match(layout, /\/staff\/achievements\/self-recorded/);
	assert.match(issued, /access:[\s\S]*?PERMISSIONS\.CERTIFICATE_READ_OWN/);
	assert.doesNotMatch(issued, /menu\s*:/);
	assert.match(selfRecorded, /access:[\s\S]*?PERMISSION_MODULES\.ACHIEVEMENT/);
	assert.doesNotMatch(selfRecorded, /menu\s*:/);
});

test('student certificate route is own-scoped and both portals share the read-only list', async () => {
	const [studentRoute, studentPage, staffPage, list] = await Promise.all([
		source('src/routes/(app)/student/certificates/+page.ts'),
		source('src/routes/(app)/student/certificates/+page.svelte'),
		source('src/routes/(app)/staff/achievements/issued/+page.svelte'),
		source('src/lib/components/certificates/MyCertificateList.svelte')
	]);

	assert.match(studentRoute, /permission:\s*PERMISSIONS\.CERTIFICATE_READ_OWN/);
	assert.match(studentRoute, /user_type:\s*['"]student['"]/);
	assert.match(studentPage, /MyCertificateList/);
	assert.match(staffPage, /MyCertificateList/);
	assert.match(list, /listOwnCertificates/);
	assert.match(list, /certificate\.status === ['"]issued['"]/);
	assert.match(list, /certificate\.capabilities\.canDownload/);
	assert.match(
		list,
		/\/verify\/certificate\/\$\{encodeURIComponent\(certificate\.certificateNumber\)\}/
	);
	assert.doesNotMatch(list, /revokeOwnCertificate|canRevoke/);
});

test('self-recorded extraction preserves existing mutation and private-image workflow', async () => {
	const component = await source('src/lib/components/achievement/SelfRecordedAchievements.svelte');

	for (const operation of [
		'getAchievements',
		'createAchievement',
		'updateAchievement',
		'deleteAchievement'
	]) {
		assert.match(component, new RegExp(`\\b${operation}\\b`));
	}
	assert.match(component, /AchievementDialog/);
	assert.match(component, /PrivateFileImage/);
	assert.match(component, /ACHIEVEMENT_READ_OWN/);
	assert.match(component, /ACHIEVEMENT_READ_ALL/);
});
