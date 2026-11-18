import { expect, test } from '@playwright/test';
import {
	mock,
	id,
	year,
	path,
	firstCycle,
	templateId,
	savedPeriod,
	timetableOption,
	selectSupervisionDate
} from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });
const detailPath = `/staff/academic/supervision/${id(30)}?academicYearId=${year}`;
const observationPath = `/api/supervision/observations/${id(30)}`;
const teacherPermissions = ['supervision.request.own', 'supervision.read.own'];

for (const colorScheme of ['light', 'dark'] as const) {
	for (const width of [390, 1280]) {
		test(`returned teacher saves and resubmits with current reason only (${colorScheme}, ${width})`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 900 });
			await page.emulateMedia({ colorScheme });
			const api = await mock(page, {
				permissions: teacherPermissions,
				bookingItems: [timetableOption(0), timetableOption(1), timetableOption(2)],
				canManage: false,
				observationStatus: 'returned',
				observation: {
					periods: [
						savedPeriod({
							lessonSnapshot: {
								subjectName: 'วิชาที่ต้องแก้ไข',
								classroomLabel: 'ม.1/1',
								periodLabel: 'คาบ 1'
							}
						})
					],
					actions: [
						{
							id: id(90),
							observationId: id(30),
							actionKind: 'request_returned',
							toStatus: 'returned',
							comment: 'เหตุผลเก่าที่ไม่ต้องแสดง',
							createdAt: '2026-10-01T00:00:00Z'
						},
						{
							id: id(91),
							observationId: id(30),
							actionKind: 'request_returned',
							toStatus: 'returned',
							comment: 'กรุณาระบุคาบให้ชัดเจน',
							createdAt: '2026-10-02T00:00:00Z'
						}
					]
				}
			});
			await page.goto(detailPath);
			await expect(page.getByTestId('supervision-workflow')).toContainText('ครูผู้ขอรับนิเทศ');
			await expect(page.getByTestId('supervision-current-return-reason')).toContainText(
				'กรุณาระบุคาบให้ชัดเจน'
			);
			await expect(page.getByText('เหตุผลเก่าที่ไม่ต้องแสดง')).toHaveCount(0);
			await expect(page.getByText('ประวัติ', { exact: true })).toHaveCount(0);
			await page.screenshot({
				path: test.info().outputPath('returned-current.png'),
				fullPage: true
			});
			await page.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
			const dialog = page.getByRole('dialog');
			await dialog.getByRole('button', { name: /เลือก คาบ 2 ม.1\/1/ }).click();
			await dialog.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			await expect(page.getByTestId('supervision-workflow')).toContainText('ผู้รับผิดชอบตรวจคำขอ');
			await expect(page.getByTestId('supervision-current-return-reason')).toHaveCount(0);
			await expect(page.getByText('คาบ 2', { exact: true })).toBeVisible();
			expect(api.writes).toEqual([`PATCH ${observationPath}/request`]);
			expect(api.payloads).toEqual([
				expect.objectContaining({
					observedDate: '2026-10-01',
					timetableBlockGroupIds: [timetableOption(1).blockGroupId],
					expectedSourceDigest: 'source-1'
				})
			]);
			await page.screenshot({
				path: test.info().outputPath('resubmitted-next-owner.png'),
				fullPage: true
			});
			expect(api.count(observationPath)).toBe(1);
			expect(api.count(`/api/supervision/cycles/${firstCycle}`)).toBe(1);
			expect(api.count(`/api/supervision/templates/${templateId}/summary`)).toBe(1);
			expect(
				await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
			).toBe(true);
		});
	}
}

