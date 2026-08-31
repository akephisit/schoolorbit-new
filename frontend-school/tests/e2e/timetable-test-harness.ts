import type { Page, Route } from '@playwright/test';

export const timetableIds = {
	year: '11000000-0000-4000-8000-000000000201',
	term: '21000000-0000-4000-8000-000000000201',
	schedule: '31000000-0000-4000-8000-000000000201',
	period1: '41000000-0000-4000-8000-000000000201',
	period2: '41000000-0000-4000-8000-000000000202',
	period3: '41000000-0000-4000-8000-000000000203',
	user: '51000000-0000-4000-8000-000000000201',
	teacherA: '51000000-0000-4000-8000-000000000202',
	teacherB: '51000000-0000-4000-8000-000000000203',
	offeringA: '61000000-0000-4000-8000-000000000201',
	offeringB: '61000000-0000-4000-8000-000000000202',
	groupA: '71000000-0000-4000-8000-000000000201',
	groupB: '71000000-0000-4000-8000-000000000202',
	homeroom: '81000000-0000-4000-8000-000000000201',
	room: '91000000-0000-4000-8000-000000000201',
	publishedVersion: 'a1000000-0000-4000-8000-000000000201',
	draftVersion: 'a1000000-0000-4000-8000-000000000202',
	changeSet: 'b1000000-0000-4000-8000-000000000201',
	entryA: 'c1000000-0000-4000-8000-000000000201',
	entryB: 'c1000000-0000-4000-8000-000000000202',
	createdEntry: 'c1000000-0000-4000-8000-000000000203'
} as const;

export type MockEntry = ReturnType<typeof makeTimetableEntry>;

export interface TimetableMockOptions {
	status?: 'draft' | 'published';
	entries?: MockEntry[];
	requiredPeriods?: number;
	eligibleInstructorIds?: string[];
	blockedPeriodId?: string;
}

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

export function makeTimetableVersion(status: 'draft' | 'published') {
	const id = status === 'draft' ? timetableIds.draftVersion : timetableIds.publishedVersion;
	return {
		id,
		academicTermId: timetableIds.term,
		academicYearId: timetableIds.year,
		bellScheduleId: timetableIds.schedule,
		changeSetId: status === 'draft' ? timetableIds.changeSet : null,
		status,
		displayState: status === 'published' ? 'current' : null,
		effectiveFrom: '2026-05-01',
		effectiveUntil: null,
		sourceVersionId: status === 'draft' ? timetableIds.publishedVersion : null,
		rowVersion: 1,
		publishedAt: status === 'published' ? '2026-05-01T00:00:00Z' : null,
		publishedBy: status === 'published' ? timetableIds.user : null,
		createdBy: timetableIds.user,
		createdAt: '2026-05-01T00:00:00Z',
		updatedAt: '2026-08-31T00:00:00Z',
		targets: [
			{
				timetableVersionId: id,
				learningOfferingId: timetableIds.offeringA,
				standardPeriodsPerWeek: 3,
				weeklyPeriodTarget: 3
			}
		]
	};
}

function instructors(ids: string[]) {
	return ids.map((userId, index) => ({
		userId,
		displayName: userId === timetableIds.teacherA ? 'ครูคณิตศาสตร์ A' : 'ครูคณิตศาสตร์ B',
		role: index === 0 ? 'primary' : 'secondary',
		subjectGroupId: null,
		subjectGroupName: null,
		subjectGroupDisplayOrder: null
	}));
}

