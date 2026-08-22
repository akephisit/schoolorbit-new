import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

test('template page uses exact campaign capability and cards expose operational state', async () => {
	const [meta, page, list] = await Promise.all([
		readProjectFile('src/routes/(app)/staff/certificates/[campaignId]/templates/+page.ts'),
		readProjectFile('src/routes/(app)/staff/certificates/[campaignId]/templates/+page.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateTemplateList.svelte')
	]);

	assert.match(meta, /PERMISSIONS\.CERTIFICATE_READ_ORGANIZATION_UNIT/);
	assert.match(meta, /PERMISSIONS\.CERTIFICATE_READ_SCHOOL/);
	assert.match(page, /PERMISSIONS\.CERTIFICATE_CREATE_(ORGANIZATION_UNIT|SCHOOL)/);
	assert.match(page, /PERMISSIONS\.CERTIFICATE_UPDATE_(ORGANIZATION_UNIT|SCHOOL)/);
	assert.match(page, /getCertificateCampaign/);
	assert.match(page, /capabilities\.canManageTemplates/);
	assert.doesNotMatch(`${meta}\n${page}`, /certificate\.(read|create|update|delete)\./);
	assert.match(list, /allowedRecipientTypes/);
	assert.match(list, /describePaper/);
	assert.match(list, /backgroundFileId|isReady/);
	assert.match(list, /isActive/);
	assert.match(list, /\/editor/);
});

test('pending uploads survive UI controls and campaign route loads are race safe', async () => {
	const [page, list, form, background, assets, fontBatch] = await Promise.all([
		readProjectFile('src/routes/(app)/staff/certificates/[campaignId]/templates/+page.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateTemplateList.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateTemplateForm.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateBackgroundUpload.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateAssetManager.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateFontBatchUpload.svelte')
	]);

	assert.match(page, /beforeNavigate/);
	assert.match(page, /afterNavigate/);
	assert.match(page, /loadGeneration/);
	assert.match(page, /Date\.parse\([^)]+updatedAt\)/);
	assert.match(page, /formHasPendingUpload/);
	assert.match(list, /pendingUploadKeys/);
	assert.match(list, /hasPendingUpload/);
	for (const component of [form, background, assets, fontBatch]) {
		assert.match(component, /onpendingchange/);
	}
	assert.match(list, /hasPendingUpload\(expandedTemplateId\)/);
	assert.match(list, /แนบหรือลบไฟล์ชั่วคราว/);
});

test('template files use typed purposes, exact filters, and template resource id', async () => {
	const [background, assets, fontBatch, sharedBatch, form] = await Promise.all([
		readProjectFile('src/lib/components/certificates/CertificateBackgroundUpload.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateAssetManager.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateFontBatchUpload.svelte'),
		readProjectFile('src/lib/components/school-fonts/SchoolFontBatchUpload.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateTemplateForm.svelte')
	]);
	const source = `${background}\n${assets}\n${fontBatch}\n${sharedBatch}\n${form}`;

	assert.match(background, /accept=["']\.pdf["']/);
	assert.match(assets, /accept=["']\.png,\.jpg,\.jpeg,\.webp["']/);
	assert.match(sharedBatch, /accept=["']\.ttf,\.otf["']/);
	assert.match(sharedBatch, /multiple/);
	assert.match(background, /certificate_template_background/);
	assert.match(assets, /certificate_template_image/);
	assert.match(fontBatch, /type:\s*['"]certificate_template['"]/);
	assert.match(sharedBatch, /uploadSchoolFontFile/);
	assert.match(source, /uploadCertificateTemplateFile\([\s\S]*templateId/);
	assert.match(source, /attachCertificateTemplate(Background|Asset)/);
	assert.doesNotMatch(source, /certificate_template_font/);
	assert.doesNotMatch(
		source,
		/type=["']number["'][^>]*(width|height)|(width|height)[^>]*type=["']number["']/i
	);
});

test('font rights and failed-attach cleanup remain explicit', async () => {
	const [assets, fontBatch, sharedBatch] = await Promise.all([
		readProjectFile('src/lib/components/certificates/CertificateAssetManager.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateFontBatchUpload.svelte'),
		readProjectFile('src/lib/components/school-fonts/SchoolFontBatchUpload.svelte')
	]);

	assert.match(sharedBatch, /rightsConfirmed/);
	assert.match(sharedBatch, /ยืนยัน.*สิทธิ์|สิทธิ์.*ฟอนต์/);
	assert.match(sharedBatch, /!rightsConfirmed/);
	assert.match(assets, /unattachedFile/);
	assert.match(assets, /deleteFile\([\s\S]*templateId/);
	assert.match(sharedBatch, /deleteSchoolFontTemporaryFile/);
	assert.match(fontBatch, /SchoolFontBatchUpload/);
});

test('font batch review is sequential, atomic, metadata-driven, and cleanup durable', async () => {
	const [assets, fontBatch, sharedBatch] = await Promise.all([
		readProjectFile('src/lib/components/certificates/CertificateAssetManager.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateFontBatchUpload.svelte'),
		readProjectFile('src/lib/components/school-fonts/SchoolFontBatchUpload.svelte')
	]);

	assert.match(assets, /CertificateFontBatchUpload/);
	assert.match(fontBatch, /SchoolFontBatchUpload/);
	assert.match(sharedBatch, /MAX_FONT_BATCH_FILES\s*=\s*40/);
	assert.match(sharedBatch, /queued[\s\S]*uploading[\s\S]*uploaded[\s\S]*upload_failed/);
	assert.match(sharedBatch, /ready[\s\S]*rejected/);
	assert.match(sharedBatch, /for \(const row[\s\S]*await uploadSchoolFontFile/);
	assert.match(fontBatch, /inspectCertificateFontUploads/);
	assert.match(fontBatch, /attachCertificateFontBatch/);
	assert.match(sharedBatch, /fontFamily/);
	assert.match(sharedBatch, /fontWeight/);
	assert.match(sharedBatch, /fontStyle/);
	assert.match(sharedBatch, /status/);
	assert.match(sharedBatch, /retryRow/);
	assert.match(sharedBatch, /removeLocalRow/);
	assert.match(sharedBatch, /cleanupTemporary/);
	assert.match(sharedBatch, /metadata\.id/);
	assert.match(assets, /handleSchoolFontsAttached\(items:[^)]*\)[\s\S]*attachedIds/);
	assert.match(
		assets,
		/schoolFonts = \[\.\.\.schoolFonts\.filter\([\s\S]*\.\.\.items\]/,
		'newly attached fonts must patch the already loaded shared catalog'
	);
	assert.match(assets, /onattached=\{handleSchoolFontsAttached\}/);
	assert.doesNotMatch(sharedBatch, /fontWeight\s*:/, 'the browser must not submit a manual weight');
	assert.doesNotMatch(sharedBatch, /Select\.Root/);
	assert.doesNotMatch(assets, /ฟอนต์ที่แนบแล้ว|ลบฟอนต์/);
});