test('cycle creation requires the observation date window and retains manual status', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-01T03:00:00Z'));
	const api = await mock(page);
	await page.goto(path('cycles'));
	await page.getByRole('button', { name: 'สร้างรอบนิเทศ', exact: true }).first().click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('เปิดรับคำขอ', { exact: true })).toBeVisible();
	await expect(dialog.locator('input[type="time"]')).toHaveCount(0);
	await selectSupervisionDate(page, 'วันเริ่มนิเทศ', '2026-10-01');
	await selectSupervisionDate(page, 'วันสุดท้ายที่นิเทศ', '2026-10-15');
	await expect(
		dialog.getByText(/เปิดจองวันที่|ปิดจองวันที่|เริ่มรอบวันที่|สิ้นสุดรอบวันที่/)
	).toHaveCount(0);
	await dialog.getByPlaceholder('เช่น นิเทศการสอน ภาคเรียนที่ 1').fill('รอบสถานะกำหนดเอง');
	await dialog.getByRole('button', { name: 'สร้างรอบนิเทศ', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(api.payloads).toEqual([
		{
			academicYearId: year,
			academicTermId: null,
			title: 'รอบสถานะกำหนดเอง',
			observationStartDate: '2026-10-01',
			observationEndDate: '2026-10-15',
			description: null,
			templateId,
			status: 'open',
			targets: [{ targetType: 'school', requiredObservations: 1, priority: 100 }]
		}
	]);
});

for (const status of ['draft', 'open', 'closed', 'archived'] as const) {
	test(`booking availability follows manual ${status} state regardless of today`, async ({
		page
	}) => {
		await mock(page, {
			permissions: teacherPermissions,
			bookingItems: [timetableOption(0), timetableOption(1), timetableOption(2)],
			cycleStatus: status,
			term: true
		});
		await page.goto(path('').replace('/supervision/?', '/supervision?'));
		await page.getByRole('button', { name: 'เปิดจองคาบนิเทศ', exact: true }).click();
		if (status === 'open') {
			await expect(
				page.getByText('เปิดรับคำขอจนกว่าผู้ดูแลจะเปลี่ยนสถานะรอบนิเทศ', { exact: true })
			).toBeVisible();
			await expect(page.getByRole('button', { name: 'สัปดาห์ก่อน', exact: true })).toBeEnabled();
			await expect(page.getByRole('button', { name: 'สัปดาห์ถัดไป', exact: true })).toBeEnabled();
		} else {
			await expect(
				page.getByText('ยังไม่มีรอบนิเทศที่เปิดให้จองในขณะนี้', { exact: true })
			).toBeVisible();
		}
	});
}

test('returned request cannot resubmit while booking is closed', async ({ page }) => {
	const api = await mock(page, {
		permissions: teacherPermissions,
		bookingItems: [timetableOption(0), timetableOption(1), timetableOption(2)],
		canManage: false,
		cycleStatus: 'closed',
		observationStatus: 'returned',
		observation: {
			periods: [
				savedPeriod({
					lessonSnapshot: {
						subjectName: 'วิชาทดสอบ',
						classroomLabel: 'ม.1/1',
						periodLabel: 'คาบ 1'
					}
				})
			]
		}
	});
	await page.goto(detailPath);
	await expect(page.getByTestId('supervision-current-return-reason')).toContainText(
		'ส่งใหม่ได้เมื่อผู้ดูแลเปิดรับคำขอในรอบนี้'
	);
	await page.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: /เลือก คาบ 2 ม.1\/1/ }).click();
	await expect(dialog.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true })).toBeDisabled();
	expect(api.writes).toEqual([]);
});

test('closed booking does not block a planned evaluator from submitting', async ({ page }) => {
	const api = await mock(page, {
		permissions: ['supervision.read.assigned', 'supervision.evaluate.assigned'],
		cycleStatus: 'closed',
		observationStatus: 'planned',
		observation: { observedUserId: id(21) }
	});
	await page.goto(path('evaluate'));
	await expect(page.getByTestId('supervision-observations')).toContainText('ผู้ประเมินทดสอบ');
	await page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: '4', exact: true }).click();
	await dialog.getByRole('button', { name: 'ส่งผลประเมิน', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(api.writes).toEqual([`POST ${observationPath}/evaluations/me/submit`]);
});

