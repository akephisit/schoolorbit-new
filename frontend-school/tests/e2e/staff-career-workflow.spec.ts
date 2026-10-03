import { expect, test, type Page } from '@playwright/test';
import { navigate } from './fixtures/supervision-route-data';
import type { components } from '../../src/lib/api/generated/school-api';
import {
	mockStaffDirectory,
	staffPath,
	secondStaff,
	firstStaff
} from './fixtures/staff-directory-route-data';
import { mockStaffHome, id, actor } from './fixtures/staff-home-route-data';
import { careerEntry, personnelInfo } from './fixtures/staff-career-route-data';
type Schemas = components['schemas'];
test.use({ serviceWorkers: 'block' });
const region = (page: Page) =>
	page.getByRole('region', { name: 'ประวัติตำแหน่งและวิทยฐานะ', exact: true });
async function setup(
	page: Page,
	options: {
		hold?: boolean;
		fail?: boolean;
		failAt?: number;
		foreignCursor?: boolean;
		conflict?: boolean;
		retry?: boolean;
		readonly?: boolean;
		empty?: boolean;
		holdMutation?: boolean;
	} = {}
) {
	const rank = careerEntry({
		fact: { kind: 'academic_rank', value: 'proficient' },
		effectiveDate: '2024-02-29',
		orderDate: '2024-03-05',
		orderNumber: '12/2567'
	});
	const position = careerEntry({
		id: id(92),
		fact: { kind: 'job_position', value: id(90) },
		jobPosition: { id: id(90), code: 'teacher', name: 'ครู', isActive: false, isSelectable: false }
	});
	const personnel = careerEntry({
		id: id(93),
		fact: { kind: 'personnel_type', value: 'civil_servant' },
		effectiveDate: '2012-05-01',
		source: 'staff_entry'
	});
	const info = personnelInfo({
		academic_rank: 'proficient',
		job_position: position.jobPosition,
		current_career: { personnelType: personnel, jobPosition: position, academicRank: rank }
	});
	const api = await mockStaffDirectory(page, {
		staffInfo: info,
		permissions: options.readonly ? ['staff_profile.read.school'] : undefined
	});
	let entries: Schemas['StaffCareerEntry'][] = options.empty
		? []
		: [
				rank,
				position,
				personnel,
				careerEntry({
					id: id(94),
					fact: { kind: 'academic_rank', value: 'none' },
					effectiveDate: '2020-01-01',
					isCurrent: false
				})
			];
	let reads = 0,
		mutations = 0,
		release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	const mutationIds: string[] = [];
	let releaseMutation = () => {};
	const pendingMutation = new Promise<void>((resolve) => (releaseMutation = resolve));
	await page.route(
		(url) => url.pathname.includes('/career-history'),
		async (route) => {
			const method = route.request().method(),
				url = new URL(route.request().url());
			if (method === 'GET') {
				reads++;
				if (options.hold && reads === 1) await held;
				if (options.fail && reads === (options.failAt ?? 1))
					return route.fulfill({
						status: 503,
						json: { success: false, error: 'โหลดประวัติไม่สำเร็จ' }
					});
				const cursor = url.searchParams.get('cursor');
				const index = cursor ? entries.findIndex((entry) => entry.id === cursor) + 1 : 0;
				if (cursor && index === 0)
					return route.fulfill({
						status: 400,
						json: { success: false, error: 'ตำแหน่งหน้ารายการไม่ถูกต้อง' }
					});
				const items = entries.slice(index, index + 3);
				return route.fulfill({
					json: {
						success: true,
						data: {
							current: options.empty
								? { personnelType: null, jobPosition: null, academicRank: null }
								: info.current_career,
							items,
							nextCursor:
								options.foreignCursor && reads === 1
									? id(999)
									: index + 3 < entries.length
										? items.at(-1)?.id
										: null
						}
					}
				});
			}
			mutations++;
			if (options.holdMutation) await pendingMutation;
			if (method === 'POST') {
				const input: Schemas['CreateStaffCareerHistoryRequest'] = route.request().postDataJSON();
				mutationIds.push(input.id);
				if (options.retry && mutations === 1)
					return route.fulfill({
						status: 503,
						json: { success: false, error: 'บันทึกไม่สำเร็จ ลองอีกครั้ง' }
					});
				entries = [
					careerEntry({ ...input.entry, id: input.id, isCurrent: false, source: 'staff_entry' }),
					...entries
				];
				return route.fulfill({ json: { success: true, data: { id: input.id, revision: 1 } } });
			}
			const input: Schemas['CorrectStaffCareerHistoryRequest'] = route.request().postDataJSON();
			if (options.conflict && mutations === 1) {
				entries = entries.map((entry) => ({ ...entry, revision: entry.revision + 1 }));
				return route.fulfill({
					status: 409,
					json: { success: false, error: 'ข้อมูลเปลี่ยนไปแล้ว' }
				});
			}
			const entryId = url.pathname.split('/').at(-1),
				previous = entries.find((entry) => entry.id === entryId)!;
			expect(input.expectedRevision).toBe(previous.revision);
			const updated = careerEntry({ ...previous, ...input.entry, revision: previous.revision + 1 });
			entries = entries.map((entry) => (entry.id === entryId ? updated : entry));
			if (updated.isCurrent) info.current_career.academicRank = updated;
			return route.fulfill({
				json: { success: true, data: { id: entryId, revision: updated.revision } }
			});
		}
	);
	return {
		api,
		release,
		releaseMutation,
		info,
		mutationIds,
		get reads() {
			return reads;
		},
		get mutations() {
			return mutations;
		}
	};
}
test('career dates, current facts and unknown legacy dates have readable summaries', async ({
	page
}, testInfo) => {
	await setup(page);
	await page.goto(staffPath());
	await expect(region(page)).toContainText('29 ก.พ. 2567');
	await expect(region(page)).toContainText('ข้าราชการครู');
	await expect(region(page)).toContainText('วันที่ยังไม่ระบุ');
	for (const theme of ['light', 'dark'])
		for (const width of [390, 1440]) {
			await page.setViewportSize({ width, height: width === 390 ? 844 : 1000 });
			await page.evaluate((value) => {
				document.documentElement.classList.toggle('dark', value === 'dark');
			}, theme);
			await region(page).scrollIntoViewIfNeeded();
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({
				path: testInfo.outputPath(`history-${width}-${theme}.png`),
				fullPage: true
			});
		}
});
test('history loads independently, owns errors, retries and paginates', async ({
	page
}, testInfo) => {
	const api = await setup(page, { fail: true });
	await page.goto(staffPath());
	await expect(page.getByRole('heading', { name: /บุคลากรแรก\s+ทดสอบ/ })).toBeVisible();
	await expect(region(page)).toContainText('โหลดประวัติไม่สำเร็จ');
	await region(page).screenshot({ path: testInfo.outputPath('history-error.png') });
	await region(page).getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(region(page)).toContainText('29 ก.พ. 2567');
	await region(page).getByRole('button', { name: 'โหลดประวัติเพิ่ม', exact: true }).click();
	await expect(region(page)).toContainText('1 ม.ค. 2563');
	expect(api.reads).toBe(3);
});
test('delayed history leaves the profile visible and read-only history has no actions', async ({
	page
}, testInfo) => {
	const api = await setup(page, { hold: true, readonly: true });
	await page.goto(staffPath());
	await expect(page.getByRole('heading', { name: /บุคลากรแรก\s+ทดสอบ/ })).toBeVisible();
	await expect(region(page).getByTestId('career-history-loading')).toBeVisible();
	await region(page).screenshot({ path: testInfo.outputPath('history-loading.png') });
	api.release();
	await expect(region(page)).toContainText('ชำนาญการ');
	await expect(
		region(page).getByRole('button', { name: 'เพิ่มประวัติย้อนหลัง', exact: true })
	).toHaveCount(0);
	expect(api.api.reads.filter((url) => url.pathname.includes('job-positions'))).toHaveLength(0);
});
test('historical append retries the same UUID and leaves current data unchanged', async ({
	page
}) => {
	const api = await setup(page, { retry: true });
	await page.goto(staffPath());
	await region(page).getByRole('button', { name: 'เพิ่มประวัติย้อนหลัง', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('เลขที่คำสั่ง', { exact: true }).fill('1/2563');
	await dialog.getByRole('button', { name: 'บันทึกประวัติ', exact: true }).click();
	await expect(dialog).toContainText('บันทึกไม่สำเร็จ');
	await expect(dialog.getByLabel('เลขที่คำสั่ง', { exact: true })).toHaveValue('1/2563');
	await dialog.getByRole('button', { name: 'บันทึกประวัติ', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(api.mutationIds).toHaveLength(2);
	expect(api.mutationIds[0]).toBe(api.mutationIds[1]);
	await expect(region(page).getByTestId('career-current-academic_rank')).toContainText(
		'29 ก.พ. 2567'
	);
});
test('a stale correction keeps draft and reason until explicit reconciliation', async ({
	page
}, testInfo) => {
	const api = await setup(page, { conflict: true });
	await page.goto(staffPath());
	await region(page)
		.getByRole('button', { name: 'แก้ไขประวัติ วิทยฐานะ ชำนาญการ', exact: true })
		.click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('เลขที่คำสั่ง', { exact: true }).fill('13/2567');
	await dialog.getByLabel('เหตุผลการแก้ไข', { exact: true }).fill('แก้วันที่ตามคำสั่ง');
	await dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect(dialog).toContainText('ข้อมูลเปลี่ยนไปแล้ว');
	await expect(dialog.getByLabel('เหตุผลการแก้ไข', { exact: true })).toHaveValue(
		'แก้วันที่ตามคำสั่ง'
	);
	await expect(dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })).toBeDisabled();
	await page.screenshot({ path: testInfo.outputPath('history-conflict.png'), fullPage: true });
	await dialog.getByRole('button', { name: 'โหลดข้อมูลล่าสุดและตรวจสอบร่าง', exact: true }).click();
	await expect(dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })).toBeEnabled();
	await dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	await expect(region(page)).toContainText('13/2567');
	expect(api.mutations).toBe(2);
	expect(api.api.count('/api/auth/me')).toBe(1);
});
test('empty history uses an empty state and own profile respects exact history capabilities', async ({
	page
}, testInfo) => {
	await setup(page, { empty: true });
	await page.goto(staffPath());
	await expect(region(page)).toContainText('ยังไม่มีประวัติ');
	await region(page).screenshot({ path: testInfo.outputPath('history-empty.png') });
	await page.unrouteAll({ behavior: 'wait' });
	const api = await mockStaffHome(page, { permissions: [] });
	await page.goto('/staff/profile');
	await expect(page.getByRole('heading', { name: 'โปรไฟล์ของฉัน', exact: true })).toBeVisible();
	await expect(region(page)).toHaveCount(0);
	expect(api.reads.filter((path) => path.includes('career-history'))).toHaveLength(0);
	await page.unrouteAll({ behavior: 'wait' });
	await mockStaffHome(page, { permissions: ['staff_profile.read.own'] });
	await page.route(`**/api/staff/${actor}/career-history**`, (route) =>
		route.fulfill({
			json: {
				success: true,
				data: {
					items: [careerEntry({ staffId: actor })],
					current: {
						personnelType: null,
						jobPosition: null,
						academicRank: careerEntry({ staffId: actor })
					},
					nextCursor: null
				}
			}
		})
	);
	await page.goto('/staff/profile');
	await expect(region(page)).toContainText('ไม่มีวิทยฐานะ');
	await expect(
		region(page).getByRole('button', { name: 'เพิ่มประวัติย้อนหลัง', exact: true })
	).toHaveCount(0);
});

