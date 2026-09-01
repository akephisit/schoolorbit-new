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
	blockA: 'c1000000-0000-4000-8000-000000000201',
	blockB: 'c1000000-0000-4000-8000-000000000202',
	createdBlock: 'c1000000-0000-4000-8000-000000000203',
	blockGroupA: 'd1000000-0000-4000-8000-000000000201',
	blockGroupB: 'd1000000-0000-4000-8000-000000000202'
} as const;

export type MockBlock = ReturnType<typeof makeTimetableBlock>;

export interface TimetableMockOptions {
	status?: 'draft' | 'published';
	blocks?: MockBlock[];
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
		targets: []
	};
}

function instructor(teacherId: string, index = 0) {
	return {
		teacherId,
		displayName: teacherId === timetableIds.teacherA ? 'ครูคณิตศาสตร์ A' : 'ครูคณิตศาสตร์ B',
		role: index === 0 ? 'primary' : 'secondary',
		orderIndex: index
	};
}

function instructors(ids: string[]) {
	return ids.map(instructor);
}

function periodDetails(periodId: string) {
	if (periodId === timetableIds.period1) {
		return { periodName: 'คาบ 1', startTime: '08:30:00', endTime: '09:20:00' };
	}
	if (periodId === timetableIds.period2) {
		return { periodName: 'คาบ 2', startTime: '09:20:00', endTime: '10:10:00' };
	}
	return { periodName: 'คาบ 3', startTime: '10:10:00', endTime: '11:00:00' };
}

