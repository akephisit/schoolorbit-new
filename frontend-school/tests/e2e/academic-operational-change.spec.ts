import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const ids = {
	year: '10000000-0000-4000-8000-000000000101',
	term: '20000000-0000-4000-8000-000000000101',
	baseVersion: '30000000-0000-4000-8000-000000000101',
	targetVersion: '30000000-0000-4000-8000-000000000102',
	changeSet: '40000000-0000-4000-8000-000000000101',
	user: '50000000-0000-4000-8000-000000000101',
	offering: '60000000-0000-4000-8000-000000000101',
	group: '70000000-0000-4000-8000-000000000101',
	membership: '80000000-0000-4000-8000-000000000101',
	studentYear: '90000000-0000-4000-8000-000000000101',
	student: 'a0000000-0000-4000-8000-000000000101'
};

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

function changeSet(effectiveFrom: string) {
	return {
		id: ids.changeSet,
		academicTermId: ids.term,
		academicYearId: ids.year,
		effectiveFrom,
		reason: 'เพิ่มรายวิชาเสริมตามมติฝ่ายวิชาการ',
		status: 'draft',
		baseTimetableVersionId: ids.baseVersion,
		targetTimetableVersionId: ids.targetVersion,
		rowVersion: 1,
		createdBy: ids.user,
		publishedBy: null,
		publishedAt: null,
		cancelledBy: null,
		cancelledAt: null,
		createdAt: '2026-08-30T00:00:00Z',
		updatedAt: '2026-08-30T00:00:00Z',
		items: []
	};
}

function warningPreview() {
	return {
		changeSetId: ids.changeSet,
		changeSetRowVersion: 1,
		effectiveFrom: '2026-09-01',
		targetTimetableVersionId: ids.targetVersion,
		targetTimetableVersionRowVersion: 3,
		previewHash: 'preview-hash-1',
		findings: [
			{
				code: 'weekly_period_excess',
				severity: 'warning',
				title: 'คาบจริงมากกว่าเป้าหมาย',
				guidance: 'ตรวจแล้วรับทราบก่อนเผยแพร่',
				affectedCount: 1,
				resourceId: null,
				learningOfferingId: null,
				learningGroupId: null,
				route: null
			},
			{
				code: 'weekly_period_excess',
				severity: 'warning',
				title: 'อีกกลุ่มมีคาบจริงมากกว่าเป้าหมาย',
				guidance: 'ใช้การรับทราบรหัสเดียวกัน',
				affectedCount: 1,
				resourceId: null,
				learningOfferingId: null,
				learningGroupId: null,
				route: null
			}
		],
		impactCounts: {
			groups: 1,
			homerooms: 1,
			membershipIntervals: 0,
			teacherAssignments: 1,
			targetTimetableEntries: 2,
			courseAssessmentPlans: 0,
			courseAssessmentCategories: 0,
			courseAssessmentItems: 0,
			learningResults: 0,
			examScheduleItems: 0,
			supervisionObservations: 0
		},
		scheduleCounts: []
	};
}

