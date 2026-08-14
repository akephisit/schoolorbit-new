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
	const [page, list, form, background, assets] = await Promise.all([
		readProjectFile('src/routes/(app)/staff/certificates/[campaignId]/templates/+page.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateTemplateList.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateTemplateForm.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateBackgroundUpload.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateAssetManager.svelte')
	]);

	assert.match(page, /beforeNavigate/);
	assert.match(page, /afterNavigate/);
	assert.match(page, /loadGeneration/);
	assert.match(page, /Date\.parse\([^)]+updatedAt\)/);
	assert.match(page, /formHasPendingUpload/);
	assert.match(list, /pendingUploadKeys/);
	assert.match(list, /hasPendingUpload/);
	for (const component of [form, background, assets]) {
		assert.match(component, /onpendingchange/);
	}
});

test('template files use typed purposes, exact filters, and template resource id', async () => {
	const [background, assets, form] = await Promise.all([
		readProjectFile('src/lib/components/certificates/CertificateBackgroundUpload.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateAssetManager.svelte'),
		readProjectFile('src/lib/components/certificates/CertificateTemplateForm.svelte')
	]);
	const source = `${background}\n${assets}\n${form}`;

	assert.match(background, /accept=["']\.pdf["']/);
	assert.match(assets, /accept=["']\.png,\.jpg,\.jpeg,\.webp["']/);
	assert.match(assets, /accept=["']\.ttf,\.otf["']/);
	assert.match(background, /certificate_template_background/);
	assert.match(assets, /certificate_template_image/);
	assert.match(assets, /certificate_template_font/);
	assert.match(source, /uploadCertificateTemplateFile\([\s\S]*templateId/);
	assert.match(source, /attachCertificateTemplate(Background|Asset)/);
	assert.doesNotMatch(
		source,
		/type=["']number["'][^>]*(width|height)|(width|height)[^>]*type=["']number["']/i
	);
});

test('font rights and failed-attach cleanup remain explicit', async () => {
	const assets = await readProjectFile(
		'src/lib/components/certificates/CertificateAssetManager.svelte'
	);

	assert.match(assets, /rightsConfirmed/);
	assert.match(assets, /ยืนยัน.*สิทธิ์|สิทธิ์.*ฟอนต์/);
	assert.match(assets, /disabled={[^}]*!rightsConfirmed/);
	assert.match(assets, /unattachedFile/);
	assert.match(assets, /deleteFile\([\s\S]*templateId/);
});