test('returned assignments do not offer evaluation until approved again', async ({ page }) => {
	const api = await mock(page, {
		permissions: ['supervision.read.assigned', 'supervision.evaluate.assigned'],
		observationStatus: 'returned'
	});
	await page.goto(path('evaluate'));
	await expect(page.getByText('ยังไม่มีรายการที่ต้องประเมิน', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true })).toHaveCount(0);
	expect(api.writes).toEqual([]);
});

test('a refused resubmission retains the correction draft and current return reason', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: teacherPermissions,
		bookingItems: [timetableOption(0), timetableOption(1), timetableOption(2)],
		canManage: false,
		observationStatus: 'returned',
		observation: {
			periods: [
				savedPeriod({
					lessonSnapshot: {
						subjectName: 'วิชาทดสอบ',
						classroomLabel: 'ม.1/1',
						periodLabel: 'คาบ 1'
					}
				})
			]
		}
	});
	await page.route(`**${observationPath}/request`, async (route) =>
		route.fulfill({
			status: 409,
			contentType: 'application/json',
			body: JSON.stringify({ success: false, error: 'รอบนิเทศถูกปิดรับคำขอแล้ว' })
		})
	);
	await page.goto(detailPath);
	await page.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: /เลือก คาบ 2 ม.1\/1/ }).click();
	await dialog.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
	await expect(page.getByText('รอบนิเทศถูกปิดรับคำขอแล้ว', { exact: true })).toBeVisible();
	await expect(dialog.getByRole('button', { name: /เลือก คาบ 2 ม.1\/1/ })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(dialog.getByTestId('supervision-booking-selection')).toContainText('1 คาบต่อเนื่อง');
	expect(api.count(observationPath)).toBe(1);
});

test('own returned row names the next teacher action and links to correction', async ({ page }) => {
	await mock(page, {
		permissions: teacherPermissions,
		bookingItems: [timetableOption(0), timetableOption(1), timetableOption(2)],
		term: true,
		canManage: false,
		observationStatus: 'returned'
	});
	await page.goto(path('').replace('/supervision/?', '/supervision?'));
	const row = page.locator('[data-supervision-own-list="cards"]');
	await expect(row).toContainText('ขั้น 1/6 คำขอ');
	await expect(row).toContainText('ถัดไป: ครูผู้ขอรับนิเทศ');
	await expect(row.getByRole('link', { name: 'แก้ไขและส่งใหม่', exact: true })).toHaveAttribute(
		'href',
		new RegExp(id(30))
	);
});

test('submitted evaluator evidence prevents returning or reassigning a legacy request', async ({
	page
}) => {
	const api = await mock(page, {
		observation: {
			evaluators: [
				{
					id: id(31),
					observationId: id(30),
					evaluatorUserId: id(21),
					evaluatorDisplayName: 'ผู้ประเมินส่งแล้ว',
					isRequired: true,
					status: 'submitted',
					createdAt: '2026-10-01T00:00:00Z',
					updatedAt: '2026-10-01T00:00:00Z'
				}
			]
		}
	});
	await page.goto(path('requests'));
	await page.getByRole('combobox', { name: /เลือกผู้ประเมิน/ }).click();
	await page.getByRole('option', { name: 'ผู้ประเมินที่ว่าง', exact: true }).click();
	await page.keyboard.press('Escape');
	await expect(page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true })).toBeDisabled();
	await expect(page.getByRole('button', { name: 'อนุมัติและมอบหมาย', exact: true })).toBeDisabled();
	expect(api.writes).toEqual([]);
});

