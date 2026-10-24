import { expect, test, type Page } from '@playwright/test';
import { PDFDocument } from 'pdf-lib';
import { mockStaffDirectory, staffPath, firstStaff } from './fixtures/staff-directory-route-data';
import { personnelInfo } from './fixtures/staff-career-route-data';
import { id } from './fixtures/staff-home-route-data';
import type { components } from '../../src/lib/api/generated/school-api';
test.use({ serviceWorkers: 'block' });
const permissions = ['staff_profile.read.school', 'staff_pii.read.school', 'staff.update.all'];
const fileId = id(96),
	previousId = id(97);
async function mockFiles(page: Page, kind: 'pdf' | 'image' = 'pdf', failUpload = false) {
	const pdf = await PDFDocument.create();
	for (let i = 1; i <= 3; i++)
		pdf.addPage([300, 400]).drawText(`Licence page ${i}`, { x: 30, y: 300, size: 20 });
	const bytes =
		kind === 'pdf'
			? Buffer.from(await pdf.save())
			: Buffer.from(
					'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZ9sAAAAASUVORK5CYII=',
					'base64'
				);
	const mime = kind === 'pdf' ? 'application/pdf' : 'image/png';
	const file: components['schemas']['FileMetadata'] = {
		id: fileId,
		byteSize: bytes.length,
		currentVersion: 1,
		detectedMimeType: mime,
		displayFilename: kind === 'pdf' ? 'licence.pdf' : 'licence.png',
		lifecycleStatus: 'ready',
		publicContentUrl: null,
		purpose: 'teaching_license'
	};
	const uploads: string[] = [];
	await page.route(
		(url) => url.pathname === '/api/files' || url.pathname.startsWith('/api/files/'),
		async (route) => {
			const request = route.request(),
				url = new URL(request.url());
			if (request.method() === 'POST' && url.pathname === '/api/files') {
				uploads.push(request.postData() ?? '');
				if (failUpload)
					return route.fulfill({
						status: 503,
						json: { success: false, error: 'อัปโหลดไม่สำเร็จ' }
					});
				expect(request.postDataBuffer()?.toString()).toContain('teaching_license');
				expect(request.postDataBuffer()?.toString()).toContain(firstStaff);
				return route.fulfill({ json: { success: true, data: file } });
			}
			expect(url.searchParams.get('resource_id')).toBe(firstStaff);
			return route.fulfill({
				json: {
					success: true,
					data: url.pathname.endsWith('/download')
						? { url: `${url.origin}/synthetic-licence-file`, expiresAt: '2099-01-01T00:00:00Z' }
						: file
				}
			});
		}
	);
	await page.route('**/synthetic-licence-file', (route) =>
		route.fulfill({ body: bytes, contentType: mime })
	);
	return { bytes, mime, uploads };
}
test('title selectors preserve custom historical values and allow both languages and clearing', async ({
	page
}) => {
	await mockStaffDirectory(page, {
		profile: { title: 'พล.ต.', title_en: 'Dr.', first_name_en: 'Alice', last_name_en: 'Example' }
	});
	await page.goto(staffPath(undefined, '/edit'));
	await expect(page.getByLabel('ระบุ คำนำหน้า (ไทย)', { exact: true })).toHaveValue('พล.ต.');
	await page.getByRole('combobox', { name: 'คำนำหน้า (ไทย)', exact: true }).click();
	await page.getByRole('option', { name: 'นางสาว', exact: true }).click();
	await expect(page.getByTestId('staff-name-fields')).toContainText('ชื่อที่แสดง: นางสาว');
	await page.getByRole('combobox', { name: 'Title (English)', exact: true }).click();
	await page.getByRole('option', { name: 'Ms.', exact: true }).click();
	await page.getByRole('combobox', { name: 'คำนำหน้า (ไทย)', exact: true }).click();
	await page.getByRole('option', { name: 'ยังไม่ระบุ', exact: true }).click();
	await expect(page.getByTestId('staff-name-fields')).toContainText('Ms. Alice Example');
});
for (const width of [390, 1440])
	for (const theme of ['light', 'dark'] as const) {
		test(`PDF upload, compact qualifications and navigation at ${width} ${theme}`, async ({
			browser
		}, info) => {
			test.setTimeout(60_000);
			const context = await browser.newContext({
				baseURL: info.project.use.baseURL,
				viewport: { width, height: 1000 },
				colorScheme: theme
			});
			const page = await context.newPage(),
				errors: string[] = [];
			page.on('pageerror', (error) => errors.push(error.message));
			await mockStaffDirectory(page, {
				permissions,
				staffInfo: personnelInfo({
					teaching_license_file_id: previousId,
					educations: [
						{
							id: id(95),
							education_level: 'bachelor',
							major: 'Mathematics',
							university: 'Example College'
						}
					]
				})
			});
			const fixture = await mockFiles(page);
			await page.goto(staffPath(undefined, '/edit?section=education'));
			const row = page.getByRole('group', { name: 'วุฒิที่ 1', exact: true });
			const deleteBox = await row
				.getByRole('button', { name: 'ลบวุฒิที่ 1', exact: true })
				.boundingBox();
			const levelBox = await row
				.getByRole('combobox', { name: 'ระดับวุฒิ รายการที่ 1', exact: true })
				.boundingBox();
			expect(deleteBox && levelBox && levelBox.y - (deleteBox.y + deleteBox.height)).toBeLessThan(
				40
			);
			await page
				.getByLabel('เปลี่ยนไฟล์ใบประกอบวิชาชีพ', { exact: true })
				.setInputFiles({ name: 'licence.pdf', mimeType: fixture.mime, buffer: fixture.bytes });
			await expect(page.getByText('licence.pdf', { exact: true })).toBeVisible();
			expect(fixture.uploads).toHaveLength(0);
			await row.screenshot({
				path: info.outputPath(`education-${width}-${theme}.png`),
				animations: 'disabled'
			});
			await page.getByRole('button', { name: 'ดูใบประกอบวิชาชีพ', exact: true }).click();
			const dialog = page.getByRole('dialog');
			await expect(dialog.getByTestId('licence-pdf-pages').locator('canvas')).toHaveCount(
				width < 768 ? 1 : 2
			);
			await expect(
				dialog.getByText(width < 768 ? 'หน้า 1 / 3' : 'หน้า 1–2 / 3', { exact: true })
			).toBeVisible();
			await expect(dialog.getByRole('button', { name: 'หน้าถัดไป', exact: true })).toBeEnabled();
			await expect
				.poll(() =>
					dialog
						.locator('canvas')
						.first()
						.evaluate((canvas: HTMLCanvasElement) => canvas.width)
				)
				.toBeGreaterThan(0);
			await page.evaluate(async () => {
				await document.fonts.ready;
			});
			await page.screenshot({
				path: info.outputPath(`licence-${width}-${theme}.png`),
				animations: 'disabled'
			});
			await dialog.getByRole('button', { name: 'หน้าถัดไป', exact: true }).click();
			await expect(
				dialog.getByText(width < 768 ? 'หน้า 2 / 3' : 'หน้า 3 / 3', { exact: true })
			).toBeVisible();
			await page.keyboard.press('Escape');
			const saved = page.waitForRequest(
				(request) =>
					request.method() === 'PUT' &&
					new URL(request.url()).pathname === `/api/staff/${firstStaff}`
			);
			await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
			expect((await saved).postDataJSON()).toEqual({
				staff_info: { teaching_license_file: { expected_file_id: previousId, file_id: fileId } }
			});
			expect(fixture.uploads).toHaveLength(1);
			expect(errors).toEqual([]);
			await context.close();
		});
	}
