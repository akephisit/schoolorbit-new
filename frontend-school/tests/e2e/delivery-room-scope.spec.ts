import { expect, test, type Page, type Route } from '@playwright/test';
import type {
	AcademicTermChangeSet,
	CurriculumOfferingPreview,
	DeliveryManagementOptions,
	LearningGroup,
	LearningOffering
} from '../../src/lib/api/learning-delivery.js';
import {
	mockDeliveryNavigation,
	navigationIds as ids,
	navigationOfferingIds as offeringIds,
	navigationVersion,
	navigationWorkspace,
	navigationSummary
} from './fixtures/delivery-navigation-route-data';

test.use({ serviceWorkers: 'block' });
const roomIds = [ids.roomA, ids.roomB, 'f0000000-0000-4000-8000-000000000106'];
const roomNames = ['ม.4/2', 'ม.4/3', 'ม.6/1'];
const groupIds = [
	'f0000000-0000-4000-8000-000000000201',
	'f0000000-0000-4000-8000-000000000202',
	'f0000000-0000-4000-8000-000000000203'
];
const stamp = '2027-05-01T00:00:00Z';
const offeringId = offeringIds.basic21;
const offeringPath = `/api/academic/offerings/${offeringId}`;
const versionPath = `/api/academic/delivery-versions/${ids.version}`;
const changeSetPath = `/api/academic/term-change-sets/${ids.changeSet}`;
const overviewHref = `/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}&expandedRooms=${roomIds.join(',')}`;

async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		json: status < 400 ? { success: true, data } : { success: false, error: data }
	});
}
function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { promise, release };
}