async function mockWorkspace(
	page: Page,
	permissions: string[],
	options: {
		changeSets?: ReturnType<typeof changeSet>[];
		latestChangeSet?: Record<string, unknown>;
		preview?: Record<string, unknown>;
		publishedRosterDetail?: boolean;
		publishConflict?: boolean;
	} = {}
) {
	let managementRequests = 0;
	let rosterPreviewRequests = 0;
	let changePreviewRequests = 0;
	let createdBody: Record<string, unknown> | null = null;
	let publishedBody: Record<string, unknown> | null = null;
	let currentChangeSets: Record<string, unknown>[] = [...(options.changeSets ?? [])];
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const changeSetPath = `/api/academic/term-change-sets/${ids.changeSet}`;
			const groupPath = `/api/academic/learning-groups/${ids.group}`;
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: ids.user,
					username: 'academic-change-test',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-08-30T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions
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
			if (url.pathname === '/api/academic/delivery/homerooms') {
				await fulfill(route, {
					academicYearId: ids.year,
					academicTermId: ids.term,
					timetableVersionId: ids.baseVersion,
					timetableVersionStatus: 'published',
					homerooms: [],
					unlinked: []
				});
				return;
			}
			if (
				options.publishedRosterDetail &&
				url.pathname === `/api/academic/offerings/${ids.offering}`
			) {
				await fulfill(route, {
					id: ids.offering,
					academicTermId: ids.term,
					academicYearId: ids.year,
					kind: 'course',
					codeSnapshot: 'ค21101',
					nameSnapshot: 'คณิตศาสตร์พื้นฐาน',
					status: 'published',
					rowVersion: 1,
					migrated: false,
					startsOn: '2026-05-01',
					endsOn: null,
					stopReason: null,
					targets: [],
					snapshot: {
						kind: 'course',
						subjectId: ids.offering,
						subjectVersionId: ids.offering,
						credit: '1.0',
						hours: '40',
						standardPeriodsPerWeek: 2,
						gradingPolicy: {
							policyCode: 'school_default',
							totalScore: '100.00',
							passingScore: '50.00'
						}
					},
					createdAt: '2026-05-01T00:00:00Z',
					updatedAt: '2026-05-01T00:00:00Z'
				});
				return;
			}
			if (options.publishedRosterDetail && url.pathname === '/api/academic/timetable-versions') {
				await fulfill(route, [
					{
						id: ids.baseVersion,
						academicTermId: ids.term,
						academicYearId: ids.year,
						bellScheduleId: ids.baseVersion,
						changeSetId: null,
						status: 'published',
						displayState: 'current',
						effectiveFrom: '2026-05-01',
						effectiveUntil: null,
						sourceVersionId: null,
						rowVersion: 1,
						publishedAt: '2026-05-01T00:00:00Z',
						publishedBy: ids.user,
						createdBy: ids.user,
						createdAt: '2026-05-01T00:00:00Z',
						updatedAt: '2026-05-01T00:00:00Z',
						targets: [
							{
								timetableVersionId: ids.baseVersion,
								learningOfferingId: ids.offering,
								standardPeriodsPerWeek: 2,
								weeklyPeriodTarget: 2
							}
						]
					}
				]);
				return;
			}
			const publishedGroup = {
				id: ids.group,
				learningOfferingId: ids.offering,
				academicTermId: ids.term,
				academicYearId: ids.year,
				code: 'M1-1',
				name: 'ม.1/1',
				description: null,
				capacity: 40,
				status: 'published',
				rosterStatus: 'published',
				rosterPublishedAt: '2026-05-01T00:00:00Z',
				teachersLocked: true,
				teacherAssignments: [],
				homeroomIds: [ids.group],
				preferredRoomIds: [],
				rowVersion: 1,
				migrated: false,
				createdAt: '2026-05-01T00:00:00Z',
				updatedAt: '2026-05-01T00:00:00Z'
			};
			if (
				options.publishedRosterDetail &&
				url.pathname === `/api/academic/offerings/${ids.offering}/groups`
			) {
				await fulfill(route, [publishedGroup]);
				return;
			}
			if (options.publishedRosterDetail && url.pathname === groupPath) {
				await fulfill(route, publishedGroup);
				return;
			}
			if (options.publishedRosterDetail && url.pathname === `${groupPath}/memberships`) {
				await fulfill(route, [
					{
						id: ids.membership,
						learningGroupId: ids.group,
						studentAcademicYearId: ids.studentYear,
						studentId: ids.student,
						studentCode: '10001',
						displayName: 'เด็กชายทดสอบ รายชื่อ',
						joinedAt: '2026-05-01',
						leftAt: null,
						membershipStatus: 'active',
						rosterSource: 'manual',
						publishedAt: '2026-05-01T00:00:00Z',
						rowVersion: 1
					}
				]);
				return;
			}
			if (options.publishedRosterDetail && url.pathname === `${groupPath}/roster`) {
				rosterPreviewRequests += 1;
				await fulfill(route, 'forbidden', 403);
				return;
			}
			if (url.pathname === '/api/academic/term-change-sets' && route.request().method() === 'GET') {
				await fulfill(route, currentChangeSets);
				return;
			}
			if (
				url.pathname === '/api/academic/term-change-sets' &&
				route.request().method() === 'POST'
			) {
				createdBody = route.request().postDataJSON() as Record<string, unknown>;
				const created = changeSet(String(createdBody.effectiveFrom));
				currentChangeSets = [created, ...currentChangeSets];
				await fulfill(route, created, 201);
				return;
			}
			if (url.pathname === changeSetPath && route.request().method() === 'GET') {
				if (options.latestChangeSet) currentChangeSets = [options.latestChangeSet];
				await fulfill(route, options.latestChangeSet ?? currentChangeSets[0]);
				return;
			}
			if (url.pathname === `${changeSetPath}/preview` && route.request().method() === 'GET') {
				changePreviewRequests += 1;
				await fulfill(route, options.preview ?? {});
				return;
			}
			if (url.pathname === `${changeSetPath}/publish` && route.request().method() === 'POST') {
				publishedBody = route.request().postDataJSON() as Record<string, unknown>;
				if (options.publishConflict) {
					await fulfill(route, { message: 'preview stale' }, 409);
					return;
				}
				const published = {
					...currentChangeSets[0],
					status: 'published' as const,
					rowVersion: 2,
					publishedAt: '2026-08-30T04:00:00Z',
					updatedAt: '2026-08-30T04:00:00Z'
				};
				currentChangeSets = [published];
				await fulfill(route, published);
				return;
			}
			if (url.pathname === '/api/academic/delivery/management-options') {
				managementRequests += 1;
				await fulfill(route, {});
				return;
			}
			if (url.pathname === '/api/academic/delivery/workspace') {
				await fulfill(route, { academicTermId: ids.term, offerings: [] });
				return;
			}
			if (url.pathname === '/api/menu/user') return void (await fulfill(route, { groups: [] }));
			if (url.pathname === '/api/me/work-items/counts')
				return void (await fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				}));
			if (url.pathname === '/api/notifications')
				return void (await fulfill(route, { items: [], unread_count: 0 }));
			if (url.pathname === '/api/notifications/stream')
				return void (await route.fulfill({
					status: 200,
					contentType: 'text/event-stream',
					body: ''
				}));
			await fulfill(route, {});
		}
	);
	return {
		managementRequestCount: () => managementRequests,
		rosterPreviewRequestCount: () => rosterPreviewRequests,
		changePreviewRequestCount: () => changePreviewRequests,
		createdRequest: () => createdBody,
		publishedRequest: () => publishedBody
	};
}

