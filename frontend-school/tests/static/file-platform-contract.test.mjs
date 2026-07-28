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
