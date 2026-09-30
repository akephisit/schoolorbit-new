import { expect, test } from '@playwright/test';
import {
	mockRequests,
	queuePath,
	requestId,
	secondRequestId,
	historyPath,
	issuedPath,
	issuedId
} from './fixtures/certificate-request-route-data';
import { campaignId, secondCampaignId } from './fixtures/certificate-campaign-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (r) =>
		r.fulfill({ contentType: 'text/css', body: '' })
	);
});
const selectedPath = `${queuePath}/${requestId}`,
	reviewRoute = `/staff/certificate-requests/${requestId}`;
for (const [route, path, label, ready] of [
	['/staff/certificate-requests', queuePath, 'กำลังโหลดคิวคำขอ', 'คำขอแรก'],
	[reviewRoute, selectedPath, 'กำลังโหลดรายละเอียดคำขอ', 'ผู้รับ รายการแรก'],
	[
		`/staff/certificates/${campaignId}/requests`,
		historyPath(),
		'กำลังโหลดประวัติคำขอ',
		'ส่งโดย ผู้ส่งทดสอบ'
	],
	[
		`/staff/certificates/${campaignId}/issued`,
		issuedPath(),
		'กำลังโหลดใบที่ออกแล้ว',
		'ผู้รับ ใบแรก'
	]
]) {
	test(`${label}: primary is loader owned without optional reads`, async ({ page }) => {
		const api = await mockRequests(page, { hold: path });
		await page.goto(route);
		await expect(page.getByRole('status', { name: label, exact: true })).toBeVisible();
		expect(api.writes).toHaveLength(0);
		api.release();
		await expect(page.getByText(ready, { exact: true }).first()).toBeVisible();
		expect(api.count(path)).toBe(1);
		expect(api.reads).toHaveLength(1);
	});
	test(`${label}: failed primary retries only owner`, async ({ page }) => {
		const api = await mockRequests(page, { fail: path });
		await page.goto(route);
		await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(page.getByText(ready, { exact: true }).first()).toBeVisible();
		expect(api.count(path)).toBe(2);
		expect(api.writes).toHaveLength(0);
	});
}
test('queue filter has URL history and rejects old all response', async ({ page }) => {
	const api = await mockRequests(page, { hold: queuePath });
	await page.goto('/staff/certificate-requests');
	await expect(page.getByRole('status', { name: 'กำลังโหลดคิวคำขอ', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'สถานะคำขอ', exact: true }).click();
	await page.getByRole('option', { name: 'ส่งกลับแล้ว', exact: true }).click();
	await expect(page).toHaveURL(/status=returned/);
	await expect(page.getByText('ไม่มีคำขอในสถานะนี้', { exact: true })).toBeVisible();
	api.release();
	await expect(page.getByText('คำขอแรก', { exact: true })).toHaveCount(0);
	expect(
		api.reads.filter((x) => x.pathname === queuePath).map((x) => x.searchParams.get('status'))
	).toEqual([null, 'returned']);
	await page.goBack();
	await expect(page).toHaveURL(
		(url) => url.pathname === '/staff/certificate-requests' && !url.searchParams.has('status')
	);
	await expect(page.getByText('คำขอแรก', { exact: true })).toBeVisible();
	expect(api.count(queuePath)).toBe(3);
});
test('queue retains rows through focused refresh failure', async ({ page }) => {
	const api = await mockRequests(page, { fail: queuePath, failAt: 2 });
	await page.goto('/staff/certificate-requests');
	await expect(page.getByText('คำขอแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'โหลดใหม่', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true })).toBeVisible();
	await expect(page.getByText('คำขอแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true })).toHaveCount(0);
	expect(api.count(queuePath)).toBe(3);
});
test('typed review start patches detail without catalogs or manifest', async ({ page }) => {
	const api = await mockRequests(page);
	await page.goto(reviewRoute);
	await page.getByRole('button', { name: 'เริ่มตรวจคำขอ', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'ออกเกียรติบัตร 1 ใบ', exact: true })
	).toBeVisible();
	expect(api.count(selectedPath)).toBe(1);
	expect(api.writes).toHaveLength(1);
});
test('late selected review response cannot restore new request', async ({ page }) => {
	const api = await mockRequests(page, { hold: selectedPath });
	await page.goto(reviewRoute);
	await expect(
		page.getByRole('status', { name: 'กำลังโหลดรายละเอียดคำขอ', exact: true })
	).toBeVisible();
	await navigate(page, `/staff/certificate-requests/${secondRequestId}`);
	await expect(page.getByText('ผู้รับ รายการสอง', { exact: true })).toBeVisible();
	api.release();
	await expect(page.getByText('ผู้รับ รายการแรก', { exact: true })).toHaveCount(0);
});
test('read-only campaign history loads without submission and hides withdrawal', async ({
	page
}) => {
	const api = await mockRequests(page, { permissions: ['certificate.read.school'] });
	await page.goto(`/staff/certificates/${campaignId}/requests`);
	await expect(page.getByText('ส่งโดย ผู้ส่งทดสอบ', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ถอนคำขอ', exact: true })).toHaveCount(0);
	expect(api.count(historyPath())).toBe(1);
});
test('withdrawal patches only campaign history', async ({ page }) => {
	const api = await mockRequests(page);
	await page.goto(`/staff/certificates/${campaignId}/requests`);
	await page.getByRole('button', { name: 'ถอนคำขอ', exact: true }).click();
	await expect(page.getByText('ถอนแล้ว', { exact: true })).toBeVisible();
	expect(api.count(historyPath())).toBe(1);
	expect(api.writes).toHaveLength(1);
});
test('late withdrawal cannot patch another campaign', async ({ page }) => {
	const api = await mockRequests(page, { hold: 'mutation' });
	await page.goto(`/staff/certificates/${campaignId}/requests`);
	await page.getByRole('button', { name: 'ถอนคำขอ', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await navigate(page, `/staff/certificates/${secondCampaignId}/requests`);
	await expect(page.getByRole('button', { name: 'ถอนคำขอ', exact: true })).toBeVisible();
	api.release();
	await expect(page.getByText('ถอนแล้ว', { exact: true })).toHaveCount(0);
});
test('issued retained refresh preserves table and retries no optional workflows', async ({
	page
}) => {
	const api = await mockRequests(page, { fail: issuedPath(), failAt: 2 });
	await page.goto(`/staff/certificates/${campaignId}/issued`);
	await expect(page.getByText('ผู้รับ ใบแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'โหลดใหม่', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true })).toBeVisible();
	await expect(page.getByText('ผู้รับ ใบแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	expect(api.writes).toHaveLength(0);
	await expect(page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true })).toHaveCount(0);
	expect(api.count(issuedPath())).toBe(3);
});
test('typed revoke patches issued row only', async ({ page }) => {
	const api = await mockRequests(page);
	await page.goto(`/staff/certificates/${campaignId}/issued`);
	await page.getByRole('button', { name: 'เพิกถอน 2569-0001-000001-0', exact: true }).click();
	await page.getByRole('dialog').getByRole('textbox').fill('เหตุผลทดสอบ');
	await page.getByRole('button', { name: 'ยืนยันเพิกถอน', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(
		page.getByRole('button', { name: 'ดาวน์โหลด 2569-0001-000001-0', exact: true })
	).toHaveCount(0);
	expect(api.count(issuedPath())).toBe(1);
	expect(api.writes).toEqual([`/api/certificates/${issuedId}/revoke`]);
});
test('disposed download cannot load renderer after held manifest', async ({ page }) => {
	const api = await mockRequests(page, { hold: 'manifest' });
	const assets: string[] = [];
	page.on('request', (r) => {
		if (/renderer-browser|pdfmake|pdf-lib|pdfjs/.test(r.url())) assets.push(r.url());
	});
	await page.goto(`/staff/certificates/${campaignId}/issued`);
	await page.getByRole('button', { name: 'ดาวน์โหลด 2569-0001-000001-0', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await navigate(page, '/staff');
	await expect(page.getByText('ผู้รับ ใบแรก', { exact: true })).toHaveCount(0);
	api.release();
	expect(assets).toHaveLength(0);
});
test('campaign reader cannot start school issue queue', async ({ page }) => {
	const api = await mockRequests(page, { permissions: ['certificate.read.school'] });
	await page.goto('/staff/certificate-requests');
	await expect(page).toHaveURL(/\/403/);
	expect(api.count(queuePath)).toBe(0);
});