async function mockRoomScope(
	page: Page,
	combined: boolean,
	options: {
		reader?: boolean;
		published?: boolean;
		synchronized?: boolean;
		conflictOnce?: boolean;
		failReadAfterSave?: boolean;
		saveGate?: Promise<void>;
		priorExcludedM43?: boolean;
		failWorkspaceOnce?: boolean;
		workspaceGate?: Promise<void>;
		workspaceGateFailure?: boolean;
		failGroups?: boolean;
	} = {}
) {
	await mockDeliveryNavigation(page, { manager: !options.reader });
	const graph = navigationVersion();
	const selected = graph.snapshot.offerings.find((item) => item.id === offeringId)!;
	selected.code = 'ว30246';
	selected.name = 'เคมีเพิ่มเติม';
	selected.weeklyPeriodTarget = 2;
	selected.sourceRequirementKind = 'required';
	const resourceKind = options.synchronized ? 'activity' : 'course';
	if (options.synchronized) {
		const activity = navigationVersion().snapshot.offerings.find(
			(item) => item.catalog.kind === 'activity'
		)!;
		if (activity.catalog.kind !== 'activity') throw new Error('Activity fixture is required');
		selected.kind = 'activity';
		selected.catalog = {
			...activity.catalog,
			activityId: offeringId,
			activityVersionId: offeringId
		};
	}
	const teacher = {
		id: 'f0000000-0000-4000-8000-000000000211',
		teacherId: ids.actor,
		displayName: 'ครูเคมีที่คงไว้',
		role: 'primary' as const,
		startsOn: '2027-05-01',
		endsOn: null,
		rowVersion: 5
	};
	const registry: LearningGroup[] = (combined ? [groupIds[0]] : groupIds).map((id, index) => ({
		id,
		learningOfferingId: offeringId,
		academicYearId: ids.year,
		academicTermId: ids.term,
		name: combined ? 'กลุ่มเรียนรวมที่คงไว้' : `กลุ่ม ${roomNames[index]}`,
		code: `G${index + 1}`,
		description: 'เก็บการตั้งค่าเดิม',
		capacity: 37,
		preferredRoomIds: [],
		rowVersion: 11,
		status: 'published',
		rosterStatus: 'draft',
		teachersLocked: true,
		teacherAssignments: [{ ...teacher, id: `f0000000-0000-4000-8000-00000000021${index + 1}` }],
		homeroomIds: combined ? [...roomIds] : [roomIds[index]],
		migrated: false,
		createdAt: stamp,
		updatedAt: stamp
	}));
	const originalRegistry = structuredClone(registry);
	selected.groups = registry.map((group) => ({
		id: group.id,
		code: group.code,
		name: group.name,
		description: group.description,
		capacity: group.capacity,
		homeroomIds: [...group.homeroomIds],
		preferredRoomIds: [],
		teachers: group.teacherAssignments.map((assignment) => ({
			assignmentId: assignment.id,
			teacherId: assignment.teacherId,
			displayName: assignment.displayName,
			role: assignment.role
		}))
	}));
	const originalOffering = structuredClone(selected);
	graph.sourceVersionId = ids.sourceVersion;
	graph.rowVersion = 4;
	graph.status = options.published ? 'published' : 'draft';
	graph.snapshot.offerings = [selected];
	let audience = new Set(roomIds);
	function project() {
		selected.homeroomIds = [...audience];
		selected.targets = [...audience].map((homeroomId) => ({
			id: `f0000000-0000-4000-8000-00000000040${roomIds.indexOf(homeroomId)}`,
			targetKind: 'homeroom',
			homeroomId,
			gradeLevelId: ids.grade,
			studyProgramId: ids.program
		}));
		selected.groups = originalOffering.groups.flatMap((group) => {
			if (options.priorExcludedM43 && group.id === groupIds[1]) return [];
			const homeroomIds = group.homeroomIds.filter((id) => audience.has(id));
			return homeroomIds.length ? [{ ...group, homeroomIds }] : [];
		});
		graph.snapshot.offerings = audience.size ? [selected] : [];
	}
	project();
	const registryTargets = structuredClone(selected.targets);
	const published = structuredClone(graph);
	published.id = ids.sourceVersion;
	published.sourceVersionId = null;
	published.status = 'published';
	published.snapshot.offerings[0].groups = structuredClone(originalOffering.groups);
	const originalPublished = structuredClone(published);
	const baseRoom = navigationWorkspace().homerooms[0];
	const sourceItem = baseRoom.items.find((item) => item.offeringId === offeringId)!;
	function workspace(versionId = ids.version) {
		const source = versionId === ids.sourceVersion ? published : graph;
		const sourceOffering = source.snapshot.offerings.find((item) => item.id === offeringId);
		const audience = new Set(sourceOffering?.homeroomIds ?? []);
		const result = navigationWorkspace();
		result.deliveryVersionId = versionId;
		result.deliveryVersionStatus = source.status;
		result.unlinked = [];
		result.homerooms = roomIds.map((id, index) => {
			const room = structuredClone(baseRoom);
			room.homeroom = { id, name: roomNames[index] };
			room.expectedCount = 1;
			room.readyCount = index === 2 && audience.has(id) ? 1 : 0;
			room.blockers = [];
			room.extraOfferings =
				index < 2 && audience.has(id)
					? [
							{
								offeringId,
								catalogVersionId: sourceItem.catalogVersionId,
								resourceKind,
								code: selected.code,
								name: selected.name,
								weeklyPeriodTarget: 2,
								startsOn: null,
								endsOn: null,
								alignmentStates: ['extra_offering']
							}
						]
					: [];
			const groups = (sourceOffering?.groups ?? []).filter((group) =>
				group.homeroomIds.includes(id)
			);
			room.items = [
				{
					...sourceItem,
					resourceKind,
					schedulingMode: options.synchronized ? 'synchronized' : null,
					subjectType: options.synchronized ? null : sourceItem.subjectType,
					standardPeriodsPerWeek: options.synchronized ? null : sourceItem.standardPeriodsPerWeek,
					requirementId:
						index === 2 ? sourceItem.requirementId : `f0000000-0000-4000-8000-00000000030${index}`,
					catalogVersionId: index === 2 ? sourceItem.catalogVersionId : offeringIds.basic23,
					code: index === 2 ? selected.code : 'ท31101',
					name: index === 2 ? selected.name : 'รายวิชาอื่นของ ม.4',
					requirementKind: 'required',
					offeringId: index === 2 && audience.has(id) ? offeringId : null,
					offeringState:
						index === 2 && audience.has(id)
							? source.status === 'published'
								? 'published'
								: 'draft'
							: 'missing',
					weeklyPeriodTarget: index === 2 && audience.has(id) ? 2 : null,
					groupMode:
						index === 2 && audience.has(id)
							? options.synchronized
								? 'central'
								: groups.some((group) => group.homeroomIds.length > 1)
									? 'combined'
									: 'normal'
							: 'missing',
					teacherState: index === 2 && audience.has(id) ? 'assigned' : 'missing_primary',
					alignmentStates:
						index === 2 && audience.has(id)
							? ['matches_curriculum']
							: ['curriculum_requirement_not_offered'],
					groups:
						index === 2 && audience.has(id)
							? groups.map((group) => ({
									id: group.id,
									code: group.code,
									name: group.name,
									homeroomIds: group.homeroomIds,
									homeroomNames: group.homeroomIds.map((id) => roomNames[roomIds.indexOf(id)]),
									primaryTeacherCount: 1,
									teachersLocked: true,
									status: 'published',
									rosterStatus: 'draft'
								}))
							: []
				}
			];
			return room;
		});
		return result;
	}
	let changeSet: AcademicTermChangeSet = {
		id: ids.changeSet,
		academicYearId: ids.year,
		academicTermId: ids.term,
		targetDeliveryVersionId: ids.version,
		baseDeliveryVersionId: ids.sourceVersion,
		status: options.published ? 'published' : 'draft',
		rowVersion: 7,
		effectiveFrom: null,
		referenceDate: '2027-05-01',
		reason: 'ปรับห้องโดยรักษาข้อมูลเดิม',
		items: [],
		changes: [],
		offeringLabels: [],
		createdAt: stamp,
		updatedAt: stamp,
		createdBy: ids.actor
	};
	const management: DeliveryManagementOptions = {
		academicYearId: ids.year,
		academicTermId: ids.term,
		homerooms: roomIds.map((id, index) => ({ id, name: roomNames[index] })),
		studyPrograms: [baseRoom.studyProgram],
		gradeLevels: [baseRoom.gradeLevel],
		learningGroups: registry,
		teachers: [{ id: ids.actor, name: teacher.displayName }],
		rooms: [],
		catalogVersions: [
			{
				id: sourceItem.catalogVersionId,
				kind: resourceKind,
				subjectType: options.synchronized ? null : 'BASIC',
				code: selected.code,
				name: selected.name,
				label: `${selected.code} — ${selected.name}`,
				versionNo: 1,
				schedulingMode: options.synchronized ? 'synchronized' : null,
				standardPeriodsPerWeek: 2
			}
		]
	};
	const resource: LearningOffering = {
		id: offeringId,
		academicYearId: ids.year,
		academicTermId: ids.term,
		kind: resourceKind,
		codeSnapshot: selected.code,
		nameSnapshot: selected.name,
		status: 'published',
		rowVersion: 13,
		startsOn: '2027-05-01',
		endsOn: null,
		stopReason: null,
		snapshot: originalOffering.catalog,
		targets: registryTargets,
		owningOrganizationUnitId: selected.owningOrganizationUnitId,
		migrated: false,
		createdAt: stamp,
		updatedAt: stamp
	};
	const writes: { method: string; path: string; body: Record<string, unknown> }[] = [];
	let failRead = false,
		conflict = options.conflictOnce ?? false;
	let failWorkspace = options.failWorkspaceOnce ?? false;
	const workspaceReads: string[] = [];
	const initialWorkspaceFinished = deferred();
	function advance() {
		graph.rowVersion++;
		changeSet = { ...changeSet, rowVersion: changeSet.rowVersion + 1 };
	}
	await page.route(
		(url) => url.pathname.startsWith('/api/academic/'),
		async (route) => {
			const request = route.request(),
				url = new URL(request.url()),
				path = url.pathname;
			if (request.method() !== 'GET')
				writes.push({ method: request.method(), path, body: request.postDataJSON() });
			if (path === '/api/academic/delivery/homerooms') {
				const versionId = url.searchParams.get('deliveryVersionId') ?? ids.version;
				workspaceReads.push(versionId);
				if (failWorkspace) {
					failWorkspace = false;
					return reply(route, 'โหลดชื่อห้องไม่สำเร็จ', 500);
				}
				if (failRead) {
					failRead = false;
					return reply(route, 'โหลดห้องหลังบันทึกไม่สำเร็จ', 503);
				}
				const response = workspace(versionId);
				const initial = workspaceReads.length === 1;
				try {
					if (initial && options.workspaceGate) await options.workspaceGate;
					if (initial && options.workspaceGateFailure)
						return await reply(route, 'คำตอบชื่อห้องเก่าล้มเหลว', 500);
					return await reply(route, response);
				} finally {
					if (initial) initialWorkspaceFinished.release();
				}
			}
			if (path === '/api/academic/delivery/management-options') return reply(route, management);
			if (path === '/api/academic/delivery-versions')
				return reply(route, [
					{
						...navigationSummary(),
						rowVersion: graph.rowVersion,
						changeSetId: options.published ? null : ids.changeSet,
						status: graph.status,
						sourceVersionId: ids.sourceVersion
					},
					{
						...navigationSummary(),
						id: ids.sourceVersion,
						changeSetId: null,
						status: 'published',
						sourceVersionId: null
					}
				]);
			if (path === versionPath) return reply(route, graph);
			if (path === `/api/academic/delivery-versions/${ids.sourceVersion}`)
				return reply(route, published);
			if (path === '/api/academic/term-change-sets')
				return reply(route, [
					{
						...changeSet,
						offeringCount: graph.snapshot.offerings.length,
						groupCount: selected.groups.length,
						teacherAssignmentCount: selected.groups.length
					}
				]);
			if (path === changeSetPath) return reply(route, changeSet);
			if (
				path.startsWith(`${changeSetPath}/offerings/${offeringId}`) &&
				request.method() === 'DELETE'
			) {
				expect(request.postDataJSON()).toEqual({
					changeSetRowVersion: changeSet.rowVersion,
					targetDeliveryVersionRowVersion: graph.rowVersion
				});
				if (conflict) {
					conflict = false;
					advance();
					return reply(route, 'ขอบเขตแบบร่างถูกแก้ไขแล้ว', 409);
				}
				if (options.saveGate) await options.saveGate;
				if (path.includes('/homerooms/')) audience.delete(path.split('/').at(-1)!);
				else audience = new Set();
				project();
				advance();
				failRead = options.failReadAfterSave ?? false;
				return reply(route, changeSet);
			}
			if (path === '/api/academic/offerings/preview-from-curriculum') {
				expect(request.postDataJSON()).toMatchObject({
					academicTermId: ids.term,
					deliveryVersionId: ids.version,
					studyProgramIds: [baseRoom.studyProgram.id]
				});
				return reply(route, {
					academicTermId: ids.term,
					curriculumSources: [],
					sourceHash: 'a'.repeat(64),
					proposals: [
						{
							proposalId: 'b'.repeat(64),
							resourceKind: 'course',
							catalogVersionId: sourceItem.catalogVersionId,
							code: selected.code,
							name: selected.name,
							schedulingMode: null,
							offeringAction: 'retain',
							existingOfferingId: offeringId,
							groupingState: 'proposed',
							conflicts: [],
							targetHomeroomIds: [roomIds[2]],
							requirementIds: [sourceItem.requirementId],
							defaultGroups: [
								{ groupKey: 'c'.repeat(64), name: 'กลุ่ม ม.6/1', homeroomIds: [roomIds[2]] }
							]
						}
					]
				} satisfies CurriculumOfferingPreview);
			}
			if (path === '/api/academic/offerings/apply-from-curriculum') {
				expect(request.postDataJSON()).toMatchObject({
					deliveryVersionId: ids.version,
					sourceHash: 'a'.repeat(64),
					choices: [
						{
							proposalId: 'b'.repeat(64),
							action: 'apply',
							groups: [{ groupKey: 'c'.repeat(64), name: 'กลุ่ม ม.6/1', homeroomIds: [roomIds[2]] }]
						}
					]
				});
				audience = new Set([roomIds[2]]);
				project();
				advance();
				return reply(route, {
					academicTermId: ids.term,
					sourceHash: 'a'.repeat(64),
					offeringIds: [offeringId],
					groupIds: selected.groups.map((group) => group.id),
					createdOfferingCount: 0,
					retainedOfferingCount: 1,
					createdGroupCount: 0,
					retainedGroupCount: selected.groups.length,
					skippedCount: 0
				});
			}
			if (path === offeringPath) return reply(route, resource);
			if (path === `${offeringPath}/groups`)
				return options.failGroups
					? reply(route, 'โหลดรายการกลุ่มไม่สำเร็จ', 500)
					: reply(route, registry);
			const group = registry.find((group) => path === `/api/academic/learning-groups/${group.id}`);
			if (group) return reply(route, group);
			if (path.endsWith('/roster-tracking'))
				return reply(route, {
					mode: 'manual',
					groupRowVersion: 11,
					effectiveFrom: null,
					startsOn: '2027-05-01',
					endsOn: '2027-10-01'
				});
			if (path.endsWith('/memberships')) return reply(route, []);
			return route.fallback();
		}
	);
	return {
		graph,
		registry,
		originalRegistry,
		published,
		originalPublished,
		writes,
		selected,
		workspaceReads,
		initialWorkspaceFinished: initialWorkspaceFinished.promise,
		removeRoomElsewhere(homeroomId: string) {
			audience.delete(homeroomId);
			project();
			advance();
		}
	};
}

