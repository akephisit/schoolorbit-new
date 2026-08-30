import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const ids = {
	year: '11000000-0000-4000-8000-000000000201',
	term: '21000000-0000-4000-8000-000000000201',
	schedule: '31000000-0000-4000-8000-000000000201',
	period: '41000000-0000-4000-8000-000000000201',
	user: '51000000-0000-4000-8000-000000000201',
	teacher: '51000000-0000-4000-8000-000000000202',
	offering: '61000000-0000-4000-8000-000000000201',
	group: '71000000-0000-4000-8000-000000000201',
	homeroom: '81000000-0000-4000-8000-000000000201',
	room: '91000000-0000-4000-8000-000000000201',
	currentVersion: 'a1000000-0000-4000-8000-000000000201',
	upcomingVersion: 'a1000000-0000-4000-8000-000000000202',
	historicalVersion: 'a1000000-0000-4000-8000-000000000203',
	draftVersion: 'a1000000-0000-4000-8000-000000000204',
	changeSet: 'b1000000-0000-4000-8000-000000000201',
	entry: 'c1000000-0000-4000-8000-000000000201',
	createdEntry: 'c1000000-0000-4000-8000-000000000202'
};

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

function timetableVersion(
	id: string,
	status: 'draft' | 'published' | 'cancelled',
	displayState: 'current' | 'upcoming' | 'historical' | null,
	effectiveFrom: string,
	effectiveUntil: string | null,
	changeSetId: string | null = null
) {
	return {
		id,
		academicTermId: ids.term,
		academicYearId: ids.year,
		bellScheduleId: ids.schedule,
		changeSetId,
		status,
		displayState,
		effectiveFrom,
		effectiveUntil,
		sourceVersionId: status === 'draft' ? ids.currentVersion : null,
		rowVersion: status === 'draft' ? 3 : 1,
		publishedAt: status === 'published' ? '2026-05-01T00:00:00Z' : null,
		publishedBy: status === 'published' ? ids.user : null,
		createdBy: ids.user,
		createdAt: '2026-05-01T00:00:00Z',
		updatedAt: '2026-08-30T00:00:00Z',
		targets: [
			{
				timetableVersionId: id,
				learningOfferingId: ids.offering,
				standardPeriodsPerWeek: 2,
				weeklyPeriodTarget: 2
			}
		]
	};
}