test('one image preview, failed upload retains saved file and removal confirms', async ({
	page
}) => {
	await mockStaffDirectory(page, {
		permissions,
		staffInfo: personnelInfo({ teaching_license_file_id: previousId })
	});
	const fixture = await mockFiles(page, 'image', true);
	await page.goto(staffPath(undefined, '/edit?section=education'));
	await page
		.getByLabel('เปลี่ยนไฟล์ใบประกอบวิชาชีพ', { exact: true })
		.setInputFiles({ name: 'licence.png', mimeType: fixture.mime, buffer: fixture.bytes });
	expect(fixture.uploads).toHaveLength(0);
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('อัปโหลดไม่สำเร็จ');
	expect(fixture.uploads).toHaveLength(1);
	await expect(
		page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true })
	).toBeEnabled();
	await page.getByRole('button', { name: 'ดูใบประกอบวิชาชีพ', exact: true }).click();
	await expect(
		page.getByRole('dialog').getByRole('img', { name: 'ใบประกอบวิชาชีพที่แนบ', exact: true })
	).toBeVisible();
	await expect(page.getByRole('dialog').locator('img')).toHaveCount(1);
	await expect(page.getByRole('dialog').locator('canvas')).toHaveCount(0);
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'นำเอกสารใบประกอบวิชาชีพออก', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'นำออก', exact: true }).click();
	const saved = page.waitForRequest(
		(request) =>
			request.method() === 'PUT' && new URL(request.url()).pathname === `/api/staff/${firstStaff}`
	);
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	expect((await saved).postDataJSON().staff_info.teaching_license_file).toEqual({
		expected_file_id: previousId,
		file_id: null
	});
});
test('profile-only readers do not render or fetch private licence files', async ({ page }) => {
	const requests: string[] = [];
	page.on('request', (request) => {
		if (request.url().includes('/api/files')) requests.push(request.url());
	});
	await mockStaffDirectory(page, {
		permissions: ['staff_profile.read.school', 'staff.update.all'],
		staffInfo: personnelInfo({ teaching_license_file_id: previousId })
	});
	await page.goto(staffPath(undefined, '/edit?section=education'));
	await expect(
		page.getByRole('heading', { name: 'ตำแหน่งและการศึกษา', exact: true })
	).toBeVisible();
	await expect(
		page.getByRole('region', { name: 'เอกสารใบประกอบวิชาชีพ', exact: true })
	).toHaveCount(0);
	await page.goto(staffPath());
	await expect(page.getByRole('button', { name: 'ดูใบประกอบวิชาชีพ', exact: true })).toHaveCount(0);
	expect(requests).toEqual([]);
});