function room(page: Page, name: string) {
	return page.locator('details').filter({ has: page.getByRole('heading', { name, exact: true }) });
}
async function removeRoom(page: Page, name: string) {
	await room(page, name)
		.getByRole('button', { name: `นำออกจากห้องนี้ · ${name} · เคมีเพิ่มเติม`, exact: true })
		.click();
	const dialog = page.getByRole('alertdialog', { name: 'นำออกจากห้องนี้', exact: true });
	await expect(dialog).toContainText(`ห้อง “${name}”`);
	await expect(dialog).toContainText('ห้องอื่นยังอยู่');
	return dialog;
}

for (const combined of [false, true])
	for (const viewport of [
		{ width: 1280, dark: false },
		{ width: 390, dark: true }
	]) {
		test(`room removal retains M6 identity and raw history for ${combined ? 'combined' : 'separate'} groups at ${viewport.width}px`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width: viewport.width, height: 900 });
			const fixture = await mockRoomScope(page, combined);
			await page.goto(overviewHref);
			if (viewport.dark) {
				await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
				await expect(page.locator('html')).toHaveClass(/\bdark\b/);
			}
			for (const name of roomNames.slice(0, 2)) {
				const scope = await removeRoom(page, name);
				await expect(scope).not.toContainText('พร้อมทุกกลุ่มเรียน');
				await scope.screenshot({
					path: testInfo.outputPath(`room-removal-${roomNames.indexOf(name)}.png`),
					animations: 'disabled'
				});
				await scope.getByRole('button', { name: 'ยืนยันนำออกจากห้องนี้', exact: true }).click();
				await expect(scope).toHaveCount(0);
				await expect(
					room(page, name).getByRole('button', { name: /นำออกจากห้องนี้.*เคมีเพิ่มเติม/ })
				).toHaveCount(0);
			}
			expect(fixture.writes.map((write) => write.path)).toEqual(
				roomIds.slice(0, 2).map((id) => `${changeSetPath}/offerings/${offeringId}/homerooms/${id}`)
			);
			expect(fixture.writes.map((write) => write.body)).toEqual([
				{ changeSetRowVersion: 7, targetDeliveryVersionRowVersion: 4 },
				{ changeSetRowVersion: 8, targetDeliveryVersionRowVersion: 5 }
			]);
			expect(fixture.selected.id).toBe(offeringId);
			const retainedId = combined ? groupIds[0] : groupIds[2];
			expect(fixture.selected.groups.map((group) => group.id)).toEqual([retainedId]);
			expect(fixture.selected.groups[0].homeroomIds).toEqual([roomIds[2]]);
			expect(fixture.registry).toEqual(fixture.originalRegistry);
			expect(fixture.published).toEqual(fixture.originalPublished);
			await page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม', exact: true }).click();
			const course = page
				.getByRole('region', { name: 'มุมมองรายวิชาและกิจกรรม', exact: true })
				.getByRole('row')
				.filter({ hasText: 'เคมีเพิ่มเติม' });
			await expect(course.getByRole('button', { name: /ลบจากรุ่นนี้/ })).toBeVisible();
			await expect(course.getByRole('button', { name: /นำออกจากห้องนี้/ })).toHaveCount(0);
			await course.getByRole('link', { name: 'รายละเอียด', exact: true }).click();
			await expect(page).toHaveURL(
				(url) =>
					url.pathname.endsWith(offeringId) &&
					url.searchParams.get('deliveryVersionId') === ids.version
			);
			const groups = page.getByTestId('delivery-groups-ready');
			await expect(
				groups.getByRole('button', {
					name: combined ? /^กลุ่มเรียนรวมที่คงไว้ ครู/ : /^กลุ่ม ม.6\/1 ครู/
				})
			).toContainText('ห้องในรุ่นที่เลือก 1 ห้อง');
			await expect(page.getByTestId('delivery-selected-group-ready')).toContainText(
				'ห้องในรุ่นที่เลือก 1 ห้อง'
			);
			await expect(
				page.getByRole('button', { name: 'เปลี่ยนครูผู้สอน', exact: true })
			).toBeVisible();
			await page.getByRole('button', { name: 'จัดการกลุ่ม', exact: true }).click();
			await expect(
				page.getByRole('button', { name: 'บันทึกข้อมูลกลุ่ม', exact: true })
			).toBeEnabled();
			await expect(page.getByLabel('ชื่อกลุ่ม', { exact: true })).toHaveValue(
				combined ? 'กลุ่มเรียนรวมที่คงไว้' : 'กลุ่ม ม.6/1'
			);
			await expect(page.getByLabel('ความจุ (ถ้ามี)', { exact: true })).toHaveValue('37');
			await expect(page.getByLabel('คำอธิบาย (ถ้ามี)', { exact: true })).toHaveValue(
				'เก็บการตั้งค่าเดิม'
			);
			if (combined) {
				await expect(page.getByText('ห้องต้นทางของกลุ่มเดิม', { exact: true })).toBeVisible();
				await expect(page.getByText(/รุ่นที่เลือกครอบคลุม ม.6\/1/)).toBeVisible();
				await expect(
					page.getByRole('button', { name: 'บันทึกห้องต้นทาง', exact: true })
				).toHaveCount(0);
			} else
				await expect(
					page.getByRole('button', { name: 'บันทึกห้องต้นทาง', exact: true })
				).toBeVisible();
			await groups.getByRole('button', { name: /^ลบกลุ่มจากรุ่นนี้/ }).click();
			const groupScope = page.getByRole('alertdialog', { name: 'ลบกลุ่มจากรุ่นนี้', exact: true });
			await expect(groupScope.getByRole('list')).toContainText('ม.6/1');
			await expect(groupScope.getByRole('list')).not.toContainText('ม.4/2');
			await expect(groupScope.getByRole('list')).not.toContainText('ม.4/3');
			await expect(groupScope).toContainText('หากไม่เหลือห้อง รายการเปิดสอนนี้จะออกจากรุ่นด้วย');
			await expect(groupScope).toContainText('รวมถึงกลุ่มที่ไม่ได้ผูกห้อง');
			await expect(groupScope).not.toContainText('กลุ่มอื่นยังอยู่');
			await groupScope.screenshot({
				path: testInfo.outputPath('selected-group-room-confirmation.png'),
				animations: 'disabled'
			});
			await page.keyboard.press('Escape');
			await expect(groupScope).toHaveCount(0);
			expect(fixture.registry).toEqual(fixture.originalRegistry);
			expect(fixture.writes).toHaveLength(2);
		});
	}