test('validation is local and pending saves cannot submit twice or close the draft', async ({
	page
}, testInfo) => {
	const api = await setup(page, { holdMutation: true });
	await page.goto(staffPath());
	await page.setViewportSize({ width: 390, height: 844 });
	await region(page)
		.getByRole('button', { name: 'แก้ไขประวัติ วิทยฐานะ ชำนาญการ', exact: true })
		.click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect(dialog).toContainText('กรุณาระบุเหตุผลการแก้ไข');
	expect(api.mutations).toBe(0);
	await dialog.getByLabel('เลขที่คำสั่ง', { exact: true }).fill('ก'.repeat(101));
	await expect(dialog.getByRole('alert').filter({ hasText: 'เลขที่คำสั่ง' })).toContainText('100');
	await expect(dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })).toBeDisabled();
	await page.screenshot({ path: testInfo.outputPath('history-validation-mobile.png') });
	await dialog.getByLabel('เลขที่คำสั่ง', { exact: true }).fill('14/2567');
	await page.keyboard.press('Tab');
	await expect(dialog.getByLabel('หมายเหตุ', { exact: true })).toBeFocused();
	await dialog.getByLabel('เหตุผลการแก้ไข', { exact: true }).fill('แก้เลขที่คำสั่ง');
	await dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true }).click();
	await expect.poll(() => api.mutations).toBe(1);
	await expect(dialog.getByRole('button', { name: 'บันทึกการแก้ไข', exact: true })).toBeDisabled();
	await expect(dialog.getByRole('button', { name: 'ยกเลิก', exact: true })).toBeDisabled();
	await page.keyboard.press('Escape');
	await expect(dialog).toBeVisible();
	await page.screenshot({ path: testInfo.outputPath('history-pending-mobile.png') });
	api.releaseMutation();
	await expect(dialog).toHaveCount(0);
	expect(api.mutations).toBe(1);
});

