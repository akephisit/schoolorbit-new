import { expect, test, type Page } from '@playwright/test';
import { mockStaffDirectory, staffPath } from './fixtures/staff-directory-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
const ref = '55000000-0000-4000-8000-000000000090';
const item = {
	id: ref,
	kind: 'job_position',
	code: 'teacher',
	name: 'ครู',
	isActive: true,
	displayOrder: 10,
	createdAt: '2026-10-01T00:00:00Z',
	updatedAt: '2026-10-01T00:00:00Z'
};
async function setup(page: Page, permissions?: string[]) {
	const api = await mockStaffDirectory(page, { permissions });
	await page.route('**/api/staff/reference-items**', (route) =>
		route.fulfill({
			json: {
				success: true,
				data:
					route.request().method() === 'GET'
						? { items: [item], total: 1, page: 1, pageSize: 25 }
						: item
			}
		})
	);
	await page.route('**/api/staff/personnel-overview**', (route) =>
		route.fulfill({
			json: {
				success: true,
				data: {
					asOf: '2026-10-01T00:00:00Z',
					total: 3,
					active: 2,
					otherStatuses: 1,
					filteredTotal: 2,
					statuses: [
						{ key: 'active', label: 'ปฏิบัติงาน', count: 2 },
						{ key: 'inactive', label: 'ไม่ใช้งาน', count: 1 }
					],
					subjectGroups: [{ key: 'unassigned', label: 'ยังไม่มีสังกัดกลุ่มสาระ', count: 2 }],
					jobPositions: [{ key: ref, label: 'ครู', count: 2 }],
					academicRanks: [{ key: 'none', label: 'ไม่มีวิทยฐานะ', count: 2 }],
					educationLevels: [{ key: 'bachelor', label: 'ปริญญาตรี', count: 2 }]
				}
			}
		})
	);
	return api;
}
test('personnel editor selects standardized degree and sends an HR-only patch', async ({
	page
}) => {
	await setup(page);
	await page.goto(staffPath(undefined, '/edit') + '?section=education');
	await page.getByRole('button', { name: 'วุฒิการศึกษาสูงสุด', exact: true }).click();
	await page.getByRole('option', { name: 'ปริญญาตรี', exact: true }).click();
	const saved = page.waitForRequest((r) => r.method() === 'PUT' && r.url().includes('/api/staff/'));
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	const payload = (await saved).postDataJSON();
	expect(payload.staff_info).toEqual({ education_level: 'bachelor' });
	expect(payload).not.toHaveProperty('role_ids');
	expect(payload).not.toHaveProperty('organization_assignments');
});
test('personnel dashboard exposes accessible drilldown and a mobile layout', async ({ page }) => {
	await setup(page);
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/staff/manage/overview');
	await expect(page.getByRole('heading', { name: 'ภาพรวมงานบุคคล', exact: true })).toBeVisible();
	await page.screenshot({
		path: '../.superpowers/sdd/personnel-overview/dashboard-mobile.png',
		fullPage: true
	});
	await page.setViewportSize({ width: 1440, height: 1000 });
	await page.screenshot({
		path: '../.superpowers/sdd/personnel-overview/dashboard-desktop.png',
		fullPage: true
	});
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(page.getByTestId('personnel-overview')).toContainText('ไม่มีวิทยฐานะ');
	const statusLabel = page
		.getByRole('region', { name: 'สถานะบุคลากรทั้งหมด', exact: true })
		.getByText('ปฏิบัติงาน', { exact: true });
	// Keep status labels readable beside counts instead of wrapping into a narrow column.
	expect(
		await statusLabel.evaluate(
			(element) =>
				element.getBoundingClientRect().height / parseFloat(getComputedStyle(element).lineHeight)
		)
	).toBeLessThanOrEqual(2);
	await page
		.getByRole('heading', { name: 'บุคลากรตามกลุ่มสาระ', exact: true })
		.scrollIntoViewIfNeeded();
	await page.screenshot({
		path: '../.superpowers/sdd/personnel-overview/dashboard-mobile-charts.png'
	});
	await expect(page.getByTestId('personnel-overview')).toContainText('หนึ่งคนอาจอยู่หลายกลุ่มสาระ');
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await page.getByRole('link', { name: 'ครู 2 คน', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`job_position_id=${ref}`));
	await page.getByRole('link', { name: 'ดูข้อมูล', exact: true }).click();
	await page.getByRole('link', { name: 'กลับรายชื่อบุคลากร', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`job_position_id=${ref}`));
});
test('personnel reference management creates a typed local row', async ({ page }) => {
	await setup(page);
	await page.route('**/api/staff/reference-items**', (route) =>
		route.request().method() === 'POST'
			? route.fulfill({
					json: {
						success: true,
						data: {
							...item,
							id: '55000000-0000-4000-8000-000000000091',
							code: 'ref_synthetic',
							name: route.request().postDataJSON().name
						}
					}
				})
			: route.fallback()
	);
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/staff/manage/reference-data');
	await expect(page.getByTestId('staff-reference-catalog')).toContainText('ครู');
	await page.getByRole('button', { name: 'เพิ่มรายการ', exact: true }).click();
	await page.getByLabel('ชื่อรายการ', { exact: true }).fill('ตำแหน่งใหม่');
	await page.getByRole('button', { name: 'บันทึกรายการ', exact: true }).click();
	await expect(page.getByTestId('staff-reference-catalog')).toContainText('ตำแหน่งใหม่');
	await expect(page.getByTestId('staff-reference-catalog')).toContainText('2 รายการ');
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
test('read-only staff cannot load reference management', async ({ page }) => {
	const api = await setup(page, ['staff_profile.read.school']);
	let catalogReads = 0;
	await page.route('**/api/staff/reference-items**', (route) => {
		catalogReads++;
		return route.fulfill({
			json: { success: true, data: { items: [], total: 0, page: 1, pageSize: 25 } }
		});
	});
	await page.goto('/staff/manage/reference-data');
	await expect(page).toHaveURL(/\/403/);
	expect(api.writes).toHaveLength(0);
	expect(catalogReads).toBe(0);
});

test('existing inactive position remains readable and clearing sends explicit null', async ({
	page
}) => {
	await mockStaffDirectory(page, {
		staffInfo: {
			job_position: { id: ref, code: 'teacher', name: 'ครูเดิม', isActive: false },
			academic_rank: 'none',
			education_level: 'bachelor',
			major: null,
			university: null
		}
	});
	let reads = 0;
	await page.route('**/api/staff/reference-items**', (route) => {
		reads++;
		return route.fulfill({
			json: { success: true, data: { items: [], total: 0, page: 1, pageSize: 50 } }
		});
	});
	await page.goto(staffPath(undefined, '/edit') + '?section=education');
	const picker = page.getByRole('button', { name: 'ตำแหน่งงาน', exact: true });
	await expect(picker).toContainText('ครูเดิม');
	await expect(picker).toContainText('ปิดใช้งาน');
	expect(reads).toBe(0);
	await picker.click();
	await page.getByRole('button', { name: 'ยังไม่ระบุ', exact: true }).click();
	const saved = page.waitForRequest((r) => r.method() === 'PUT' && r.url().includes('/api/staff/'));
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	expect((await saved).postDataJSON()).toEqual({ staff_info: { job_position_id: null } });
});

test('new personnel selection survives draft reload and appears in review and creation', async ({
	page
}) => {
	await setup(page);
	await page.route('**/api/staff/reference-items**', (route) => {
		const kind = new URL(route.request().url()).searchParams.get('kind');
		const choice =
			kind === 'major'
				? { ...item, id: '55000000-0000-4000-8000-000000000092', kind, name: 'วิทยาศาสตร์' }
				: kind === 'university'
					? { ...item, id: '55000000-0000-4000-8000-000000000093', kind, name: 'มหาวิทยาลัยทดสอบ' }
					: item;
		return route.fulfill({
			json: { success: true, data: { items: [choice], total: 1, page: 1, pageSize: 50 } }
		});
	});
	await page.goto('/staff/manage/new');
	await page.getByPlaceholder('ชื่อ', { exact: true }).fill('บุคลากรใหม่');
	await page.getByPlaceholder('นามสกุล', { exact: true }).fill('ทดสอบ');
	await page.getByRole('button', { name: 'ตำแหน่งงาน', exact: true }).click();
	await page.getByRole('option', { name: 'ครู', exact: true }).click();
	await page.getByRole('button', { name: 'วิทยฐานะ', exact: true }).click();
	await page.getByRole('option', { name: 'ไม่มีวิทยฐานะ', exact: true }).click();
	await page.getByRole('button', { name: 'วุฒิการศึกษาสูงสุด', exact: true }).click();
	await page.getByRole('option', { name: 'ปริญญาตรี', exact: true }).click();
	await page.getByRole('button', { name: 'สาขาวิชา', exact: true }).click();
	await page.getByRole('option', { name: 'วิทยาศาสตร์', exact: true }).click();
	await page.getByRole('button', { name: 'สถาบันการศึกษา', exact: true }).click();
	await page.getByRole('option', { name: 'มหาวิทยาลัยทดสอบ', exact: true }).click();
	await page.locator('input[type=password]').nth(0).fill('synthetic-passphrase');
	await page.locator('input[type=password]').nth(1).fill('synthetic-passphrase');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.reload();
	await expect(page.getByRole('button', { name: 'ตำแหน่งงาน', exact: true })).toContainText('ครู');
	await page.locator('input[type=password]').nth(0).fill('synthetic-passphrase');
	await page.locator('input[type=password]').nth(1).fill('synthetic-passphrase');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: 'บทบาทตัวเลือก staff ระดับ 10', exact: true }).click();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: '+ เพิ่มหน่วยงาน', exact: true }).click();
	await page.getByRole('button', { name: 'เลือกหน่วยงาน', exact: true }).click();
	await page.getByRole('option', { name: /สังกัด หน่วยงานตัวเลือก/ }).click();
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page.getByTestId('staff-create-review')).toContainText('ตำแหน่ง: ครู');
	const saved = page.waitForRequest(
		(r) => r.method() === 'POST' && new URL(r.url()).pathname === '/api/staff'
	);
	await page.getByRole('button', { name: 'สร้างบุคลากร', exact: true }).click();
	expect((await saved).postDataJSON().staff_info).toMatchObject({
		job_position_id: ref,
		academic_rank: 'none',
		education_level: 'bachelor',
		major_id: '55000000-0000-4000-8000-000000000092',
		university_id: '55000000-0000-4000-8000-000000000093'
	});
});