test('profile crop stays local and a failed profile save retries the known file ID', async ({
	page
}) => {
	await mockStaffDirectory(page, { permissions });
	const fixture = await mockFiles(page, 'image');
	let uploads = 0,
		saves = 0;
	await page.route('**/api/files', async (route) => {
		uploads++;
		expect(route.request().postData()).toContain('profile_image');
		expect(route.request().postData()).toContain(firstStaff);
		await route.fulfill({
			json: {
				success: true,
				data: {
					id: fileId,
					byteSize: fixture.bytes.length,
					currentVersion: 1,
					detectedMimeType: 'image/jpeg',
					displayFilename: 'profile_avatar.jpg',
					lifecycleStatus: 'ready',
					publicContentUrl: null,
					purpose: 'profile_image'
				}
			}
		});
	});
	await page.route(`**/api/staff/${firstStaff}`, async (route) => {
		if (route.request().method() !== 'PUT') return route.fallback();
		saves++;
		expect(route.request().postDataJSON()).toEqual({ profile_image_file_id: fileId });
		await route.fulfill(
			saves === 1
				? { status: 503, json: { success: false, error: 'บันทึกรูปไม่สำเร็จ' } }
				: { json: { success: true, data: {} } }
		);
	});
	await page.goto(staffPath(undefined, '/edit'));
	await page
		.locator('input[type="file"]')
		.setInputFiles({ name: 'avatar.png', mimeType: fixture.mime, buffer: fixture.bytes });
	const crop = page.getByRole('dialog');
	await expect(crop.getByRole('button', { name: 'บันทึกรูปภาพ', exact: true })).toBeEnabled();
	expect(uploads).toBe(0);
	expect(saves).toBe(0);
	await crop.getByRole('button', { name: 'บันทึกรูปภาพ', exact: true }).click();
	const retry = page.getByRole('button', { name: 'ลองบันทึกรูปอีกครั้ง', exact: true });
	await expect(retry).toBeVisible();
	expect(uploads).toBe(1);
	await retry.click();
	await expect(retry).toHaveCount(0);
	expect(saves).toBe(2);
	expect(uploads).toBe(1);
});

test('stale licence save retains draft until reviewer chooses latest or draft', async ({
	page
}) => {
	const staffInfo = personnelInfo({ teaching_license_file_id: previousId });
	await mockStaffDirectory(page, { permissions, staffInfo });
	const fixture = await mockFiles(page);
	await page.goto(staffPath(undefined, '/edit?section=education'));
	await page
		.getByLabel('เปลี่ยนไฟล์ใบประกอบวิชาชีพ', { exact: true })
		.setInputFiles({ name: 'licence.pdf', mimeType: fixture.mime, buffer: fixture.bytes });
	await expect(page.getByText('licence.pdf', { exact: true })).toBeVisible();
	const latestId = id(98);
	let saves = 0;
	await page.route(`**/api/staff/${firstStaff}`, async (route) => {
		if (route.request().method() !== 'PUT') return route.fallback();
		saves++;
		if (saves === 1) {
			staffInfo.teaching_license_file_id = latestId;
			return route.fulfill({ status: 409, json: { success: false, error: 'เอกสารเปลี่ยนแล้ว' } });
		}
		expect(route.request().postDataJSON().staff_info.teaching_license_file).toEqual({
			expected_file_id: latestId,
			file_id: fileId
		});
		return route.fulfill({ json: { success: true, data: {} } });
	});
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	await page.getByRole('button', { name: 'โหลดข้อมูลล่าสุดและตรวจสอบร่าง', exact: true }).click();
	await expect(page.getByText('เอกสารล่าสุดในระบบ', { exact: true })).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true })
	).toBeDisabled();
	await page.getByRole('button', { name: 'ใช้ร่างแทนเอกสารล่าสุด', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	await expect.poll(() => saves).toBe(2);
	expect(fixture.uploads).toHaveLength(1);
});
