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

test('school font route is an exact manager-only settings workspace', async () => {
	const route = await readRepoFile('frontend-school/src/routes/(app)/staff/school-fonts/+page.ts');
	const page = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/school-fonts/+page.svelte'
	);

	assert.match(route, /permission:\s*PERMISSIONS\.FONT_MANAGE_SCHOOL/);
	assert.match(route, /group:\s*['"]settings['"]/);
	assert.match(route, /workspace:\s*['"]settings['"]/);
	assert.match(page, /SchoolFontLibrary/);
	assert.match(page, /PERMISSIONS\.FONT_MANAGE_SCHOOL/);
	assert.doesNotMatch(page, /\$lib\/api\/certificates/);
	assert.doesNotMatch(page, /campaign|templateId|certificate/i);
});

test('school font wrappers use only concrete generated shared schemas', async () => {
	const wrapper = await readRepoFile('frontend-school/src/lib/api/school-fonts.ts');
	assert.match(wrapper, /type Schemas = components\['schemas'\]/);
	for (const schema of [
		'SchoolFontSummary',
		'SchoolFontListResponse',
		'InspectSchoolFontUploadsRequest',
		'AttachSchoolFontBatchRequest',
		'SchoolFontUploadInspection',
		'SchoolFontUploadInspectionFile',
		'SchoolFontUploadStatus',
		'SchoolFontDeleteConflict'
	]) {
		assert.match(wrapper, new RegExp(`Schemas\\['${schema}'\\]`));
	}
	for (const operation of [
		'listSchoolFonts',
		'inspectSchoolFontUploads',
		'attachSchoolFontBatch',
		'deleteSchoolFont'
	]) {
		assert.match(wrapper, new RegExp(`export async function ${operation}\\b`));
	}
	assert.doesNotMatch(wrapper, /CertificateFont|unknown\b|\bany\b/);
});

test('reusable uploader preserves central and exact-template file relationships', async () => {
	const files = await readRepoFile('frontend-school/src/lib/api/files.ts');
	const uploader = await readRepoFile(
		'frontend-school/src/lib/components/school-fonts/SchoolFontBatchUpload.svelte'
	);
	const library = await readRepoFile(
		'frontend-school/src/lib/components/school-fonts/SchoolFontLibrary.svelte'
	);

	assert.match(files, /type:\s*['"]central['"]/);
	assert.match(files, /type:\s*['"]certificate_template['"]/);
	assert.match(files, /uploadFile\(file,\s*['"]school_font['"]\)/);
	assert.match(files, /uploadFile\(file,\s*['"]school_font['"],\s*context\.templateId\)/);
	assert.match(files, /deleteFile\(fileId\)/);
	assert.match(files, /deleteFile\(fileId,\s*context\.templateId\)/);
	assert.doesNotMatch(files, /certificate_template_font/);

	assert.match(uploader, /MAX_FONT_BATCH_FILES\s*=\s*40/);
	assert.match(uploader, /for \(const row of selectedRows\)/);
	assert.match(uploader, /rightsConfirmed/);
	assert.match(uploader, /cleanupTemporary/);
	assert.match(uploader, /onattached/);
	assert.match(library, /ApiClientError<SchoolFontDeleteConflict>/);
	assert.match(library, /referenceCount/);
	assert.match(library, /SchoolFontBatchUpload/);
});