test('overview owns its first error and retains data during refresh', async ({ page }) => {
	await setup(page);
	let count = 0,
		release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/staff/personnel-overview**', async (route) => {
		count++;
		if (count === 1)
			return route.fulfill({ status: 503, json: { success: false, error: 'ภาพรวมไม่พร้อม' } });
		if (count === 3) await held;
		return route.fulfill({
			json: {
				success: true,
				data: {
					asOf: '2026-10-01T00:00:00Z',
					total: 0,
					active: 0,
					otherStatuses: 0,
					filteredTotal: 0,
					statuses: [],
					subjectGroups: [],
					jobPositions: [],
					academicRanks: [],
					educationLevels: []
				}
			}
		});
	});
	await page.goto('/staff/manage/overview');
	const region = page.getByTestId('personnel-overview');
	await expect(region).toContainText('ภาพรวมไม่พร้อม');
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(region).toContainText('บุคลากรทั้งหมด');
	await page.getByRole('button', { name: 'รีเฟรชข้อมูล', exact: true }).click();
	await expect(region).toHaveAttribute('aria-busy', 'true');
	await expect(region).toContainText('บุคลากรทั้งหมด');
	release();
	await expect(region).toHaveAttribute('aria-busy', 'false');
	await expect(region).toContainText('ยังไม่มีบุคลากรในขอบเขตที่ดูได้');
	expect(await region.locator('circle[pathLength]').count()).toBe(0);
});