function draftChangeSet(effectiveFrom = '2026-09-15') {
	return {
		id: ids.changeSet,
		academicTermId: ids.term,
		academicYearId: ids.year,
		effectiveFrom,
		reason: 'ปรับตารางหลังเปลี่ยนจำนวนคาบและห้องเรียน',
		status: 'draft' as const,
		baseTimetableVersionId: ids.currentVersion,
		targetTimetableVersionId: ids.draftVersion,
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

function readinessPreview(
	finding: 'none' | 'blocking' | 'warning' = 'none'
): Record<string, unknown> {
	const findings =
		finding === 'blocking'
			? [
					{
						code: 'weekly_period_deficit',
						severity: 'blocking',
						title: 'คาบยังไม่ครบตามเป้าหมาย',
						guidance: 'เพิ่มอีก 1 คาบให้กลุ่ม ม.1/1',
						affectedCount: 1,
						resourceId: ids.group,
						learningOfferingId: ids.offering,
						learningGroupId: ids.group,
						route: null
					}
				]
			: finding === 'warning'
				? [
						{
							code: 'weekly_period_excess',
							severity: 'warning',
							title: 'คาบจริงมากกว่าเป้าหมาย',
							guidance: 'ตรวจสอบและรับทราบก่อนเผยแพร่',
							affectedCount: 1,
							resourceId: ids.group,
							learningOfferingId: ids.offering,
							learningGroupId: ids.group,
							route: null
						}
					]
				: [];
	return {
		changeSetId: ids.changeSet,
		changeSetRowVersion: 1,
		effectiveFrom: '2026-09-15',
		targetTimetableVersionId: ids.draftVersion,
		targetTimetableVersionRowVersion: 3,
		previewHash: 'a'.repeat(64),
		findings,
		impactCounts: {
			groups: 1,
			homerooms: 1,
			membershipIntervals: 30,
			teacherAssignments: 1,
			targetTimetableEntries: 1,
			courseAssessmentPlans: 1,
			courseAssessmentCategories: 4,
			courseAssessmentItems: 8,
			learningResults: 0,
			examScheduleItems: 0,
			supervisionObservations: 0
		},
		scheduleCounts: [
			{
				learningOfferingId: ids.offering,
				learningGroupId: ids.group,
				offeringLabel: 'ค21101 · คณิตศาสตร์พื้นฐาน',
				learningGroupLabel: 'ม.1/1',
				actualPeriods: finding === 'warning' ? 3 : 1,
				targetPeriods: 2
			}
		]
	};
}

function timetableEntry(versionId: string, id = ids.entry) {
	return {
		id,
		timetableVersionId: versionId,
		academicTermId: ids.term,
		academicYearId: ids.year,
		bellScheduleId: ids.schedule,
		bellSchedulePeriodId: ids.period,
		learningGroupId: ids.group,
		learningGroupCode: 'M1-1',
		learningGroupName: 'ม.1/1',
		offeringId: ids.offering,
		offeringCode: 'ค21101',
		offeringName: 'คณิตศาสตร์พื้นฐาน',
		homeroomId: null,
		homeroomName: null,
		roomId: ids.room,
		roomCode: 'MATH-1',
		dayOfWeek: 'MON',
		startTime: '08:30:00',
		endTime: '09:20:00',
		periodName: 'คาบ 1',
		entryType: 'COURSE',
		title: null,
		note: null,
		isActive: true,
		instructors: [],
		rowVersion: 1,
		createdAt: '2026-08-30T00:00:00Z',
		updatedAt: '2026-08-30T00:00:00Z'
	};
}

interface MockOptions {
	permissions?: string[];
	versions?: ReturnType<typeof timetableVersion>[];
	changeSets?: ReturnType<typeof draftChangeSet>[];
	preview?: Record<string, unknown>;
	entries?: Record<string, unknown>[];
}

async function mockTimetable(page: Page, options: MockOptions = {}) {
	let versions = options.versions ?? [
		timetableVersion(ids.currentVersion, 'published', 'current', '2026-05-01', null)
	];
	let changeSets: Record<string, unknown>[] = [...(options.changeSets ?? [])];
	let entries: Record<string, unknown>[] = [...(options.entries ?? [])];
	let createdChangeBody: Record<string, unknown> | null = null;
	let createdEntryBody: Record<string, unknown> | null = null;
	let updatedEntryBody: Record<string, unknown> | null = null;
	let deletedEntryVersionId: string | null = null;
	let publishedBody: Record<string, unknown> | null = null;
	let previewRequests = 0;
	let managementRequests = 0;

	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const method = route.request().method();
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: ids.user,
					username: 'timetable-version-test',
					firstName: 'ตารางสอน',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-08-30T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: options.permissions ?? ['*']
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
			if (url.pathname === '/api/academic/offerings') {
				await fulfill(route, [
					{
						id: ids.offering,
						academicTermId: ids.term,
						academicYearId: ids.year,
						kind: 'course',
						codeSnapshot: 'ค21101',
						nameSnapshot: 'คณิตศาสตร์พื้นฐาน',
						status: 'published',
						rowVersion: 1,
						targets: [],
						snapshot: { kind: 'course', standardPeriodsPerWeek: 2 }
					}
				]);
				return;
			}
			if (url.pathname === '/api/academic/learning-groups') {
				await fulfill(route, [
					{
						id: ids.group,
						learningOfferingId: ids.offering,
						academicTermId: ids.term,
						academicYearId: ids.year,
						code: 'M1-1',
						name: 'ม.1/1',
						status: 'published',
						rosterStatus: 'published',
						rowVersion: 1,
						teacherAssignments: [
							{ teacherId: ids.teacher, role: 'primary', teacherName: 'ครูคณิตศาสตร์' }
						],
						homeroomIds: [ids.homeroom],
						preferredRoomIds: [ids.room]
					}
				]);
				return;
			}
			if (url.pathname === '/api/academic/homerooms') {
				await fulfill(route, [
					{ id: ids.homeroom, academicYearId: ids.year, code: 'M1-1', name: 'ม.1/1' }
				]);
				return;
			}
			if (url.pathname === '/api/academic/bell-schedules') {
				await fulfill(route, [
					{
						id: ids.schedule,
						academicYearId: ids.year,
						code: 'DEFAULT',
						name: 'ตารางเวลาปกติ',
						isDefault: true,
						isActive: true
					}
				]);
				return;
			}
			if (url.pathname === `/api/academic/bell-schedules/${ids.schedule}/periods`) {
				await fulfill(route, [
					{
						id: ids.period,
						bellScheduleId: ids.schedule,
						orderIndex: 1,
						name: 'คาบ 1',
						startTime: '08:30:00',
						endTime: '09:20:00',
						applicableDays: 'MON,TUE,WED,THU,FRI',
						isActive: true
					}
				]);
				return;
			}
			if (url.pathname === '/api/lookup/rooms') {
				await fulfill(route, [{ id: ids.room, code: 'MATH-1', name_th: 'ห้องคณิตศาสตร์' }]);
				return;
			}
			if (url.pathname === '/api/academic/timetable-versions') {
				await fulfill(route, versions);
				return;
			}
			if (url.pathname === '/api/academic/term-change-sets' && method === 'GET') {
				await fulfill(route, changeSets);
				return;
			}
			if (url.pathname === '/api/academic/term-change-sets' && method === 'POST') {
				createdChangeBody = route.request().postDataJSON() as Record<string, unknown>;
				const created = draftChangeSet(String(createdChangeBody.effectiveFrom));
				changeSets = [created];
				versions = [
					...versions,
					timetableVersion(
						ids.draftVersion,
						'draft',
						null,
						created.effectiveFrom,
						null,
						ids.changeSet
					)
				];
				await fulfill(route, created, 201);
				return;
			}
			if (url.pathname === `/api/academic/term-change-sets/${ids.changeSet}` && method === 'GET') {
				await fulfill(route, changeSets[0]);
				return;
			}
			if (
				url.pathname === `/api/academic/term-change-sets/${ids.changeSet}/preview` &&
				method === 'GET'
			) {
				previewRequests += 1;
				await fulfill(route, options.preview ?? readinessPreview());
				return;
			}
			if (
				url.pathname === `/api/academic/term-change-sets/${ids.changeSet}/publish` &&
				method === 'POST'
			) {
				publishedBody = route.request().postDataJSON() as Record<string, unknown>;
				const published = {
					...changeSets[0],
					status: 'published',
					rowVersion: 2,
					publishedBy: ids.user,
					publishedAt: '2026-08-30T06:00:00Z',
					updatedAt: '2026-08-30T06:00:00Z'
				};
				changeSets = [published];
				versions = versions.map((version) =>
					version.id === ids.draftVersion
						? {
								...version,
								status: 'published' as const,
								displayState: 'upcoming' as const,
								rowVersion: 4,
								publishedAt: '2026-08-30T06:00:00Z',
								publishedBy: ids.user
							}
						: version
				);
				await fulfill(route, published);
				return;
			}
			if (url.pathname === '/api/academic/timetable' && method === 'GET') {
				const requestedVersion = url.searchParams.get('timetableVersionId');
				await fulfill(
					route,
					entries.filter((entry) => entry.timetableVersionId === requestedVersion)
				);
				return;
			}
			if (url.pathname === '/api/academic/timetable' && method === 'POST') {
				createdEntryBody = route.request().postDataJSON() as Record<string, unknown>;
				const created = timetableEntry(
					String(createdEntryBody.timetableVersionId),
					ids.createdEntry
				);
				entries = [...entries, created];
				await fulfill(route, created, 201);
				return;
			}
			if (url.pathname === `/api/academic/timetable/${ids.createdEntry}` && method === 'PUT') {
				updatedEntryBody = route.request().postDataJSON() as Record<string, unknown>;
				const updated = {
					...timetableEntry(String(updatedEntryBody.timetableVersionId), ids.createdEntry),
					dayOfWeek: updatedEntryBody.dayOfWeek,
					note: updatedEntryBody.note,
					rowVersion: 2
				};
				entries = entries.map((entry) => (entry.id === ids.createdEntry ? updated : entry));
				await fulfill(route, updated);
				return;
			}
			if (url.pathname === `/api/academic/timetable/${ids.createdEntry}` && method === 'DELETE') {
				deletedEntryVersionId = url.searchParams.get('timetableVersionId');
				const deleted = entries.find((entry) => entry.id === ids.createdEntry) ?? null;
				entries = entries.filter((entry) => entry.id !== ids.createdEntry);
				await fulfill(route, deleted);
				return;
			}
			if (url.pathname === '/api/academic/delivery/management-options') {
				managementRequests += 1;
				await fulfill(route, {});
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
			if (url.pathname === '/api/notifications/stream') {
				await route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
				return;
			}
			await fulfill(route, {});
		}
	);

	return {
		createdChangeRequest: () => createdChangeBody,
		createdEntryRequest: () => createdEntryBody,
		updatedEntryRequest: () => updatedEntryBody,
		deletedEntryVersion: () => deletedEntryVersionId,
		publishedRequest: () => publishedBody,
		previewRequestCount: () => previewRequests,
		managementRequestCount: () => managementRequests
	};
}

const allVersions = () => [
	timetableVersion(ids.currentVersion, 'published', 'current', '2026-05-01', null),
	timetableVersion(ids.upcomingVersion, 'published', 'upcoming', '2026-10-01', null),
	timetableVersion(ids.historicalVersion, 'published', 'historical', '2025-05-01', '2026-03-31'),
	timetableVersion(ids.draftVersion, 'draft', null, '2026-09-15', null, ids.changeSet)
];

test('version selector shows current upcoming history and a draft with editing isolated to the draft', async ({
	page
}) => {
	await mockTimetable(page, {
		versions: allVersions(),
		changeSets: [draftChangeSet()],
		entries: [timetableEntry(ids.currentVersion)]
	});
	await page.goto(
		`/staff/academic/timetable?academicYearId=${ids.year}&academicTermId=${ids.term}&timetableVersionId=${ids.currentVersion}`
	);

	await expect(
		page.getByText('อ่านอย่างเดียว ข้อมูลที่เผยแพร่แล้วไม่ถูกแก้ย้อนหลัง')
	).toBeVisible();
	await page.getByRole('button', { name: 'เลือกรุ่นตารางสอน' }).click();
	await expect(page.getByRole('option', { name: /เผยแพร่แล้ว · กำลังใช้/ })).toBeVisible();
	await expect(page.getByRole('option', { name: /เผยแพร่แล้ว · รอเริ่มใช้/ })).toBeVisible();
	await expect(page.getByRole('option', { name: /เผยแพร่แล้ว · ประวัติ/ })).toBeVisible();
	await page.getByRole('option', { name: /แบบร่าง · 2026-09-15/ }).click();

	await expect(page.getByText('แก้ไขคาบได้')).toBeVisible();
	await expect(page.getByRole('heading', { name: 'ขั้นตอนของรุ่นตารางสอนนี้' })).toBeVisible();
	await expect(page.getByTitle('เพิ่มคาบ').first()).toBeEnabled();
});

test('manager creates a date-effective timetable revision and lands on its draft version', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-08-30T08:00:00+07:00'));
	const mocked = await mockTimetable(page);
	await page.goto(
		`/staff/academic/timetable?academicYearId=${ids.year}&academicTermId=${ids.term}`
	);

	await page.getByRole('button', { name: 'สร้างรุ่นตารางสอนใหม่' }).click();
	await page.getByRole('button', { name: 'เลือกวันที่เริ่มใช้รุ่นใหม่' }).click();
	await page.getByRole('button', { name: 'วันอาทิตย์ที่ 30 สิงหาคม 2569', exact: true }).click();
	await page.getByLabel('เหตุผลการเปลี่ยนแปลง').fill('ปรับตารางหลังเปลี่ยนจำนวนคาบและห้องเรียน');
	await page.getByRole('button', { name: 'สร้างแบบร่าง' }).click();

	await expect(page.getByText('กำลังแก้รุ่นแบบร่าง')).toBeVisible();
	expect(mocked.createdChangeRequest()).toMatchObject({
		academicTermId: ids.term,
		effectiveFrom: '2026-08-30',
		reason: 'ปรับตารางหลังเปลี่ยนจำนวนคาบและห้องเรียน'
	});
});

