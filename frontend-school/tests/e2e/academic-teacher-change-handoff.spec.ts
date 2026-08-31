import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });
test.describe.configure({ mode: 'serial' });

const ids = {
	year: '10000000-0000-4000-8000-000000000201',
	term: '20000000-0000-4000-8000-000000000201',
	baseVersion: '30000000-0000-4000-8000-000000000201',
	targetVersion: '30000000-0000-4000-8000-000000000202',
	changeSet: '40000000-0000-4000-8000-000000000201',
	addItem: '41000000-0000-4000-8000-000000000201',
	stopItem: '41000000-0000-4000-8000-000000000202',
	user: '50000000-0000-4000-8000-000000000201',
	offering: '60000000-0000-4000-8000-000000000201',
	group: '70000000-0000-4000-8000-000000000201',
	assignmentA: '71000000-0000-4000-8000-000000000201',
	teacherA: '80000000-0000-4000-8000-000000000201',
	teacherB: '80000000-0000-4000-8000-000000000202',
	entry1: '90000000-0000-4000-8000-000000000201',
	entry2: '90000000-0000-4000-8000-000000000202',
	period1: 'a0000000-0000-4000-8000-000000000201',
	period2: 'a0000000-0000-4000-8000-000000000202'
};

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

function teacherChangeSet() {
	const base = {
		createdAt: '2026-08-31T00:00:00Z',
		createdBy: ids.user,
		rowVersion: 1,
		updatedAt: '2026-08-31T00:00:00Z'
	};
	return {
		id: ids.changeSet,
		academicTermId: ids.term,
		academicYearId: ids.year,
		effectiveFrom: '2026-09-01',
		reason: 'เปลี่ยนครูผู้สอนระหว่างภาคเรียน',
		status: 'draft',
		baseTimetableVersionId: ids.baseVersion,
		targetTimetableVersionId: ids.targetVersion,
		rowVersion: 2,
		createdBy: ids.user,
		publishedBy: null,
		publishedAt: null,
		cancelledBy: null,
		cancelledAt: null,
		createdAt: '2026-08-31T00:00:00Z',
		updatedAt: '2026-08-31T01:00:00Z',
		items: [
			{
				...base,
				id: ids.addItem,
				actionKind: 'add_group_teacher',
				learningGroupId: ids.group,
				learningGroupLabel: 'M1-1 · ม.1/1',
				teacherId: ids.teacherB,
				teacherLabel: 'ครูบี รับช่วง',
				teacherRole: 'primary'
			},
			{
				...base,
				id: ids.stopItem,
				actionKind: 'stop_group_teacher',
				learningGroupId: ids.group,
				learningGroupLabel: 'M1-1 · ม.1/1',
				learningGroupTeacherId: ids.assignmentA,
				teacherId: ids.teacherA,
				teacherLabel: 'ครูเอ คนเดิม',
				teacherRole: 'primary'
			}
		]
	};
}

function entry(entryId: string, periodId: string, periodLabel: string, afterTeacher = false) {
	return {
		entryId,
		rowVersion: 1,
		learningGroupId: ids.group,
		learningGroupLabel: 'M1-1 · ม.1/1',
		offeringLabel: 'ค21101 · คณิตศาสตร์พื้นฐาน',
		dayOfWeek: 'monday',
		bellSchedulePeriodId: periodId,
		periodLabel,
		roomLabel: 'M101 · ห้อง ม.1/1',
		beforeInstructors: [
			{ instructorId: ids.teacherA, displayName: 'ครูเอ คนเดิม', role: 'primary' }
		],
		afterInstructors: afterTeacher
			? [{ instructorId: ids.teacherB, displayName: 'ครูบี รับช่วง', role: 'primary' }]
			: [{ instructorId: ids.teacherA, displayName: 'ครูเอ คนเดิม', role: 'primary' }]
	};
}

