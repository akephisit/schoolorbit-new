import { expect, test, type Page, type Route } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';
type Schemas = components['schemas'];
test.use({ serviceWorkers: 'block' });
const year = '10000000-0000-4000-8000-000000000001';
const nextYear = '10000000-0000-4000-8000-000000000002';
const term = '20000000-0000-4000-8000-000000000001';
const student = '30000000-0000-4000-8000-000000000001';
const actor = '40000000-0000-4000-8000-000000000001';
const subject = '50000000-0000-4000-8000-000000000001';
const policy = '60000000-0000-4000-8000-000000000001';
const revisionId = '70000000-0000-4000-8000-000000000001';
const url = `/staff/academic/results/annual?academicYearId=${year}`;
const at = '2026-09-11T00:00:00Z';
const totals: Schemas['CourseCreditTotals'] = {
	attemptedCredits: '1.00',
	gradedCredits: '1.00',
	earnedCredits: '0.00',
	unresolvedCredits: '0.00',
	weightedGradePoints: '0.0000',
	provisionalGpa: '0.00',
	missingResultCount: 0,
	exceptionalResultCount: 0,
	coverageComplete: true,
	allOutcomesNumeric: true
};

function sourceRevision(held: boolean): Schemas['TermAggregateRevision'] {
	const sourceTotals = held
		? {
				...totals,
				gradedCredits: '0.00',
				unresolvedCredits: '1.00',
				provisionalGpa: null,
				exceptionalResultCount: 1,
				allOutcomesNumeric: false
			}
		: totals;
	return {
		id: revisionId,
		revision: 1,
		officialGpa: held ? null : '0.00',
		holdReason: held ? 'ตรวจผล ร แล้ว' : null,
		lockedBy: actor,
		lockedAt: at,
		isCurrent: true,
		snapshot: {
			policy: {
				id: policy,
				name: 'เกณฑ์ที่โรงเรียนตรวจแล้ว',
				passingGrade: '1.00',
				minimumLearnerLevel: 1,
				allowReviewedHolds: held,
				approvedBy: actor,
				approvedAt: at
			},
			canLock: true,
			blockers: [],
			holdFindings: held ? ['exceptional_course_outcomes'] : [],
			sourceChecksum: 'b'.repeat(64),
			results: {
				academicYearId: year,
				academicTermId: term,
				studentAcademicYearId: student,
				passingGrade: '1.00',
				sourceChecksum: 'c'.repeat(64),
				totals: sourceTotals,
				courses: [
					{
						subjectId: subject,
						learningOfferingId: subject,
						resultId: revisionId,
						effectiveVersion: 1,
						credits: '1.00',
						outcome: held ? 'incomplete' : 'numeric',
						numericGrade: held ? null : '0.00'
					}
				],
				activities: [],
				activityTotals: {
					expectedGroupCount: 0,
					passedGroupCount: 0,
					failedGroupCount: 0,
					missingResultCount: 0,
					coverageComplete: true,
					allPassed: true
				}
			},
			learnerEvaluations: {
				studentAcademicYearId: student,
				policyVersionId: policy,
				domains: (['desirable_characteristic', 'reading_thinking_writing'] as const).map(
					(domain, index) => ({
						domain,
						average: { numerator: '3', denominator: '1', decimal: '3.00' },
						qualityLevel: 3,
						complete: true,
						missingSubjects: [],
						catalogCriteria: [],
						subjects: [
							{
								subjectId: subject,
								average: { numerator: '3', denominator: '1', decimal: '3.00' },
								criteria: [
									{
										id: `80000000-0000-4000-8000-00000000000${index + 1}`,
										subjectId: subject,
										domain,
										subjectTermCriterionId: policy,
										schoolCriterionId: null,
										name: 'หัวข้อประเมิน',
										qualityLevel: 3,
										rowVersion: 1
									}
								]
							}
						]
					})
				)
			}
		}
	};
}