test('draft create move and deactivate use the selected version and invalidate readiness', async ({
	page
}) => {
	const mocked = await mockTimetable(page, {
		versions: allVersions(),
		changeSets: [draftChangeSet()],
		preview: readinessPreview()
	});
	await page.goto(
		`/staff/academic/timetable?academicYearId=${ids.year}&academicTermId=${ids.term}&timetableVersionId=${ids.draftVersion}`
	);
	await page.getByRole('button', { name: 'ตรวจความพร้อม' }).click();
	await expect(page.getByText('ไม่มีจุดบล็อกการเผยแพร่')).toBeVisible();

	await page.getByTitle('เพิ่มคาบ').first().click();
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();

	await expect(page.getByText('ยังไม่ได้ตรวจความพร้อม')).toBeVisible();
	expect(mocked.previewRequestCount()).toBe(1);
	expect(mocked.createdEntryRequest()).toMatchObject({
		timetableVersionId: ids.draftVersion,
		learningGroupId: ids.group,
		instructorIds: []
	});

	await page.getByRole('button', { name: /ค21101/ }).click();
	await page.getByLabel('วัน').click();
	await page.getByRole('option', { name: 'อังคาร' }).click();
	await page.getByLabel('หมายเหตุ').fill('ย้ายคาบในรุ่นแบบร่าง');
	await page.getByRole('button', { name: 'บันทึก', exact: true }).click();
	expect(mocked.updatedEntryRequest()).toMatchObject({
		timetableVersionId: ids.draftVersion,
		dayOfWeek: 'TUE',
		note: 'ย้ายคาบในรุ่นแบบร่าง'
	});

	await page.getByRole('button', { name: /ค21101/ }).click();
	await page.getByRole('button', { name: 'ลบคาบ' }).click();
	expect(mocked.deletedEntryVersion()).toBe(ids.draftVersion);
});