async function mockTeacherHandoff(
	page: Page,
	options: { conflict?: boolean; stalePreviewOnce?: boolean } = {}
) {
	let applyRequests = 0;
	let handoffPreviewRequests = 0;
	let lastApplyBody: Record<string, unknown> | null = null;
	let stalePreviewRemaining = options.stalePreviewOnce ? 1 : 0;
	const changeSet = teacherChangeSet();
	const timetableRoute = `/staff/academic/timetable?academicYearId=${ids.year}&academicTermId=${ids.term}&timetableVersionId=${ids.targetVersion}&view=group&ownerId=${ids.group}`;

	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const changeSetPath = `/api/academic/term-change-sets/${ids.changeSet}`;
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: ids.user,
					username: 'teacher-handoff-test',
					firstName: 'ฝ่ายวิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					createdAt: '2026-08-31T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: ['*']
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
					timetableVersionEffectiveFrom: '2026-05-01',
					homerooms: [],
					unlinked: []
				});
				return;
			}
			if (url.pathname === '/api/academic/term-change-sets' && route.request().method() === 'GET') {
				await fulfill(route, [changeSet]);
				return;
			}
			if (url.pathname === changeSetPath && route.request().method() === 'GET') {
				await fulfill(route, changeSet);
				return;
			}
			if (url.pathname === `${changeSetPath}/preview` && route.request().method() === 'GET') {
				await fulfill(route, {
					changeSetId: ids.changeSet,
					changeSetRowVersion: 2,
					effectiveFrom: '2026-09-01',
					targetTimetableVersionId: ids.targetVersion,
					targetTimetableVersionRowVersion: 3,
					previewHash: 'a'.repeat(64),
					findings: [],
					impactCounts: {
						groups: 1,
						homerooms: 1,
						membershipIntervals: 0,
						teacherAssignments: 2,
						targetTimetableEntries: 2,
						courseAssessmentPlans: 0,
						courseAssessmentCategories: 0,
						courseAssessmentItems: 0,
						learningResults: 0,
						examScheduleItems: 0,
						supervisionObservations: 0
					},
					scheduleCounts: []
				});
				return;
			}
			if (url.pathname === '/api/academic/delivery/management-options') {
				await fulfill(route, {
					academicYearId: ids.year,
					academicTermId: ids.term,
					catalogVersions: [],
					gradeLevels: [],
					studyPrograms: [],
					organizationUnits: [],
					homerooms: [],
					rooms: [],
					teachers: [
						{ id: ids.teacherA, name: 'ครูเอ คนเดิม', title: 'ครู' },
						{ id: ids.teacherB, name: 'ครูบี รับช่วง', title: 'ครู' }
					],
					learningGroups: [
						{
							id: ids.group,
							learningOfferingId: ids.offering,
							academicTermId: ids.term,
							academicYearId: ids.year,
							code: 'M1-1',
							name: 'ม.1/1',
							description: null,
							capacity: 40,
							status: 'published',
							teachersLocked: true,
							rosterStatus: 'published',
							rosterPublishedAt: '2026-05-01T00:00:00Z',
							rowVersion: 1,
							migrated: false,
							createdAt: '2026-05-01T00:00:00Z',
							updatedAt: '2026-05-01T00:00:00Z',
							teacherAssignments: [
								{
									id: ids.assignmentA,
									teacherId: ids.teacherA,
									displayName: 'ครูเอ คนเดิม',
									role: 'primary',
									startsOn: '2026-05-01',
									endsOn: null,
									rowVersion: 1
								}
							],
							homeroomIds: [ids.group],
							preferredRoomIds: []
						}
					]
				});
				return;
			}
			if (
				url.pathname === `${changeSetPath}/teacher-handoff/preview` &&
				route.request().method() === 'POST'
			) {
				handoffPreviewRequests += 1;
				if (stalePreviewRemaining > 0) {
					stalePreviewRemaining -= 1;
					await fulfill(route, 'ข้อมูลรุ่นตารางเปลี่ยนแล้ว', 409);
					return;
				}
				const body = route.request().postDataJSON() as {
					mode: 'assign_one' | 'assign_coteachers' | 'manual';
					entryIds?: string[];
				};
				const allEntries = [
					entry(ids.entry1, ids.period1, 'คาบ 1'),
					entry(ids.entry2, ids.period2, 'คาบ 2')
				];
				const selectedIds = body.entryIds?.length
					? body.entryIds
					: allEntries.map((item) => item.entryId);
				const proposed =
					body.mode === 'manual'
						? []
						: allEntries
								.filter((item) => selectedIds.includes(item.entryId))
								.map((item) =>
									entry(item.entryId, item.bellSchedulePeriodId, item.periodLabel, true)
								);
				const conflicts = options.conflict
					? [
							{
								kind: 'instructor_collision',
								message: 'ครูบี รับช่วง มีคาบอื่นในวันและคาบเดียวกัน',
								entryIds: [ids.entry1],
								instructorIds: [ids.teacherB],
								timetableRoute
							}
						]
					: [];
				await fulfill(route, {
					changeSetId: ids.changeSet,
					changeSetRowVersion: 2,
					teacherChangeItemId: ids.stopItem,
					targetTimetableVersionId: ids.targetVersion,
					targetTimetableVersionRowVersion: 3,
					mode: body.mode,
					affectedEntries: allEntries,
					proposedEntries: proposed,
					conflicts,
					previewHash: body.mode === 'manual' ? null : 'b'.repeat(64),
					canApply: body.mode !== 'manual' && proposed.length > 0 && conflicts.length === 0,
					timetableRoute
				});
				return;
			}
			if (
				url.pathname === `${changeSetPath}/teacher-handoff/apply` &&
				route.request().method() === 'POST'
			) {
				applyRequests += 1;
				lastApplyBody = route.request().postDataJSON() as Record<string, unknown>;
				await fulfill(route, {
					handoff: {
						changeSetId: ids.changeSet,
						changeSetRowVersion: 2,
						teacherChangeItemId: ids.stopItem,
						targetTimetableVersionId: ids.targetVersion,
						targetTimetableVersionRowVersion: 3,
						mode: 'assign_one',
						affectedEntries: [entry(ids.entry1, ids.period1, 'คาบ 1')],
						proposedEntries: [entry(ids.entry1, ids.period1, 'คาบ 1', true)],
						conflicts: [],
						previewHash: 'b'.repeat(64),
						canApply: true,
						timetableRoute
					},
					updatedEntries: [
						{ entryId: ids.entry1, rowVersion: 2 },
						{ entryId: ids.entry2, rowVersion: 2 }
					],
					replayed: false
				});
				return;
			}
			if (url.pathname === '/api/academic/delivery/workspace') {
				await fulfill(route, { academicTermId: ids.term, offerings: [] });
				return;
			}
			if (url.pathname === '/api/menu/user') return void (await fulfill(route, { groups: [] }));
			if (url.pathname === '/api/me/work-items/counts') {
				return void (await fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				}));
			}
			if (url.pathname === '/api/notifications') {
				return void (await fulfill(route, { items: [], unread_count: 0 }));
			}
			if (url.pathname === '/api/notifications/stream') {
				return void (await route.fulfill({
					status: 200,
					contentType: 'text/event-stream',
					body: ''
				}));
			}
			await fulfill(route, {});
		}
	);

	return {
		applyRequestCount: () => applyRequests,
		previewRequestCount: () => handoffPreviewRequests,
		lastApplyRequest: () => lastApplyBody,
		timetableRoute
	};
}