test('an already removed room in a stale workspace cannot submit on open or explicit scope reload', async ({
	page
}) => {
	const fixture = await mockRoomScope(page, true);
	await page.goto(overviewHref);
	const trigger = room(page, roomNames[0]).getByRole('button', {
		name: `นำออกจากห้องนี้ · ${roomNames[0]} · เคมีเพิ่มเติม`,
		exact: true
	});
	await expect(trigger).toBeVisible();
	fixture.removeRoomElsewhere(roomIds[0]);
	await trigger.click();
	const scope = page.getByRole('alertdialog', { name: 'นำออกจากห้องนี้', exact: true });
	await expect(scope.getByRole('alert')).toHaveText(
		'รายการนี้ไม่อยู่ในห้องที่เลือกแล้ว กรุณาตรวจข้อมูลรายห้องล่าสุด'
	);
	await expect(
		scope.getByRole('button', { name: 'ยืนยันนำออกจากห้องนี้', exact: true })
	).toHaveCount(0);
	const freshRead = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname === versionPath && response.request().method() === 'GET'
	);
	await scope.getByRole('button', { name: 'ตรวจขอบเขตล่าสุดอีกครั้ง', exact: true }).click();
	await (await freshRead).finished();
	await expect(scope.getByRole('alert')).toHaveText(
		'รายการนี้ไม่อยู่ในห้องที่เลือกแล้ว กรุณาตรวจข้อมูลรายห้องล่าสุด'
	);
	await expect(
		scope.getByRole('button', { name: 'ยืนยันนำออกจากห้องนี้', exact: true })
	).toHaveCount(0);
	expect(fixture.writes).toEqual([]);
	expect(fixture.registry).toEqual(fixture.originalRegistry);
	expect(fixture.published).toEqual(fixture.originalPublished);
});

