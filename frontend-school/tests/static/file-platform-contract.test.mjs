import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('generated contract publishes the provider-neutral file platform routes', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const expected = [
		['/api/files', 'post', 'uploadFile'],
		['/api/files/{id}', 'get', 'getFileMetadata'],
		['/api/files/{id}/download', 'post', 'downloadFile'],
		['/api/files/{id}', 'delete', 'deleteFile'],
		['/api/public/files/{id}/content', 'get', 'getPublicFileContent']
	];

	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId);
		assert.match(generated, new RegExp(`\\b${operationId}:\\s*\\{`));
	}

	const metadata = JSON.stringify(contract.components?.schemas?.FileMetadata);
	for (const forbidden of [
		'storagePath',
		'thumbnailPath',
		'objectKey',
		'bucket',
		'provider',
		'checksum',
		'signedUrl'
	]) {
		assert.doesNotMatch(metadata, new RegExp(forbidden, 'i'));
	}
});

test('canonical file upload path is proxied without a trailing-slash redirect', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const nginx = await readRepoFile('nginx-configs/school-api.schoolorbit.app.conf');
	const uploadPath = Object.entries(contract.paths).find(
		([, operations]) => operations.post?.operationId === 'uploadFile'
	)?.[0];

	assert.equal(uploadPath, '/api/files');

	const locations = [
		...nginx.matchAll(/^\s*location\s+(?:(?:=|\^~|~\*?)\s+)?([^\s{]+)\s*\{/gm)
	].map((match) => match[1]);
	assert.ok(
		locations.includes(uploadPath),
		`${uploadPath} must have a slash-safe upload location; a proxy location ending in / redirects POST as GET`
	);
});

test('backend deployment validates and installs the tracked school API proxy config', async () => {
	const workflow = await readRepoFile('.github/workflows/deploy-backend-school.yml');

	assert.match(workflow, /nginx-configs\/school-api\.schoolorbit\.app\.conf/);
	assert.match(
		workflow,
		/proxy_matches="\$\(grep -l 'server_name school-api\\\.schoolorbit\\\.app;' \/opt\/stack\/nginx\/conf\.d\/\*\.conf/
	);
	assert.match(workflow, /proxy_match_count=/);
	assert.match(workflow, /\[ "\$proxy_match_count" -ne 1 \]/);
	assert.match(workflow, /proxy_target="\$proxy_matches"/);
	assert.match(workflow, /podman exec schoolorbit-nginx nginx -t/);
	assert.match(workflow, /cp "\$proxy_backup" "\$proxy_target"/);
	assert.match(workflow, /podman exec schoolorbit-nginx nginx -s reload/);
});

test('typed file helper uses generated DTOs and file IDs as identity', async () => {
	const source = await readRepoFile('frontend-school/src/lib/api/files.ts');

	assert.match(
		source,
		/import\s+type\s+\{\s*components\s*\}\s+from\s+['"]\$lib\/api\/generated\/school-api['"]/
	);
	assert.match(source, /type\s+Schemas\s*=\s*components\['schemas'\]/);
	assert.match(source, /export\s+type\s+FileMetadata\s*=\s*Schemas\['FileMetadata'\]/);
	assert.match(source, /formData\.append\('purpose',\s*purpose\)/);
	assert.match(source, /apiClient\.postMultipart<FileMetadata>\('\/api\/files'/);
	assert.match(source, /apiClient\s*\.\s*get<FileMetadata>\(`\/api\/files\/\$\{fileId\}/);
	assert.match(source, /apiClient\s*\.\s*delete<FileDeleteResult>\(`\/api\/files\/\$\{fileId\}/);
	assert.match(source, /\/api\/public\/files\/\$\{fileId\}\/content/);
	assert.match(source, /\/api\/files\/\$\{fileId\}\/download/);

	for (const forbidden of [
		'storage_path',
		'thumbnail_path',
		'/api/files?path=',
		"formData.append('file_type'",
		'R2_',
		'objectKey',
		'bucketName'
	]) {
		assert.doesNotMatch(source, new RegExp(forbidden.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
	}
});

test('school frontend consumers use file IDs instead of provider paths or persisted URLs', async () => {
	const consumerPaths = [
		'frontend-school/src/lib/api/admission.ts',
		'frontend-school/src/lib/api/auth.ts',
		'frontend-school/src/lib/api/school.ts',
		'frontend-school/src/lib/components/achievement/AchievementCard.svelte',
		'frontend-school/src/lib/components/achievement/AchievementDialog.svelte',
		'frontend-school/src/lib/components/files/PortalFileImage.svelte',
		'frontend-school/src/lib/components/files/PrivateFileImage.svelte',
		'frontend-school/src/lib/components/forms/ProfileImageUpload.svelte',
		'frontend-school/src/lib/components/question-bank/QuestionContent.svelte',
		'frontend-school/src/lib/stores/auth.ts',
		'frontend-school/src/routes/(app)/parent/+page.svelte',
		'frontend-school/src/routes/(app)/parent/student/[id]/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/admission/[id]/applications/[appId]/+page.svelte',
		'frontend-school/src/routes/(app)/staff/academic/question-bank/+page.svelte',
		'frontend-school/src/routes/(app)/staff/achievements/+page.svelte',
		'frontend-school/src/routes/(app)/staff/manage/[id]/+page.svelte',
		'frontend-school/src/routes/(app)/staff/manage/[id]/edit/+page.svelte',
		'frontend-school/src/routes/(app)/staff/profile/+page.svelte',
		'frontend-school/src/routes/(app)/staff/school-settings/+page.svelte',
		'frontend-school/src/routes/(app)/staff/view/[id]/+page.svelte',
		'frontend-school/src/routes/(public)/apply/+page.svelte',
		'frontend-school/src/routes/(public)/apply/[id]/+page.svelte',
		'frontend-school/src/routes/(public)/apply/status/+page.svelte'
	];
	const sources = await Promise.all(consumerPaths.map(readRepoFile));
	const source = sources.join('\n');

	for (const forbidden of [
		'storage_path',
		'thumbnail_path',
		'/api/files?path=',
		"formData.append('file_type'",
		'profile_image_url',
		'profileImageUrl',
		'image_path',
		'fileUrl',
		'logoPath',
		'logoUrl'
	]) {
		assert.doesNotMatch(
			source,
			new RegExp(forbidden.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')),
			`legacy file locator remains: ${forbidden}`
		);
	}

	assert.match(source, /profileImageFileId/);
	assert.match(source, /image_file_id|imageFileId/);
	assert.match(source, /logoFileId/);
	assert.match(source, /publicFileUrl/);
	assert.match(source, /downloadFile/);
});

test('portal document credentials stay in request bodies and downloads use blob delivery', async () => {
	const admission = await readRepoFile('frontend-school/src/lib/api/admission.ts');
	const client = await readRepoFile('frontend-school/src/lib/api/client.ts');

	assert.match(client, /postBlobWithBody/);
	assert.match(admission, /apiClient\.deleteWithBody/);
	assert.match(admission, /apiClient\.postBlobWithBody/);
	assert.doesNotMatch(admission, /URLSearchParams\(\{\s*nationalId/);
	assert.doesNotMatch(admission, /national_id=.*date_of_birth=/);
});
