import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

async function source(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

test('campaign purge uses generated contracts and removes the legacy DELETE wrapper', async () => {
	const openapi = JSON.parse(
		await readFile(path.resolve(projectRoot, '../contracts/openapi/school-api.json'), 'utf8')
	);
	for (const [route, method, operationId] of [
		[
			'/api/certificates/campaigns/{campaign_id}/purge-impact',
			'get',
			'getCertificateCampaignPurgeImpact'
		],
		['/api/certificates/campaigns/{campaign_id}/purge', 'post', 'startCertificateCampaignPurge'],
		[
			'/api/certificates/campaigns/{campaign_id}/purge-status',
			'get',
			'getCertificateCampaignPurgeStatus'
		],
		[
			'/api/certificates/campaigns/{campaign_id}/purge/retry',
			'post',
			'retryCertificateCampaignPurge'
		]
	]) {
		assert.equal(openapi.paths?.[route]?.[method]?.operationId, operationId);
	}
	assert.equal(
		openapi.paths?.['/api/certificates/campaigns/{campaign_id}']?.delete,
		undefined,
		'legacy draft-only DELETE route must not remain'
	);

	const wrapper = await source('src/lib/api/certificates.ts');
	for (const schema of [
		'CertificateCampaignPurgeCounts',
		'StartCertificateCampaignPurgeRequest',
		'CertificateCampaignPurgeImpact',
		'CertificateCampaignPurgePhase',
		'CertificateCampaignPurgeStatus'
	]) {
		assert.match(wrapper, new RegExp(`Schemas\\['${schema}'\\]`));
	}
	for (const operation of [
		'getCertificateCampaignPurgeImpact',
		'startCertificateCampaignPurge',
		'getCertificateCampaignPurgeStatus',
		'retryCertificateCampaignPurge'
	]) {
		assert.match(wrapper, new RegExp(`export async function ${operation}\\b`));
	}
	assert.match(wrapper, /options: ApiRequestOptions = \{\}/);
	assert.doesNotMatch(wrapper, /deleteCertificateCampaign/);
});

test('permanent purge dialog explains every impact and owns cancellable polling', async () => {
	const dialog = await source(
		'src/lib/components/certificates/CertificateCampaignPurgeDialog.svelte'
	);
	for (const copy of [
		'ลบกิจกรรมถาวร',
		'แม่แบบ',
		'รายชื่อผู้รับ',
		'คำขอออก',
		'คำขอที่ยังไม่จบ',
		'เกียรติบัตรที่ออกแล้ว',
		'เกียรติบัตรที่เพิกถอน',
		'ไฟล์',
		'พื้นที่ไฟล์',
		'พิมพ์ชื่อกิจกรรม',
		'ย้อนกลับไม่ได้',
		'ตรวจสอบเกียรติบัตรไม่ได้ทันที',
		'ลองลบต่อ'
	]) {
		assert.match(dialog, new RegExp(copy));
	}
	assert.match(dialog, /AbortController/);
	assert.match(dialog, /clearTimeout/);
	assert.match(dialog, /1_500/);
	assert.match(dialog, /getCertificateCampaignPurgeImpact/);
	assert.match(dialog, /startCertificateCampaignPurge/);
	assert.match(dialog, /getCertificateCampaignPurgeStatus/);
	assert.match(dialog, /retryCertificateCampaignPurge/);
	assert.match(dialog, /error\.status === 409/);
	assert.match(dialog, /error\.status === 404/);
	assert.doesNotMatch(dialog, /console\.(?:log|info|debug|warn|error)/);
});

test('campaign overview and list mount purge UI only when needed', async () => {
	const overview = await source(
		'src/routes/(app)/staff/certificates/[campaignId]/overview/+page.svelte'
	);
	assert.match(overview, /CertificateCampaignPurgeDialog/);
	assert.match(overview, /ลบกิจกรรมถาวร/);
	assert.match(overview, /\{#if deleteOpen[^}]*\}[\s\S]*<CertificateCampaignPurgeDialog/);
	assert.doesNotMatch(overview, /deleteCertificateCampaign|AlertDialog/);

	const list = await source('src/lib/components/certificates/CertificateCampaignList.svelte');
	assert.match(list, /purging: 'กำลังลบ'/);
	assert.match(list, /ดูสถานะการลบ/);
	assert.match(list, /CertificateCampaignPurgeDialog/);
	assert.match(list, /onpurged/);
});
