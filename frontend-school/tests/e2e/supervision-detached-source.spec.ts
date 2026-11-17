import { expect, test } from '@playwright/test';
import type { LessonSnapshot } from '../../src/lib/api/supervision';
import { mock, id, year, savedPeriod, timetableOption } from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });
const path = `/staff/academic/supervision/${id(30)}?academicYearId=${year}`;
const resource = `/api/supervision/observations/${id(30)}`;
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

for (const [name, snapshot] of [
	['rich', rich],
	['minimal', minimal]
] as const) {
	test(`${name} detached evidence remains readable with saved evaluations and no writes`, async ({
		page
	}) => {
		const api = await mock(page, {
			observationStatus: 'evaluators_submitted',
			averageRating: 4,
			observation: { periods: [savedPeriod({ lessonSnapshot: snapshot })] }
		});
		const original = api.observation();
		await page.goto(path);
		await expect(page.getByTestId('supervision-saved-periods')).toContainText(
			'ตารางสอนเดิม (ข้อมูลคาบที่บันทึกไว้)'
		);
		await expect(
			page.getByText(
				'คาบเดิมถูกนำออกจากตารางปัจจุบัน ข้อมูลคาบและผลการนิเทศที่บันทึกไว้ยังคงอยู่',
				{ exact: true }
			)
		).toBeVisible();
		if (name === 'rich')
			await expect(page.getByTestId('supervision-saved-periods')).toContainText(
				'คาบเดิมที่นิเทศจริง'
			);
		await page.getByRole('button', { name: 'ดูผลประเมินรายข้อ', exact: true }).click();
		await expect(page.getByTestId('supervision-review')).toContainText(
			'หัวข้อ rubric ที่โหลดเมื่อเปิด'
		);
		expect(api.observation()).toEqual(original);
		expect(original.evaluators[0].status).toBe('submitted');
		expect(original.averageRating).toBe(4);
		expect(api.writes).toEqual([]);
	});
}

test('partial historic manual facts and every saved period remain readable without manual inputs', async ({
	page
}) => {
	const api = await mock(page, {
		observation: {
			periods: [
				savedPeriod({
					id: id(70),
					lessonSnapshot: { source: 'manual' },
					manualLesson: {
						periodLabel: 'คาบเดิมบางส่วน',
						reason: 'หมายเหตุที่บันทึกไว้เดิม',
						classroomLabel: 'ม.3/2'
					}
				}),
				savedPeriod({
					id: id(71),
					position: 2,
					lessonSnapshot: {
						source: 'removed_timetable',
						subjectName: 'วิชาคาบที่สอง',
						periodLabel: 'คาบ 2',
						classroomLabel: 'ม.3/2'
					}
				})
			]
		}
	});
	await page.goto(path);
	const evidence = page.getByTestId('supervision-saved-periods');
	await expect(evidence).toContainText('คาบเดิมบางส่วน');
	await expect(evidence).toContainText('หมายเหตุที่บันทึกไว้เดิม');
	await expect(evidence).toContainText('วิชาคาบที่สอง');
	await page.getByRole('button', { name: 'แก้คาบ/วันเวลา', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.locator('input[type="time"], input[id^="supervision-manual-"]')).toHaveCount(
		0
	);
	await expect(dialog.getByTestId('supervision-saved-periods')).toContainText(
		'หมายเหตุที่บันทึกไว้เดิม'
	);
	await expect(dialog.getByRole('button', { name: 'บันทึกคาบนิเทศ', exact: true })).toBeDisabled();
	expect(api.writes).toEqual([]);
});

test('returned legacy evidence is never rebound until the teacher explicitly selects the current dated range', async ({
	page
}) => {
	const choices = [timetableOption(0), timetableOption(1), timetableOption(2)];
	const api = await mock(page, {
		permissions: ['supervision.request.own', 'supervision.read.own'],
		canManage: false,
		observationStatus: 'returned',
		bookingItems: choices,
		observation: { periods: [savedPeriod({ lessonSnapshot: rich })] }
	});
	await page.goto(path);
	await page.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByTestId('supervision-saved-periods')).toContainText(
		'คาบเดิมที่นิเทศจริง'
	);
	await expect(dialog.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true })).toBeDisabled();
	await dialog.getByRole('button', { name: /เลือก คาบ 1 ม.1\/1/ }).click();
	await dialog.getByRole('button', { name: 'เพิ่ม คาบ 2 ม.1/1', exact: true }).click();
	await dialog.getByRole('button', { name: 'เพิ่ม คาบ 3 ม.1/1', exact: true }).click();
	await dialog.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(api.writes).toEqual([`PATCH ${resource}/request`]);
	expect(api.payloads).toEqual([
		{
			observedDate: '2026-10-01',
			timetableBlockGroupIds: choices.map((item) => item.blockGroupId),
			expectedSourceDigest: 'source-1'
		}
	]);
	await expect(page.getByTestId('supervision-saved-periods')).toContainText('วิชาปัจจุบัน 3');
	expect(api.count(resource)).toBe(1);
});