function deliveryUrl() {
	return `/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&changeSetId=${ids.changeSet}&teacherChangeItemId=${ids.stopItem}`;
}

test('assigns one replacement teacher to the selected affected entries', async ({ page }) => {
	const mocked = await mockTeacherHandoff(page);
	await page.goto(deliveryUrl());

	await expect(page.getByRole('heading', { name: 'ส่งต่อคาบของ ครูเอ คนเดิม' })).toBeVisible();
	await page.getByRole('combobox', { name: 'เลือกจากทีมสอนหลังเริ่มใช้' }).click();
	await page.getByText('ครูบี รับช่วง', { exact: true }).last().click();
	await expect(page.getByText('เลือกแล้ว 2 จาก 2 คาบ')).toBeVisible();
	await page.getByRole('button', { name: 'ส่งต่อครูให้คาบที่เลือก' }).click();
	await expect(page.getByText('ส่งต่อครูให้ 2 คาบแล้ว')).toBeVisible();

	expect(mocked.applyRequestCount()).toBe(1);
	expect(mocked.lastApplyRequest()).toMatchObject({
		teacherChangeItemId: ids.stopItem,
		mode: 'assign_one',
		instructorIds: [ids.teacherB],
		previewHash: 'b'.repeat(64)
	});
});

test('keeps apply disabled when the replacement teacher has a collision', async ({ page }) => {
	const mocked = await mockTeacherHandoff(page, { conflict: true });
	await page.goto(deliveryUrl());

	await page.getByRole('combobox', { name: 'เลือกจากทีมสอนหลังเริ่มใช้' }).click();
	await page.getByText('ครูบี รับช่วง', { exact: true }).last().click();
	await expect(page.getByText('ครูบี รับช่วง มีคาบอื่นในวันและคาบเดียวกัน')).toBeVisible();
	await expect(page.getByRole('button', { name: 'ส่งต่อครูให้คาบที่เลือก' })).toBeDisabled();
	await expect(page.getByRole('link', { name: /แก้ในตารางสอน/ })).toHaveAttribute(
		'href',
		mocked.timetableRoute
	);
	expect(mocked.applyRequestCount()).toBe(0);
});

