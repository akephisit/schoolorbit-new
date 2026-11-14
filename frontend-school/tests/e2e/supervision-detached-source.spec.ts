import { expect, test } from '@playwright/test';
import type {
	LessonSnapshot,
	UpdateSupervisionObservationRequest,
	SupervisionTimetableOption,
	ManualLesson
} from '../../src/lib/api/supervision';
import { mock, id, year } from './fixtures/supervision-route-data';
test.use({ serviceWorkers: 'block' });
const path = `/staff/academic/supervision/${id(30)}?academicYearId=${year}`,
	resource = `/api/supervision/observations/${id(30)}`;
const rich: LessonSnapshot = {
	source: 'timetable',
	timetableBlockGroupId: id(80),
	subjectName: 'คาบเดิมที่นิเทศจริง',
	classroomLabel: 'ม.1/1',
	roomLabel: 'ห้องเดิม',
	periodLabel: 'คาบเดิม',
	observedAt: '2026-10-01T09:00:00Z'
};
const minimal: LessonSnapshot = {
	source: 'removed_timetable',
	timetableBlockGroupId: id(80),
	observedAt: '2026-10-01T09:00:00Z'
};
const current: SupervisionTimetableOption = {
	blockGroupId: id(81),
	blockId: id(82),
	bellSchedulePeriodId: id(83),
	dayOfWeek: 'THU',
	startTime: '08:30:00',
	endTime: '09:20:00',
	homeroomNames: ['ม.1/2'],
	learningGroupId: id(84),
	learningGroupName: 'กลุ่มปัจจุบัน',
	learningOfferingId: id(85),
	offeringCode: 'ค21101',
	offeringName: 'วิชาปัจจุบัน',
	periodName: 'คาบใหม่',
	periodOrderIndex: 1,
	roomName: 'ห้องปัจจุบัน'
};

for (const [name, snapshot] of [
	['rich', rich],
	['minimal', minimal]
] as const)
	test(`${name} detached timetable evidence remains readable with saved evaluation results`, async ({
		page
	}) => {
		const fixture = await mock(page, {
				observationStatus: 'evaluators_submitted',
				averageRating: 4,
				observation: {
					timetableBlockGroupId: null,
					learningGroupId: null,
					lessonSnapshot: snapshot
				}
			}),
			saved = fixture.observation();
		await page.goto(path);
		await expect(
			page.getByText('ตารางสอนเดิม (ข้อมูลคาบที่บันทึกไว้)', { exact: true })
		).toBeVisible();
		await expect(
			page.getByText(
				'คาบเดิมถูกนำออกจากตารางปัจจุบัน ข้อมูลคาบและผลการนิเทศที่บันทึกไว้ยังคงอยู่',
				{ exact: true }
			)
		).toBeVisible();
		await expect(page.getByText('กำหนดเอง', { exact: true })).toHaveCount(0);
		await expect(page.getByText('ผู้ประเมินทดสอบ', { exact: true }).first()).toBeVisible();
		await page.getByRole('button', { name: 'ดูผลประเมินรายข้อ', exact: true }).click();
		await expect(page.getByTestId('supervision-review')).toContainText(
			'หัวข้อ rubric ที่โหลดเมื่อเปิด'
		);
		expect(fixture.observation()).toEqual(saved);
		expect(saved.evaluators[0].status).toBe('submitted');
		expect(saved.averageRating).toBe(4);
		expect(fixture.writes).toEqual([]);
		expect(fixture.count(`${resource}/timetable-options`)).toBe(0);
	});

