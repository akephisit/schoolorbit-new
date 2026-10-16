import { expect, test, type Page } from '@playwright/test';
import { mockStaffDirectory, firstStaff } from './fixtures/staff-directory-route-data';
import { id } from './fixtures/staff-home-route-data';
import type { StaffListItem } from '../../src/lib/api/staff';

test.use({ serviceWorkers: 'block' });
const portraitId = id(95);
const people: StaffListItem[] = [
	{
		id: firstStaff,
		username: 'staff-001',
		title: 'นางสาว',
		first_name: 'กมล',
		display_name: 'นางสาวกมล บุคลากรทดสอบ',
		last_name: 'บุคลากรทดสอบ',
		status: 'active',
		profile_image_file_id: portraitId,
		roles: ['ครูผู้สอน'],
		organization_units: ['กลุ่มสาระการเรียนรู้คณิตศาสตร์'],
		job_position: null,
		academic_rank: null
	},
	{
		id: id(22),
		username: 'staff-002',
		title: 'นาย',
		first_name: 'ขจร',
		display_name: 'นายขจร นามสกุลสำหรับทดสอบการแสดงชื่อที่มีความยาวมากในหน้าจอขนาดเล็ก',
		last_name: 'นามสกุลสำหรับทดสอบการแสดงชื่อที่มีความยาวมากในหน้าจอขนาดเล็ก',
		status: 'active',
		profile_image_file_id: null,
		roles: ['ครูผู้สอน', 'ที่ปรึกษา'],
		organization_units: ['กลุ่มสาระการเรียนรู้วิทยาศาสตร์และเทคโนโลยี'],
		job_position: null,
		academic_rank: null
	}
];
async function setup(page: Page, readOnly = false) {
	const api = await mockStaffDirectory(page, {
		permissions: readOnly ? ['staff_profile.read.school'] : undefined
	});
	await page.route(
		(url) => url.pathname === '/api/staff',
		(route) =>
			route.fulfill({
				json: {
					success: true,
					data: {
						items: people,
						total: 22,
						page: Number(new URL(route.request().url()).searchParams.get('page') ?? 1),
						page_size: 20,
						total_pages: 2
					}
				}
			})
	);
	const grants: URL[] = [];
	await page.route(`**/api/files/${portraitId}/download**`, (route) => {
		grants.push(new URL(route.request().url()));
		return route.fulfill({
			json: {
				success: true,
				data: { url: 'https://images.example.test/portrait.svg', expiresAt: '2026-12-01T00:00:00Z' }
			}
		});
	});
	await page.route('https://images.example.test/portrait.svg', (route) =>
		route.fulfill({
			contentType: 'image/svg+xml',
			body: '<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><rect width="96" height="96" fill="#ddd"/><circle cx="48" cy="35" r="18" fill="#888"/><path d="M12 96a36 36 0 0 1 72 0" fill="#888"/></svg>'
		})
	);
	return { api, grants };
}

