import { test, expect } from '@playwright/test';
import {
	mockPreparation,
	campaignPath,
	templatesPath,
	candidatesPath,
	templatePath,
	recipientRoute,
	templateRoute,
	editorRoute,
	campaignId,
	secondCampaignId
} from './fixtures/certificate-preparation-route-data';
for (const [route, path, label, ready] of [
	[recipientRoute(), candidatesPath, 'กำลังโหลดรายชื่อผู้รับ', 'ผู้รับ คนแรก'],
	[templateRoute, templatesPath, 'กำลังโหลดแบบเกียรติบัตร', 'แบบแรก'],
	[editorRoute, templatePath, 'กำลังโหลดแบบสำหรับ editor', 'ออกแบบ · แบบแรก']
]) {
	test(`${route}: primary delay has first skeleton and no eager workflow`, async ({ page }) => {
		const api = await mockPreparation(page, { hold: path });
		await page.goto(route);
		await expect(page.getByRole('status', { name: label, exact: true })).toBeVisible();
		expect(api.writes).toEqual([]);
		api.release();
		await expect(page.getByText(ready, { exact: true }).first()).toBeVisible();
		expect(api.count(path)).toBe(1);
	});
}
test('recipients render while both independent references are pending', async ({ page }) => {
	const api = await mockPreparation(page, { hold: templatesPath });
	await page.goto(recipientRoute());
	await expect(page.getByText('ผู้รับ คนแรก', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'นำเข้า Excel/CSV', exact: true })).toHaveCount(0);
	expect(api.count(candidatesPath)).toBe(1);
	api.release();
	await expect(page.getByRole('button', { name: 'นำเข้า Excel/CSV', exact: true })).toBeVisible();
	expect(api.writes).toEqual([]);
});
test('templates do not wait for selected campaign', async ({ page }) => {
	const api = await mockPreparation(page, { hold: campaignPath });
	await page.goto(templateRoute);
	await expect(page.getByText('แบบแรก', { exact: true })).toBeVisible();
	expect(api.writes).toEqual([]);
	api.release();
});
for (const [route, path, retry, ready] of [
	[recipientRoute(), templatesPath, 'ลองแบบอีกครั้ง', 'ผู้รับ คนแรก'],
	[recipientRoute(), candidatesPath, 'ลองอีกครั้ง', 'ผู้รับ คนแรก'],
	[templateRoute, templatesPath, 'ลองอีกครั้ง', 'แบบแรก']
]) {
	test(`${route}: retry ${path} only`, async ({ page }) => {
		const api = await mockPreparation(page, { fail: path });
		await page.goto(route);
		await page.getByRole('button', { name: retry, exact: true }).click();
		await expect(page.getByText(ready, { exact: true }).first()).toBeVisible();
		expect(api.count(path)).toBe(2);
		expect(api.count(campaignPath)).toBe(1);
		expect(api.writes).toEqual([]);
	});
}
test('recipient filter history refreshes only candidates', async ({ page }) => {
	const api = await mockPreparation(page);
	await page.goto(recipientRoute());
	await expect(page.getByText('ผู้รับ คนแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: /ต้องตรวจสอบ.*ต้องตัดสินใจ/ }).click();
	await expect(page).toHaveURL(/status=needs_review/);
	await expect(page.getByText('ผู้รับ คนแรก', { exact: true })).toHaveCount(0);
	await page.goBack();
	await expect(page.getByText('ผู้รับ คนแรก', { exact: true })).toBeVisible();
	expect(api.count(candidatesPath)).toBe(3);
	expect(api.count(templatesPath)).toBe(1);
	expect(api.count(campaignPath)).toBe(1);
});
for (const [route, path, refresh, ready] of [
	[recipientRoute(), candidatesPath, 'โหลดรายชื่อใหม่', 'ผู้รับ คนแรก'],
	[templateRoute, templatesPath, 'โหลดแบบใหม่', 'แบบแรก']
]) {
	test(`${route}: retained refresh failure`, async ({ page }) => {
		const api = await mockPreparation(page, { fail: path, failAt: 2 });
		await page.goto(route);
		await expect(page.getByText(ready, { exact: true }).first()).toBeVisible();
		await page.getByRole('button', { name: refresh, exact: true }).click();
		await expect(page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true })).toBeVisible();
		await expect(page.getByText(ready, { exact: true }).first()).toBeVisible();
		expect(api.count(campaignPath)).toBe(1);
	});
}
test('read-only template catalog never starts update references or manifest', async ({ page }) => {
	const api = await mockPreparation(page, { reader: true });
	await page.goto(templateRoute);
	await expect(page.getByText('แบบแรก', { exact: true })).toBeVisible();
	await page.getByRole('link', { name: 'เปิด editor', exact: true }).first().hover({ force: true });
	expect(api.writes).toEqual([]);
	expect(api.count(`${templatePath}/fonts`)).toBe(0);
	await page.goto(editorRoute);
	await expect(page.getByText('ไม่มีสิทธิ์แก้แบบนี้', { exact: true })).toBeVisible();
	expect(api.count(`${templatePath}/variables`)).toBe(0);
	expect(api.writes).toEqual([]);
});
test('editor validates campaign before references or manifest', async ({ page }) => {
	const api = await mockPreparation(page);
	await page.goto(editorRoute.replace(campaignId, secondCampaignId));
	await expect(
		page.getByText('แบบเกียรติบัตรนี้ไม่ได้อยู่ในกิจกรรมตาม URL', { exact: true })
	).toBeVisible();
	expect(api.count(`${templatePath}/fonts`)).toBe(0);
	expect(api.writes).toEqual([]);
});
test('editor reference failure has local retry and preserves opened editor', async ({ page }) => {
	const api = await mockPreparation(page, { fail: `${templatePath}/fonts` });
	await page.goto(editorRoute);
	await expect(page.getByTestId('certificate-editor')).toBeVisible();
	await page.getByRole('button', { name: 'ลองฟอนต์อีกครั้ง', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ลองฟอนต์อีกครั้ง', exact: true })).toHaveCount(0);
	expect(api.count(templatePath)).toBe(1);
	expect(api.writes).toHaveLength(1);
});
test('typed candidate delete patches list and unfiltered summary without GET', async ({ page }) => {
	const api = await mockPreparation(page);
	await page.goto(recipientRoute());
	await expect(page.getByText('ผู้รับ คนแรก', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: /ลบ.*ผู้รับ คนแรก/ }).click();
	await page.getByRole('button', { name: 'ลบรายชื่อ', exact: true }).click();
	await expect(page.getByText('ผู้รับ คนแรก', { exact: true })).toHaveCount(0);
	expect(api.count(candidatesPath)).toBe(1);
	expect(api.count(campaignPath)).toBe(1);
	expect(api.count(templatesPath)).toBe(1);
});
test('typed template save patches only list and closed form stays closed', async ({ page }) => {
	const api = await mockPreparation(page);
	await page.goto(templateRoute);
	await page.getByRole('button', { name: 'แก้ข้อมูล', exact: true }).click();
	await page.getByLabel('ชื่อแบบเกียรติบัตร', { exact: true }).fill('แบบที่แก้แล้ว');
	await page.getByRole('button', { name: 'บันทึกข้อมูลแบบ', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByText('แบบที่แก้แล้ว', { exact: true })).toBeVisible();
	expect(api.count(templatesPath)).toBe(1);
	expect(api.count(campaignPath)).toBe(1);
});

test('read-only recipients cannot open action workflows even with stale capabilities', async ({
	page
}) => {
	const api = await mockPreparation(page, { reader: true });
	await page.goto(recipientRoute());
	await expect(page.getByText('ผู้รับ คนแรก', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'นำเข้า Excel/CSV', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'เพิ่มจากบัญชี', exact: true })).toHaveCount(0);
	expect(api.writes).toEqual([]);
});
