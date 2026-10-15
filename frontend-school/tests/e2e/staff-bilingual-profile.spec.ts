import { expect, test } from '@playwright/test';
import { mockStaffDirectory, staffPath } from './fixtures/staff-directory-route-data';
import { id } from './fixtures/staff-home-route-data';
import { personnelInfo } from './fixtures/staff-career-route-data';
test.use({ serviceWorkers: 'block' });
const qualifications = personnelInfo({
	education_revision: 4,
	highest_education_level: 'master',
	educations: [
		{ id: id(92), education_level: 'master', major: 'Science', university: 'Example University' },
		{ id: id(93), education_level: 'bachelor', major: 'Mathematics', university: 'Example College' }
	]
});
const english = {
	title: null,
	first_name: '',
	last_name: '',
	title_en: 'Ms.',
	first_name_en: 'Alice',
	last_name_en: 'Example',
	display_name: 'Ms. Alice Example',
	has_complete_name: true
};

test('English-only creation requires a whole language set and reviews all qualifications', async ({
	page
}) => {
	const api = await mockStaffDirectory(page, { profile: english });
	await page.goto('/staff/manage/new');
	await page.getByLabel('คำนำหน้า (ไทย)', { exact: true }).fill('นาย');
	await page.getByLabel('ชื่อ (ไทย)', { exact: true }).fill('ชื่อบางส่วน');
	await page.getByLabel('Last name (English)', { exact: true }).fill('Example');
	await page.locator('input[type=password]').nth(0).fill('Synthetic-123!');
	await page.locator('input[type=password]').nth(1).fill('Synthetic-123!');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('อย่างน้อยหนึ่งภาษา');
	expect(api.writes).toHaveLength(0);
	await page.getByLabel('Title (English)', { exact: true }).fill('Ms.');
	await page.getByLabel('First name (English)', { exact: true }).fill('Alice');
	await expect(page.getByTestId('staff-name-fields')).toContainText('Ms. Alice Example');
	for (const [index, label] of ['ปริญญาโท', 'ปริญญาตรี'].entries()) {
		await page.getByRole('button', { name: 'เพิ่มวุฒิการศึกษา', exact: true }).click();
		await page
			.getByRole('button', { name: `ระดับวุฒิ รายการที่ ${index + 1}`, exact: true })
			.click();
		await page.getByRole('option', { name: label, exact: true }).click();
	}
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: 'บทบาทตัวเลือก staff ระดับ 10', exact: true }).click();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: '+ เพิ่มหน่วยงาน', exact: true }).click();
	await page.getByRole('button', { name: 'เลือกหน่วยงาน', exact: true }).click();
	await page.getByRole('option', { name: /สังกัด หน่วยงานตัวเลือก/ }).click();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	const review = page.getByTestId('staff-create-review');
	await expect(review).toContainText('Ms. Alice Example');
	const text = await review.innerText();
	expect(text.indexOf('ปริญญาตรี')).toBeLessThan(text.indexOf('ปริญญาโท'));
	const request = page.waitForRequest(
		(r) => r.method() === 'POST' && new URL(r.url()).pathname === '/api/staff'
	);
	await page.getByRole('button', { name: 'สร้างบุคลากร', exact: true }).click();
	const body = (await request).postDataJSON();
	expect(body).toMatchObject({
		title: 'นาย',
		first_name: 'ชื่อบางส่วน',
		last_name: '',
		title_en: 'Ms.',
		first_name_en: 'Alice',
		last_name_en: 'Example'
	});
	expect(body.staff_info.educations).toHaveLength(2);
	await expect(page).toHaveURL(/\/staff\/manage\/55000000/);
});

test('name editor prefers complete Thai and explicitly clears a Thai prefix to select English', async ({
	page
}) => {
	await mockStaffDirectory(page, {
		profile: {
			...english,
			title: 'นาย',
			first_name: 'กมล',
			last_name: 'ทดสอบ',
			display_name: 'นายกมล ทดสอบ'
		}
	});
	await page.goto(staffPath(undefined, '/edit'));
	const names = page.getByTestId('staff-name-fields');
	await expect(names).toContainText('ชื่อที่แสดง: นายกมล ทดสอบ');
	await page.getByLabel('คำนำหน้า (ไทย)', { exact: true }).fill('');
	await expect(names).toContainText('ชื่อที่แสดง: Ms. Alice Example');
	const request = page.waitForRequest(
		(r) => r.method() === 'PUT' && r.url().includes('/api/staff/')
	);
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	expect((await request).postDataJSON()).toEqual({ title: '' });
});