test('central activity group confirmation preserves participation rather than promising unchanged group coverage', async ({
	page
}) => {
	const fixture = await mockRoomScope(page, true, { synchronized: true });
	await page.goto(
		`/staff/academic/delivery/${offeringId}?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}&groupId=${groupIds[0]}`
	);
	await page
		.getByRole('button', { name: 'ลบกลุ่มจากรุ่นนี้ · กลุ่มเรียนรวมที่คงไว้', exact: true })
		.click();
	const scope = page.getByRole('alertdialog', { name: 'ลบกลุ่มจากรุ่นนี้', exact: true });
	await expect(scope).toContainText(
		'กิจกรรมคาบพร้อมกัน: นำเฉพาะกลุ่มนี้ออก โดยห้องที่เข้าร่วมกิจกรรมในรุ่นนี้ยังคงเดิม'
	);
	await expect(scope).not.toContainText('หากไม่เหลือห้อง');
	await expect(scope).not.toContainText('กลุ่มอื่นยังอยู่');
	for (const name of roomNames) await expect(scope.getByRole('list')).toContainText(name);
	await expect(scope).toContainText('คะแนน และประวัติเดิมยังคงอยู่');
	await page.keyboard.press('Escape');
	await expect(scope).toHaveCount(0);
	expect(fixture.writes).toEqual([]);
	expect(fixture.selected.homeroomIds).toEqual(roomIds);
	expect(fixture.registry).toEqual(fixture.originalRegistry);
	expect(fixture.published).toEqual(fixture.originalPublished);
});