test('catalog keeps failed duplicate draft and patches deactivation without reloading', async ({
	page
}) => {
	await setup(page);
	let reads = 0,
		writes = 0;
	await page.route('**/api/staff/reference-items**', (route) => {
		if (route.request().method() === 'GET') {
			reads++;
			return route.fulfill({
				json: { success: true, data: { items: [item], total: 1, page: 1, pageSize: 25 } }
			});
		}
		writes++;
		if (writes === 1)
			return route.fulfill({
				status: 409,
				json: { success: false, error: 'รายการชื่อนี้มีอยู่แล้ว' }
			});
		return route.fulfill({
			json: { success: true, data: { ...item, ...route.request().postDataJSON() } }
		});
	});
	await page.goto('/staff/manage/reference-data');
	await page.getByRole('button', { name: 'เพิ่มรายการ', exact: true }).click();
	await page.getByLabel('ชื่อรายการ', { exact: true }).fill('ครูซ้ำ');
	await page.getByRole('button', { name: 'บันทึกรายการ', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('รายการชื่อนี้มีอยู่แล้ว');
	await expect(page.getByLabel('ชื่อรายการ', { exact: true })).toHaveValue('ครูซ้ำ');
	await page.getByRole('button', { name: 'Close', exact: true }).click();
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.getByLabel('ชื่อรายการ', { exact: true }).fill('ครูที่ปรับชื่อ');
	await page.getByRole('button', { name: 'บันทึกรายการ', exact: true }).click();
	await expect(page.getByTestId('staff-reference-catalog')).toContainText('ครูที่ปรับชื่อ');
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	await page.getByRole('button', { name: 'ปิดใช้งาน', exact: true }).click();
	await expect(page.getByTestId('staff-reference-catalog')).toContainText('ไม่พบรายการ');
	expect(reads).toBe(1);
});

test('a superseded overview status cannot paint a late result', async ({ page }) => {
	await setup(page);
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/staff/personnel-overview**', async (route) => {
		const status = new URL(route.request().url()).searchParams.get('status');
		if (status === 'inactive') await held;
		const label = status === 'inactive' ? 'ผลเก่าที่ช้า' : 'ครู';
		return route.fulfill({
			json: {
				success: true,
				data: {
					asOf: '2026-10-01T00:00:00Z',
					total: 1,
					active: 1,
					otherStatuses: 0,
					filteredTotal: 1,
					statuses: [{ key: 'active', label: 'ปฏิบัติงาน', count: 1 }],
					subjectGroups: [],
					jobPositions: [{ key: ref, label, count: 1 }],
					academicRanks: [],
					educationLevels: []
				}
			}
		});
	});
	await page.goto('/staff/manage/overview');
	await expect(page.getByRole('link', { name: 'ครู 1 คน', exact: true })).toBeVisible();
	await navigate(page, '/staff/manage/overview?status=inactive');
	await expect(page.getByTestId('personnel-overview')).toHaveAttribute('aria-busy', 'true');
	await expect(page.getByRole('link', { name: 'ครู 1 คน', exact: true })).toHaveCount(0);
	await page
		.getByRole('link', { name: 'test navigation', exact: true })
		.evaluate((el) => el.remove());
	await navigate(page, '/staff/manage/overview?status=all');
	await expect(page.getByRole('link', { name: 'ครู 1 คน', exact: true })).toBeVisible();
	release();
	await expect(page.getByTestId('personnel-overview')).not.toContainText('ผลเก่าที่ช้า');
	await expect(page).toHaveURL(/status=all/);
});

