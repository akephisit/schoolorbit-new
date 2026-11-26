import type { Page, Route } from '@playwright/test';
import { mockLeave, leaveId, exitTypeId } from './staff-leave';
import { PERMISSIONS } from '../../../src/lib/permissions/registry.generated';
import { actor, firstStaff, secondStaff, id } from './staff-home-route-data';
import type {
	LeaveCoveragePlan,
	LeaveCoverageLesson,
	LeaveCoverageOptions,
	LeaveCoverageSelection,
	LeaveCoverageCandidate,
	DatedTeachingLesson,
	LeaveCoverageTask,
	UpdateLeaveRequest,
	LeaveKind
} from '../../../src/lib/api/staff-leave';

export const coverageYear = id(1),
	coverageTerm = id(800),
	homeroomId = id(810),
	gradeId = id(850),
	englishOfferingId = id(820),
	mathOfferingId = id(821);
export function teachingLesson(index: number, date = '2026-10-19'): DatedTeachingLesson {
	return {
		lessonKey: `lesson-${index}`,
		date,
		blockId: id(830 + index),
		allocationId: id(840 + index),
		teacherId: actor,
		bellSchedulePeriodId: id(860 + index),
		periodName: `คาบ ${index}`,
		startTime: index === 1 ? '09:00:00' : '10:00:00',
		endTime: index === 1 ? '09:50:00' : '10:50:00',
		learningGroupId: id(870),
		learningGroupName: 'คณิตศาสตร์ ม.2/1',
		offeringId: mathOfferingId,
		offeringName: 'คณิตศาสตร์',
		homeroomIds: [homeroomId],
		homeroomNames: ['ม.2/1'],
		roomId: id(880),
		roomName: 'อาคารเรียน 1 ห้อง 201',
		sourceRevision: 1,
		allocationRevision: 1,
		gradeLevelIds: [gradeId]
	};
}
export function coveragePlan(): LeaveCoveragePlan {
	const lessons: LeaveCoverageLesson[] = [1, 2, 3].map((index) => ({
		...teachingLesson(index, index === 3 ? '2026-10-20' : '2026-10-19'),
		assignment: null
	}));
	return {
		revision: 1,
		planHash: 'plan-1',
		state: 'incomplete',
		sourceDigest: 'timetable-digest',
		academicYearId: coverageYear,
		academicTermId: coverageTerm,
		timetableState: 'ready',
		days: [
			{ date: '2026-10-19', lessons: lessons.slice(0, 2) },
			{ date: '2026-10-20', lessons: lessons.slice(2) }
		],
		canManage: false,
		emergencyReason: null,
		blockingReason: null
	};
}
export async function mockTeachingCoverage(
	page: Page,
	options: {
		plan?: LeaveCoveragePlan;
		status?: 'draft' | 'returned' | 'submitted';
		assigned?: boolean;
		failSave?: boolean;
		manager?: boolean;
		updatedCoverage?: LeaveCoveragePlan;
		kind?: LeaveKind;
		refreshedCoverage?: LeaveCoveragePlan;
		requireCoverageRefresh?: boolean;
		failRefresh?: boolean;
		holdRefresh?: boolean;
	} = {}
) {
	const plan = options.plan ?? coveragePlan();
	const api = await mockLeave(page, {
		coverage: plan,
		status: options.status ?? 'draft',
		role: options.manager ? 'reviewer' : 'owner',
		...(options.assigned
			? {
					permissions: [
						PERMISSIONS.STAFF_LEAVE_READ_ASSIGNED,
						PERMISSIONS.STAFF_LEAVE_UPDATE_ASSIGNED
					]
				}
			: {})
	});
	const detail = api.detail();
	detail.request.endDate = '2026-10-20';
	detail.request.amount = 2;
	if (options.kind === 'exit') {
		detail.request = {
			...detail.request,
			kind: 'exit',
			typeId: exitTypeId,
			typeName: 'ออกนอกโรงเรียน',
			unit: 'minute',
			startTime: '09:00:00',
			endTime: '11:00:00',
			endDate: detail.request.startDate,
			amount: 120
		};
		if (detail.policy)
			detail.policy = {
				...detail.policy,
				typeId: exitTypeId,
				unit: 'minute',
				countingMode: 'calendar_days',
				maxPerRequest: 120,
				annualLimit: null,
				evidenceFromAmount: null
			};
	}
	if (options.assigned) {
		detail.request.ownerId = firstStaff;
		for (const day of detail.coverage.days)
			for (const lesson of day.lessons)
				if (!lesson.originalTeacherName) lesson.teacherId = firstStaff;
	}
	if (options.manager) {
		detail.request.ownerId = id(11);
		for (const day of detail.coverage.days)
			for (const lesson of day.lessons) lesson.teacherId = id(11);
		detail.instance = null;
		detail.coverage.canManage = true;
		detail.coverage.emergencyReason = 'ปฏิบัติหน้าที่เร่งด่วน';
		detail.coverage.state =
			detail.coverage.timetableState === 'ready' &&
			detail.coverage.days.every((day) => day.lessons.every((lesson) => lesson.assignment))
				? detail.coverage.days.every((day) =>
						day.lessons.every((lesson) => lesson.assignment?.consent === 'confirmed')
					)
					? 'confirmed'
					: 'awaiting_confirmation'
				: 'emergency';
		detail.canAct = false;
		detail.availableActions = [];
	}
	await page.route(
		(url) => url.pathname === '/api/notifications',
		(route) =>
			route.fulfill({
				contentType: 'application/json',
				body: JSON.stringify({ success: true, data: { items: [], total: 0, hasMore: false } })
			})
	);
	const calls: Array<{ path: string; body: Record<string, unknown> }> = [];
	let failed = false,
		failedRefresh = false,
		refreshed = false;
	let releaseRefresh = () => {};
	const refreshGate = new Promise<void>((resolve) => (releaseRefresh = resolve));
	const refreshReceipts = new Set<string>();
	let workflowStarts = 0;
	const reads: string[] = [];
	const coverageOptions: LeaveCoverageOptions = {
		contexts: [
			{
				academicYearId: coverageYear,
				academicYearName: '2569',
				academicTermId: coverageTerm,
				academicTermName: '1'
			}
		],
		defaultAcademicYearId: coverageYear,
		defaultAcademicTermId: coverageTerm
	};
	const teachers: LeaveCoverageCandidate[] = [
		{
			teacherId: firstStaff,
			displayName: 'ครูภาษาอังกฤษ ข้ามกลุ่มสาระ',
			subjectGroups: [{ id: id(901), name: 'ภาษาต่างประเทศ' }],
			gradeLevels: [{ id: gradeId, name: 'มัธยมศึกษาปีที่ 2' }],
			isSameSubjectGroup: false,
			taughtOfferings: [{ id: englishOfferingId, name: 'ภาษาอังกฤษ ม.2' }]
		},
		{
			teacherId: secondStaff,
			displayName: 'ครูคณิตศาสตร์ กลุ่มเดียวกัน',
			subjectGroups: [{ id: id(902), name: 'คณิตศาสตร์' }],
			gradeLevels: [{ id: gradeId, name: 'มัธยมศึกษาปีที่ 2' }],
			isSameSubjectGroup: true,
			taughtOfferings: [{ id: mathOfferingId, name: 'คณิตศาสตร์ ม.2' }]
		}
	];
	const returnLesson = {
		...teachingLesson(9, '2026-10-22'),
		lessonKey: 'return-english',
		teacherId: firstStaff,
		learningGroupId: id(871),
		learningGroupName: 'ภาษาอังกฤษ ม.2/1',
		offeringId: englishOfferingId,
		offeringName: 'ภาษาอังกฤษ'
	};
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	function task(): LeaveCoverageTask {
		return {
			requestId: leaveId,
			requestRevision: detail.request.revision,
			planRevision: detail.coverage.revision,
			planHash: detail.coverage.planHash,
			ownerName: detail.request.ownerName,
			assignments: detail.coverage.days
				.flatMap((day) => day.lessons)
				.filter(
					(lesson) =>
						lesson.assignment?.replacementTeacherId === (options.assigned ? actor : firstStaff)
				)
		};
	}
	await page.route(
		(url) =>
			url.pathname.startsWith('/api/staff-leave/coverage') ||
			url.pathname.startsWith(`/api/staff-leave/requests/${leaveId}`),
		async (route) => {
			const request = route.request(),
				path = new URL(request.url()).pathname,
				method = request.method();
			const body = method === 'GET' ? {} : request.postDataJSON();
			if (method !== 'GET') calls.push({ path, body });
			else reads.push(path);
			if (path.endsWith('/coverage/options')) return reply(route, coverageOptions);
			if (path.endsWith('/coverage/assigned'))
				return reply(route, { items: options.assigned ? [task()] : [], hasMore: false });

			if (path.endsWith('/coverage/refresh')) {
				if (options.holdRefresh) await refreshGate;
				if (options.failRefresh && !failedRefresh) {
					failedRefresh = true;
					return reply(route, 'ปรับแผนยังไม่พร้อม', 503);
				}
				if (refreshReceipts.has(body.requestId)) return reply(route, detail);
				if (body.expectedRevision !== detail.request.revision)
					return reply(route, 'คำขอมีการเปลี่ยนแปลง', 409);
				const previousRevision = detail.coverage.revision;
				const emergencyReason = detail.coverage.emergencyReason;
				detail.coverage = structuredClone(options.refreshedCoverage ?? detail.coverage);
				detail.coverage.revision = previousRevision + 1;
				detail.coverage.planHash = `refreshed-plan-${detail.coverage.revision}`;
				detail.coverage.canManage =
					!!options.manager && detail.request.status === 'submitted' && detail.instance === null;
				if (options.manager) detail.coverage.emergencyReason = emergencyReason;
				detail.request.revision++;
				refreshed = true;
				refreshReceipts.add(body.requestId);
				return reply(route, detail);
			}
			const planOperation =
				path.endsWith('/coverage/candidates') ||
				path.endsWith('/coverage/return-lessons') ||
				(path.endsWith('/coverage') && method === 'PUT') ||
				path.endsWith('/submit');
			if (options.requireCoverageRefresh && planOperation) {
				if (!refreshed) return reply(route, 'ตารางสอนเปลี่ยน กรุณาปรับคาบตามตารางล่าสุด', 409);
				if (
					body.expectedRevision !== detail.request.revision ||
					('sourceDigest' in body && body.sourceDigest !== detail.coverage.sourceDigest)
				)
					return reply(route, 'ยังใช้แผนตารางฉบับเก่า', 409);
			}
			if (path.endsWith('/coverage/candidates')) {
				let candidates = teachers.filter(
					(teacher) =>
						(!body.sameSubjectGroupOnly || teacher.isSameSubjectGroup) &&
						(!body.subjectGroupId ||
							teacher.subjectGroups.some((group) => group.id === body.subjectGroupId)) &&
						(!body.gradeLevelId ||
							teacher.gradeLevels.some((grade) => grade.id === body.gradeLevelId)) &&
						(!body.search || teacher.displayName.includes(body.search))
				);
				// This teacher is busy on the second date and must never appear for a multi-day batch.
				if (!(body.lessonKeys as string[]).includes('lesson-3'))
					candidates = [
						...candidates,
						{ ...teachers[1], teacherId: id(999), displayName: 'ครูว่างเฉพาะวันแรก' }
					];
				return reply(route, {
					candidates,
					subjectGroups: [
						{ id: id(901), name: 'ภาษาต่างประเทศ' },
						{ id: id(902), name: 'คณิตศาสตร์' }
					],
					gradeLevels: [{ id: gradeId, name: 'มัธยมศึกษาปีที่ 2' }],
					hasMore: false
				});
			}
			if (path.endsWith('/coverage/return-lessons'))
				return reply(route, { lessons: body.date === '2026-10-22' ? [returnLesson] : [] });
			if (path.endsWith('/coverage/confirm')) {
				if (body.expectedRevision !== detail.coverage.revision)
					return reply(route, 'แผนเปลี่ยนแปลง', 409);
				for (const day of detail.coverage.days)
					for (const lesson of day.lessons)
						if (lesson.assignment && body.assignmentIds.includes(lesson.assignment.id)) {
							lesson.assignment.consent = body.decision;
							lesson.assignment.confirmedAt = '2026-10-18T03:00:00Z';
						}
				return reply(route, task());
			}
			if (
				path === `/api/staff-leave/requests/${leaveId}` &&
				method === 'PUT' &&
				options.updatedCoverage
			) {
				const input = request.postDataJSON() as UpdateLeaveRequest;
				detail.request = {
					...detail.request,
					...input.period,
					revision: detail.request.revision + 1
				};
				if (detail.request.kind === 'exit') detail.request.amount = 60;
				detail.evidenceFileIds = input.period.evidenceFileIds ?? [];
				detail.coverage = structuredClone(options.updatedCoverage);
				return reply(route, detail);
			}
			if (path.endsWith('/coverage') && method === 'PUT') {
				if (options.failSave && !failed) {
					failed = true;
					return reply(route, 'บันทึกแผนยังไม่พร้อม', 503);
				}
				const assignments = body.assignments as LeaveCoverageSelection[];
				for (const day of detail.coverage.days)
					for (const lesson of day.lessons) {
						const selected = assignments.find((value) => value.lessonKey === lesson.lessonKey);
						const teacher = teachers.find(
							(value) => value.teacherId === selected?.replacementTeacherId
						);
						lesson.assignment =
							selected && teacher
								? {
										id: id(950 + Number(lesson.lessonKey.split('-')[1])),
										replacementTeacherId: teacher.teacherId,
										replacementTeacherName: teacher.displayName,
										mode: selected.mode,
										taughtOfferingId: selected.taughtOfferingId,
										taughtOfferingName:
											teacher.taughtOfferings.find(
												(value) => value.id === selected.taughtOfferingId
											)?.name ?? null,
										taughtActivity: selected.taughtActivity,
										returnLessonKey: selected.returnLessonKey,
										returnDate: selected.returnLessonKey ? '2026-10-22' : null,
										returnLesson: selected.returnLessonKey ? returnLesson : null,
										consent: 'pending',
										confirmedAt: null,
										taughtLearningGroupId:
											selected.taughtOfferingId === englishOfferingId ? id(871) : id(870)
									}
								: null;
					}
				detail.request.revision++;
				detail.coverage.revision++;
				detail.coverage.planHash = `plan-${detail.coverage.revision}`;
				const complete =
					detail.coverage.timetableState === 'ready' &&
					detail.coverage.days.every((day) => day.lessons.every((lesson) => lesson.assignment));
				const confirmed =
					complete &&
					detail.coverage.days.every((day) =>
						day.lessons.every((lesson) => lesson.assignment?.consent === 'confirmed')
					);
				detail.coverage.state = confirmed
					? 'confirmed'
					: complete
						? 'awaiting_confirmation'
						: 'incomplete';
				if (options.manager && detail.coverage.canManage && detail.instance === null && complete) {
					workflowStarts++;
					detail.instance = structuredClone(api.detail().instance);
					if (detail.instance) detail.instance.revision = detail.request.revision;
					detail.coverage.canManage = false;
				}

				return reply(route, detail);
			}
			if (path.endsWith('/coverage/replan')) {
				detail.request.status = 'returned';
				detail.request.revision++;
				detail.canEdit = true;
				detail.canSubmit = true;
				return reply(route, detail);
			}
			if (path.endsWith('/submit')) {
				detail.request.status = 'submitted';
				detail.canEdit = false;
				detail.canSubmit = false;
				if (body.emergencyReason) {
					detail.coverage.emergencyReason = body.emergencyReason;
					detail.coverage.state = 'emergency';
				}
				return reply(route, detail);
			}
			if (path === `/api/staff-leave/requests/${leaveId}` && method === 'GET')
				return reply(route, detail);
			return route.fallback();
		}
	);
	return {
		api,
		calls,
		reads,
		coverageOptions,
		detail: () => detail,
		teachers,
		returnLesson,
		task,
		releaseRefresh,
		workflowStarts: () => workflowStarts
	};
}