test('a normal curriculum row removes only its own room and retains other room groups', async ({
	page
}) => {
	const fixture = await mockRoomScope(page, false);
	await page.goto(overviewHref);
	const scope = await removeRoom(page, roomNames[2]);
	await scope.getByRole('button', { name: 'ยืนยันนำออกจากห้องนี้', exact: true }).click();
	await expect(scope).toHaveCount(0);
	await expect(
		room(page, roomNames[2]).getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true })
	).toBeVisible();
	expect(fixture.writes.map((write) => write.path)).toEqual([
		`${changeSetPath}/offerings/${offeringId}/homerooms/${roomIds[2]}`
	]);
	expect(fixture.selected.groups.map((group) => group.id)).toEqual(groupIds.slice(0, 2));
	expect(fixture.selected.homeroomIds).toEqual(roomIds.slice(0, 2));
	expect(fixture.registry).toEqual(fixture.originalRegistry);
	expect(fixture.published).toEqual(fixture.originalPublished);
});

test('room-scope conflict reloads revisions and an accepted removal retries only its read', async ({
	page
}) => {
	const gate = deferred();
	const fixture = await mockRoomScope(page, true, {
		conflictOnce: true,
		failReadAfterSave: true,
		saveGate: gate.promise
	});
	await page.goto(overviewHref);
	const scope = await removeRoom(page, roomNames[0]);
	const confirm = scope.getByRole('button', { name: 'ยืนยันนำออกจากห้องนี้', exact: true });
	await confirm.click();
	await expect(scope.getByRole('alert')).toContainText('ขอบเขตแบบร่างถูกแก้ไขแล้ว');
	expect(fixture.writes).toHaveLength(1);
	await scope.getByRole('button', { name: 'ตรวจขอบเขตล่าสุดอีกครั้ง', exact: true }).click();
	await expect(confirm).toBeEnabled();
	try {
		await confirm.click();
		await expect(scope.getByRole('button', { name: 'กำลังนำออก', exact: true })).toBeDisabled();
		await page.keyboard.press('Enter');
		await page.keyboard.press('Escape');
		await expect(scope).toBeVisible();
		expect(fixture.writes).toHaveLength(2);
	} finally {
		gate.release();
	}
	await expect(scope.getByRole('status')).toContainText('นำออกจากแบบร่างแล้ว');
	await expect(scope.getByRole('alert')).toContainText('โหลดห้องหลังบันทึกไม่สำเร็จ');
	await scope.getByRole('button', { name: 'แสดงข้อมูลล่าสุด', exact: true }).click();
	await expect(scope).toHaveCount(0);
	expect(fixture.writes).toHaveLength(2);
	expect(fixture.writes[1].body).toEqual({
		changeSetRowVersion: 8,
		targetDeliveryVersionRowVersion: 5
	});
	expect(fixture.registry).toEqual(fixture.originalRegistry);
});