async function mock(
	page: Page,
	mode: 'reader' | 'manager' | 'held' | 'conflict' | 'denied' = 'reader'
) {
	const held = mode === 'held';
	const ready = mode !== 'reader' && mode !== 'denied';
	const source = sourceRevision(held);
	const preview: Schemas['AnnualResultPreview'] = {
		academicYearId: year,
		studentAcademicYearId: student,
		terms: [
			{
				academicTermId: term,
				termName: 'ภาคเรียนที่ 1',
				sequence: 1,
				isCurrent: ready,
				revision: ready ? source : null
			}
		],
		totals: ready
			? source.snapshot.results.totals
			: {
					...totals,
					gradedCredits: '0.00',
					provisionalGpa: null,
					coverageComplete: false,
					allOutcomesNumeric: false
				},
		canLock: ready,
		needsHold: held,
		sourceChecksum: 'a'.repeat(64)
	};
	const row: Schemas['AnnualResultStudent'] = {
		studentAcademicYearId: student,
		studentCode: 'E2E001',
		studentName: 'นักเรียน ทดสอบ',
		gradeLevelName: 'มัธยมศึกษาปีที่ 1',
		studyProgramName: 'ทั่วไป',
		closure: {
			studentAcademicYearId: student,
			revisionId: null,
			revision: null,
			isCurrent: false,
			holdReason: null
		}
	};
	let history: Schemas['AnnualResultRevision'][] = [];
	const writes: Schemas['AnnualLockInput'][] = [];
	let details = 0;
	let rosterReads = 0;
	const respond = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			headers: { 'X-CSRF-Token': 'synthetic-csrf' },
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route(
		(request) => request.pathname.startsWith('/api/'),
		async (route) => {
			const request = new URL(route.request().url());
			const path = request.pathname;
			if (path === '/api/auth/me')
				return respond(route, {
					id: actor,
					username: 'E2E-LIFECYCLE-office',
					firstName: 'ฝ่าย',
					lastName: 'วิชาการ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: ready
						? ['*']
						: [
								'academic_context.read.school',
								'academic_result.read.school',
								...(mode === 'denied' ? [] : ['academic_learner_evaluation.read.school'])
							]
				} satisfies Schemas['CurrentUserResponse']);
			if (path === '/api/academic/context/options')
				return respond(route, {
					activeAcademicYearId: year,
					activeAcademicTermId: term,
					years: [
						{
							id: year,
							year: 2569,
							name: 'ปีการศึกษา 2569',
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2027-04-30'
						}
					],
					terms: [
						{
							id: term,
							academicYearId: year,
							code: '1',
							name: 'ภาคเรียนที่ 1',
							sequence: 1,
							termType: 'regular',
							status: 'closed',
							startDate: '2026-05-16',
							plannedEndDate: null,
							closedOn: '2026-09-30',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				} satisfies Schemas['AcademicContextOptions']);
			if (path.includes('/annual-')) {
				expect(request.searchParams.get('academicYearId')).toBe(year);
				expect(request.searchParams.has('academicTermId')).toBe(false);
			}
			if (path.endsWith('/annual-students')) {
				rosterReads++;
				return respond(route, [row]);
			}
			if (path.endsWith('/annual-preview')) {
				details++;
				return respond(route, preview);
			}
			if (path.endsWith('/annual-revisions')) {
				if (route.request().method() === 'GET') return respond(route, history);
				const body: Schemas['AnnualLockInput'] = route.request().postDataJSON();
				writes.push(body);
				if (mode === 'conflict') return respond(route, 'ผลต้นทางเปลี่ยน กรุณาตรวจใหม่', 409);
				const locked: Schemas['AnnualResultRevision'] = {
					id: '90000000-0000-4000-8000-000000000001',
					revision: 1,
					snapshot: preview,
					officialGpa: held ? null : '0.00',
					holdReason: body.holdReason,
					lockedBy: actor,
					lockedAt: at,
					isCurrent: true
				};
				history = [locked];
				return respond(route, locked);
			}
			if (path === '/api/auth/session/refresh') return respond(route, null);
			if (path === '/api/notifications/stream')
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (path === '/api/menu/user') return respond(route, { groups: [] });
			if (path === '/api/school/public')
				return respond(route, { schoolName: 'โรงเรียนทดสอบ', logoFileId: null });
			if (path === '/api/me/work-items/counts')
				return respond(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
			if (path === '/api/notifications') return respond(route, { items: [], unread_count: 0 });
			return respond(route, 'ไม่มีสิทธิ์ในชุดทดสอบ', 403);
		}
	);
	return {
		writes,
		get details() {
			return details;
		},
		get rosterReads() {
			return rosterReads;
		}
	};
}

test('annual reader sees missing sources without loading every student or showing lock controls', async ({
	page
}) => {
	const requests = await mock(page);
	await page.goto(url);
	await expect(page.getByRole('heading', { name: 'สรุปผลรายปี', exact: true })).toBeVisible();
	expect(requests.details).toBe(0);
	await page.getByRole('button', { name: 'นักเรียน ทดสอบ' }).click();
	await expect(page.getByText('ยังไม่มีผลรายภาคที่ยืนยันแล้ว', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ยืนยันผลรายปี', exact: true })).toHaveCount(0);
	expect(requests.details).toBe(1);
	expect(requests.writes).toHaveLength(0);
});

test('direct annual learner preview renders before the independent roster resolves', async ({
	page
}) => {
	const requests = await mock(page);
	let releaseRoster = () => {};
	const rosterGate = new Promise<void>((resolve) => {
		releaseRoster = resolve;
	});
	await page.route('**/api/academic/results/annual-students', async (route) => {
		await rosterGate;
		await route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({ success: true, data: [] })
		});
	});
	await page.goto(`${url}&studentAcademicYearId=${student}`);
	await expect(page.getByText('ผลรายภาคที่นับรวมในปีนี้')).toBeVisible();
	expect(requests.details).toBe(1);
	releaseRoster();
});

test('switching years ignores a late roster from the previous year', async ({ page }) => {
	await mock(page);
	let releaseOldRoster = () => {};
	const oldRosterGate = new Promise<void>((resolve) => {
		releaseOldRoster = resolve;
	});
	const respond = (route: Route, data: unknown) =>
		route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({ success: true, data })
		});
	await page.route('**/api/academic/context/options', (route) =>
		respond(route, {
			activeAcademicYearId: year,
			activeAcademicTermId: term,
			years: [
				{
					id: year,
					year: 2569,
					name: 'ปีการศึกษา 2569',
					status: 'active',
					startDate: '2026-05-01',
					endDate: '2027-04-30'
				},
				{
					id: nextYear,
					year: 2570,
					name: 'ปีการศึกษา 2570',
					status: 'planning',
					startDate: '2027-05-01',
					endDate: '2028-04-30'
				}
			],
			terms: []
		} satisfies Schemas['AcademicContextOptions'])
	);
	let oldRequestStarted = () => {};
	const oldRequest = new Promise<void>((resolve) => {
		oldRequestStarted = resolve;
	});
	await page.route('**/api/academic/results/annual-students*', async (route) => {
		const requestedYear = new URL(route.request().url()).searchParams.get('academicYearId');
		if (requestedYear === year) {
			oldRequestStarted();
			await oldRosterGate;
			return respond(route, [
				{
					studentAcademicYearId: student,
					studentCode: 'OLD001',
					studentName: 'นักเรียนปีเดิม',
					gradeLevelName: 'มัธยมศึกษาปีที่ 1',
					studyProgramName: 'ทั่วไป',
					closure: {
						studentAcademicYearId: student,
						revisionId: null,
						revision: null,
						isCurrent: false,
						holdReason: null
					}
				}
			]);
		}
		expect(requestedYear).toBe(nextYear);
		return respond(route, [
			{
				studentAcademicYearId: '30000000-0000-4000-8000-000000000002',
				studentCode: 'NEW001',
				studentName: 'นักเรียนปีใหม่',
				gradeLevelName: 'มัธยมศึกษาปีที่ 2',
				studyProgramName: 'ทั่วไป',
				closure: {
					studentAcademicYearId: '30000000-0000-4000-8000-000000000002',
					revisionId: null,
					revision: null,
					isCurrent: false,
					holdReason: null
				}
			}
		]);
	});
	try {
		await page.goto(url);
		await oldRequest;
		await page.getByLabel('เลือกปีการศึกษา', { exact: true }).click();
		await page.getByRole('option', { name: /ปีการศึกษา 2570/ }).click();
		await expect(page).toHaveURL(new RegExp(`academicYearId=${nextYear}`));
		await expect(page.getByRole('button', { name: 'นักเรียนปีใหม่' })).toBeVisible();
	} finally {
		releaseOldRoster();
	}
	await expect(page.getByRole('button', { name: 'นักเรียนปีเดิม' })).toHaveCount(0);
});

test('annual roster refresh retains visible data and recovers through a local retry', async ({
	page
}) => {
	await mock(page);
	await page.goto(url);
	await expect(page.getByRole('button', { name: 'นักเรียน ทดสอบ' })).toBeVisible();
	let attempts = 0;
	let releaseRefresh = () => {};
	const refreshGate = new Promise<void>((resolve) => {
		releaseRefresh = resolve;
	});
	await page.route('**/api/academic/results/annual-students*', async (route) => {
		attempts++;
		if (attempts === 1) {
			await refreshGate;
			return route.fulfill({
				status: 503,
				contentType: 'application/json',
				body: JSON.stringify({ success: false, error: 'อ่านรายชื่อชั่วคราวไม่สำเร็จ' })
			});
		}
		return route.fallback();
	});
	await page.getByRole('button', { name: 'ตรวจข้อมูลล่าสุด' }).click();
	await expect(page.getByText('กำลังอัปเดตรายชื่อนักเรียน...')).toBeVisible();
	await expect(page.getByRole('button', { name: 'นักเรียน ทดสอบ' })).toBeVisible();
	releaseRefresh();
	await expect(page.getByText('อ่านรายชื่อชั่วคราวไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByRole('button', { name: 'นักเรียน ทดสอบ' })).toBeVisible();
	expect(attempts).toBe(2);
});

test('annual manager explicitly confirms exact sources and preserves official zero', async ({
	page
}) => {
	const requests = await mock(page, 'manager');
	await page.goto(url);
	await page.getByRole('button', { name: 'นักเรียน ทดสอบ' }).click();
	await page.getByRole('button', { name: 'ยืนยันผลรายปี', exact: true }).click();
	expect(requests.writes).toHaveLength(0);
	await page
		.getByRole('dialog')
		.getByRole('button', { name: 'ยืนยันผลรายปี', exact: true })
		.click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(requests.writes).toHaveLength(1);
	expect(requests.writes[0]).toMatchObject({
		expectedRevision: null,
		sourceChecksum: 'a'.repeat(64),
		holdReason: null
	});
	expect(requests.writes[0].requestId).toMatch(/^[0-9a-f-]{36}$/);
	await expect(page.getByTestId('annual-official-gpa')).toHaveText('0.00');
});

test('annual hold requires a reason and never turns exceptional results into zero', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	const requests = await mock(page, 'held');
	await page.goto(url);
	await page.getByRole('button', { name: 'นักเรียน ทดสอบ' }).click();
	await page.getByRole('button', { name: 'ยืนยันผลรายปี', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByRole('button', { name: 'Close', exact: true })).toBeVisible();
	await page.screenshot({
		path: test.info().outputPath('annual-mobile-confirmation.png'),
		fullPage: true,
		animations: 'disabled'
	});
	await expect(dialog.getByRole('button', { name: 'ยืนยันผลรายปี', exact: true })).toBeDisabled();
	await dialog.getByLabel('เหตุผลที่พิจารณาผลค้าง').fill('รอติดตามการแก้ผล ร');
	await dialog.getByRole('button', { name: 'ยืนยันผลรายปี', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(requests.writes[0].holdReason).toBe('รอติดตามการแก้ผล ร');
	await expect(page.getByTestId('annual-official-gpa')).toHaveText('—');
});

test('annual conflict disables blind retry until sources are inspected again', async ({ page }) => {
	const requests = await mock(page, 'conflict');
	await page.goto(url);
	await page.getByRole('button', { name: 'นักเรียน ทดสอบ' }).click();
	await page.getByRole('button', { name: 'ยืนยันผลรายปี', exact: true }).click();
	const confirm = page
		.getByRole('dialog')
		.getByRole('button', { name: 'ยืนยันผลรายปี', exact: true });
	await confirm.click();
	await expect(confirm).toBeDisabled();
	expect(requests.writes).toHaveLength(1);
});

test('annual office roster requires both result and evaluation school permissions', async ({
	page
}) => {
	const requests = await mock(page, 'denied');
	await page.goto(url);
	await expect(
		page.getByText('ต้องมีสิทธิ์อ่านผลการเรียนและผลประเมินระดับโรงเรียนทั้งสองส่วน', {
			exact: true
		})
	).toBeVisible();
	expect(requests.rosterReads).toBe(0);
});

test('academic office can reach annual results from the existing results workspace', async ({
	page
}) => {
	await mock(page);
	await page.goto(`/staff/academic/results?academicYearId=${year}&academicTermId=${term}`);
	await page.getByRole('link', { name: 'สรุปผลรายปี', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'สรุปผลรายปี', exact: true })).toBeVisible();
});
