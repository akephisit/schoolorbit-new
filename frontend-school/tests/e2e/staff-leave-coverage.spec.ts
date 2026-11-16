import { test, expect, type Page } from '@playwright/test';
import {
	mockTeachingCoverage,
	coveragePlan,
	teachingLesson,
	coverageYear,
	englishOfferingId
} from './fixtures/staff-leave-coverage';
import { leaveId } from './fixtures/staff-leave';
import { actor, firstStaff, id } from './fixtures/staff-home-route-data';

const editPath = `/staff/leave/requests/${leaveId}/edit?step=coverage&returnTo=${encodeURIComponent('/staff/leave?status=draft&offset=20')}`;
test.use({ serviceWorkers: 'block' });
async function choose(page: Page, field: string, label: string) {
	await page.locator(`#${field}`).click();
	await page.getByRole('option', { name: label, exact: true }).click();
}
async function batchTeacher(page: Page) {
	await page.getByRole('button', { name: 'เลือกทุกคาบ', exact: true }).click();
	await page.getByRole('button', { name: 'ค้นหาครูที่ว่างทุกคาบ', exact: true }).click();
	await page.getByRole('button', { name: /ครูภาษาอังกฤษ ข้ามกลุ่มสาระ/ }).click();
	await page.locator('#coverage-activity').fill('กิจกรรมภาษาอังกฤษเสริม');
	await page.getByRole('button', { name: 'ใช้กับ 3 คาบที่เลือก', exact: true }).click();
}