test('whole-offering removal is separate and re-add previews only the selected draft M6 scope', async ({
	page
}) => {
	const fixture = await mockRoomScope(page, true);
	await page.goto(`${overviewHref}&view=offerings`);
	const course = page
		.getByRole('region', { name: 'มุมมองรายวิชาและกิจกรรม', exact: true })
		.getByRole('row')
		.filter({ hasText: 'เคมีเพิ่มเติม' });
	await course.getByRole('button', { name: /^ลบจากรุ่นนี้/ }).click();
	const whole = page.getByRole('alertdialog', { name: 'ลบจากรุ่นนี้', exact: true });
	await expect(whole).toContainText('พร้อมทุกกลุ่มเรียน 1 กลุ่ม');
	await whole.getByRole('button', { name: 'ยืนยันลบจากรุ่นนี้', exact: true }).click();
	await expect(whole).toHaveCount(0);
	await page.getByRole('tab', { name: 'มุมมองรายห้อง', exact: true }).click();
	await room(page, roomNames[2])
		.getByRole('button', { name: 'เพิ่มการจัดการเรียนการสอน', exact: true })
		.click();
	const preparation = page.getByRole('dialog', { name: 'เปิดรายวิชา เคมีเพิ่มเติม', exact: true });
	await expect(preparation).toContainText('รวม 1 ห้อง');
	await preparation.getByRole('button', { name: 'ตรวจและจัดกลุ่มก่อน', exact: true }).click();
	await expect(preparation).toContainText('ใช้รายการเดิม');
	await expect(preparation).not.toContainText('ครอบคลุมห้องนอกเป้าหมายหลักสูตร');
	await preparation.getByRole('button', { name: 'ยืนยันเปิดรายวิชา', exact: true }).click();
	await expect(preparation).toHaveCount(0);
	expect(fixture.selected.id).toBe(offeringId);
	expect(fixture.selected.homeroomIds).toEqual([roomIds[2]]);
	expect(fixture.selected.groups[0].id).toBe(groupIds[0]);
	expect(fixture.selected.groups[0].homeroomIds).toEqual([roomIds[2]]);
	expect(fixture.registry).toEqual(fixture.originalRegistry);
	expect(fixture.published).toEqual(fixture.originalPublished);
	expect(fixture.writes.map((write) => write.path)).toEqual([
		`${changeSetPath}/offerings/${offeringId}`,
		'/api/academic/offerings/preview-from-curriculum',
		'/api/academic/offerings/apply-from-curriculum'
	]);
	for (const name of roomNames.slice(0, 2))
		await expect(
			room(page, name).getByRole('button', { name: /นำออกจากห้องนี้.*เคมีเพิ่มเติม/ })
		).toHaveCount(0);
});

function detailHref() {
	return `/staff/academic/delivery/${offeringId}?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}&groupId=${groupIds[2]}&returnTo=${encodeURIComponent(overviewHref)}`;
}
function audienceRoom(page: Page, name: string) {
	return page
		.getByRole('region', { name: 'ห้องที่เปิดสอนในรุ่นนี้', exact: true })
		.getByRole('listitem')
		.filter({ has: page.getByRole('heading', { name, exact: true }) });
}
async function removeAudienceRoom(page: Page, name: string) {
	const trigger = audienceRoom(page, name).getByRole('button', {
		name: `นำออกจากห้องนี้ · ${name} · เคมีเพิ่มเติม`,
		exact: true
	});
	await trigger.focus();
	await page.keyboard.press('Enter');
	const scope = page.getByRole('alertdialog', { name: 'นำออกจากห้องนี้', exact: true });
	await expect(scope).toContainText(`ห้อง “${name}”`);
	await scope.getByRole('button', { name: 'ยืนยันนำออกจากห้องนี้', exact: true }).click();
	await expect(scope).toHaveCount(0);
	await expect(audienceRoom(page, name)).toHaveCount(0);
}

for (const priorExcludedM43 of [false, true])
	for (const viewport of [
		{ width: 1280, dark: false },
		{ width: 390, dark: true }
	]) {
		test(`detail retains every audience room ${priorExcludedM43 ? 'without restoring a previously excluded M4/3 group' : 'and its remaining groups'} at ${viewport.width}px`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width: viewport.width, height: 900 });
			const fixture = await mockRoomScope(page, false, { priorExcludedM43 });
			await page.goto(detailHref());
			if (viewport.dark)
				await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
			const audience = page.getByRole('region', { name: 'ห้องที่เปิดสอนในรุ่นนี้', exact: true });
			await expect(audience.getByRole('listitem')).toHaveCount(3);
			await expect(audienceRoom(page, roomNames[1])).toContainText(
				priorExcludedM43 ? 'ยังไม่มีกลุ่มเรียนสำหรับห้องนี้' : 'มีกลุ่มเรียนสำหรับห้องนี้ 1 กลุ่ม'
			);
			await removeAudienceRoom(page, roomNames[0]);
			await expect(audience.getByRole('listitem')).toHaveCount(2);
			await expect(audienceRoom(page, roomNames[1])).toContainText(
				priorExcludedM43 ? 'ยังไม่มีกลุ่มเรียนสำหรับห้องนี้' : 'มีกลุ่มเรียนสำหรับห้องนี้ 1 กลุ่ม'
			);
			await expect(audienceRoom(page, roomNames[2])).toContainText(
				'มีกลุ่มเรียนสำหรับห้องนี้ 1 กลุ่ม'
			);
			await expect(audience).not.toContainText(roomIds[1]);
			expect(fixture.selected.homeroomIds).toEqual(roomIds.slice(1));
			expect(fixture.selected.groups.map((group) => group.id)).toEqual(
				priorExcludedM43 ? [groupIds[2]] : groupIds.slice(1)
			);
			await expect(
				page.getByTestId('delivery-groups-ready').getByRole('button', { name: /^กลุ่ม ม.6\/1 ครู/ })
			).toBeVisible();
			if (!priorExcludedM43)
				await expect(
					page
						.getByTestId('delivery-groups-ready')
						.getByRole('button', { name: /^กลุ่ม ม.4\/3 ครู/ })
				).toBeVisible();
			await audience.scrollIntoViewIfNeeded();
			await audience.screenshot({
				path: testInfo.outputPath('remaining-delivery-audience.png'),
				animations: 'disabled'
			});
			if (priorExcludedM43) {
				await removeAudienceRoom(page, roomNames[1]);
				await expect(audience.getByRole('listitem')).toHaveCount(1);
				expect(fixture.selected.homeroomIds).toEqual([roomIds[2]]);
				expect(fixture.selected.groups.map((group) => group.id)).toEqual([groupIds[2]]);
			}
			expect(fixture.writes.map((write) => write.path)).toEqual(
				(priorExcludedM43 ? roomIds.slice(0, 2) : roomIds.slice(0, 1)).map(
					(id) => `${changeSetPath}/offerings/${offeringId}/homerooms/${id}`
				)
			);
			expect(fixture.registry).toEqual(fixture.originalRegistry);
			expect(fixture.published).toEqual(fixture.originalPublished);
			expect(fixture.workspaceReads).toEqual([ids.version]);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
		});
	}

