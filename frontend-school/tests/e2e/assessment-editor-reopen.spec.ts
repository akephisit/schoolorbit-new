import type {
	SaveAssessmentPlanRequest,
	UpdateAssessmentPhaseControlRequest
} from '../../src/lib/api/academicAssessments';
import { sessionState } from './fixtures/session-state';
import { revalidatePage, expectSilentBackground } from './fixtures/page-revalidation';
import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const academicYearId = '10000000-0000-4000-8000-000000000001';
const academicTermId = '20000000-0000-4000-8000-000000000001';
const teacherId = '30000000-0000-4000-8000-000000000001';
const offeringIds = {
	mathematics: '40000000-0000-4000-8000-000000000001',
	science: '40000000-0000-4000-8000-000000000002'
};

const phaseCodes = ['before_midterm', 'midterm', 'after_midterm', 'final'] as const;

function phases(scores = ['20', '20', '30', '30']) {
	return phaseCodes.map((phaseCode, index) => ({
		id: `50000000-0000-4000-8000-00000000000${index + 1}`,
		phaseCode,
		label: ['ก่อนกลางภาค', 'กลางภาค', 'หลังกลางภาค', 'ปลายภาค'][index],
		order: index + 1,
		maxScore: scores[index],
		examArrangement: phaseCode === 'midterm' || phaseCode === 'final' ? 'in_timetable' : 'none',
		examDurationMinutes: phaseCode === 'midterm' || phaseCode === 'final' ? 60 : null,
		rowVersion: 1
	}));
}

function assessmentPlan(
	offeringId: string,
	offeringCode: string,
	offeringName: string,
	scores?: string[]
) {
	return {
		id: `60000000-0000-4000-8000-${offeringId.slice(-12)}`,
		planId: `60000000-0000-4000-8000-${offeringId.slice(-12)}`,
		offeringId,
		offeringCode,
		offeringName,
		subjectId: `70000000-0000-4000-8000-${offeringId.slice(-12)}`,
		subjectVersionDisplayLabel: 'มัธยมศึกษาปีที่ 1',
		academicYearId,
		academicTermId,
		learningGroupIds: [`80000000-0000-4000-8000-${offeringId.slice(-12)}`],
		learningGroupCount: 1,
		assessmentCoordinatorId: teacherId,
		assessmentCoordinatorName: 'ครูทดสอบ ระบบ',
		suggestedCoordinatorId: teacherId,
		suggestedCoordinatorName: 'ครูทดสอบ ระบบ',
		coordinatorCandidates: [
			{
				teacherId,
				displayName: 'ครูทดสอบ ระบบ',
				learningGroupCount: 1,
				primaryLearningGroupCount: 1
			}
		],
		assessmentTotalScore: '100',
		phases: phases(scores),
		readiness: {
			ready: true,
			findings: [],
			totalScore: (scores ?? ['20', '20', '30', '30'])
				.reduce((total, score) => total + Number(score), 0)
				.toString(),
			expectedTotalScore: '100'
		},
		rowVersion: 1
	};
}

