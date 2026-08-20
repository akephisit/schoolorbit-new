import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const projectRoot = new URL('../../', import.meta.url);
const files = [
	'src/lib/components/certificates/CertificateRecipientWorkspace.svelte',
	'src/lib/components/certificates/CertificateImportDialog.svelte',
	'src/lib/components/certificates/CertificateCandidateTable.svelte',
	'src/lib/components/certificates/CertificateCandidateEditDialog.svelte',
	'src/lib/components/certificates/CertificateAccountSearchDialog.svelte',
	'src/lib/components/certificates/CertificateManualExternalDialog.svelte',
	'src/routes/(app)/staff/certificates/[campaignId]/recipients/+page.svelte',
	'src/routes/(app)/staff/certificates/[campaignId]/recipients/+page.ts'
];

test('recipient workspace exposes the complete approved review workflow', async () => {
	const source = (
		await Promise.all(files.map((file) => readFile(new URL(file, projectRoot), 'utf8')))
	).join('\n');
	for (const label of [
		'พร้อมออก',
		'ต้องตรวจสอบ',
		'ข้อมูลไม่ถูกต้อง',
		'นำเข้า Excel/CSV',
		'ดาวน์โหลดไฟล์ตัวอย่าง',
		'เพิ่มจากบัญชี',
		'เพิ่มบุคคลภายนอก',
		'แก้ไขรายชื่อ',
		'กำหนดแบบให้รายการที่เลือก',
		'ใช้ชื่อจากบัญชี',
		'ใช้ชื่อจากไฟล์',
		'ยืนยันเป็นบุคคลภายนอก',
		'บัญชีที่พบแล้วไม่สามารถเปลี่ยนเป็นบุคคลภายนอกได้'
	]) {
		assert.match(source, new RegExp(label), `missing recipient workflow label: ${label}`);
	}
	assert.match(source, /min-w-\[(?:1200|1280|1360)px\]/);
	assert.match(source, /overflow-x-auto/);
	assert.match(source, /capabilities\.canConfirmExternal/);
	assert.match(source, /matchStatus\s*===\s*['"]inactive['"]/);
	assert.match(source, /operation:\s*['"]assign_template['"]/);
	assert.match(source, /operation:\s*['"]choose_name['"]/);
	assert.match(source, /operation:\s*['"]confirm_external['"]/);
	assert.match(source, /operation:\s*['"]confirm_duplicate['"]/);
});

test('recipient route sends only parsed typed rows and uses dedicated account/manual APIs', async () => {
	const source = (
		await Promise.all(files.map((file) => readFile(new URL(file, projectRoot), 'utf8')))
	).join('\n');
	for (const symbol of [
		'parseCertificateImport',
		'importCertificateCandidates',
		'searchCertificateCandidateAccounts',
		'createAccountCertificateCandidate',
		'createManualCertificateCandidate',
		'updateCertificateCandidate',
		'bulkUpdateCertificateCandidates',
		'listCertificateCandidates'
	]) {
		assert.match(source, new RegExp(symbol), `missing typed recipient boundary: ${symbol}`);
	}
	assert.match(source, /importCertificateCandidates\([^,]+,\s*parsed\)/);
	assert.doesNotMatch(source, /(?:FormData|uploadFile|filePlatform|apiClient\.)/);
	assert.doesNotMatch(source, /(?:Record<string, unknown>|ApiResponse<unknown>|\bas any\b)/);
	assert.match(source, /PERMISSIONS\.CERTIFICATE_READ_ORGANIZATION_UNIT/);
	assert.match(source, /PERMISSIONS\.CERTIFICATE_READ_SCHOOL/);
	assert.match(source, /afterNavigate/);
	assert.match(source, /loadGeneration/);
});

test('recipient review invalidates stale route work and keeps external conflicts actionable', async () => {
	const workspace = await readFile(
		new URL('src/lib/components/certificates/CertificateRecipientWorkspace.svelte', projectRoot),
		'utf8'
	);
	const accountDialog = await readFile(
		new URL('src/lib/components/certificates/CertificateAccountSearchDialog.svelte', projectRoot),
		'utf8'
	);
	assert.match(workspace, /candidateLoadGeneration\s*\+=\s*1/);
	assert.match(workspace, /tableLoading\s*=\s*false/);
	for (const reset of [
		'importOpen = false',
		'accountOpen = false',
		'manualOpen = false',
		'editTarget = null',
		'deleteTarget = null'
	]) {
		assert.match(workspace, new RegExp(reset));
	}
	assert.match(workspace, /externalConfirmationIssues/);
	assert.match(workspace, /ApiClientError/);
	assert.match(accountDialog, /searchGeneration/);
});