test('audience metadata failure is independent of profile and snapshot coverage survives a failed registry list', async ({
	page
}) => {
	const fixture = await mockRoomScope(page, false, { failWorkspaceOnce: true, failGroups: true });
	await page.goto(detailHref());
	await expect(page.getByTestId('delivery-offering-ready')).toBeVisible();
	const audience = page.getByRole('region', { name: 'ห้องที่เปิดสอนในรุ่นนี้', exact: true });
	await expect(audience.getByRole('alert')).toContainText('โหลดชื่อห้องไม่สำเร็จ');
	await expect(audience.getByRole('button', { name: /^นำออกจากห้องนี้/ })).toHaveCount(0);
	await audience.getByRole('button', { name: 'โหลดชื่อห้องอีกครั้ง', exact: true }).click();
	for (const name of roomNames)
		await expect(audienceRoom(page, name)).toContainText('มีกลุ่มเรียนสำหรับห้องนี้ 1 กลุ่ม');
	await expect(audience).not.toContainText('ยังไม่มีกลุ่มเรียนสำหรับห้องนี้');
	expect(fixture.workspaceReads).toEqual([ids.version, ids.version]);
	expect(fixture.writes).toEqual([]);
});

test('a delayed old audience metadata failure cannot replace a newly selected published version', async ({
	page
}) => {
	const gate = deferred();
	const fixture = await mockRoomScope(page, false, {
		workspaceGate: gate.promise,
		workspaceGateFailure: true
	});
	try {
		await page.goto(detailHref());
		await expect(page.getByTestId('delivery-offering-ready')).toBeVisible();
		const audience = page.getByRole('region', { name: 'ห้องที่เปิดสอนในรุ่นนี้', exact: true });
		await expect(audience).toHaveAttribute('aria-busy', 'true');
		await page.getByRole('combobox', { name: 'เลือกรุ่นเปิดสอน', exact: true }).click();
		await page.getByRole('option', { name: /^เผยแพร่แล้ว/ }).click();
		await expect(page).toHaveURL(
			(url) => url.searchParams.get('deliveryVersionId') === ids.sourceVersion
		);
		await expect(audience).toHaveAttribute('aria-busy', 'false');
		await expect(audience.getByRole('listitem')).toHaveCount(3);
		gate.release();
		await fixture.initialWorkspaceFinished;
		await page.evaluate(
			() =>
				new Promise<void>((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
				)
		);
		await expect(audience.getByRole('alert')).toHaveCount(0);
		await expect(audience.getByRole('listitem')).toHaveCount(3);
		await expect(audience.getByRole('button', { name: /^นำออกจากห้องนี้/ })).toHaveCount(0);
		expect(fixture.workspaceReads).toEqual([ids.version, ids.sourceVersion]);
		expect(fixture.writes).toEqual([]);
	} finally {
		gate.release();
	}
});

for (const condition of ['reader', 'published'] as const)
	test(`room actions stay hidden for ${condition}`, async ({ page }) => {
		const fixture = await mockRoomScope(page, true, {
			reader: condition === 'reader',
			published: condition === 'published'
		});
		await page.goto(overviewHref);
		await expect(page.getByTestId('delivery-homerooms-ready')).toBeVisible();
		await expect(page.getByRole('button', { name: /^นำออกจากห้องนี้/ })).toHaveCount(0);
		await page.goto(detailHref());
		const audience = page.getByRole('region', { name: 'ห้องที่เปิดสอนในรุ่นนี้', exact: true });
		await expect(audience.getByRole('listitem')).toHaveCount(3);
		await expect(audience.getByRole('button', { name: /^นำออกจากห้องนี้/ })).toHaveCount(0);
		expect(fixture.writes).toEqual([]);
	});