test('catalog kind changes and closing pending editors ignore late results', async ({ page }) => {
	await setup(page);
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/staff/reference-items**', async (route) => {
		const request = route.request(),
			kind = new URL(request.url()).searchParams.get('kind');
		if (request.method() !== 'GET') {
			await held;
			return route.fulfill({ json: { success: true, data: { ...item, name: 'บันทึกที่ช้า' } } });
		}
		return route.fulfill({
			json: {
				success: true,
				data: {
					items: kind === 'major' ? [] : [item],
					total: kind === 'major' ? 0 : 1,
					page: 1,
					pageSize: 25
				}
			}
		});
	});
	await page.goto('/staff/manage/reference-data');
	await page.getByRole('button', { name: 'เพิ่มรายการ', exact: true }).click();
	await page.getByLabel('ชื่อรายการ', { exact: true }).fill('บันทึกที่ช้า');
	await page.getByRole('button', { name: 'บันทึกรายการ', exact: true }).click();
	await page.getByRole('button', { name: 'Close', exact: true }).click();
	await page.getByRole('button', { name: 'ชนิดรายการ', exact: true }).click();
	await page.getByRole('option', { name: 'สาขาวิชา', exact: true }).click();
	await expect(page.getByTestId('staff-reference-catalog')).toContainText('ไม่พบรายการ');
	release();
	await expect(page.getByTestId('staff-reference-catalog')).not.toContainText('บันทึกที่ช้า');
});