function fulfill(route: Route, data: unknown) {
	return route.fulfill({
		status: 200,
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

async function mockAssessmentApis(
	page: Page,
	options: {
		plansWait?: Promise<void>;
		plansRefreshWait?: Promise<void>;
		controlsWait?: Promise<void>;
		onPlansRead?: () => void;
		failPlansOnce?: boolean;
		failPlansOnRead?: number;
		failControlsOnce?: boolean;
		assignedTeacher?: boolean;
		phaseEditing?: Partial<Record<(typeof phaseCodes)[number], { score: boolean; exam: boolean }>>;
		controlWriteWait?: Promise<void>;
		failControlWriteOnce?: boolean;
		onControlWrite?: (controlId: string, payload: UpdateAssessmentPhaseControlRequest) => void;
		onPlanWrite?: (payload: SaveAssessmentPlanRequest) => void;
	} = {}
) {
	let mathematics = assessmentPlan(offeringIds.mathematics, 'ค21101', 'คณิตศาสตร์พื้นฐาน');
	const science = assessmentPlan(offeringIds.science, 'ว21101', 'วิทยาศาสตร์พื้นฐาน');
	let plansFailed = false;
	let controlsFailed = false;
	let planReads = 0;
	let controlWriteFailed = false;
	const controls = phaseCodes.map((phaseCode, index) => ({
		id: `90000000-0000-4000-8000-00000000000${index + 1}`,
		academicYearId,
		academicTermId,
		phaseCode,
		planEditingEnabled: options.phaseEditing?.[phaseCode]?.score ?? true,
		examArrangementEditingEnabled:
			(phaseCode === 'midterm' || phaseCode === 'final') &&
			(options.phaseEditing?.[phaseCode]?.exam ?? true),
		rowVersion: 1
	}));

	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: teacherId,
					username: 'academic-admin',
					firstName: 'ทดสอบ',
					displayName: 'ทดสอบ ระบบ',
					lastName: 'ระบบ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					createdAt: '2026-09-02T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: options.assignedTeacher
						? ['academic_assessment.read.assigned', 'academic_assessment.manage.assigned']
						: ['academic_assessment.read.school', 'academic_assessment.manage.school']
				});
				return;
			}
			if (url.pathname === '/api/academic/context/options') {
				await fulfill(route, {
					defaultAcademicYearId: academicYearId,
					defaultAcademicTermId: academicTermId,
					years: [
						{
							isVisible: true,
							id: academicYearId,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active'
						}
					],
					terms: [
						{
							isVisible: true,
							id: academicTermId,
							academicYearId,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
				return;
			}
			if (url.pathname === '/api/academic/assessments/plans') {
				planReads += 1;
				options.onPlansRead?.();
				await (planReads === 1 ? options.plansWait : options.plansRefreshWait);
				if ((options.failPlansOnce && !plansFailed) || options.failPlansOnRead === planReads) {
					plansFailed = true;
					await route.fulfill({
						status: 500,
						contentType: 'application/json',
						body: '{"success":false,"error":"plans failed"}'
					});
					return;
				}
				await fulfill(route, [mathematics, science]);
				return;
			}
			if (url.pathname === '/api/academic/assessments/phase-controls') {
				await options.controlsWait;
				if (options.failControlsOnce && !controlsFailed) {
					controlsFailed = true;
					await route.fulfill({
						status: 500,
						contentType: 'application/json',
						body: '{"success":false,"error":"controls failed"}'
					});
					return;
				}
				await fulfill(route, controls);
				return;
			}
			if (url.pathname.startsWith('/api/academic/assessments/phase-controls/')) {
				const controlId = url.pathname.split('/').at(-1);
				const control = controls.find((item) => item.id === controlId);
				if (!control) throw new Error('Unknown assessment phase control');
				const body = route.request().postDataJSON() as UpdateAssessmentPhaseControlRequest;
				options.onControlWrite?.(control.id, body);
				await options.controlWriteWait;
				if (options.failControlWriteOnce && !controlWriteFailed) {
					controlWriteFailed = true;
					control.rowVersion += 1;
					await route.fulfill({
						status: 409,
						contentType: 'application/json',
						body: JSON.stringify({ success: false, error: 'phase control conflict' })
					});
					return;
				}
				Object.assign(control, body, { rowVersion: control.rowVersion + 1 });
				await fulfill(route, control);
				return;
			}
			if (url.pathname === `/api/academic/assessments/offerings/${offeringIds.mathematics}`) {
				if (route.request().method() === 'PUT') {
					const body = route.request().postDataJSON() as SaveAssessmentPlanRequest;
					options.onPlanWrite?.(body);
					mathematics = {
						...mathematics,
						rowVersion: mathematics.rowVersion + 1,
						phases: mathematics.phases.map((phase) => {
							const saved = body.phases.find((item) => item.phaseCode === phase.phaseCode);
							if (!saved) throw new Error('Missing assessment phase');
							return {
								...phase,
								maxScore: saved.maxScore,
								examArrangement: saved.examArrangement,
								examDurationMinutes: saved.examDurationMinutes ?? null
							};
						})
					};
				}
				await fulfill(route, mathematics);
				return;
			}
			if (url.pathname === `/api/academic/assessments/offerings/${offeringIds.science}`) {
				await fulfill(route, science);
				return;
			}
			if (url.pathname === '/api/notifications/stream') {
				await route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
				return;
			}
			if (url.pathname === '/api/menu/user') {
				await fulfill(route, { groups: [] });
				return;
			}
			if (url.pathname === '/api/me/work-items/counts') {
				await fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
				return;
			}
			if (url.pathname === '/api/notifications') {
				await fulfill(route, { items: [], unread_count: 0 });
				return;
			}
			await fulfill(route, {});
		}
	);
}

test('phase controls render while the plan list is still pending', async ({ page }) => {
	const plans = deferred();
	await mockAssessmentApis(page, { plansWait: plans.promise });
	try {
		await page.goto(
			`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
		);
		await expect(page.getByRole('heading', { name: 'ช่วงการทำงานของครู' })).toBeVisible();
		await expect(page.getByLabel('แก้คะแนนเต็ม', { exact: true }).first()).toBeVisible();
		await expect(page.getByTestId('assessment-plans-loading')).toBeVisible();
	} finally {
		plans.resolve();
	}
	await expect(page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
});

test('plan list renders while phase controls are still pending', async ({ page }) => {
	const controls = deferred();
	await mockAssessmentApis(page, { controlsWait: controls.promise });
	try {
		await page.goto(
			`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
		);
		await expect(page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
		await expect(page.getByTestId('assessment-controls-loading')).toBeVisible();
	} finally {
		controls.resolve();
	}
	await expect(page.getByLabel('แก้คะแนนเต็ม', { exact: true }).first()).toBeVisible();
});

test('a failed plan list retries without hiding ready phase controls', async ({ page }) => {
	await mockAssessmentApis(page, { failPlansOnce: true });
	await page.goto(
		`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
	);
	await expect(page.getByLabel('แก้คะแนนเต็ม', { exact: true }).first()).toBeVisible();
	await expect(page.getByText('plans failed')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
});

test('failed phase controls retry without hiding the plan list', async ({ page }) => {
	await mockAssessmentApis(page, { failControlsOnce: true });
	await page.goto(
		`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
	);
	await expect(page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
	await expect(page.getByText('controls failed')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByLabel('แก้คะแนนเต็ม', { exact: true }).first()).toBeVisible();
});

test('page revalidation retains the plan list during a delayed failure and retries only that region', async ({
	page
}) => {
	const refresh = deferred();
	let plansReadCount = 0;
	await mockAssessmentApis(page, {
		plansRefreshWait: refresh.promise,
		failPlansOnRead: 2,
		onPlansRead: () => (plansReadCount += 1)
	});
	await page.goto(
		`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
	);
	await expect(page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
	await revalidatePage(page);
	try {
		await expectSilentBackground(
			page,
			page
				.locator('main [aria-busy]')
				.filter({ has: page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true }) })
				.last()
		);
		await expect(page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
	} finally {
		refresh.resolve();
	}
	await expect(page.getByText('plans failed')).toBeVisible();
	await expect(page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('plans failed')).toBeHidden();
	await expect.poll(() => plansReadCount).toBe(3);
});

test('closes after an in-flight autosave and opens the next subject editor', async ({ page }) => {
	let plansReadCount = 0;
	await mockAssessmentApis(page, { onPlansRead: () => (plansReadCount += 1) });
	await page.goto(
		`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
	);

	await page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true }).click();
	await expect(page.getByRole('heading', { name: 'ค21101 · คณิตศาสตร์พื้นฐาน' })).toBeVisible();

	await page.getByLabel('คะแนนเต็ม', { exact: true }).first().fill('21');
	await page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' }).click();
	await expect(page.getByRole('heading', { name: 'ค21101 · คณิตศาสตร์พื้นฐาน' })).toBeHidden();
	await expect.poll(() => plansReadCount).toBe(1);
	await expect(
		page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true }).locator('..').locator('..')
	).toContainText('21');

	await page.getByText('ว21101 · วิทยาศาสตร์พื้นฐาน', { exact: true }).click();
	await expect(page.getByRole('heading', { name: 'ว21101 · วิทยาศาสตร์พื้นฐาน' })).toBeVisible();
	await expect(page.getByLabel('คะแนนเต็ม', { exact: true }).first()).toHaveValue('20');
});

