import { expect, test, type Page, type Route } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';
type Schemas = components['schemas'];
test.use({ serviceWorkers: 'block' });
const year = '10000000-0000-4000-8000-000000000001';
const term = '20000000-0000-4000-8000-000000000001';
const student = '30000000-0000-4000-8000-000000000001';
const policyId = '40000000-0000-4000-8000-000000000001';
const actor = '50000000-0000-4000-8000-000000000001';
const url = `/staff/academic/results/aggregates?academicYearId=${year}&academicTermId=${term}`;
const policy: Schemas['AggregatePolicyVersion'] = {
	id: policyId,
	name: 'นโยบายที่โรงเรียนตรวจแล้ว',
	passingGrade: '1.00',
	minimumLearnerLevel: 1,
	allowReviewedHolds: false,
	approvedBy: actor,
	approvedAt: '2026-09-11T00:00:00Z'
};
const preview: Schemas['TermAggregatePreview'] = {
	policy,
	canLock: true,
	blockers: [],
	holdFindings: [],
	sourceChecksum: 'a'.repeat(64),
	learnerEvaluations: { studentAcademicYearId: student, policyVersionId: policyId, domains: [] },
	results: {
		academicYearId: year,
		academicTermId: term,
		studentAcademicYearId: student,
		passingGrade: '1.00',
		courses: [
			{
				subjectId: '60000000-0000-4000-8000-000000000001',
				learningOfferingId: '70000000-0000-4000-8000-000000000001',
				resultId: '80000000-0000-4000-8000-000000000001',
				effectiveVersion: 1,
				credits: '1.00',
				outcome: 'numeric',
				numericGrade: '0.00'
			}
		],
		activities: [],
		sourceChecksum: 'b'.repeat(64),
		totals: {
			attemptedCredits: '1.00',
			gradedCredits: '1.00',
			earnedCredits: '0.00',
			unresolvedCredits: '0.00',
			weightedGradePoints: '0.00',
			provisionalGpa: '0.00',
			missingResultCount: 0,
			exceptionalResultCount: 0,
			coverageComplete: true,
			allOutcomesNumeric: true
		},
		activityTotals: {
			expectedGroupCount: 0,
			passedGroupCount: 0,
			failedGroupCount: 0,
			missingResultCount: 0,
			coverageComplete: true,
			allPassed: true
		}
	}
};

async function mock(page: Page, manager = false, conflict = false) {
	const writes: Schemas['AggregateLockInput'][] = [];
	let studentReads = 0;
	let revisions: Schemas['TermAggregateRevision'][] = [];
	const row: Schemas['AggregateStudent'] = {
		studentAcademicYearId: student,
		studentCode: 'E2E001',
		studentName: 'นักเรียน ทดสอบ',
		gradeLevelName: 'มัธยมศึกษาปีที่ 1',
		studyProgramName: 'ทั่วไป',
		closure: {
			studentAcademicYearId: student,
			revisionId: null,
			revision: null,
			policyId: null,
			isCurrent: false,
			blockers: [],
			holdReason: null,
			currentSourceChecksum: null
		}
	};
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
			const path = new URL(route.request().url()).pathname;
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
					permissions: manager
						? ['*']
						: [
								'academic_context.read.school',
								'academic_result.read.school',
								'academic_learner_evaluation.read.school'
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
							status: 'closing',
							startDate: '2026-05-16',
							plannedEndDate: null,
							closedOn: null,
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				} satisfies Schemas['AcademicContextOptions']);
			if (path === '/api/academic/results/aggregate-students') return respond(route, [row]);
			if (path === '/api/academic/results/aggregate-policies') return respond(route, [policy]);
			if (path.endsWith('/aggregate-preview')) {
				studentReads++;
				return respond(route, preview);
			}
			if (path.endsWith('/aggregate-revisions')) {
				if (route.request().method() === 'GET') return respond(route, revisions);
				const body: Schemas['AggregateLockInput'] = route.request().postDataJSON();
				writes.push(body);
				if (conflict) return respond(route, 'ผลต้นทางเปลี่ยนแล้ว กรุณาคำนวณใหม่', 409);
				const revision: Schemas['TermAggregateRevision'] = {
					id: '90000000-0000-4000-8000-000000000001',
					revision: 1,
					snapshot: preview,
					officialGpa: '0.00',
					holdReason: null,
					lockedBy: actor,
					lockedAt: '2026-09-11T00:00:00Z',
					isCurrent: true
				};
				revisions = [revision];
				row.closure = {
					...row.closure,
					revisionId: revision.id,
					revision: 1,
					isCurrent: true,
					policyId,
					currentSourceChecksum: preview.sourceChecksum
				};
				return respond(route, revision);
			}
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
	return { writes, studentReads: () => studentReads };
}

test('school result readers can reach aggregate inspection from the existing results page', async ({
	page
}) => {
	await mock(page);
	await page.goto(`/staff/academic/results?academicYearId=${year}&academicTermId=${term}`);
	await page.getByRole('link', { name: 'สรุปผลรายภาค', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'สรุปผลรายภาค', exact: true })).toBeVisible();
});

test('aggregate reader sees missing learners, loads only the selected learner and cannot lock', async ({
	page
}) => {
	const observed = await mock(page);
	await page.goto(url);
	await expect(page.getByRole('heading', { name: 'สรุปผลรายภาค', exact: true })).toBeVisible();
	await expect(page.getByText('ยังไม่ล็อกผลสรุป', { exact: true })).toBeVisible();
	expect(observed.studentReads()).toBe(0);
	await page.getByRole('button', { name: 'ตรวจผล นักเรียน ทดสอบ' }).click();
	await expect(page.getByText('ค่าเฉลี่ยเฉพาะผลตัวเลข')).toBeVisible();
	await expect(page.getByRole('button', { name: 'ล็อกผลสรุป', exact: true })).toHaveCount(0);
	expect(observed.writes).toHaveLength(0);
	expect(observed.studentReads()).toBe(1);
});

test('aggregate lock is explicit, sends pinned versions and preserves numeric zero as official GPA', async ({
	page
}) => {
	const observed = await mock(page, true);
	await page.goto(url);
	await page.getByRole('button', { name: 'ตรวจผล นักเรียน ทดสอบ' }).click();
	await page.getByRole('button', { name: 'ล็อกผลสรุป', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();
	expect(observed.writes).toHaveLength(0);
	await dialog.getByRole('button', { name: 'ยืนยันล็อกผลสรุป' }).click();
	await expect(dialog).toBeHidden();
	expect(observed.writes[0]).toMatchObject({
		policyId,
		sourceChecksum: 'a'.repeat(64),
		expectedRevision: null,
		holdReason: null
	});
	await expect(page.getByText('GPA ทางการ: 0.00', { exact: true })).toBeVisible();
});

test('stale aggregate lock retains confirmation but cannot blindly retry', async ({ page }) => {
	const observed = await mock(page, true, true);
	await page.goto(url);
	await page.getByRole('button', { name: 'ตรวจผล นักเรียน ทดสอบ' }).click();
	await page.getByRole('button', { name: 'ล็อกผลสรุป', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'ยืนยันล็อกผลสรุป' }).click();
	await expect(dialog.getByText('ผลต้นทางเปลี่ยนแล้ว กรุณาคำนวณใหม่')).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'ยืนยันล็อกผลสรุป' })).toBeDisabled();
	await expect(dialog.getByRole('button', { name: 'คำนวณข้อมูลล่าสุด' })).toBeVisible();
	expect(observed.writes).toHaveLength(1);
});