export function makeTimetableEntry(
	id: string,
	periodId: string,
	options: {
		versionId?: string;
		groupId?: string;
		offeringId?: string;
		code?: string;
		name?: string;
		instructorIds?: string[];
	} = {}
) {
	const groupId = options.groupId ?? timetableIds.groupA;
	const offeringId = options.offeringId ?? timetableIds.offeringA;
	const code = options.code ?? 'ค21101';
	const name = options.name ?? 'คณิตศาสตร์พื้นฐาน';
	return {
		id,
		timetableVersionId: options.versionId ?? timetableIds.draftVersion,
		academicTermId: timetableIds.term,
		academicYearId: timetableIds.year,
		bellScheduleId: timetableIds.schedule,
		bellSchedulePeriodId: periodId,
		learningGroupId: groupId,
		learningGroupCode: groupId === timetableIds.groupA ? 'M1-1-A' : 'M1-1-B',
		learningGroupName: groupId === timetableIds.groupA ? 'ม.1/1 คณิตศาสตร์' : 'ม.1/1 วิทยาศาสตร์',
		offeringId,
		offeringCode: code,
		offeringName: name,
		homeroomId: null,
		homeroomName: null,
		roomId: timetableIds.room,
		roomCode: 'MATH-1',
		dayOfWeek: 'MON',
		startTime:
			periodId === timetableIds.period1
				? '08:30:00'
				: periodId === timetableIds.period2
					? '09:20:00'
					: '10:10:00',
		endTime:
			periodId === timetableIds.period1
				? '09:20:00'
				: periodId === timetableIds.period2
					? '10:10:00'
					: '11:00:00',
		periodName:
			periodId === timetableIds.period1
				? 'คาบ 1'
				: periodId === timetableIds.period2
					? 'คาบ 2'
					: 'คาบ 3',
		entryType: 'COURSE',
		title: null,
		note: null,
		isActive: true,
		instructors: instructors(options.instructorIds ?? [timetableIds.teacherA]),
		rowVersion: 1,
		createdAt: '2026-08-31T00:00:00Z',
		updatedAt: '2026-08-31T00:00:00Z'
	};
}

function periods() {
	return [
		[timetableIds.period1, 1, 'คาบ 1', '08:30:00', '09:20:00'],
		[timetableIds.period2, 2, 'คาบ 2', '09:20:00', '10:10:00'],
		[timetableIds.period3, 3, 'คาบ 3', '10:10:00', '11:00:00']
	].map(([id, orderIndex, name, startTime, endTime]) => ({
		id,
		bellScheduleId: timetableIds.schedule,
		orderIndex,
		name,
		startTime,
		endTime,
		applicableDays: 'MON,TUE,WED,THU,FRI',
		isActive: true
	}));
}

function changeSet() {
	return {
		id: timetableIds.changeSet,
		academicTermId: timetableIds.term,
		academicYearId: timetableIds.year,
		effectiveFrom: '2026-05-01',
		reason: 'ปรับตารางสอนสำหรับทดสอบ',
		status: 'draft',
		baseTimetableVersionId: timetableIds.publishedVersion,
		targetTimetableVersionId: timetableIds.draftVersion,
		rowVersion: 1,
		createdBy: timetableIds.user,
		publishedBy: null,
		publishedAt: null,
		cancelledBy: null,
		cancelledAt: null,
		createdAt: '2026-08-31T00:00:00Z',
		updatedAt: '2026-08-31T00:00:00Z',
		items: []
	};
}

