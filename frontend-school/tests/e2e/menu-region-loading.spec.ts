import { expect, test } from '@playwright/test';
import {
	mockMenu,
	workspacePath,
	groupPath,
	itemPath,
	previewPath
} from './fixtures/menu-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
});
const path = '/staff/menu';
test('ready menu items do not wait for workspace labels and template is lazy', async ({ page }) => {
	const api = await mockMenu(page, { hold: workspacePath });
	await page.goto(path);
	await expect(page.getByTestId('menu-items')).toContainText('เมนูการสอน');
	await expect(
		page.getByRole('status', { name: 'กำลังโหลดกลุ่มบริหาร', exact: true })
	).toBeVisible();
	expect(api.count(previewPath)).toBe(0);
	expect(api.count(groupPath)).toBe(1);
	expect(api.count(itemPath)).toBe(1);
	api.release();
	await expect(page.getByTestId('menu-items')).toContainText('กลุ่มวิชาการ');
});
test('workspace tab remains usable while items are pending', async ({ page }) => {
	const api = await mockMenu(page, { hold: itemPath });
	await page.goto(path);
	await page.getByRole('tab', { name: 'กลุ่มบริหาร', exact: true }).click();
	await expect(page.getByTestId('menu-workspaces')).toContainText('กลุ่มวิชาการ');
	api.release();
});
for (const [endpoint, label] of [
	[workspacePath, 'กลุ่มบริหาร'],
	[groupPath, 'ฝ่าย/งาน'],
	[itemPath, 'เมนูบริการ']
])
	test(`${label} retries only its failed owner`, async ({ page }) => {
		const api = await mockMenu(page, { fail: endpoint });
		await page.goto(path);
		await page.getByRole('button', { name: `ลองโหลด${label}อีกครั้ง`, exact: true }).click();
		await expect(page.getByTestId('menu-items')).toContainText('เมนูการสอน');
		await expect.poll(() => api.count(endpoint)).toBe(2);
		for (const other of [workspacePath, groupPath, itemPath].filter((v) => v !== endpoint))
			expect(api.count(other)).toBe(1);
	});