export function makeTimetableBlock(
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
	const blockGroupId =
		groupId === timetableIds.groupA ? timetableIds.blockGroupA : timetableIds.blockGroupB;
	return {
		id,
		timetableVersionId: options.versionId ?? timetableIds.draftVersion,
		academicTermId: timetableIds.term,
		academicYearId: timetableIds.year,
		bellScheduleId: timetableIds.schedule,
		bellSchedulePeriodId: periodId,
		blockKind: 'course',
		learningOfferingId: offeringId,
		offeringCode: code,
		offeringName: name,
		schedulingMode: null,
		structuralKind: null,
		seriesId: null,
		dayOfWeek: 'MON',
		...periodDetails(periodId),
		title: null,
		note: null,
		isActive: true,
		groups: [
			{
				id: blockGroupId,
				learningGroupId: groupId,
				learningOfferingId: offeringId,
				code: groupId === timetableIds.groupA ? 'M1-1-A' : 'M1-1-B',
				name: groupId === timetableIds.groupA ? 'ม.1/1 คณิตศาสตร์' : 'ม.1/1 วิทยาศาสตร์',
				homeroomIds: [timetableIds.homeroom],
				instructors: instructors(options.instructorIds ?? [timetableIds.teacherA]),
				roomId: timetableIds.room,
				roomCode: 'MATH-1',
				rowVersion: 1,
				isActive: true,
				syncStatus: null
			}
		],
		homerooms: [],
		teachers: [],
		syncStates: [],
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
	let blocks = [...(options.blocks ?? [])];
	const eligibleInstructorIds = options.eligibleInstructorIds ?? [timetableIds.teacherA];
	const requiredPeriods = options.requiredPeriods ?? 3;
	let workspaceRequests = 0;
	let previewRequests = 0;
	let createRequests = 0;
	let updateRequests = 0;
	let swapRequests = 0;

	const workspace = () => {
		const scheduledPeriods = blocks.filter((block) =>
			block.groups.some((group) => group.learningGroupId === timetableIds.groupA)
		).length;
		return {
			version: selectedVersion,
			bellPeriods: periods(),
			blocks,
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
					eligibleInstructors: instructors(eligibleInstructorIds)
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
					eligibleInstructors: instructors([timetableIds.teacherB])
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
			ordinaryDemands: [
				{
					learningGroupId: timetableIds.groupA,
					learningOfferingId: timetableIds.offeringA,
					offeringCode: 'ค21101',
					offeringName: 'คณิตศาสตร์พื้นฐาน',
					requiredPeriods,
					scheduledPeriods,
					remainingPeriods: Math.max(requiredPeriods - scheduledPeriods, 0),
					homeroomIds: [timetableIds.homeroom],
					eligibleInstructors: instructors(eligibleInstructorIds)
				}
			],
			synchronizedDemands: [],
			summary: {
				blockCount: blocks.length,
				ordinaryDemandCount: Math.max(requiredPeriods - scheduledPeriods, 0) > 0 ? 1 : 0,
				synchronizedDemandCount: 0,
				linkedGroupCount: 0,
				waitingGroupCount: 0,
				conflictGroupCount: 0,
				excludedGroupCount: 0
			}
		};
	};

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
			if (url.pathname === '/api/academic/timetable-blocks/workspace') {
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
			if (url.pathname === '/api/academic/timetable-blocks/placement-preview') {
				previewRequests += 1;
				const body = route.request().postDataJSON();
				const sourceBlockId = body.source.kind === 'existing_block' ? body.source.blockId : null;
				const target = blocks.find(
					(block) =>
						block.id !== sourceBlockId &&
						block.dayOfWeek === body.targetDayOfWeek &&
						block.bellSchedulePeriodId === body.targetBellSchedulePeriodId
				);
				if (body.targetBellSchedulePeriodId === options.blockedPeriodId) {
					await fulfill(route, {
						state: 'blocked',
						sourceBlockId,
						targetBlockId: target?.id ?? null,
						targetDayOfWeek: body.targetDayOfWeek,
						targetBellSchedulePeriodId: body.targetBellSchedulePeriodId,
						normalizedCandidate: body.candidate,
						conflicts: [
							{
								code: 'teacher_conflict',
								conflictType: 'teacher',
								existingBlockId: timetableIds.blockB,
								targetKind: 'teacher',
								targetId: timetableIds.teacherA,
								message: 'ครูคณิตศาสตร์ A มีคาบสอนอยู่แล้ว'
							}
						],
						mutation: null
					});
					return;
				}
				await fulfill(route, {
					state: target ? 'swap' : 'move',
					sourceBlockId,
					targetBlockId: target?.id ?? null,
					targetDayOfWeek: body.targetDayOfWeek,
					targetBellSchedulePeriodId: body.targetBellSchedulePeriodId,
					normalizedCandidate: body.candidate,
					conflicts: [],
					mutation: body.source.kind === 'ordinary_demand' ? 'create' : target ? 'swap' : 'move'
				});
				return;
			}
			if (url.pathname === '/api/academic/timetable-blocks/ordinary' && method === 'POST') {
				createRequests += 1;
				const body = route.request().postDataJSON();
				const created = {
					...makeTimetableBlock(timetableIds.createdBlock, body.bellSchedulePeriodId, {
						versionId: body.timetableVersionId,
						groupId: body.learningGroupId,
						instructorIds: body.instructorIds
					}),
					dayOfWeek: body.dayOfWeek
				};
				blocks = [...blocks, created];
				await fulfill(route, created, 201);
				return;
			}
			if (url.pathname === '/api/academic/timetable-blocks/swap' && method === 'POST') {
				swapRequests += 1;
				const body = route.request().postDataJSON();
				const blockA = blocks.find((block) => block.id === body.blockAId);
				const blockB = blocks.find((block) => block.id === body.blockBId);
				if (!blockA || !blockB) throw new Error('Mock swap blocks not found');
				const nextA = {
					...blockA,
					dayOfWeek: blockB.dayOfWeek,
					bellSchedulePeriodId: blockB.bellSchedulePeriodId,
					...periodDetails(blockB.bellSchedulePeriodId),
					rowVersion: blockA.rowVersion + 1
				};
				const nextB = {
					...blockB,
					dayOfWeek: blockA.dayOfWeek,
					bellSchedulePeriodId: blockA.bellSchedulePeriodId,
					...periodDetails(blockA.bellSchedulePeriodId),
					rowVersion: blockB.rowVersion + 1
				};
				blocks = blocks.map((block) =>
					block.id === nextA.id ? nextA : block.id === nextB.id ? nextB : block
				);
				await fulfill(route, { blockA: nextA, blockB: nextB });
				return;
			}
			if (/^\/api\/academic\/timetable-blocks\/[^/]+$/.test(url.pathname) && method === 'PUT') {
				updateRequests += 1;
				const id = url.pathname.split('/').at(-1);
				const body = route.request().postDataJSON();
				const existing = blocks.find((block) => block.id === id);
				if (!existing) throw new Error('Mock update block not found');
				const periodId = body.bellSchedulePeriodId ?? existing.bellSchedulePeriodId;
				const updated = {
					...existing,
					dayOfWeek: body.dayOfWeek ?? existing.dayOfWeek,
					bellSchedulePeriodId: periodId,
					...periodDetails(periodId),
					groups: body.instructorIds
						? existing.groups.map((group) => ({
								...group,
								instructors: instructors(body.instructorIds)
							}))
						: existing.groups,
					rowVersion: existing.rowVersion + 1
				};
				blocks = blocks.map((block) => (block.id === updated.id ? updated : block));
				await fulfill(route, updated);
				return;
			}
			if (/^\/api\/academic\/timetable-blocks\/[^/]+$/.test(url.pathname) && method === 'DELETE') {
				const id = url.pathname.split('/').at(-1);
				const existing = blocks.find((block) => block.id === id);
				if (!existing) throw new Error('Mock delete block not found');
				blocks = blocks.filter((block) => block.id !== id);
				await fulfill(route, { ...existing, isActive: false, rowVersion: existing.rowVersion + 1 });
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
		blocks: () => blocks
	};
}
