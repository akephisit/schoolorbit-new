import { expect, test } from '@playwright/test';
import { mock, id, year, path, firstCycle, templateId } from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });
const detailPath = `/staff/academic/supervision/${id(30)}?academicYearId=${year}`;
const observationPath = `/api/supervision/observations/${id(30)}`;
const teacherPermissions = ['supervision.request.own', 'supervision.read.own'];

for (const colorScheme of ['light', 'dark'] as const) {
	for (const width of [390, 1280]) {
		test(`returned teacher saves and resubmits with current reason only (${colorScheme}, ${width})`, async ({ page }) => {
			await page.setViewportSize({ width, height: 900 });
			await page.emulateMedia({ colorScheme });
			const api = await mock(page, {
				permissions: teacherPermissions,
				canManage: false,
				observationStatus: 'returned',
				observation: {
					lessonSnapshot: { subjectName: 'วิชาที่ต้องแก้ไข', classroomLabel: 'ม.1/1', periodLabel: 'คาบ 1' },
					actions: [
						{ id: id(90), observationId: id(30), actionKind: 'request_returned', toStatus: 'returned', comment: 'เหตุผลเก่าที่ไม่ต้องแสดง', createdAt: '2026-10-01T00:00:00Z' },
						{ id: id(91), observationId: id(30), actionKind: 'request_returned', toStatus: 'returned', comment: 'กรุณาระบุคาบให้ชัดเจน', createdAt: '2026-10-02T00:00:00Z' }
					]
				}
			});
			await page.goto(detailPath);
			await expect(page.getByTestId('supervision-workflow')).toContainText('ครูผู้ขอรับนิเทศ');
			await expect(page.getByTestId('supervision-current-return-reason')).toContainText('กรุณาระบุคาบให้ชัดเจน');
			await expect(page.getByText('เหตุผลเก่าที่ไม่ต้องแสดง')).toHaveCount(0);
			await expect(page.getByRole('heading', { name: 'ประวัติ', exact: true })).toHaveCount(0);
			await page.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
			const dialog = page.getByRole('dialog');
			await dialog.getByLabel('คาบ', { exact: true }).fill('คาบ 2');
			await dialog.getByLabel('เหตุผล/หมายเหตุ', { exact: true }).fill('แก้ไขข้อมูลคาบตามคำแนะนำ');
			await dialog.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			await expect(page.getByTestId('supervision-workflow')).toContainText('ผู้รับผิดชอบตรวจคำขอ');
			await expect(page.getByTestId('supervision-current-return-reason')).toHaveCount(0);
			expect(api.writes).toEqual([`PATCH ${observationPath}/request`]);
			expect(api.payloads).toEqual([expect.objectContaining({ manualLesson: expect.objectContaining({ periodLabel: 'คาบ 2', observedAt: '2026-10-01T09:00:00.000Z' }) })]);
			expect(api.count(observationPath)).toBe(1);
			expect(api.count(`/api/supervision/cycles/${firstCycle}`)).toBe(1);
			expect(api.count(`/api/supervision/templates/${templateId}/summary`)).toBe(1);
			await expect(page.locator('body')).not.toHaveJSProperty('scrollWidth', 0);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
		});
	}
}

test('cycle creation has manual status and no scheduling inputs or payload fields', async ({ page }) => {
	const api = await mock(page);
	await page.goto(path('cycles'));
	await page.getByRole('button', { name: 'สร้างรอบนิเทศ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('เปิดรับคำขอ', { exact: true })).toBeVisible();
	await expect(dialog.locator('input[type="time"], input[type="date"]')).toHaveCount(0);
	await expect(dialog.getByText(/เปิดจองวันที่|ปิดจองวันที่|เริ่มรอบวันที่|สิ้นสุดรอบวันที่/)).toHaveCount(0);
	await dialog.getByPlaceholder('เช่น นิเทศการสอน ภาคเรียนที่ 1').fill('รอบสถานะกำหนดเอง');
	await dialog.getByRole('button', { name: 'สร้างรอบนิเทศ', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(api.payloads).toEqual([{ academicYearId: year, academicTermId: null, title: 'รอบสถานะกำหนดเอง', description: null, templateId, status: 'open', targets: [{ targetType: 'school', requiredObservations: 1, priority: 100 }] }]);
});

for (const status of ['draft', 'open', 'closed', 'archived'] as const) {
	test(`booking availability follows manual ${status} state regardless of today`, async ({ page }) => {
		await mock(page, { permissions: teacherPermissions, cycleStatus: status, term: true });
		await page.goto(path('').replace('/supervision/?', '/supervision?'));
		await page.getByRole('button', { name: 'เปิดจองคาบนิเทศ', exact: true }).click();
		if (status === 'open') {
			await expect(page.getByText('เปิดรับคำขอจนกว่าผู้ดูแลจะเปลี่ยนสถานะรอบนิเทศ', { exact: true })).toBeVisible();
			await expect(page.getByRole('button', { name: 'สัปดาห์ก่อน', exact: true })).toBeEnabled();
			await expect(page.getByRole('button', { name: 'สัปดาห์ถัดไป', exact: true })).toBeEnabled();
		} else {
			await expect(page.getByText('ยังไม่มีรอบนิเทศที่เปิดให้จองในขณะนี้', { exact: true })).toBeVisible();
		}
	});
}

test('returned request cannot resubmit while booking is closed', async ({ page }) => {
	const api = await mock(page, { permissions: teacherPermissions, canManage: false, cycleStatus: 'closed', observationStatus: 'returned', observation: { lessonSnapshot: { subjectName: 'วิชาทดสอบ', classroomLabel: 'ม.1/1', periodLabel: 'คาบ 1' } } });
	await page.goto(detailPath);
	await expect(page.getByTestId('supervision-current-return-reason')).toContainText('ส่งใหม่ได้เมื่อผู้ดูแลเปิดรับคำขอในรอบนี้');
	await page.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('เหตุผล/หมายเหตุ').fill('แก้ไขข้อมูล');
	await expect(dialog.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true })).toBeDisabled();
	expect(api.writes).toEqual([]);
});