test('school managers toggle score and exam editing independently with minimal versioned patches', async ({
	page
}) => {
	const writes: Array<{ controlId: string; payload: UpdateAssessmentPhaseControlRequest }> = [];
	const pending = deferred();
	await mockAssessmentApis(page, {
		controlWriteWait: pending.promise,
		onControlWrite: (controlId, payload) => writes.push({ controlId, payload })
	});
	await page.goto(
		`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
	);
	const scores = page.getByLabel('แก้คะแนนเต็ม', { exact: true });
	const exams = page.getByLabel('แก้การจัดสอบและเวลาสอบ', { exact: true });
	await expect(scores).toHaveCount(4);
	await expect(exams).toHaveCount(2);
	await expect(scores.nth(1)).toBeChecked();
	await expect(exams.first()).toBeChecked();
	await scores.nth(1).click();
	try {
		await expect.poll(() => writes.length).toBe(1);
		await expect(scores.nth(1)).toBeDisabled();
		await expect(exams.first()).toBeDisabled();
		expect(writes[0]).toEqual({
			controlId: '90000000-0000-4000-8000-000000000002',
			payload: { rowVersion: 1, planEditingEnabled: false }
		});
	} finally {
		pending.resolve();
	}
	await expect(scores.nth(1)).not.toBeChecked();
	await expect(exams.first()).toBeChecked();
	await exams.first().focus();
	await page.keyboard.press('Space');
	await expect(exams.first()).not.toBeChecked();
	expect(writes[1]).toEqual({
		controlId: '90000000-0000-4000-8000-000000000002',
		payload: { rowVersion: 2, examArrangementEditingEnabled: false }
	});
	await expect(scores.nth(1)).not.toBeChecked();
	await expect(scores.last()).toBeChecked();
	await expect(exams.last()).toBeChecked();
	await page.screenshot({
		path: '/tmp/schoolorbit-assessment-controls-desktop.png',
		fullPage: true
	});

	// School managers retain the existing override when both teacher controls are closed.
	await page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true }).click();
	await expect(page.locator('#phase-score-midterm')).toBeEnabled();
	await expect(page.locator('#phase-arrangement-midterm')).toBeEnabled();
	await expect(page.locator('#phase-duration-midterm')).toBeEnabled();
});

test('a rejected phase-control patch keeps saved switches and reloads the latest revision for retry', async ({
	page
}) => {
	const writes: UpdateAssessmentPhaseControlRequest[] = [];
	await mockAssessmentApis(page, {
		failControlWriteOnce: true,
		onControlWrite: (_id, payload) => writes.push(payload)
	});
	await page.goto(
		`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
	);
	const score = page.getByLabel('แก้คะแนนเต็ม', { exact: true }).nth(1);
	const exam = page.getByLabel('แก้การจัดสอบและเวลาสอบ', { exact: true }).first();
	await score.click();
	const error = page.getByRole('alert').filter({ hasText: 'phase control conflict' });
	await expect(error).toBeVisible();
	await expect(score).toBeEnabled();
	await expect(score).toBeChecked();
	await expect(exam).toBeChecked();
	expect(writes).toEqual([{ rowVersion: 1, planEditingEnabled: false }]);
	await page.screenshot({
		path: '/tmp/schoolorbit-assessment-controls-conflict.png',
		fullPage: true
	});
	await error.getByRole('button', { name: 'ลองใหม่', exact: true }).click();
	await expect(error).toBeHidden();
	await expect(score).toBeChecked();
	await score.click();
	await expect(score).not.toBeChecked();
	await expect(exam).toBeChecked();
	expect(writes[1]).toEqual({ rowVersion: 2, planEditingEnabled: false });
});