test('background history failure retains current facts until its own retry succeeds', async ({
	page
}) => {
	const api = await setup(page, { fail: true, failAt: 2 });
	await page.goto(staffPath());
	const current = region(page).getByTestId('career-current-academic_rank');
	await expect(current).toContainText('29 ก.พ. 2567');
	await region(page).getByRole('button', { name: 'รีเฟรชประวัติ', exact: true }).click();
	await expect(region(page)).toContainText('โหลดประวัติไม่สำเร็จ');
	await expect(current).toContainText('29 ก.พ. 2567');
	await region(page).getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(region(page)).not.toContainText('โหลดประวัติไม่สำเร็จ');
	expect(api.reads).toBe(3);
});
test('a late history read cannot paint the next persons region', async ({ page }) => {
	const api = await setup(page, { hold: true });
	await page.route(
		(url) => url.pathname === `/api/staff/${secondStaff}/career-history`,
		(route) =>
			route.fulfill({
				json: {
					success: true,
					data: {
						items: [],
						current: { personnelType: null, jobPosition: null, academicRank: null },
						nextCursor: null
					}
				}
			})
	);
	await page.goto(staffPath());
	await expect(region(page).getByTestId('career-history-loading')).toBeVisible();
	await navigate(page, staffPath(secondStaff));
	await expect(region(page)).toContainText('ยังไม่มีประวัติ');
	const settled = page.waitForResponse(
		(response) => new URL(response.url()).pathname === `/api/staff/${firstStaff}/career-history`
	);
	api.release();
	await settled;
	await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
	await expect(region(page)).not.toContainText('29 ก.พ. 2567');
	await expect(region(page)).toContainText('ยังไม่มีประวัติ');
});

test('a foreign pagination cursor produces a bounded error and keeps usable history', async ({
	page
}) => {
	const api = await setup(page, { foreignCursor: true });
	await page.goto(staffPath());
	await expect(region(page)).toContainText('29 ก.พ. 2567');
	await region(page).getByRole('button', { name: 'โหลดประวัติเพิ่ม', exact: true }).click();
	await expect(region(page)).toContainText('ตำแหน่งหน้ารายการไม่ถูกต้อง');
	await expect(region(page).getByTestId('career-current-academic_rank')).toContainText(
		'29 ก.พ. 2567'
	);
	await region(page).getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(region(page)).not.toContainText('ตำแหน่งหน้ารายการไม่ถูกต้อง');
	expect(api.reads).toBe(3);
});
