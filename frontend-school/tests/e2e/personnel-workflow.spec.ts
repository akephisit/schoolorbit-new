import { careerEntry, personnelInfo, rankOverview } from './fixtures/staff-career-route-data';
import { expect, test, type Page } from '@playwright/test';
import { mockStaffDirectory, staffPath } from './fixtures/staff-directory-route-data';
import { navigate } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
const ref = '55000000-0000-4000-8000-000000000090';
const item = {
	id: ref,
	code: 'teacher',
	name: 'ครู',
	isActive: true,
	isSelectable: true,
	displayOrder: 10,
	createdAt: '2026-10-01T00:00:00Z',
	updatedAt: '2026-10-01T00:00:00Z'
};
async function setup(page: Page, permissions?: string[]) {
	const api = await mockStaffDirectory(page, { permissions });
	await page.route('**/api/staff/job-positions**', (route) =>
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
	await page.route('**/api/staff/personnel-rank-milestones**', (route) =>
		route.fulfill({ json: { success: true, data: rankOverview() } })
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
test('personnel dashboard exposes accessible drilldown and a mobile layout', async ({
	page
}, testInfo) => {
	await setup(page);
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/staff/manage/overview');
	await expect(page.getByRole('heading', { name: 'ภาพรวมงานบุคคล', exact: true })).toBeVisible();
	await page.screenshot({
		path: testInfo.outputPath('dashboard-mobile.png'),
		fullPage: true
	});
	await page.setViewportSize({ width: 1440, height: 1000 });
	await page.screenshot({
		path: testInfo.outputPath('dashboard-desktop.png'),
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
		path: testInfo.outputPath('dashboard-mobile-charts.png')
	});
	await expect(page.getByTestId('personnel-overview')).toContainText('หนึ่งคนอาจอยู่หลายกลุ่มสาระ');
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await page.getByRole('link', { name: 'ครู 2 คน', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`job_position_id=${ref}`));
	await page.getByRole('link', { name: 'ดูข้อมูล', exact: true }).click();
	await page.getByRole('link', { name: 'กลับรายชื่อบุคลากร', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`job_position_id=${ref}`));
});
test('existing inactive position remains readable and clearing sends explicit null', async ({
	page
}) => {
	await mockStaffDirectory(page, {
		staffInfo: personnelInfo({
			job_position: {
				id: ref,
				code: 'teacher',
				name: 'ครูเดิม',
				isActive: false,
				isSelectable: false
			},
			current_career: {
				personnelType: null,
				jobPosition: careerEntry({
					id: ref,
					fact: { kind: 'job_position', value: ref },
					jobPosition: {
						id: ref,
						code: 'teacher',
						name: 'ครูเดิม',
						isActive: false,
						isSelectable: false
					}
				}),
				academicRank: careerEntry()
			},
			academic_rank: 'none',
			education_level: 'bachelor',
			major: null,
			university: null
		})
	});
	let reads = 0;
	await page.route('**/api/staff/job-positions**', (route) => {
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
	expect((await saved).postDataJSON()).toEqual({
		staff_info: {
			career: {
				changes: [
					{
						expectedCurrent: { id: ref, revision: 1 },
						entry: {
							fact: { kind: 'job_position', value: null },
							effectiveDate: null,
							orderDate: null,
							orderNumber: null,
							note: null
						}
					}
				]
			}
		}
	});
});

test('new personnel selection survives draft reload and appears in review and creation', async ({
	page
}) => {
	await setup(page);
	await page.goto('/staff/manage/new');
	await page.getByPlaceholder('ชื่อ', { exact: true }).fill('บุคลากรใหม่');
	await page.getByPlaceholder('นามสกุล', { exact: true }).fill('ทดสอบ');
	await page.getByRole('button', { name: 'ตำแหน่งงาน', exact: true }).click();
	await page.getByRole('option', { name: 'ครู', exact: true }).click();
	await page.getByRole('button', { name: 'วิทยฐานะ', exact: true }).click();
	await page.getByRole('option', { name: 'ไม่มีวิทยฐานะ', exact: true }).click();
	await page.getByRole('button', { name: 'วุฒิการศึกษาสูงสุด', exact: true }).click();
	await page.getByRole('option', { name: 'ปริญญาตรี', exact: true }).click();
	await page.getByLabel('สาขาวิชา', { exact: true }).fill('วิทยาศาสตร์');
	await page.getByLabel('สถาบันการศึกษา', { exact: true }).fill('มหาวิทยาลัยทดสอบ');
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
	await expect(page.getByTestId('staff-create-review')).toContainText('สาขา: วิทยาศาสตร์');
	await expect(page.getByTestId('staff-create-review')).toContainText('สถาบัน: มหาวิทยาลัยทดสอบ');
	const saved = page.waitForRequest(
		(r) => r.method() === 'POST' && new URL(r.url()).pathname === '/api/staff'
	);
	await page.getByRole('button', { name: 'สร้างบุคลากร', exact: true }).click();
	expect((await saved).postDataJSON().staff_info).toMatchObject({
		career: {
			entries: [
				{
					fact: { kind: 'job_position', value: ref },
					effectiveDate: null,
					orderDate: null,
					orderNumber: null,
					note: null
				},
				{
					fact: { kind: 'academic_rank', value: 'none' },
					effectiveDate: null,
					orderDate: null,
					orderNumber: null,
					note: null
				}
			]
		},
		education_level: 'bachelor',
		major: 'วิทยาศาสตร์',
		university: 'มหาวิทยาลัยทดสอบ'
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

test('position picker retries and discards superseded search choices', async ({ page }) => {
	await setup(page);
	let reads = 0,
		release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/staff/job-positions**', async (route) => {
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

test('education text validation and failed save retain draft without catalog requests', async ({
	page
}) => {
	const api = await setup(page);
	let obsolete = 0;
	await page.route('**/api/staff/reference-items**', (route) => {
		obsolete++;
		return route.abort();
	});
	await page.goto(staffPath(undefined, '/edit') + '?section=education');
	const major = page.getByLabel('สาขาวิชา', { exact: true });
	await major.fill('ก'.repeat(201));
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	await expect(major).toHaveAttribute('aria-invalid', 'true');
	expect(api.writes).toHaveLength(0);
	await major.fill('  คณิตศาสตร์  ประยุกต์  ');
	await page.getByLabel('สถาบันการศึกษา', { exact: true }).fill('สถาบันทดสอบ');
	await page.route('**/api/staff/*', (route) =>
		route.request().method() === 'PUT'
			? route.fulfill({ status: 503, json: { success: false, error: 'บันทึกไม่สำเร็จ' } })
			: route.fallback()
	);
	const saved = page.waitForRequest((r) => r.method() === 'PUT');
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง', exact: true }).click();
	expect((await saved).postDataJSON().staff_info).toEqual({
		major: 'คณิตศาสตร์  ประยุกต์',
		university: 'สถาบันทดสอบ'
	});
	await expect(major).toHaveValue('  คณิตศาสตร์  ประยุกต์  ');
	expect(obsolete).toBe(0);
});
test('retired reference management route is absent and read-only directory keeps overview', async ({
	page
}) => {
	await setup(page, ['staff_profile.read.school']);
	await page.goto('/staff/manage');
	await expect(page.getByRole('link', { name: 'จัดการรายการกลาง', exact: true })).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'ภาพรวมงานบุคคล', exact: true })).toBeVisible();
	await page.goto('/staff/manage/reference-data');
	await expect(page.getByText('404', { exact: true })).toBeVisible();
});

test('personnel fields and expanded dates remain readable on mobile and desktop in both themes', async ({
	page
}, testInfo) => {
	await setup(page);
	for (const width of [390, 768, 1440]) {
		await page.setViewportSize({ width, height: 1000 });
		await page.goto(staffPath(undefined, '/edit') + '?section=education');
		const fields = page.getByTestId('staff-personnel-fields');
		await expect(fields).toBeVisible();
		await page.getByLabel('สาขาวิชา', { exact: true }).fill('คณิตศาสตร์  ประยุกต์');
		await page.getByLabel('สถาบันการศึกษา', { exact: true }).fill('มหาวิทยาลัยทดสอบ');
		for (const kind of ['ประเภทบุคลากร', 'ตำแหน่ง', 'วิทยฐานะ']) {
			await fields.getByRole('button', { name: `วันที่และคำสั่ง · ${kind}`, exact: true }).click();
		}
		await expect(fields.getByRole('button', { name: /^วันที่(มีผล|ออกคำสั่ง)$/ })).toHaveCount(6);
		for (const theme of ['light', 'dark']) {
			await page.evaluate(
				(dark) => document.documentElement.classList.toggle('dark', dark),
				theme === 'dark'
			);
			await page.evaluate(() => document.fonts.ready);
			const controls = await fields.evaluate((element) =>
				['ประเภทบุคลากร', 'ตำแหน่งงาน'].map((name) => {
					const control = element.querySelector(`[aria-label="${name}"]`)!;
					const rect = control.getBoundingClientRect();
					return { top: rect.top, height: rect.height };
				})
			);
			expect(Math.abs(controls[0].height - controls[1].height)).toBeLessThan(1);
			if (width >= 768) expect(Math.abs(controls[0].top - controls[1].top)).toBeLessThan(1);
			const education = await fields.evaluate((element) =>
				['[aria-label="วุฒิการศึกษาสูงสุด"]', '#staff-major', '#staff-university'].map(
					(selector) => {
						const control = element.querySelector(selector)!;
						const rect = control.getBoundingClientRect();
						const label = control.parentElement!.querySelector('label, p')!;
						return {
							top: rect.top,
							height: rect.height,
							labelHeight: label.getBoundingClientRect().height
						};
					}
				)
			);
			for (const control of education) {
				expect(Math.abs(control.height - education[0].height)).toBeLessThan(1);
				expect(Math.abs(control.labelHeight - education[0].labelHeight)).toBeLessThan(1);
			}
			if (width >= 768) expect(Math.abs(education[0].top - education[1].top)).toBeLessThan(1);
			await fields
				.locator(':scope > div')
				.last()
				.screenshot({
					path: testInfo.outputPath(`personnel-education-${width}-${theme}.png`)
				});

			await fields
				.locator(':scope > div')
				.first()
				.screenshot({
					path: testInfo.outputPath(`personnel-fields-${width}-${theme}.png`)
				});

			for (const label of await fields.locator('[data-slot="collapsible-content"] label').all()) {
				const ink = await label.evaluate((element) => {
					const style = getComputedStyle(element);
					const context = document.createElement('canvas').getContext('2d')!;
					context.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
					const metrics = context.measureText(element.textContent!);
					const range = document.createRange();
					range.selectNodeContents(element);
					const textBox = range.getBoundingClientRect();
					const content = element.closest('[data-slot="collapsible-content"]')!;
					return {
						glyphTop:
							textBox.bottom - metrics.fontBoundingBoxDescent - metrics.actualBoundingBoxAscent,
						clipTop: content.getBoundingClientRect().top
					};
				});
				expect(ink.glyphTop).toBeGreaterThanOrEqual(ink.clipTop);
			}
			for (const [index, panel] of (
				await fields.locator('[data-slot="collapsible"]').all()
			).entries()) {
				await panel.evaluate((element) => element.scrollIntoView({ block: 'center' }));
				await panel.screenshot({
					path: testInfo.outputPath(`personnel-dates-${index}-${width}-${theme}.png`)
				});
			}
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
		}
		const picker = fields.getByRole('button', { name: 'วันที่มีผล', exact: true }).first();
		await picker.focus();
		await page.keyboard.press('Enter');
		await expect(picker).toHaveAttribute('aria-expanded', 'true');
		await page.keyboard.press('Escape');
		await expect(picker).toHaveAttribute('aria-expanded', 'false');
	}
});