test('multi-day batch keeps cross-department candidates, actual teaching activity, draft retry and complete-plan submit', async ({
	page
}) => {
	const api = await mockTeachingCoverage(page, { failSave: true });
	await page.goto(editPath);
	await expect(page.getByTestId('leave-coverage-editor')).toContainText(
		'เลือกครู 0/3 · ตอบรับ 0/3'
	);
	await batchTeacher(page);
	const query = api.calls.find((call) => call.path.endsWith('/candidates'))!;
	expect(query.body).toMatchObject({
		lessonKeys: ['lesson-1', 'lesson-2', 'lesson-3'],
		sameSubjectGroupOnly: false
	});
	await expect(page.getByRole('button', { name: 'ครูว่างเฉพาะวันแรก' })).toHaveCount(0);
	await expect(page.getByTestId('leave-coverage-editor')).toContainText(
		'เลือกครู 3/3 · ตอบรับ 0/3'
	);
	await page.getByRole('button', { name: 'บันทึกแผนคาบสอน', exact: true }).click();
	await expect(page.getByTestId('leave-coverage-editor')).toContainText('บันทึกแผนยังไม่พร้อม');
	await page.getByRole('button', { name: 'บันทึกแผนคาบสอน', exact: true }).click();
	const saves = api.calls.filter((call) => call.path.endsWith('/coverage'));
	expect(saves).toHaveLength(2);
	expect(saves[0].body.requestId).toBe(saves[1].body.requestId);
	expect(saves[1].body.assignments).toEqual(
		expect.arrayContaining([
			expect.objectContaining({
				lessonKey: 'lesson-3',
				replacementTeacherId: firstStaff,
				taughtActivity: 'กิจกรรมภาษาอังกฤษเสริม',
				taughtOfferingId: null
			})
		])
	);
	await page.getByRole('button', { name: 'ตรวจสอบคำขอ', exact: true }).click();
	await expect(page.getByTestId('leave-review-step')).toContainText('รอครูตอบรับ');
	await expect(page.getByRole('button', { name: 'ส่งคำขอพิจารณา', exact: true })).toBeEnabled();
	await page.getByRole('button', { name: 'ส่งคำขอพิจารณา', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('รอพิจารณา');
	await page.getByRole('link', { name: 'กลับรายการคำขอ', exact: true }).click();
	await expect(page).toHaveURL(/status=draft&offset=20/);
});

for (const theme of ['light', 'dark'] as const)
	for (const width of [390, 1440])
		test(`reciprocal return actual date and different-course same-room ${theme} ${width}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			await page.addInitScript(
				(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
				theme
			);
			await page.clock.setFixedTime(new Date('2026-10-18T03:00:00Z'));
			const api = await mockTeachingCoverage(page);
			await page.goto(editPath);
			await page.getByRole('checkbox', { name: 'เลือก 2026-10-19 คาบ 1', exact: true }).check();
			await page.getByRole('button', { name: 'ค้นหาครูที่ว่างทุกคาบ', exact: true }).click();
			await page.getByRole('button', { name: /ครูภาษาอังกฤษ ข้ามกลุ่มสาระ/ }).click();
			await choose(page, 'coverage-mode', 'แลกคาบสอน');
			await page.locator('#coverage-return-date').click();
			await page
				.locator('[role="application"]:visible [data-calendar-day]:not([data-outside-month])')
				.filter({ hasText: /^22$/ })
				.click();
			await page.getByRole('button', { name: 'ค้นหาคาบสอนคืน', exact: true }).click();
			await page.locator('#coverage-return-lesson').click();
			await page.getByRole('option', { name: /ภาษาอังกฤษ.*ม.2\/1/ }).click();
			await page.getByRole('button', { name: 'ใช้กับ 1 คาบที่เลือก', exact: true }).click();
			await page.getByRole('button', { name: 'บันทึกแผนคาบสอน', exact: true }).click();
			const saved = api.calls.find((call) => call.path.endsWith('/coverage'))!;
			expect(saved.body.assignments).toEqual([
				expect.objectContaining({
					lessonKey: 'lesson-1',
					mode: 'reciprocal',
					taughtOfferingId: englishOfferingId,
					returnLessonKey: 'return-english'
				})
			]);
			expect(api.returnLesson.learningGroupId).not.toBe(
				api.detail().coverage.days[0].lessons[0].learningGroupId
			);
			expect(api.returnLesson.homeroomIds).toEqual(
				api.detail().coverage.days[0].lessons[0].homeroomIds
			);
			await expect(page.getByTestId('leave-coverage-editor')).toContainText('สอนคณิตศาสตร์ คืน');
			await expect(page.getByTestId('leave-coverage-editor')).toContainText(
				'10:00–10:50 (แทนคาบภาษาอังกฤษ)'
			);
			await expect(page.getByTestId('leave-coverage-editor')).toContainText('ภาษาอังกฤษ');
			await page.getByText('สอนคณิตศาสตร์ คืน', { exact: false }).first().scrollIntoViewIfNeeded();
			await page.screenshot({
				path: testInfo.outputPath(`reciprocal-${theme}-${width}.png`),
				fullPage: true
			});
		});

test('draft details survive step history and reload resumes saved coverage deep link', async ({
	page
}) => {
	await mockTeachingCoverage(page);
	await page.goto(editPath.replace('step=coverage', 'step=details'));
	await page.getByLabel('เหตุผล *', { exact: true }).fill('ฉบับร่างที่ยังไม่บันทึก');
	await page.getByRole('button', { name: '2. จัดคาบสอน', exact: true }).click();
	await page.goBack();
	await expect(page.getByLabel('เหตุผล *', { exact: true })).toHaveValue('ฉบับร่างที่ยังไม่บันทึก');
	await page.getByRole('button', { name: '2. จัดคาบสอน', exact: true }).click();
	await page.reload();
	await expect(page.getByTestId('leave-coverage-editor')).toContainText('เลือกครู 0/3');
});

test('emergency sends only with explicit reason while ordinary incomplete plan stays blocked', async ({
	page
}) => {
	const api = await mockTeachingCoverage(page);
	await page.goto(editPath.replace('step=coverage', 'step=review'));
	await expect(page.getByRole('button', { name: 'ส่งคำขอพิจารณา', exact: true })).toBeDisabled();
	await expect(page.getByRole('button', { name: 'ส่งคำขอเร่งด่วน', exact: true })).toBeDisabled();
	await page.locator('#leave-emergency-reason').fill('ไปราชการตามคำสั่งเร่งด่วน');
	await page.getByRole('button', { name: 'ส่งคำขอเร่งด่วน', exact: true }).click();
	expect(api.calls.find((call) => call.path.endsWith('/submit'))!.body.emergencyReason).toBe(
		'ไปราชการตามคำสั่งเร่งด่วน'
	);
	await expect(page.getByTestId('leave-detail')).toContainText('คำขอเร่งด่วน');
});

test('missing timetable and valid zero lessons remain distinct', async ({ page }) => {
	const plan = coveragePlan();
	plan.days = [];
	plan.timetableState = 'missing';
	plan.sourceDigest = null;
	await mockTeachingCoverage(page, { plan });
	await page.goto(editPath);
	await expect(page.getByTestId('leave-coverage-editor')).toContainText('ยังไม่มีตารางสอน');
	await expect(page.getByText('ไม่มีคาบสอนที่ได้รับผลกระทบ', { exact: true })).toHaveCount(0);
	plan.timetableState = 'ready';
	plan.sourceDigest = 'ready-zero';
	plan.state = 'confirmed';
	await page.reload();
	await expect(page.getByText('ไม่มีคาบสอนที่ได้รับผลกระทบ', { exact: true })).toBeVisible();
	await page.goto(editPath.replace('step=coverage', 'step=details'));
	await expect(
		page.getByText('เวลาสิ้นสุดช่วงเช้าของโรงเรียน: 11:30', { exact: true })
	).toBeVisible();
});

test('recipient confirms multiple own rows through redacted task without detail or evidence reads', async ({
	page
}, testInfo) => {
	const plan = coveragePlan();
	for (const day of plan.days)
		for (const lesson of day.lessons)
			lesson.assignment = {
				id: id(950 + Number(lesson.lessonKey.split('-')[1])),
				replacementTeacherId: actor,
				replacementTeacherName: 'ครูภาษาอังกฤษ',
				mode: 'substitution',
				taughtOfferingId: englishOfferingId,
				taughtOfferingName: 'ภาษาอังกฤษ',
				taughtActivity: null,
				returnLessonKey: null,
				returnDate: null,
				returnLesson: null,
				consent: 'pending',
				confirmedAt: null,
				taughtLearningGroupId: id(871)
			};
	plan.state = 'awaiting_confirmation';
	const api = await mockTeachingCoverage(page, { plan, status: 'submitted', assigned: true });
	await page.goto('/staff/leave/assigned');
	await expect(page.getByTestId('leave-assigned-tasks')).toContainText('เลือก 3 คาบ');
	await expect(page.getByTestId('leave-assigned-tasks')).not.toContainText('เข้ารับการตรวจรักษา');
	expect(api.api.count(`/api/staff-leave/requests/${leaveId}`)).toBe(0);
	expect(api.api.fileReads).toHaveLength(0);
	await page.getByRole('button', { name: 'ตอบรับคาบที่เลือก', exact: true }).click();
	await expect(page.getByText('ตอบรับแล้ว', { exact: true })).toHaveCount(3);
	await page.screenshot({ path: testInfo.outputPath('assigned-desktop.png'), fullPage: true });
	const confirmation = api.calls.find((call) => call.path.endsWith('/confirm'))!;
	expect(confirmation.body).toMatchObject({
		expectedRevision: 1,
		assignmentIds: [id(951), id(952), id(953)],
		decision: 'confirmed'
	});
});

for (const theme of ['light', 'dark'] as const)
	for (const width of [390, 1440])
		test(`coverage layout and candidate keyboard ${theme} ${width}`, async ({ page }, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			await page.addInitScript(
				(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
				theme
			);
			await mockTeachingCoverage(page);
			await page.goto(editPath);
			await page.getByRole('button', { name: 'เลือกทุกคาบ', exact: true }).click();
			await page.getByRole('checkbox', { name: 'สอนระดับชั้นเดียวกัน', exact: true }).check();
			await page
				.getByRole('checkbox', { name: 'สอนนักเรียนกลุ่มเดียวกัน/ห้องเดียวกัน', exact: true })
				.check();
			await page.getByRole('button', { name: 'ค้นหาครูที่ว่างทุกคาบ', exact: true }).click();
			const teacher = page.getByRole('button', { name: /ครูภาษาอังกฤษ ข้ามกลุ่มสาระ/ });
			await teacher.focus();
			await page.keyboard.press('Enter');
			await expect(page.locator('#coverage-activity')).toBeVisible();
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({
				path: testInfo.outputPath(`coverage-${theme}-${width}.png`),
				fullPage: true
			});
			await page.locator('#coverage-activity').scrollIntoViewIfNeeded();
			await page.screenshot({
				path: testInfo.outputPath(`coverage-teaching-${theme}-${width}.png`),
				fullPage: true
			});
		});

test('missing half-day boundary is explicit and manager saves actual school setting', async ({
	page
}) => {
	const plan = coveragePlan();
	plan.sourceDigest = null;
	plan.days = [];
	plan.timetableState = 'legacy';
	plan.blockingReason = 'โรงเรียนยังไม่กำหนดเวลาครึ่งวัน';
	const api = await mockTeachingCoverage(page, { plan, missingBoundary: true });
	await page.goto(editPath);
	await expect(page.getByTestId('leave-coverage-editor')).toContainText(
		'โรงเรียนยังไม่กำหนดเวลาครึ่งวัน'
	);
	await page.getByRole('button', { name: '1. รายละเอียด', exact: true }).click();
	await expect(page.getByTestId('leave-request-editor')).toContainText('ยังไม่กำหนด');
	await expect(page.getByTestId('leave-request-editor')).not.toContainText('12:00');
	await page.goto('/staff/leave/settings');
	await page.locator('#coverage-half-day-boundary').fill('11:20');
	await page.getByRole('button', { name: 'บันทึกเวลาครึ่งวัน', exact: true }).click();
	await expect.poll(() => api.settings.halfDayBoundary).toBe('11:20:00');
	await page.goto(editPath.replace('step=coverage', 'step=details'));
	await expect(page.getByTestId('leave-request-editor')).toContainText(
		'เวลาสิ้นสุดช่วงเช้าของโรงเรียน: 11:20'
	);
});

test('emergency manager completes coverage while administrative approval remains awaiting consent', async ({
	page
}) => {
	const api = await mockTeachingCoverage(page, { manager: true, status: 'submitted' });
	await page.goto(`/staff/leave/requests/${leaveId}`);
	await expect(page.getByTestId('leave-detail')).toContainText('คำขอเร่งด่วน');
	await expect(page.getByRole('button', { name: 'อนุมัติ', exact: true })).toHaveCount(0);
	await batchTeacher(page);
	await page.getByRole('button', { name: 'บันทึกแผนคาบสอน', exact: true }).click();
	await expect(page.getByTestId('leave-coverage-editor')).toContainText(
		'เลือกครู 3/3 · ตอบรับ 0/3'
	);
	expect(api.calls.filter((call) => call.path.endsWith('/coverage'))).toHaveLength(1);
	await expect(page.getByRole('button', { name: 'อนุมัติ', exact: true })).toHaveCount(0);
});

test('superseded candidate response cannot reopen cleared lesson selection', async ({ page }) => {
	const api = await mockTeachingCoverage(page);
	let release = () => {};
	const gate = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/coverage/candidates', async (route) => {
		await gate;
		await route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify({
				success: true,
				data: { candidates: api.teachers, subjectGroups: [], gradeLevels: [], hasMore: false }
			})
		});
	});
	await page.goto(editPath);
	const row = page.getByRole('checkbox', { name: 'เลือก 2026-10-19 คาบ 1', exact: true });
	await row.check();
	await page.getByRole('button', { name: 'ค้นหาครูที่ว่างทุกคาบ', exact: true }).click();
	await row.uncheck();
	release();
	await expect(page.getByText('เลือกครูสำหรับ 0 คาบ', { exact: true })).toBeVisible();
	await expect(page.getByRole('list', { name: 'ครูที่ว่าง', exact: true })).toHaveCount(0);
});

for (const theme of ['light', 'dark'] as const)
	test(`recipient mobile confirmation privacy ${theme}`, async ({ page }, testInfo) => {
		await page.setViewportSize({ width: 390, height: 844 });
		await page.addInitScript(
			(theme) => localStorage.setItem('ui-preferences', JSON.stringify({ theme })),
			theme
		);
		const plan = coveragePlan();
		for (const day of plan.days)
			for (const lesson of day.lessons)
				lesson.assignment = {
					id: id(950 + Number(lesson.lessonKey.split('-')[1])),
					replacementTeacherId: actor,
					replacementTeacherName: 'ครูผู้รับมอบหมาย',
					mode: 'reciprocal',
					taughtOfferingId: englishOfferingId,
					taughtOfferingName: 'ภาษาอังกฤษ',
					taughtActivity: null,
					returnLessonKey: 'return-english',
					returnDate: '2026-10-22',
					returnLesson:
						lesson.lessonKey === 'lesson-1'
							? {
									...teachingLesson(9, '2026-10-22'),
									teacherId: actor,
									learningGroupId: id(871),
									offeringId: englishOfferingId,
									offeringName: 'ภาษาอังกฤษ'
								}
							: null,
					consent: 'pending',
					confirmedAt: null,
					taughtLearningGroupId: id(871)
				};
		plan.state = 'awaiting_confirmation';
		await mockTeachingCoverage(page, { plan, status: 'submitted', assigned: true });
		await page.goto('/staff/leave/assigned');
		await expect(page.getByTestId('leave-assigned-task')).toContainText('ภาษาอังกฤษ');
		await expect(page.getByTestId('leave-assigned-task')).not.toContainText('เข้ารับการตรวจรักษา');
		await expect(page.getByTestId('leave-assigned-task')).toContainText('ผู้ขอจะสอนคณิตศาสตร์ คืน');
		await expect(page.getByTestId('leave-assigned-task')).not.toContainText('· ·');
		await expect(page.getByTestId('leave-assigned-task')).toContainText(
			'10:00–10:50 (แทนคาบภาษาอังกฤษ)'
		);
		await page
			.getByRole('button', { name: 'ปฏิเสธคาบที่เลือก', exact: true })
			.scrollIntoViewIfNeeded();
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
		await page.screenshot({
			path: testInfo.outputPath(`assigned-mobile-${theme}.png`),
			fullPage: true
		});
		await page.getByRole('button', { name: 'ปฏิเสธคาบที่เลือก', exact: true }).click();
		await expect(page.getByText('ปฏิเสธแล้ว', { exact: true })).toHaveCount(3);
	});

test('declined coverage returns request to revision without withdrawal and preserves request identity', async ({
	page
}) => {
	const plan = coveragePlan();
	const lesson = plan.days[0].lessons[0];
	lesson.assignment = {
		id: id(951),
		replacementTeacherId: firstStaff,
		replacementTeacherName: 'ครูภาษาอังกฤษ',
		mode: 'substitution',
		taughtOfferingId: englishOfferingId,
		taughtOfferingName: 'ภาษาอังกฤษ',
		taughtActivity: null,
		returnLessonKey: null,
		returnDate: null,
		returnLesson: null,
		consent: 'declined',
		confirmedAt: '2026-10-18T03:00:00Z',
		taughtLearningGroupId: id(871)
	};
	const api = await mockTeachingCoverage(page, { plan, status: 'submitted' });
	await page.goto(`/staff/leave/requests/${leaveId}`);
	await page.getByRole('button', { name: 'นำกลับมาจัดแผนใหม่', exact: true }).click();
	await expect(page.getByRole('link', { name: 'แก้ไขฉบับร่าง', exact: true })).toBeVisible();
	expect(
		api.calls.find((call) => call.path.endsWith('/coverage/replan'))!.body.expectedRevision
	).toBe(1);
	expect(api.calls.some((call) => call.path.endsWith('/cancel'))).toBe(false);
	await page.getByRole('link', { name: 'แก้ไขฉบับร่าง', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`${leaveId}/edit`));
});

test('editing saved exit date time and valid academic context consumes reconciled plan then can select save and send', async ({
	page
}) => {
	const initial = coveragePlan();
	initial.days = initial.days.slice(0, 1);
	const updated = coveragePlan();
	updated.academicTermId = id(801);
	updated.sourceDigest = 'digest-reconciled-details';
	updated.planHash = 'plan-after-details';
	updated.revision = 3;
	updated.days = [
		{
			date: '2026-10-21',
			lessons: [
				{
					...teachingLesson(4, '2026-10-21'),
					startTime: '11:00:00',
					endTime: '11:50:00',
					assignment: null
				}
			]
		}
	];
	const api = await mockTeachingCoverage(page, {
		plan: initial,
		kind: 'exit',
		updatedCoverage: updated
	});
	api.settings.contexts.push({
		academicYearId: coverageYear,
		academicYearName: '2569',
		academicTermId: id(801),
		academicTermName: '2'
	});
	await page.goto(editPath);
	await page.getByRole('button', { name: 'เลือกทุกคาบ', exact: true }).click();
	await page.getByRole('button', { name: 'ค้นหาครูที่ว่างทุกคาบ', exact: true }).click();
	await page.getByRole('button', { name: /ครูภาษาอังกฤษ ข้ามกลุ่มสาระ/ }).click();
	await page.locator('#coverage-activity').fill('กิจกรรมก่อนเปลี่ยนช่วงเวลา');
	await page.getByRole('button', { name: 'ใช้กับ 2 คาบที่เลือก', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกแผนคาบสอน', exact: true }).click();
	await expect(page.getByTestId('leave-coverage-editor')).toContainText('เลือกครู 2/2');

	let releaseOld = () => {},
		startedOld = () => {},
		finishedOld = () => {};
	const oldGate = new Promise<void>((resolve) => (releaseOld = resolve));
	const oldStarted = new Promise<void>((resolve) => (startedOld = resolve));
	const oldFinished = new Promise<void>((resolve) => (finishedOld = resolve));
	let holdOld = true;
	await page.route('**/coverage/candidates', async (route) => {
		if (!holdOld) return route.fallback();
		holdOld = false;
		startedOld();
		await oldGate;
		await route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify({
				success: true,
				data: { candidates: api.teachers, subjectGroups: [], gradeLevels: [], hasMore: false }
			})
		});
		finishedOld();
	});
	await page.getByRole('checkbox', { name: 'เลือก 2026-10-19 คาบ 1', exact: true }).check();
	await page.getByRole('button', { name: 'ค้นหาครูที่ว่างทุกคาบ', exact: true }).click();
	await oldStarted;
	await page.getByRole('button', { name: '1. รายละเอียด', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'ใช้ข้อมูลล่าสุดและเก็บฉบับร่าง', exact: true })
	).toHaveCount(0);
	for (const field of ['leave-start-date', 'leave-end-date']) {
		await page.locator(`#${field}`).click();
		await page
			.locator('[role="application"]:visible [data-calendar-day]:not([data-outside-month])')
			.filter({ hasText: /^21$/ })
			.click();
	}
	await page.locator('#leave-start-time').fill('11:00');
	await page.locator('#leave-end-time').fill('12:00');
	await choose(page, 'leave-academic-term', '2');
	await page.getByRole('button', { name: 'บันทึกและจัดคาบสอนต่อ', exact: true }).click();
	const editor = page.getByTestId('leave-coverage-editor');
	await expect(editor).toContainText('เลือกครู 0/1');
	releaseOld();
	await oldFinished;
	await expect(page.getByRole('list', { name: 'ครูที่ว่าง', exact: true })).toHaveCount(0);
	await expect(editor).toContainText('11:00–11:50');
	await expect(
		page.getByRole('checkbox', { name: 'เลือก 2026-10-21 คาบ 4', exact: true })
	).toBeVisible();
	await expect(
		page.getByRole('checkbox', { name: 'เลือก 2026-10-19 คาบ 1', exact: true })
	).toHaveCount(0);
	expect(
		api.calls.find((call) => call.path === `/api/staff-leave/requests/${leaveId}`)!.body.period
	).toMatchObject({
		startDate: '2026-10-21',
		endDate: '2026-10-21',
		startTime: '11:00:00',
		endTime: '12:00:00',
		academicTermId: id(801)
	});
	await page.getByRole('checkbox', { name: 'เลือก 2026-10-21 คาบ 4', exact: true }).check();
	await page.getByRole('button', { name: 'ค้นหาครูที่ว่างทุกคาบ', exact: true }).click();
	await page.getByRole('button', { name: /ครูภาษาอังกฤษ ข้ามกลุ่มสาระ/ }).click();
	await page.locator('#coverage-activity').fill('กิจกรรมหลังเปลี่ยนช่วงเวลา');
	await page.getByRole('button', { name: 'ใช้กับ 1 คาบที่เลือก', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกแผนคาบสอน', exact: true }).click();
	expect(
		api.calls.filter((call) => call.path.endsWith('/coverage/candidates')).at(-1)!.body
	).toMatchObject({
		sourceDigest: 'digest-reconciled-details',
		lessonKeys: ['lesson-4'],
		expectedRevision: 3
	});
	expect(api.calls.some((call) => call.path.endsWith('/coverage/preview'))).toBe(false);
	await page.getByRole('button', { name: 'ตรวจสอบคำขอ', exact: true }).click();
	await page.getByRole('button', { name: 'ส่งคำขอพิจารณา', exact: true }).click();
	await expect(page.getByTestId('leave-detail')).toContainText('รอพิจารณา');
});