test('readiness shows per-group target completion and blocks publication when a deficit remains', async ({
	page
}) => {
	await mockTimetable(page, {
		versions: allVersions(),
		changeSets: [draftChangeSet()],
		preview: readinessPreview('blocking')
	});
	await page.goto(
		`/staff/academic/timetable?academicYearId=${ids.year}&academicTermId=${ids.term}&timetableVersionId=${ids.draftVersion}`
	);
	await page.getByRole('button', { name: 'ตรวจความพร้อม' }).click();

	await expect(page.getByText('คาบยังไม่ครบตามเป้าหมาย')).toBeVisible();
	const groupRow = page.getByRole('row').filter({ hasText: 'ม.1/1' });
	await expect(groupRow).toContainText('1');
	await expect(groupRow).toContainText('2');
	await expect(page.getByRole('button', { name: /เผยแพร่ตั้งแต่/ })).toBeDisabled();
});

test('warning acknowledgement publishes the exact preview and makes the version read-only', async ({
	page
}) => {
	const mocked = await mockTimetable(page, {
		versions: allVersions(),
		changeSets: [draftChangeSet()],
		preview: readinessPreview('warning')
	});
	await page.goto(
		`/staff/academic/timetable?academicYearId=${ids.year}&academicTermId=${ids.term}&timetableVersionId=${ids.draftVersion}`
	);
	await page.getByRole('button', { name: 'ตรวจความพร้อม' }).click();
	await page.getByRole('checkbox', { name: 'รับทราบ คาบจริงมากกว่าเป้าหมาย' }).click();
	await page.getByRole('button', { name: /เผยแพร่ตั้งแต่/ }).click();

	await expect(
		page.getByText('อ่านอย่างเดียว ข้อมูลที่เผยแพร่แล้วไม่ถูกแก้ย้อนหลัง')
	).toBeVisible();
	await expect(page.getByTitle('เพิ่มคาบ').first()).toBeDisabled();
	expect(mocked.publishedRequest()).toMatchObject({
		rowVersion: 1,
		targetTimetableVersionRowVersion: 3,
		previewHash: 'a'.repeat(64),
		acknowledgedWarningCodes: ['weekly_period_excess']
	});
});

test('read-only staff can inspect a draft but cannot create revisions or mutate entries', async ({
	page
}) => {
	const mocked = await mockTimetable(page, {
		permissions: ['learning_offering.read.school'],
		versions: allVersions(),
		changeSets: [draftChangeSet()]
	});
	await page.goto(
		`/staff/academic/timetable?academicYearId=${ids.year}&academicTermId=${ids.term}&timetableVersionId=${ids.draftVersion}`
	);

	await expect(page.getByText('อ่านอย่างเดียว คุณไม่มีสิทธิ์แก้ไขคาบ')).toBeVisible();
	await expect(
		page.getByText('คุณดูแบบร่างนี้ได้ แต่ไม่มีสิทธิ์เพิ่ม แก้ไข หรือลบคาบ')
	).toBeVisible();
	await expect(page.getByRole('button', { name: 'สร้างรุ่นตารางสอนใหม่' })).toHaveCount(0);
	await expect(page.getByTitle('เพิ่มคาบ').first()).toBeDisabled();
	expect(mocked.managementRequestCount()).toBe(0);
});