test('reference picker retries and discards superseded search choices', async ({ page }) => {
	await setup(page);
	let reads = 0,
		release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/staff/reference-items**', async (route) => {
		reads++;
		const search = new URL(route.request().url()).searchParams.get('search');
		if (reads === 1)
			return route.fulfill({ status: 503, json: { success: false, error: 'ตัวเลือกไม่พร้อม' } });
		if (search === 'ช้า') await held;
		return route.fulfill({
			json: {
				success: true,
				data: {
					items: [{ ...item, name: search === 'ช้า' ? 'ตัวเลือกเก่า' : 'ครู' }],
					total: 1,
					page: 1,
					pageSize: 50
				}
			}
		});
	});
	await page.goto(staffPath(undefined, '/edit') + '?section=education');
	await page.getByRole('button', { name: 'ตำแหน่งงาน', exact: true }).click();
	await expect(page.getByText('ตัวเลือกไม่พร้อม', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByRole('option', { name: 'ครู', exact: true })).toBeVisible();
	const search = page.getByRole('textbox', { name: 'ค้นหาตำแหน่งงาน', exact: true });
	await search.fill('ช้า');
	await expect.poll(() => reads).toBe(3);
	await search.fill('ครู');
	await expect(page.getByRole('option', { name: 'ครู', exact: true })).toBeVisible();
	release();
	await expect(page.getByRole('option', { name: 'ตัวเลือกเก่า', exact: true })).toHaveCount(0);
});

test('catalog retries its first read and switches away from a pending page', async ({ page }) => {
	await setup(page);
	let reads = 0,
		release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/staff/reference-items**', async (route) => {
		reads++;
		if (reads === 1)
			return route.fulfill({ status: 503, json: { success: false, error: 'รายการกลางไม่พร้อม' } });
		const query = new URL(route.request().url()).searchParams;
		const pageNumber = Number(query.get('page') || 1),
			major = query.get('kind') === 'major';
		if (pageNumber === 2) await held;
		return route.fulfill({
			json: {
				success: true,
				data: {
					items: major ? [] : [{ ...item, name: pageNumber === 2 ? 'หน้าที่ช้า' : 'ครู' }],
					total: major ? 0 : 26,
					page: pageNumber,
					pageSize: 25
				}
			}
		});
	});
	await page.goto('/staff/manage/reference-data');
	await expect(page.getByTestId('staff-reference-catalog')).toContainText('รายการกลางไม่พร้อม');
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('staff-reference-catalog')).toContainText('ครู');
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(page).toHaveURL(/page=2/);
	await expect(page.getByTestId('staff-reference-catalog')).toHaveAttribute('aria-busy', 'true');
	await page.getByRole('button', { name: 'ชนิดรายการ', exact: true }).click();
	await page.getByRole('option', { name: 'สาขาวิชา', exact: true }).click();
	await expect(page).not.toHaveURL(/page=2/);
	await expect(page.getByTestId('staff-reference-catalog')).toContainText('ไม่พบรายการ');
	release();
	await expect(page.getByTestId('staff-reference-catalog')).not.toContainText('หน้าที่ช้า');
});