test('closed template preview does not replace a reopened session', async ({ page }) => {
	const api = await mockMenu(page, { hold: previewPath });
	await page.goto(path);
	await page.getByRole('button', { name: 'ใช้โครงสร้างงานวิชาการแนะนำ', exact: true }).click();
	await expect(page.getByRole('status', { name: 'กำลังโหลดตัวอย่างโครงสร้างเมนู' })).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('button', { name: 'ใช้โครงสร้างงานวิชาการแนะนำ', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText('งานใหม่ 2');
	api.release();
	await expect(page.getByRole('dialog')).toContainText('งานใหม่ 2');
});
test('typed item update patches item list without administration catalog rereads', async ({
	page
}) => {
	const api = await mockMenu(page);
	await page.goto(path);
	await page.getByRole('button', { name: 'แก้ไขเมนู เมนูการสอน', exact: true }).click();
	await page.getByLabel('ชื่อเมนู *', { exact: true }).fill('เมนูแก้ไข');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByTestId('menu-items')).toContainText('เมนูแก้ไข');
	for (const endpoint of [workspacePath, groupPath, itemPath]) expect(api.count(endpoint)).toBe(1);
});
test('late closed update patches the list but keeps the new editor draft', async ({ page }) => {
	const api = await mockMenu(page, { hold: 'mutation' });
	await page.goto(path);
	await page.getByRole('button', { name: 'แก้ไขเมนู เมนูการสอน', exact: true }).click();
	await page.getByLabel('ชื่อเมนู *', { exact: true }).fill('เมนูแก้ไข');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('button', { name: 'แก้ไขเมนู เมนูการสอน', exact: true }).click();
	await page.getByLabel('ชื่อเมนู *', { exact: true }).fill('ร่างใหม่');
	api.release();
	await expect(page.getByTestId('menu-items')).toContainText('เมนูแก้ไข');
	await expect(page.getByLabel('ชื่อเมนู *', { exact: true })).toHaveValue('ร่างใหม่');
});
test('group deletion refreshes moved items only', async ({ page }) => {
	const api = await mockMenu(page);
	await page.goto(path);
	await page.getByRole('tab', { name: 'ฝ่าย/งาน', exact: true }).click();
	await page
		.getByRole('listitem')
		.filter({ hasText: 'งานสอน' })
		.getByRole('button', { name: 'แก้ไข', exact: true })
		.click();
	page.once('dialog', (dialog) => dialog.accept());
	await page.getByRole('dialog').getByRole('button', { name: 'ลบ', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('tab', { name: 'เมนูบริการ', exact: true }).click();
	await expect(page.getByTestId('menu-items')).toContainText('เมนูการสอน');
	expect(api.count(itemPath)).toBe(2);
	expect(api.count(groupPath)).toBe(1);
	expect(api.count(workspacePath)).toBe(1);
});
test('read-only visit has no mutation or template requests', async ({ page }) => {
	const api = await mockMenu(page, { permissions: ['menu.read.all'] });
	await page.goto(path);
	await expect(page.getByTestId('menu-items')).toContainText('เมนูการสอน');
	await expect(page.getByRole('button', { name: 'แก้ไขเมนู เมนูการสอน', exact: true })).toHaveCount(
		0
	);
	expect(api.writes).toHaveLength(0);
	expect(api.count(previewPath)).toBe(0);
});
test('pending menu route does not restore disposed content', async ({ page }) => {
	const api = await mockMenu(page, { hold: itemPath });
	await page.goto(path);
	await expect(
		page.getByRole('status', { name: 'กำลังโหลดเมนูบริการ', exact: true })
	).toBeVisible();
	await navigate(page, '/staff');
	await expect(page).toHaveURL(/\/staff(?:\?|$)/);
	await expect(page.getByTestId('menu-items')).toHaveCount(0);
	api.release();
	await expect(page.getByTestId('menu-items')).toHaveCount(0);
});
test('workspace deletion refreshes moved groups without items reread', async ({ page }) => {
	const api = await mockMenu(page);
	await page.goto(path);
	await page.getByRole('tab', { name: 'กลุ่มบริหาร', exact: true }).click();
	await page
		.getByRole('listitem')
		.filter({ hasText: 'กลุ่มวิชาการ' })
		.getByRole('button', { name: 'แก้ไข', exact: true })
		.click();
	page.once('dialog', (dialog) => dialog.accept());
	await page.getByRole('dialog').getByRole('button', { name: 'ลบ', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.getByRole('tab', { name: 'ฝ่าย/งาน', exact: true }).click();
	await expect(page.getByTestId('menu-groups')).toContainText('งานสอน');
	expect(api.count(groupPath)).toBe(2);
	expect(api.count(itemPath)).toBe(1);
	expect(api.count(workspacePath)).toBe(1);
});
test('failed workspace reorder retries only the authoritative workspace catalog', async ({
	page
}) => {
	const api = await mockMenu(page, { fail: 'mutation' });
	await page.goto(path);
	await page.getByRole('tab', { name: 'กลุ่มบริหาร', exact: true }).click();
	const source = page.getByRole('listitem').filter({ hasText: 'กลุ่มบริหารทั่วไป' }),
		target = page.getByRole('listitem').filter({ hasText: 'กลุ่มวิชาการ' });
	const transfer = await page.evaluateHandle(() => new DataTransfer());
	await source.dispatchEvent('dragstart', { dataTransfer: transfer });
	await target.dispatchEvent('dragenter', { dataTransfer: transfer });
	await source.dispatchEvent('dragend', { dataTransfer: transfer });
	await expect.poll(() => api.count(workspacePath)).toBe(2);
	await expect(page.getByTestId('menu-workspaces').getByRole('listitem').first()).toContainText(
		'กลุ่มบริหารทั่วไป'
	);
	expect(api.count(groupPath)).toBe(1);
	expect(api.count(itemPath)).toBe(1);
});
test('typed workspace creation does not reread the administration catalogs', async ({ page }) => {
	const api = await mockMenu(page);
	await page.goto(path);
	await page.getByRole('tab', { name: 'กลุ่มบริหาร', exact: true }).click();
	await page.getByRole('button', { name: 'สร้างกลุ่มบริหาร', exact: true }).click();
	await page.getByLabel('รหัส *', { exact: true }).fill('new');
	await page.getByLabel('ชื่อกลุ่มบริหาร *', { exact: true }).fill('กลุ่มใหม่');
	await page.getByRole('dialog').getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByTestId('menu-workspaces')).toContainText('กลุ่มใหม่');
	for (const endpoint of [workspacePath, groupPath, itemPath]) expect(api.count(endpoint)).toBe(1);
});
