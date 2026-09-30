import { expect, test } from '@playwright/test';
import {
	mockCampaigns,
	campaignsPath,
	yearsPath,
	ownersPath,
	overviewPath,
	campaignId,
	secondCampaignId
} from './fixtures/certificate-campaign-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (r) =>
		r.fulfill({ contentType: 'text/css', body: '' })
	);
});
test('campaign list owns its read and leaves edit and purge workflows unopened', async ({
	page
}) => {
	const api = await mockCampaigns(page, { hold: campaignsPath });
	await page.goto('/staff/certificates');
	await expect(
		page.getByRole('status', { name: 'กำลังโหลดกิจกรรมเกียรติบัตร', exact: true })
	).toBeVisible();
	expect(api.count(yearsPath)).toBe(0);
	expect(api.count(ownersPath)).toBe(0);
	expect(api.writes).toHaveLength(0);
	api.release();
	await expect(page.getByText('กิจกรรมแรก', { exact: true })).toBeVisible();
	expect(api.count(campaignsPath)).toBe(1);
});
test('failed campaign read retries its owner', async ({ page }) => {
	const api = await mockCampaigns(page, { fail: campaignsPath });
	await page.goto('/staff/certificates');
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByText('กิจกรรมแรก', { exact: true })).toBeVisible();
	expect(api.count(campaignsPath)).toBe(2);
	expect(api.count(ownersPath)).toBe(0);
});
test('new campaign independent choices preserve name while one is delayed', async ({ page }) => {
	const api = await mockCampaigns(page, { hold: yearsPath });
	await page.goto('/staff/certificates/new');
	await expect(page.getByTestId('campaign-years').getByRole('status')).toBeVisible();
	await expect(page.getByTestId('campaign-owners').getByRole('status')).toHaveCount(0);
	await expect.poll(() => api.count(ownersPath)).toBe(1);
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('ร่างระหว่างรอ');
	await expect(page.getByRole('button', { name: 'สร้างกิจกรรม', exact: true })).toBeDisabled();
	api.release();
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('ร่างระหว่างรอ');
	await expect(page.getByRole('button', { name: 'สร้างกิจกรรม', exact: true })).toBeEnabled();
	expect(api.count(yearsPath)).toBe(1);
	expect(api.count(campaignsPath)).toBe(0);
});
test('new campaign retries failed choices without resetting draft or refetching successful choices', async ({
	page
}) => {
	const api = await mockCampaigns(page, { fail: ownersPath });
	await page.goto('/staff/certificates/new');
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('ร่างที่เก็บไว้');
	await page.getByRole('button', { name: 'ลองโหลดหน่วยงานอีกครั้ง', exact: true }).click();
	await expect(page.getByRole('button', { name: 'สร้างกิจกรรม', exact: true })).toBeEnabled();
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('ร่างที่เก็บไว้');
	expect(api.count(yearsPath)).toBe(1);
	expect(api.count(ownersPath)).toBe(2);
});
test('overview is ready before edit references and opens both together', async ({ page }) => {
	const api = await mockCampaigns(page, { hold: yearsPath });
	await page.goto(overviewPath());
	await expect(
		page.getByRole('heading', { name: 'กิจกรรมแรก', exact: true }).first()
	).toBeVisible();
	expect(api.count(yearsPath)).toBe(0);
	expect(api.count(ownersPath)).toBe(0);
	await page.getByRole('button', { name: 'แก้ข้อมูล', exact: true }).click();
	await expect(page.getByRole('dialog')).toBeVisible();
	await expect.poll(() => api.count(ownersPath)).toBe(1);
	await expect(page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })).toBeDisabled();
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('แก้ระหว่างรอ');
	api.release();
	await expect(page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })).toBeEnabled();
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('แก้ระหว่างรอ');
});
test('overview editor focused retry retains draft', async ({ page }) => {
	const api = await mockCampaigns(page, { fail: ownersPath });
	await page.goto(overviewPath());
	await page.getByRole('button', { name: 'แก้ข้อมูล', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('ร่างแก้ไข');
	await page.getByRole('button', { name: 'ลองโหลดหน่วยงานอีกครั้ง', exact: true }).click();
	await expect(page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })).toBeEnabled();
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('ร่างแก้ไข');
	expect(api.count(yearsPath)).toBe(1);
	expect(api.count(`${campaignsPath}/${campaignId}`)).toBe(1);
});
test('late closed update patches campaign without replacing reopened draft', async ({ page }) => {
	const api = await mockCampaigns(page, { hold: 'mutation' });
	await page.goto(overviewPath());
	await page.getByRole('button', { name: 'แก้ข้อมูล', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('ชื่อที่บันทึก');
	await page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('button', { name: 'แก้ข้อมูล', exact: true }).click();
	await page.getByLabel('ชื่อกิจกรรม', { exact: true }).fill('ร่างใหม่');
	api.release();
	await expect(
		page.getByRole('heading', { name: 'ชื่อที่บันทึก', exact: true }).first()
	).toBeVisible();
	await expect(page.getByLabel('ชื่อกิจกรรม', { exact: true })).toHaveValue('ร่างใหม่');
	expect(api.count(`${campaignsPath}/${campaignId}`)).toBe(1);
});
test('status mutation patches only selected campaign', async ({ page }) => {
	const api = await mockCampaigns(page);
	await page.goto(overviewPath());
	await page.getByRole('button', { name: 'ปิดกิจกรรม', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'เปิดกิจกรรมอีกครั้ง', exact: true })
	).toBeVisible();
	expect(api.count(`${campaignsPath}/${campaignId}`)).toBe(1);
	expect(api.count(yearsPath)).toBe(0);
	expect(api.writes).toHaveLength(1);
});
test('old selected campaign response cannot restore another campaign', async ({ page }) => {
	const api = await mockCampaigns(page, { hold: `${campaignsPath}/${campaignId}` });
	await page.goto(overviewPath());
	await expect(
		page.getByRole('status', { name: 'กำลังโหลดกิจกรรมเกียรติบัตร', exact: true })
	).toBeVisible();
	await navigate(page, overviewPath(secondCampaignId));
	await expect(
		page.getByRole('heading', { name: 'กิจกรรมที่สอง', exact: true }).first()
	).toBeVisible();
	api.release();
	await expect(page.getByRole('heading', { name: 'กิจกรรมแรก', exact: true })).toHaveCount(0);
});
test('issued campaign immutable references need no lookups', async ({ page }) => {
	const api = await mockCampaigns(page, { issued: true });
	await page.goto(overviewPath());
	await page.getByRole('button', { name: 'แก้ข้อมูล', exact: true }).click();
	await expect(page.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })).toBeEnabled();
	expect(api.count(yearsPath)).toBe(0);
	expect(api.count(ownersPath)).toBe(0);
});
test('reader never opens action only choices or purge reads', async ({ page }) => {
	const api = await mockCampaigns(page, { permissions: ['certificate.read.school'] });
	await page.goto(overviewPath());
	await expect(
		page.getByRole('heading', { name: 'กิจกรรมแรก', exact: true }).first()
	).toBeVisible();
	await expect(page.getByRole('button', { name: 'แก้ข้อมูล', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'ลบกิจกรรมถาวร', exact: true })).toHaveCount(0);
	expect(api.count(ownersPath)).toBe(0);
	expect(api.reads).toHaveLength(1);
});