test('manager creates a dated midterm draft without changing curriculum', async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-08-30T08:00:00+07:00'));
	const mocked = await mockWorkspace(page, ['*']);
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);

	await page.getByRole('button', { name: 'เพิ่ม/ปรับ/หยุดกลางภาค' }).click();
	await expect(page.getByText('ไม่เปลี่ยนหลักสูตร')).toBeVisible();
	await page.getByRole('button', { name: 'เลือกวันที่เริ่มใช้จริง' }).click();
	await page.getByRole('button', { name: 'วันอาทิตย์ที่ 30 สิงหาคม 2569', exact: true }).click();
	await page.getByLabel('เหตุผลการเปลี่ยนแปลง').fill('เพิ่มรายวิชาเสริมตามมติฝ่ายวิชาการ');
	await page.getByRole('button', { name: 'สร้างแบบร่าง' }).click();

	await expect(
		page.getByRole('heading', { name: 'การเปลี่ยนแปลงกลางภาค', exact: true })
	).toBeVisible();
	expect(mocked.createdRequest()).toMatchObject({
		academicTermId: ids.term,
		effectiveFrom: '2026-08-30',
		reason: 'เพิ่มรายวิชาเสริมตามมติฝ่ายวิชาการ'
	});
});

test('read-only staff never loads management options or sees mutation controls', async ({
	page
}) => {
	const mocked = await mockWorkspace(page, ['learning_offering.read.school']);
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);

	await expect(page.getByRole('heading', { name: 'จัดการการเปิดสอน' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่ม/ปรับ/หยุดกลางภาค' })).toHaveCount(0);
	expect(mocked.managementRequestCount()).toBe(0);
});

test('read-only staff loads dated roster history without management-only preview', async ({
	page
}) => {
	const mocked = await mockWorkspace(page, ['learning_offering.read.school'], {
		publishedRosterDetail: true
	});
	await page.goto(
		`/staff/academic/delivery/${ids.offering}?groupId=${ids.group}&academicYearId=${ids.year}&academicTermId=${ids.term}`
	);

	await expect(page.getByRole('heading', { name: 'ประวัติสมาชิกกลุ่มเรียน' })).toBeVisible();
	await expect(page.getByText('เด็กชายทดสอบ รายชื่อ')).toBeVisible();
	expect(mocked.managementRequestCount()).toBe(0);
	expect(mocked.rosterPreviewRequestCount()).toBe(0);
});

test('manager acknowledges the current preview warning before publishing', async ({ page }) => {
	const mocked = await mockWorkspace(page, ['*'], {
		changeSets: [changeSet('2026-09-01')],
		preview: warningPreview()
	});
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);

	await page.getByRole('button', { name: 'ตรวจความพร้อม' }).click();
	const publish = page.getByRole('button', { name: /เผยแพร่ตั้งแต่/ });
	await expect(publish).toBeDisabled();
	await page.getByRole('checkbox', { name: 'รับทราบ คาบจริงมากกว่าเป้าหมาย' }).click();
	await expect(publish).toBeEnabled();
	await publish.click();

	expect(mocked.publishedRequest()).toMatchObject({
		rowVersion: 1,
		targetTimetableVersionRowVersion: 3,
		previewHash: 'preview-hash-1',
		acknowledgedWarningCodes: ['weekly_period_excess']
	});
});

test('publish conflict invalidates the preview and requires a new readiness check', async ({
	page
}) => {
	await mockWorkspace(page, ['*'], {
		changeSets: [changeSet('2026-09-01')],
		preview: warningPreview(),
		publishConflict: true
	});
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);

	await page.getByRole('button', { name: 'ตรวจความพร้อม' }).click();
	await page.getByRole('checkbox', { name: 'รับทราบ คาบจริงมากกว่าเป้าหมาย' }).click();
	const publish = page.getByRole('button', { name: /เผยแพร่ตั้งแต่/ });
	await publish.click();

	await expect(
		page.getByText('ข้อมูลเปลี่ยนหลังตรวจความพร้อม กรุณาตรวจความพร้อมใหม่ก่อนเผยแพร่')
	).toBeVisible();
	await expect(page.getByText('ยังไม่ได้ตรวจความพร้อม')).toBeVisible();
	await expect(publish).toBeDisabled();
});

test('readiness refreshes a stale draft before requesting a preview', async ({ page }) => {
	const initial = changeSet('2026-09-01');
	const mocked = await mockWorkspace(page, ['*'], {
		changeSets: [initial],
		latestChangeSet: {
			...initial,
			rowVersion: 2,
			reason: 'เหตุผลล่าสุดจากผู้ใช้อีกคน',
			updatedAt: '2026-08-30T05:00:00Z'
		}
	});
	await page.goto(`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}`);

	await page.getByRole('button', { name: 'ตรวจความพร้อม' }).click();
	await expect(page.getByText('เหตุผลล่าสุดจากผู้ใช้อีกคน')).toBeVisible();
	expect(mocked.changePreviewRequestCount()).toBe(0);
});