test('qualification removal confirms and a conflict retains the draft until reconciled', async ({
	page
}) => {
	await mockStaffDirectory(page, { staffInfo: qualifications });
	await page.goto(staffPath(undefined, '/edit') + '?section=education');
	await page.getByRole('button', { name: 'ลบวุฒิที่ 1', exact: true }).click();
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await expect(page.getByRole('button', { name: /ระดับวุฒิ รายการที่/ })).toHaveCount(2);
	await page.getByRole('button', { name: 'ลบวุฒิที่ 1', exact: true }).click();
	await page.getByRole('button', { name: 'นำออก', exact: true }).click();
	await page.route('**/api/staff/*', (r) =>
		r.request().method() === 'PUT'
			? r.fulfill({ status: 409, json: { success: false, error: 'รายการวุฒิเปลี่ยนแปลงแล้ว' } })
			: r.fallback()
	);
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('ร่างของคุณยังอยู่');
	await expect(page.getByRole('button', { name: /ระดับวุฒิ รายการที่/ })).toHaveCount(1);
	await expect(page.getByLabel('สาขาวิชา', { exact: true })).toHaveValue('Mathematics');
});

test('create edit and complete profile fit mobile and desktop in both themes', async ({
	browser
}, testInfo) => {
	test.setTimeout(60000);
	for (const width of [390, 1440])
		for (const theme of ['light', 'dark'] as const) {
			const context = await browser.newContext({
				baseURL: testInfo.project.use.baseURL,
				viewport: { width, height: 1000 },
				colorScheme: theme,
				serviceWorkers: 'block'
			});
			const page = await context.newPage();
			const pageErrors: string[] = [];
			page.on('pageerror', (error) => pageErrors.push(error.message));
			await mockStaffDirectory(page, {
				staffInfo: qualifications,
				profile: {
					...english,
					nickname: 'Alice',
					email: 'alice@example.invalid',
					phone: '000-000-0000',
					emergency_contact: '000-000-0001',
					line_id: 'example',
					address: 'Synthetic address',
					date_of_birth: '1990-01-01',
					hired_date: '2020-01-01'
				}
			});
			for (const [label, path] of [
				['detail', staffPath()],
				['edit', staffPath(undefined, '/edit')],
				['new', '/staff/manage/new']
			] as const) {
				await page.goto(path);
				await expect(
					page.getByTestId(
						label === 'detail'
							? 'staff-profile'
							: label === 'edit'
								? 'staff-edit-profile'
								: 'staff-create-form'
					)
				).toBeVisible();
				await page.evaluate(async (mode) => {
					document.documentElement.classList.toggle('dark', mode === 'dark');
					await document.fonts.ready;
					await new Promise<void>((r) =>
						requestAnimationFrame(() => requestAnimationFrame(() => r()))
					);
				}, theme);
				if (label !== 'detail') await expect(page.getByTestId('staff-name-fields')).toBeVisible();
				if (label === 'detail') {
					await expect(
						page.getByRole('heading', { name: 'Ms. Alice Example', exact: true })
					).toBeVisible();
					await expect(page.getByText('Synthetic address', { exact: true })).toBeVisible();
					await expect(page.getByText('Mathematics', { exact: true })).toBeVisible();
					const degrees = await page.locator('ol').innerText();
					expect(degrees.indexOf('ปริญญาตรี')).toBeLessThan(degrees.indexOf('ปริญญาโท'));
				}
				expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
					true
				);
				await page.screenshot({
					path: testInfo.outputPath(`${label}-${width}-${theme}.png`),
					fullPage: true,
					animations: 'disabled'
				});
			}
			expect(pageErrors).toEqual([]);
			await context.close();
		}
});