test('view switch preserves filters, paging and return navigation without loading profiles', async ({
	page
}) => {
	const { api, grants } = await setup(page);
	let reads = 0;
	page.on('request', (r) => {
		if (new URL(r.url()).pathname === '/api/staff') reads++;
	});
	await page.goto('/staff/manage?search=ครู&page=2&status=active');
	await expect(page.getByRole('table')).toContainText('นางสาวกมล บุคลากรทดสอบ');
	const photo = page.getByRole('img', { name: 'รูป นางสาวกมล บุคลากรทดสอบ' });
	await expect(photo).toBeVisible();
	await expect.poll(() => photo.evaluate((img) => (img as HTMLImageElement).naturalWidth)).toBe(96);
	const initialReads = reads;
	const cards = page.getByRole('radio', { name: 'มุมมองการ์ด' });
	await cards.click();
	await expect(cards).toHaveAttribute('aria-checked', 'true');
	await expect(page).toHaveURL(/view=cards/);
	await expect(page).toHaveURL(/page=2/);
	await expect(page.getByRole('list', { name: 'การ์ดบุคลากร' }).getByRole('listitem')).toHaveCount(
		2
	);
	await cards.click();
	await expect(cards).toHaveAttribute('aria-checked', 'true');
	expect(reads).toBe(initialReads);
	expect(api.reads.some((url) => /^\/api\/staff\/[\da-f-]+$/.test(url.pathname))).toBe(false);
	expect(grants.length).toBeGreaterThan(0);
	expect(grants.every((url) => url.searchParams.get('resource_id') === firstStaff)).toBe(true);
	await page.getByRole('button', { name: 'ก่อนหน้า', exact: true }).click();
	await expect(page).not.toHaveURL(/page=2/);
	await expect(page).toHaveURL(/view=cards/);
	await page.getByRole('link', { name: 'ดูข้อมูล', exact: true }).first().click();
	await page.getByRole('link', { name: 'กลับรายชื่อบุคลากร', exact: true }).click();
	await expect(page).toHaveURL(/view=cards/);
	await expect(page).toHaveURL(/search=/);
	await expect(page.getByRole('list', { name: 'การ์ดบุคลากร' })).toBeVisible();
	await page.getByRole('radio', { name: 'มุมมองรายชื่อ' }).focus();
	await page.keyboard.press('Space');
	await expect(page.getByRole('table')).toBeVisible();
	await page.goBack();
	await expect(page.getByRole('list', { name: 'การ์ดบุคลากร' })).toBeVisible();
	await page.getByRole('button', { name: 'ปิดการใช้งาน', exact: true }).first().click();
	await expect(page.getByRole('dialog')).toContainText('กมล');
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	expect(api.writes).toHaveLength(0);
});

test('directory views wrap names and keep read-only actions in mobile and desktop themes', async ({
	browser
}, testInfo) => {
	for (const width of [390, 1440, 1543]) {
		for (const theme of ['light', 'dark']) {
			const context = await browser.newContext({
				baseURL: testInfo.project.use.baseURL,
				viewport: { width, height: 1000 },
				serviceWorkers: 'block',
				hasTouch: width === 390
			});
			const page = await context.newPage();
			const { api } = await setup(page, true);
			await page.goto('/staff/manage?view=cards');
			await expect(page.getByRole('list', { name: 'การ์ดบุคลากร' })).toBeVisible();
			await page.evaluate(
				(dark) => document.documentElement.classList.toggle('dark', dark),
				theme === 'dark'
			);
			await page.evaluate(async () => {
				await document.fonts.ready;
				await new Promise<void>((r) =>
					requestAnimationFrame(() => requestAnimationFrame(() => r()))
				);
			});
			for (const view of ['cards', 'list']) {
				await page
					.getByRole('radio', { name: view === 'cards' ? 'มุมมองการ์ด' : 'มุมมองรายชื่อ' })
					.click();
				await expect(
					page.getByRole('radio', { name: view === 'cards' ? 'มุมมองการ์ด' : 'มุมมองรายชื่อ' })
				).toHaveAttribute('aria-checked', 'true');
				await expect(
					view === 'cards'
						? page.getByRole('list', { name: 'การ์ดบุคลากร' })
						: page.getByRole('table')
				).toBeVisible();
				await expect(page.getByRole('link', { name: 'แก้ไข', exact: true })).toHaveCount(0);
				await expect(page.getByRole('button', { name: 'ปิดการใช้งาน', exact: true })).toHaveCount(
					0
				);
				expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
					true
				);
				if (view === 'cards') {
					const portrait = page.getByRole('img', {
						name: `รูป ${people[0].display_name}`,
						exact: true
					});
					await expect(portrait).toBeVisible();
					const box = await portrait.boundingBox();
					const rootFont = await page.evaluate(() =>
						Number.parseFloat(getComputedStyle(document.documentElement).fontSize)
					);
					expect(box?.width).toBeCloseTo(9 * rootFont, 1);
					expect(box?.height).toBeCloseTo(9 * rootFont, 1);
				}
				await page.getByTestId('staff-directory').screenshot({
					path: testInfo.outputPath(`directory-${view}-${width}-${theme}.png`),
					animations: 'disabled'
				});
			}
			expect(api.writes).toHaveLength(0);
			await context.close();
		}
	}
});