for (const theme of ['light', 'dark'] as const) {
	test(`assigned teachers edit only the separately enabled fields on mobile in ${theme} mode`, async ({
		page
	}) => {
		await page.setViewportSize({ width: 390, height: 844 });
		await page.emulateMedia({ colorScheme: theme });
		const writes: SaveAssessmentPlanRequest[] = [];
		await mockAssessmentApis(page, {
			assignedTeacher: true,
			phaseEditing: {
				before_midterm: { score: false, exam: false },
				midterm: { score: false, exam: true },
				final: { score: true, exam: false }
			},
			onPlanWrite: (payload) => writes.push(payload)
		});
		await page.goto(
			`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
		);
		await expect(page.getByRole('heading', { name: 'ช่วงการทำงานของครู' })).toHaveCount(0);
		await page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true }).click();
		await expect(page.locator('#phase-score-before_midterm')).toBeDisabled();
		await expect(page.locator('#phase-score-midterm')).toBeDisabled();
		await expect(page.locator('#phase-arrangement-midterm')).toBeEnabled();
		await expect(page.locator('#phase-duration-midterm')).toBeEnabled();
		await expect(page.locator('#phase-score-final')).toBeEnabled();
		await expect(page.locator('#phase-arrangement-final')).toBeDisabled();
		await expect(page.locator('#phase-duration-final')).toBeDisabled();
		const midterm = page.locator('section').filter({ has: page.locator('#phase-score-midterm') });
		const final = page.locator('section').filter({ has: page.locator('#phase-score-final') });
		await expect(midterm).toContainText('ปิดแก้คะแนนเต็ม');
		await expect(midterm).not.toContainText('ปิดแก้การจัดสอบ');
		await expect(final).toContainText('ปิดแก้การจัดสอบ');
		await expect(final).not.toContainText('ปิดแก้คะแนนเต็ม');
		await page.locator('#phase-score-midterm').scrollIntoViewIfNeeded();
		await page.screenshot({
			path: `/tmp/schoolorbit-assessment-assigned-mobile-${theme}.png`,
			fullPage: true
		});
		await page.locator('#phase-duration-midterm').fill('75');
		await page.locator('#phase-score-final').fill('31');
		await page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' }).click();
		await expect(page.getByRole('heading', { name: 'ค21101 · คณิตศาสตร์พื้นฐาน' })).toBeHidden();
		await expect.poll(() => writes.length).toBeGreaterThan(0);
		expect(writes.at(-1)?.phases).toEqual([
			expect.objectContaining({
				phaseCode: 'before_midterm',
				maxScore: '20',
				examArrangement: 'none',
				examDurationMinutes: null
			}),
			expect.objectContaining({
				phaseCode: 'midterm',
				maxScore: '20',
				examArrangement: 'in_timetable',
				examDurationMinutes: 75
			}),
			expect.objectContaining({
				phaseCode: 'after_midterm',
				maxScore: '30',
				examArrangement: 'none',
				examDurationMinutes: null
			}),
			expect.objectContaining({
				phaseCode: 'final',
				maxScore: '31',
				examArrangement: 'in_timetable',
				examDurationMinutes: 60
			})
		]);
		await page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true }).click();
		await expect(page.locator('#phase-duration-midterm')).toHaveValue('75');
		await expect(page.locator('#phase-score-final')).toHaveValue('31');
		await page.locator('#phase-arrangement-midterm').click();
		await page.getByRole('option', { name: 'สอบนอกตารางสอบ', exact: true }).click();
		await expect(page.locator('#phase-duration-midterm')).toHaveCount(0);
		await page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' }).click();
		await expect(page.getByRole('heading', { name: 'ค21101 · คณิตศาสตร์พื้นฐาน' })).toBeHidden();
		expect(writes.at(-1)?.phases.find((phase) => phase.phaseCode === 'midterm')).toEqual(
			expect.objectContaining({
				maxScore: '20',
				examArrangement: 'outside_timetable',
				examDurationMinutes: null
			})
		);
	});
}
