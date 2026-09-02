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
		gradingPolicy: { policyCode: 'score', totalScore: '100', passingScore: '50' },
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

async function mockAssessmentApis(page: Page) {
	let mathematics = assessmentPlan(offeringIds.mathematics, 'ค21101', 'คณิตศาสตร์พื้นฐาน');
	const science = assessmentPlan(offeringIds.science, 'ว21101', 'วิทยาศาสตร์พื้นฐาน');

	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: teacherId,
					username: 'academic-admin',
					firstName: 'ทดสอบ',
					lastName: 'ระบบ',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-09-02T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: ['academic_assessment.read.school', 'academic_assessment.manage.school']
				});
				return;
			}
			if (url.pathname === '/api/academic/context/options') {
				await fulfill(route, {
					activeAcademicYearId: academicYearId,
					activeAcademicTermId: academicTermId,
					years: [
						{
							id: academicYearId,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2027-03-31'
						}
					],
					terms: [
						{
							id: academicTermId,
							academicYearId,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2026-10-31',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
				return;
			}
			if (url.pathname === '/api/academic/assessments/plans') {
				await fulfill(route, [mathematics, science]);
				return;
			}
			if (url.pathname === '/api/academic/assessments/phase-controls') {
				await fulfill(
					route,
					phaseCodes.map((phaseCode, index) => ({
						id: `90000000-0000-4000-8000-00000000000${index + 1}`,
						academicYearId,
						academicTermId,
						phaseCode,
						label: ['ก่อนกลางภาค', 'กลางภาค', 'หลังกลางภาค', 'ปลายภาค'][index],
						order: index + 1,
						planEditingEnabled: true,
						scoreEntryEnabled: true,
						rowVersion: 1
					}))
				);
				return;
			}
			if (url.pathname === `/api/academic/assessments/offerings/${offeringIds.mathematics}`) {
				if (route.request().method() === 'PUT') {
					const body = route.request().postDataJSON() as { phases: Array<{ maxScore: string }> };
					mathematics = assessmentPlan(
						offeringIds.mathematics,
						'ค21101',
						'คณิตศาสตร์พื้นฐาน',
						body.phases.map((phase) => phase.maxScore)
					);
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

test('closes after an in-flight autosave and opens the next subject editor', async ({ page }) => {
	await mockAssessmentApis(page);
	await page.goto(
		`/staff/academic/assessments?academicYearId=${academicYearId}&academicTermId=${academicTermId}`
	);

	await page.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true }).click();
	await expect(page.getByRole('heading', { name: 'ค21101 · คณิตศาสตร์พื้นฐาน' })).toBeVisible();

	await page.getByLabel('คะแนนเต็ม').first().fill('21');
	await page.getByRole('button', { name: 'ปิดหน้ากรอกคะแนน' }).click();
	await expect(page.getByRole('heading', { name: 'ค21101 · คณิตศาสตร์พื้นฐาน' })).toBeHidden();

	await page.getByText('ว21101 · วิทยาศาสตร์พื้นฐาน', { exact: true }).click();
	await expect(page.getByRole('heading', { name: 'ว21101 · วิทยาศาสตร์พื้นฐาน' })).toBeVisible();
	await expect(page.getByLabel('คะแนนเต็ม').first()).toHaveValue('20');
});