export async function installTimetableMock(page: Page, options: TimetableMockOptions = {}) {
	const status = options.status ?? 'draft';
	const selectedVersion = makeTimetableVersion(status);
	const versions = [makeTimetableVersion('published'), makeTimetableVersion('draft')];
	let entries = [...(options.entries ?? [])];
	const eligibleInstructorIds = options.eligibleInstructorIds ?? [timetableIds.teacherA];
	const requiredPeriods = options.requiredPeriods ?? 3;
	let workspaceRequests = 0;
	let previewRequests = 0;
	let createRequests = 0;
	let updateRequests = 0;
	let swapRequests = 0;

	const workspace = () => ({
		version: selectedVersion,
		bellPeriods: periods(),
		entries,
		learningGroups: [
			{
				id: timetableIds.groupA,
				learningOfferingId: timetableIds.offeringA,
				code: 'M1-1-A',
				name: 'ม.1/1 คณิตศาสตร์',
				status: 'published',
				rosterStatus: 'published',
				offeringKind: 'course',
				offeringCode: 'ค21101',
				offeringName: 'คณิตศาสตร์พื้นฐาน',
				homeroomIds: [timetableIds.homeroom],
				eligibleInstructorIds
			},
			{
				id: timetableIds.groupB,
				learningOfferingId: timetableIds.offeringB,
				code: 'M1-1-B',
				name: 'ม.1/1 วิทยาศาสตร์',
				status: 'published',
				rosterStatus: 'published',
				offeringKind: 'course',
				offeringCode: 'ว21101',
				offeringName: 'วิทยาศาสตร์พื้นฐาน',
				homeroomIds: [timetableIds.homeroom],
				eligibleInstructorIds: [timetableIds.teacherB]
			}
		],
		homerooms: [
			{
				id: timetableIds.homeroom,
				code: 'M1-1',
				name: 'ม.1/1',
				gradeLevelId: '81000000-0000-4000-8000-000000000301',
				gradeLevelType: 'secondary',
				gradeLevelYear: 1,
				roomNumber: '1',
				isActive: true
			}
		],
		rooms: [{ id: timetableIds.room, code: 'MATH-1', name: 'ห้องคณิตศาสตร์', status: 'ACTIVE' }],
		staff: [
			{ id: timetableIds.teacherA, displayName: 'ครูคณิตศาสตร์ A', status: 'active' },
			{ id: timetableIds.teacherB, displayName: 'ครูคณิตศาสตร์ B', status: 'active' }
		],
		unscheduledDemands: [
			{
				learningGroupId: timetableIds.groupA,
				learningOfferingId: timetableIds.offeringA,
				offeringCode: 'ค21101',
				offeringName: 'คณิตศาสตร์พื้นฐาน',
				requiredPeriods,
				scheduledPeriods: entries.filter((entry) => entry.learningGroupId === timetableIds.groupA)
					.length,
				remainingPeriods: Math.max(
					requiredPeriods -
						entries.filter((entry) => entry.learningGroupId === timetableIds.groupA).length,
					0
				),
				homeroomIds: [timetableIds.homeroom],
				eligibleInstructorIds
			}
		]
	});

	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const method = route.request().method();
			if (url.pathname === '/api/auth/me') {
				await fulfill(route, {
					id: timetableIds.user,
					username: 'timetable-test',
					firstName: 'ตารางสอน',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					permissions: ['*']
				});
				return;
			}
			if (url.pathname === '/api/academic/context/options') {
				await fulfill(route, {
					activeAcademicYearId: timetableIds.year,
					activeAcademicTermId: timetableIds.term,
					years: [
						{
							id: timetableIds.year,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active',
							startDate: '2026-05-01',
							endDate: '2027-03-31'
						}
					],
					terms: [
						{
							id: timetableIds.term,
							academicYearId: timetableIds.year,
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
			if (url.pathname === '/api/academic/timetable-versions') {
				await fulfill(route, versions);
				return;
			}
			if (url.pathname === '/api/academic/timetable/workspace') {
				workspaceRequests += 1;
				await fulfill(route, workspace());
				return;
			}
			if (url.pathname === `/api/academic/term-change-sets/${timetableIds.changeSet}`) {
				await fulfill(route, changeSet());
				return;
			}
			if (url.pathname.endsWith('/preview') && url.pathname.includes('/term-change-sets/')) {
				await fulfill(route, {
					changeSetId: timetableIds.changeSet,
					changeSetRowVersion: 1,
					effectiveFrom: '2026-05-01',
					targetTimetableVersionId: timetableIds.draftVersion,
					targetTimetableVersionRowVersion: 1,
					previewHash: 'a'.repeat(64),
					findings: [],
					impactCounts: {},
					scheduleCounts: []
				});
				return;
			}
			if (url.pathname === '/api/academic/timetable/placement-preview') {
				previewRequests += 1;
				const body = route.request().postDataJSON();
				const sourceEntryId = body.source.kind === 'existing_entry' ? body.source.entryId : null;
				const target = entries.find(
					(entry) =>
						entry.id !== sourceEntryId &&
						entry.dayOfWeek === body.targetDayOfWeek &&
						entry.bellSchedulePeriodId === body.targetBellSchedulePeriodId
				);
				if (body.targetBellSchedulePeriodId === options.blockedPeriodId) {
					await fulfill(route, {
						state: 'blocked',
						sourceEntryId,
						targetEntryId: target?.id ?? null,
						targetDayOfWeek: body.targetDayOfWeek,
						targetBellSchedulePeriodId: body.targetBellSchedulePeriodId,
						normalizedCandidate: body.candidate,
						conflicts: [
							{
								conflictType: 'instructor',
								existingEntryId: timetableIds.entryB,
								message: 'ครูคณิตศาสตร์ A มีคาบสอนอยู่แล้ว'
							}
						],
						mutation: null
					});
					return;
				}
				const mutation =
					body.source.kind === 'unscheduled_demand' ? 'create' : target ? 'swap' : 'move';
				await fulfill(route, {
					state: target ? 'swap' : 'move',
					sourceEntryId,
					targetEntryId: target?.id ?? null,
					targetDayOfWeek: body.targetDayOfWeek,
					targetBellSchedulePeriodId: body.targetBellSchedulePeriodId,
					normalizedCandidate: body.candidate,
					conflicts: [],
					mutation
				});
				return;
			}
			if (url.pathname === '/api/academic/timetable' && method === 'POST') {
				createRequests += 1;
				const body = route.request().postDataJSON();
				const created = {
					...makeTimetableEntry(timetableIds.createdEntry, body.bellSchedulePeriodId, {
						versionId: body.timetableVersionId,
						groupId: body.learningGroupId,
						instructorIds: body.instructorIds
					}),
					dayOfWeek: body.dayOfWeek
				};
				entries = [...entries, created];
				await fulfill(route, created, 201);
				return;
			}
			if (url.pathname === '/api/academic/timetable/swap' && method === 'POST') {
				swapRequests += 1;
				const body = route.request().postDataJSON();
				const entryA = entries.find((entry) => entry.id === body.entryAId);
				const entryB = entries.find((entry) => entry.id === body.entryBId);
				if (!entryA || !entryB) throw new Error('Mock swap entries not found');
				const nextA = {
					...entryA,
					dayOfWeek: entryB.dayOfWeek,
					bellSchedulePeriodId: entryB.bellSchedulePeriodId,
					periodName: entryB.periodName,
					rowVersion: entryA.rowVersion + 1
				};
				const nextB = {
					...entryB,
					dayOfWeek: entryA.dayOfWeek,
					bellSchedulePeriodId: entryA.bellSchedulePeriodId,
					periodName: entryA.periodName,
					rowVersion: entryB.rowVersion + 1
				};
				entries = entries.map((entry) =>
					entry.id === nextA.id ? nextA : entry.id === nextB.id ? nextB : entry
				);
				await fulfill(route, { entryA: nextA, entryB: nextB });
				return;
			}
			if (url.pathname.startsWith('/api/academic/timetable/') && method === 'PUT') {
				updateRequests += 1;
				const id = url.pathname.split('/').at(-1);
				const body = route.request().postDataJSON();
				const existing = entries.find((entry) => entry.id === id);
				if (!existing) throw new Error('Mock update entry not found');
				const updated = {
					...existing,
					dayOfWeek: body.dayOfWeek ?? existing.dayOfWeek,
					bellSchedulePeriodId: body.bellSchedulePeriodId ?? existing.bellSchedulePeriodId,
					roomId: body.clearRoom ? null : (body.roomId ?? existing.roomId),
					instructors: body.instructorIds ? instructors(body.instructorIds) : existing.instructors,
					rowVersion: existing.rowVersion + 1
				};
				entries = entries.map((entry) => (entry.id === updated.id ? updated : entry));
				await fulfill(route, updated);
				return;
			}
			if (url.pathname.startsWith('/api/academic/timetable/') && method === 'DELETE') {
				const id = url.pathname.split('/').at(-1);
				const existing = entries.find((entry) => entry.id === id);
				if (!existing) throw new Error('Mock delete entry not found');
				const deleted = { ...existing, isActive: false, rowVersion: existing.rowVersion + 1 };
				entries = entries.filter((entry) => entry.id !== id);
				await fulfill(route, deleted);
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
		workspaceRequestCount: () => workspaceRequests,
		previewRequestCount: () => previewRequests,
		createRequestCount: () => createRequests,
		updateRequestCount: () => updateRequests,
		swapRequestCount: () => swapRequests,
		entries: () => entries
	};
}