test('minimal detached evidence never invents manual labels or a reason; explicit valid manual entry is required', async ({
	page
}) => {
	const fixture = await mock(page, {
			observationStatus: 'planned',
			observation: { timetableBlockGroupId: null, lessonSnapshot: minimal }
		}),
		saved = fixture.observation(),
		writes: UpdateSupervisionObservationRequest[] = [];
	await page.route(`**${resource}`, (route) => {
		if (route.request().method() !== 'PATCH') return route.fallback();
		const body: UpdateSupervisionObservationRequest = route.request().postDataJSON();
		writes.push(body);
		return route.fulfill({
			json: {
				success: true,
				data: {
					...saved,
					manualLesson: body.manualLesson,
					lessonSnapshot: { source: 'manual', ...body.manualLesson }
				}
			}
		});
	});
	await page.goto(path);
	await page.getByRole('button', { name: 'แก้คาบ/วันเวลา', exact: true }).click();
	const dialog = page.getByRole('dialog'),
		save = dialog.getByRole('button', { name: 'บันทึกคาบนิเทศ', exact: true });
	for (const label of ['วิชา', 'คาบ', 'ชั้น/ห้อง', 'ห้องเรียน', 'เหตุผล/หมายเหตุ'])
		await expect(dialog.getByLabel(label, { exact: true })).toHaveValue('');
	await expect(save).toBeDisabled();
	await save.evaluate((button: HTMLButtonElement) => button.click());
	expect(writes).toEqual([]);
	await dialog.getByLabel('วิชา', { exact: true }).fill('คาบกิจกรรมที่ระบุเอง');
	await dialog.getByLabel('คาบ', { exact: true }).fill('ช่วงบ่าย');
	await dialog.getByLabel('ชั้น/ห้อง', { exact: true }).fill('ม.1/1');
	await dialog.getByLabel('เหตุผล/หมายเหตุ', { exact: true }).fill('   ');
	await expect(save).toBeDisabled();
	await dialog
		.getByLabel('เหตุผล/หมายเหตุ', { exact: true })
		.fill(' ยืนยันเปลี่ยนเป็นกิจกรรมนอกตารางตามข้อเท็จจริง ');
	await save.click();
	await expect.poll(() => writes.length).toBe(1);
	expect(writes[0].manualLesson).toMatchObject({
		subjectName: 'คาบกิจกรรมที่ระบุเอง',
		periodLabel: 'ช่วงบ่าย',
		classroomLabel: 'ม.1/1',
		reason: 'ยืนยันเปลี่ยนเป็นกิจกรรมนอกตารางตามข้อเท็จจริง'
	});
	expect(writes[0].timetableBlockGroupId ?? null).toBeNull();
	expect(fixture.observation().evaluators).toEqual(saved.evaluators);
});

test('detached rich evidence can be deliberately rebound to an actual current timetable option', async ({
	page
}) => {
	const fixture = await mock(page, {
			observationStatus: 'planned',
			observation: { timetableBlockGroupId: null, lessonSnapshot: rich }
		}),
		writes: UpdateSupervisionObservationRequest[] = [];
	await page.route(`**${resource}/timetable-options`, (route) =>
		route.fulfill({ json: { success: true, data: { items: [current] } } })
	);
	await page.route(`**${resource}`, (route) => {
		if (route.request().method() !== 'PATCH') return route.fallback();
		const body: UpdateSupervisionObservationRequest = route.request().postDataJSON();
		writes.push(body);
		return route.fulfill({
			json: {
				success: true,
				data: {
					...fixture.observation(),
					timetableBlockGroupId: body.timetableBlockGroupId,
					manualLesson: null
				}
			}
		});
	});
	await page.goto(path);
	await page.getByRole('button', { name: 'แก้คาบ/วันเวลา', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByLabel('เหตุผล/หมายเหตุ', { exact: true })).toHaveValue('');
	await expect(dialog.getByRole('button', { name: 'บันทึกคาบนิเทศ', exact: true })).toBeDisabled();
	await dialog.getByRole('button', { name: /วิชาปัจจุบัน/ }).click();
	await dialog.getByRole('button', { name: 'บันทึกคาบนิเทศ', exact: true }).click();
	await expect.poll(() => writes.length).toBe(1);
	expect(writes[0]).toEqual({
		timetableBlockGroupId: current.blockGroupId,
		observedAt: '2026-10-01T01:30:00.000Z',
		manualLesson: null
	});
});

for (const origin of ['manual', 'linked'] as const)
	test(`ordinary ${origin} evidence keeps its actual source and existing reason`, async ({
		page
	}) => {
		const manual: ManualLesson = {
			subjectName: 'วิชาเดิม',
			classroomLabel: 'ม.1/1',
			periodLabel: 'คาบเดิม',
			roomLabel: null,
			observedAt: '2026-10-01T09:00:00Z',
			reason: 'เหตุผลที่บันทึกไว้จริง'
		};
		const fixture = await mock(page, {
			observationStatus: 'planned',
			observation: {
				timetableBlockGroupId: origin === 'linked' ? current.blockGroupId : null,
				manualLesson: origin === 'manual' ? manual : null,
				lessonSnapshot: origin === 'manual' ? { source: 'manual', ...manual } : rich
			}
		});
		await page.goto(path);
		await expect(
			page.getByText(origin === 'manual' ? 'กำหนดเอง' : 'ตารางสอน', { exact: true })
		).toBeVisible();
		await page.getByRole('button', { name: 'แก้คาบ/วันเวลา', exact: true }).click();
		await expect(
			page.getByRole('dialog').getByLabel('เหตุผล/หมายเหตุ', { exact: true })
		).toHaveValue(origin === 'manual' ? manual.reason : '');
		expect(fixture.writes).toEqual([]);
	});