test('own legacy returned request can cancel without editing submitted results', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: teacherPermissions,
		bookingItems: [timetableOption(0), timetableOption(1), timetableOption(2)],
		canManage: false,
		observationStatus: 'returned',
		observation: {
			evaluators: [
				{
					id: id(31),
					observationId: id(30),
					evaluatorUserId: id(21),
					evaluatorDisplayName: 'ผู้ประเมินส่งแล้ว',
					isRequired: true,
					status: 'submitted',
					createdAt: '2026-10-01T00:00:00Z',
					updatedAt: '2026-10-01T00:00:00Z'
				}
			]
		}
	});
	await page.goto(detailPath);
	await expect(page.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await expect(
		page.getByRole('dialog').getByRole('button', { name: 'ยืนยันยกเลิก', exact: true })
	).toBeEnabled();
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยันยกเลิก', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByTestId('supervision-workflow')).toContainText('รายการถูกยกเลิก');
	expect(api.writes).toEqual([`DELETE ${observationPath}/request`]);
	expect(api.observation().evaluators[0].status).toBe('submitted');
	expect(api.count(observationPath)).toBe(1);
});

test('an optional assigned evaluator can submit while the item waits for certification', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: ['supervision.read.assigned', 'supervision.evaluate.assigned'],
		observationStatus: 'evaluators_submitted',
		observation: {
			observedUserId: id(21),
			evaluators: [
				{
					id: id(31),
					observationId: id(30),
					evaluatorUserId: id(20),
					evaluatorDisplayName: 'ผู้ประเมินเสริม',
					isRequired: false,
					status: 'assigned',
					createdAt: '2026-10-01T00:00:00Z',
					updatedAt: '2026-10-01T00:00:00Z'
				}
			]
		}
	});
	await page.goto(path('evaluate'));
	const row = page.locator('[data-supervision-assigned-list="cards"]');
	await expect(row).toContainText('รอรับรองผล');
	await expect(row).toContainText('ถัดไป: ผู้มีสิทธิ์รับรองผล');
	await row.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: '4', exact: true }).click();
	await dialog.getByRole('button', { name: 'ส่งผลประเมิน', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true })).toHaveCount(0);
	expect(api.writes).toEqual([`POST ${observationPath}/evaluations/me/submit`]);
	expect(api.count('/api/supervision/observations')).toBe(1);
});

for (const referenceState of ['pending', 'failed'] as const) {
	test(`a ${referenceState} cycle reference keeps resubmission conservative without claiming booking is closed`, async ({
		page
	}) => {
		const api = await mock(page, {
			permissions: teacherPermissions,
			bookingItems: [timetableOption(0), timetableOption(1), timetableOption(2)],
			canManage: false,
			observationStatus: 'returned',
			hold: referenceState === 'pending' ? 'selected-cycle' : undefined,
			fail: referenceState === 'failed' ? 'selected-cycle' : undefined,
			observation: {
				periods: [
					savedPeriod({
						lessonSnapshot: {
							subjectName: 'วิชาทดสอบ',
							classroomLabel: 'ม.1/1',
							periodLabel: 'คาบ 1'
						}
					})
				]
			}
		});
		await page.goto(detailPath);
		const cycle = page.getByTestId('supervision-detail-cycle');
		if (referenceState === 'pending') await expect(cycle.getByRole('status')).toBeVisible();
		else await expect(cycle.getByText('region ไม่พร้อม', { exact: true })).toBeVisible();
		const reason = page.getByTestId('supervision-current-return-reason');
		await expect(reason).toBeVisible();
		await expect(
			reason.getByText('ส่งใหม่ได้เมื่อผู้ดูแลเปิดรับคำขอในรอบนี้', { exact: true })
		).toHaveCount(0);
		await page.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
		const dialog = page.getByRole('dialog');
		await dialog.getByRole('button', { name: /เลือก คาบ 2 ม.1\/1/ }).click();
		const save = dialog.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true });
		await expect(save).toBeDisabled();
		if (referenceState === 'pending') {
			api.release();
			await expect(save).toBeEnabled();
		}
		expect(api.writes).toEqual([]);
		expect(api.count(observationPath)).toBe(1);
		expect(api.count(`/api/supervision/cycles/${firstCycle}`)).toBe(1);
	});
}
