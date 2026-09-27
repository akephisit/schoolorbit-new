import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const ids = {
	year: '10000000-0000-4000-8000-000000000001',
	term: '20000000-0000-4000-8000-000000000001',
	user: '30000000-0000-4000-8000-000000000001',
	subject: '40000000-0000-4000-8000-000000000001',
	offering: '50000000-0000-4000-8000-000000000001',
	group: '60000000-0000-4000-8000-000000000001'
};

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(
			status < 400 ? { success: true, data } : { success: false, error: String(data) }
		)
	});
}

async function mockLockQueue(
	page: Page,
	options: { courseReady?: boolean; holdResultRead?: boolean } = {}
) {
	const learnerLockRequests: string[] = [];
	const reads = { results: 0, learner: 0 };
	let releaseResultRead = () => {};
	const resultReadGate = new Promise<void>((resolve) => {
		releaseResultRead = resolve;
	});
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const request = route.request();
			const url = new URL(request.url());
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: ids.user,
					username: 'academic',
					firstName: 'ฝ่าย',
					lastName: 'วิชาการ',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-09-01T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: ['academic_result.lock.school', 'academic_learner_evaluation.lock.school']
				});
				return;
			}
			if (url.pathname === '/api/academic/context/options') {
				await fulfill(route, {
					activeAcademicYearId: ids.year,
					activeAcademicTermId: ids.term,
					years: [
						{
							id: ids.year,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2027-03-31'
						}
					],
					terms: [
						{
							id: ids.term,
							academicYearId: ids.year,
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
			if (url.pathname === '/api/academic/results/readiness') {
				reads.results += 1;
				if (options.holdResultRead) await resultReadGate;
				await fulfill(route, {
					courses: [
						{
							subjectId: ids.subject,
							ready: Boolean(options.courseReady),
							groups: [
								{
									learningGroupId: ids.group,
									learningOfferingId: ids.offering,
									subjectId: ids.subject,
									groupName: 'ม.1/1',
									offeringName: 'คณิตศาสตร์พื้นฐาน',
									assigned: false,
									ready: Boolean(options.courseReady),
									locked: false,
									blockers: options.courseReady ? [] : [{ code: 'missing_group_confirmation' }]
								}
							]
						}
					],
					activities: []
				});
				return;
			}
			if (url.pathname === '/api/academic/learner-evaluations/lock-readiness') {
				reads.learner += 1;
				await fulfill(route, [
					{
						subjectId: ids.subject,
						code: 'ค21101',
						name: 'คณิตศาสตร์พื้นฐาน',
						domain: 'desirable_characteristic',
						locked: false,
						ready: true,
						groups: [
							{
								learningGroupId: ids.group,
								groupName: 'ม.1/1',
								ready: true,
								blockers: []
							}
						]
					},
					{
						subjectId: ids.subject,
						code: 'ค21101',
						name: 'คณิตศาสตร์พื้นฐาน',
						domain: 'reading_thinking_writing',
						locked: false,
						ready: true,
						groups: [
							{
								learningGroupId: ids.group,
								groupName: 'ม.1/1',
								ready: true,
								blockers: []
							}
						]
					}
				]);
				return;
			}
			if (
				request.method() === 'POST' &&
				url.pathname === `/api/academic/results/subjects/${ids.subject}/lock`
			) {
				await fulfill(route, {
					lock: { id: '70000000-0000-4000-8000-000000000001', subjectId: ids.subject },
					groups: [
						{
							learningGroupId: ids.group,
							learningOfferingId: ids.offering,
							subjectId: ids.subject,
							groupName: 'ม.1/1',
							offeringName: 'คณิตศาสตร์พื้นฐาน',
							assigned: false,
							ready: true,
							locked: false,
							blockers: []
						}
					]
				});
				return;
			}
			if (
				request.method() === 'POST' &&
				url.pathname.startsWith(
					`/api/academic/learner-evaluations/subjects/${ids.subject}/domains/`
				) &&
				url.pathname.endsWith('/lock')
			) {
				learnerLockRequests.push(url.pathname);
				const domain = url.pathname.includes('desirable_characteristic')
					? 'desirable_characteristic'
					: 'reading_thinking_writing';
				await fulfill(route, {
					blockers: [],
					lock: {
						id: '70000000-0000-4000-8000-000000000001',
						subjectId: ids.subject,
						domain,
						sourceChecksum: 'source-1',
						rosterChecksum: 'roster-1',
						rowVersion: 1
					}
				});
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
			if (url.pathname === '/api/school/settings') {
				await fulfill(route, 'forbidden', 403);
				return;
			}
			await fulfill(route, {});
		}
	);
	return { learnerLockRequests, reads, releaseResultRead };
}

test('learner-evaluation domains lock independently and course blockers are visible', async ({
	page
}) => {
	const { learnerLockRequests, reads } = await mockLockQueue(page);
	await page.goto(
		`/staff/academic/result-locks?academicYearId=${ids.year}&academicTermId=${ids.term}`
	);

	await expect(page.getByText('ครูหลักยังไม่ยืนยันผล')).toBeVisible();
	expect(reads).toEqual({ results: 1, learner: 0 });
	await page.getByRole('tab', { name: 'ผลประเมินผู้เรียน' }).click();
	await expect(page.getByRole('button', { name: 'ล็อกด้านนี้' })).toHaveCount(2);
	expect(reads).toEqual({ results: 1, learner: 1 });
	await page.getByRole('button', { name: 'ล็อกด้านนี้' }).first().click();
	await expect(page.getByText('ล็อกแล้ว')).toBeVisible();
	await expect(page.getByRole('button', { name: 'ล็อกด้านนี้' })).toHaveCount(1);
	expect(learnerLockRequests).toEqual([
		`/api/academic/learner-evaluations/subjects/${ids.subject}/domains/desirable_characteristic/lock`
	]);
	expect(reads).toEqual({ results: 1, learner: 1 });
});

test('successful course lock marks returned groups locked without a queue refetch', async ({
	page
}) => {
	const { reads } = await mockLockQueue(page, { courseReady: true });
	await page.goto(
		`/staff/academic/result-locks?academicYearId=${ids.year}&academicTermId=${ids.term}`
	);
	await expect(page.getByRole('button', { name: 'ล็อกผลรายวิชา' })).toBeVisible();
	await page.getByRole('button', { name: 'ล็อกผลรายวิชา' }).click();
	await expect(page.getByText('ล็อกแล้ว')).toHaveCount(2);
	expect(reads).toEqual({ results: 1, learner: 0 });
});

test('lock route paints a focused skeleton while its active queue is pending', async ({ page }) => {
	const observed = await mockLockQueue(page, { holdResultRead: true });
	await page.goto(
		`/staff/academic/result-locks?academicYearId=${ids.year}&academicTermId=${ids.term}`
	);
	await expect(page.getByRole('tab', { name: 'รายวิชา' })).toBeVisible();
	await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	observed.releaseResultRead();
	await expect(page.getByText('ครูหลักยังไม่ยืนยันผล')).toBeVisible();
	expect(observed.reads).toEqual({ results: 1, learner: 0 });
});

test('direct learner-tab link does not read course readiness', async ({ page }) => {
	const observed = await mockLockQueue(page);
	await page.goto(
		`/staff/academic/result-locks?academicYearId=${ids.year}&academicTermId=${ids.term}&tab=learner`
	);
	await expect(page.getByRole('button', { name: 'ล็อกด้านนี้' })).toHaveCount(2);
	expect(observed.reads).toEqual({ results: 0, learner: 1 });
});