test('manual mode opens the exact target timetable and never sends apply', async ({ page }) => {
	const mocked = await mockTeacherHandoff(page);
	await page.goto(deliveryUrl());

	await page.getByRole('button', { name: 'ให้ครูคนเดียวสอนทุกคาบที่เลือก' }).click();
	await page.getByRole('option', { name: 'จัดเองในหน้าตารางสอน' }).click();
	await expect(page.getByText('ระบบจะไม่เปลี่ยนครูในคาบให้อัตโนมัติ')).toBeVisible();
	await expect(page.getByRole('link', { name: /เปิดหน้าตารางสอนเพื่อจัดเอง/ })).toHaveAttribute(
		'href',
		mocked.timetableRoute
	);
	await expect(page.getByRole('button', { name: 'ส่งต่อครูให้คาบที่เลือก' })).toHaveCount(0);
	expect(mocked.previewRequestCount()).toBeGreaterThan(0);
	expect(mocked.applyRequestCount()).toBe(0);
});

test('preserves the selected handoff mode and teacher after a stale preview', async ({ page }) => {
	const mocked = await mockTeacherHandoff(page, { stalePreviewOnce: true });
	await page.goto(deliveryUrl());

	const teacherCombobox = page.getByRole('combobox', { name: 'เลือกจากทีมสอนหลังเริ่มใช้' });
	await teacherCombobox.click();
	await page.getByText('ครูบี รับช่วง', { exact: true }).last().click();
	await expect(
		page.getByText('ข้อมูลเปลี่ยนระหว่างตรวจ ระบบโหลดรุ่นล่าสุดแล้ว กรุณาตรวจอีกครั้ง')
	).toBeVisible();
	await expect(teacherCombobox).toContainText('ครูบี รับช่วง');

	await page.getByRole('button', { name: 'ตรวจใหม่' }).click();
	await expect(page.getByText('เลือกแล้ว 2 จาก 2 คาบ')).toBeVisible();
	await expect(teacherCombobox).toContainText('ครูบี รับช่วง');
	expect(mocked.previewRequestCount()).toBe(2);
	expect(mocked.applyRequestCount()).toBe(0);
});
