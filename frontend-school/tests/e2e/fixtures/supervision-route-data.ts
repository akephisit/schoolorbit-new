import type {
	SupervisionObservation,
	SupervisionCycle,
	SupervisionObservationPeriod,
	SupervisionTimetableOption,
	SupervisionObservationStatus,
	SupervisionCycleStatus,
	SupervisionEvaluator,
	SupervisionEvaluatorAvailability,
	ReplaceObservationEvaluatorsRequest,
	ReturnObservationRequest,
	UpdateRequestedObservationRequest
} from '../../../src/lib/api/supervision';
import type { AcademicTermOption } from '../../../src/lib/api/academic-context';
import { sessionState } from './session-state';
import type { Page, Route } from '@playwright/test';
export const id = (n: number) => `44000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
export const year = id(1);
export const nextYear = id(2);
export const firstCycle = id(3);
export const secondCycle = id(4);
export const templateId = id(5);
export const path = (section: string, academicYearId = year, cycleId = '') =>
	`/staff/academic/supervision/${section}?academicYearId=${academicYearId}${cycleId ? `&cycleId=${cycleId}` : ''}`;

export const savedPeriod = (
	patch: Partial<SupervisionObservationPeriod> = {}
): SupervisionObservationPeriod => ({
	id: id(70),
	observationId: id(30),
	position: 1,
	timetableBlockGroupId: null,
	learningGroupId: null,
	homeroomId: null,
	observedAt: '2026-10-01T09:00:00Z',
	endsAt: null,
	manualLesson: null,
	lessonSnapshot: { subjectName: 'วิชาทดสอบ', classroomLabel: 'ม.1/1', periodLabel: 'คาบ 1' },
	...patch
});
export const timetableOption = (
	index = 0,
	patch: Partial<SupervisionTimetableOption> = {}
): SupervisionTimetableOption => ({
	observedDate: '2026-10-01',
	blockGroupId: id(80 + index),
	blockId: id(90 + index),
	bellPeriodId: id(100 + index),
	periodName: `คาบ ${index + 1}`,
	startTime: `${String(8 + index).padStart(2, '0')}:00:00`,
	endTime: `${String(9 + index).padStart(2, '0')}:00:00`,
	learningGroupId: id(110 + index),
	learningGroupName: 'กลุ่มปัจจุบัน',
	subjectName: `วิชาปัจจุบัน ${index + 1}`,
	homeroomId: id(120),
	homeroomName: 'ม.1/1',
	roomName: 'ห้องปัจจุบัน',
	bookingAvailability: { pendingCount: 0, confirmedCount: 0, blocked: false },
	...patch
});

function gate() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => (release = resolve));
	return { promise, release };
}

async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

export async function mock(
	page: Page,
	options: {
		permissions?: string[];
		cycleStatus?: SupervisionCycleStatus;
		observationStartDate?: string | null;
		observationEndDate?: string | null;
		hold?:
			| 'cycles'
			| 'teacher-status'
			| 'detail'
			| 'patch'
			| 'observations'
			| 'summaries'
			| 'timetable'
			| 'evaluator'
			| 'progress'
			| 'selected-cycle'
			| 'summary'
			| 'observation-detail'
			| 'review'
			| 'timetable-detail';
		fail?:
			| 'cycles'
			| 'teacher-status'
			| 'detail'
			| 'observations'
			| 'summaries'
			| 'timetable'
			| 'evaluator'
			| 'progress'
			| 'selected-cycle'
			| 'summary'
			| 'observation-detail'
			| 'review'
			| 'timetable-detail';
		failAt?: number;
		failStatus?: number;
		holdAt?: number;
		term?: boolean | Pick<AcademicTermOption, 'name' | 'sequence' | 'termType'>;
		observationStatus?: SupervisionObservationStatus;
		observation?: Partial<SupervisionObservation>;
		additionalObservations?: (Partial<SupervisionObservation> &
			Pick<SupervisionObservation, 'id'>)[];
		bookingItems?: SupervisionTimetableOption[];
		evaluatorAvailability?: SupervisionEvaluatorAvailability[];
		canManage?: boolean;
		canReview?: boolean;
		averageRating?: number;
	} = {}
) {
	const reads: string[] = [];
	const writes: string[] = [];
	const payloads: unknown[] = [];
	const held = gate();
	const counts = new Map<string, number>();
	let authFirstName = 'นิเทศ';
	let authSessionId = sessionState().sessionId;
	let authPermissions = options.permissions ?? [
		'supervision.manage.school',
		'supervision.read.school'
	];
	const cycleStatus = options.cycleStatus ?? 'open';
	const cyclePatches: Record<string, Partial<SupervisionCycle>> = {};
	let observationStatus = options.observationStatus ?? 'requested';
	let periodsLocked =
		options.observation?.periodsLocked ?? !['requested', 'returned'].includes(observationStatus);
	let observationPatch: Partial<SupervisionObservation> = {};
	let observationScope: 'manage' | 'read' | 'hidden' | undefined;
	const evaluatorAvailability = options.evaluatorAvailability ?? [
		{
			id: id(32),
			name: 'ผู้ประเมินที่ว่าง',
			available: true,
			conflictReason: null,
			conflict: null
		}
	];
	const observation = (academicYearId = year): SupervisionObservation => ({
		canManage: observationScope ? observationScope === 'manage' : (options.canManage ?? true),
		canReview: options.canReview ?? true,
		averageRating: options.averageRating ?? null,
		id: id(30),
		cycleId: firstCycle,
		academicYearId,
		academicTermId: id(8),
		templateId,
		observedUserId: id(20),
		observedDisplayName: academicYearId === nextYear ? 'ครู ปีใหม่' : 'ครู ในคิว',
		observedAt: '2026-10-01T09:00:00Z',
		requestedAt: '2026-09-30T09:00:00Z',
		createdAt: '2026-09-30T09:00:00Z',
		updatedAt: '2026-10-01T09:00:00Z',
		status: observationStatus,
		periodsLocked,
		periods: [savedPeriod()],
		evaluators:
			observationStatus === 'requested'
				? []
				: [
						{
							id: id(31),
							observationId: id(30),
							createdAt: '2026-09-30T09:00:00Z',
							updatedAt: '2026-10-01T09:00:00Z',
							evaluatorUserId: id(20),
							evaluatorDisplayName: 'ผู้ประเมินทดสอบ',
							roleLabel: 'ผู้ประเมิน',
							isRequired: true,
							status: observationStatus === 'evaluators_submitted' ? 'submitted' : 'assigned'
						}
					],
		actions: [],
		...options.observation,
		...observationPatch
	});
	function applyEvaluators(route: Route) {
		const { evaluators }: ReplaceObservationEvaluatorsRequest = route.request().postDataJSON();
		const previous = observation().evaluators;
		observationPatch = {
			...observationPatch,
			evaluators: (evaluators ?? []).map((evaluator, index) => ({
				...evaluator,
				isRequired: evaluator.isRequired ?? true,
				id: id(150 + index),
				observationId: id(30),
				evaluatorDisplayName:
					previous.find((item) => item.evaluatorUserId === evaluator.evaluatorUserId)
						?.evaluatorDisplayName ??
					evaluatorAvailability.find((item) => item.id === evaluator.evaluatorUserId)?.name ??
					null,
				createdAt: '2026-10-01T09:00:00Z',
				updatedAt: '2026-10-01T09:00:00Z',
				status: 'assigned'
			}))
		};
	}
	let templateTitle = 'แบบประเมินทดสอบ';
	const detail = () => ({
		id: templateId,
		title: templateTitle,
		description: 'รายละเอียดแบบประเมิน',
		status: 'active',
		ratingMin: 1,
		ratingMax: 5,
		createdAt: '2026-09-01T00:00:00Z',
		updatedAt: '2026-09-01T00:00:00Z',
		sections: [
			{
				id: id(6),
				templateId,
				title: 'หมวดกิจกรรม',
				sortOrder: 1,
				items: [
					{
						id: id(7),
						sectionId: id(6),
						label: 'หัวข้อ rubric ที่โหลดเมื่อเปิด',
						itemType: 'rating',
						required: true,
						sortOrder: 1
					}
				]
			}
		],
		steps: []
	});
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const resource = url.pathname;
			const method = route.request().method();
			if (method === 'GET') {
				reads.push(resource);
				counts.set(resource, (counts.get(resource) ?? 0) + 1);
			} else {
				writes.push(`${method} ${resource}`);
				if (route.request().postData()) payloads.push(route.request().postDataJSON());
			}
			if (resource === '/api/auth/me')
				return reply(route, {
					id: id(20),
					username: 'E2E-supervision',
					firstName: authFirstName,
					displayName: `${authFirstName} ทดสอบ`,
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					session: sessionState(Date.now(), { sessionId: authSessionId }),
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: authPermissions
				});
			if (resource === '/api/auth/activity') {
				if (route.request().postDataJSON().expectedSessionId !== authSessionId)
					return reply(route, 'session changed', 409);
				return reply(route, sessionState(Date.now(), { sessionId: authSessionId }));
			}
			if (resource === '/api/notifications/stream')
				return route.fulfill({
					status: 200,
					contentType: 'text/event-stream',
					body: ': connected\n\n'
				});
			if (resource === '/api/academic/context/options')
				return reply(route, {
					defaultAcademicYearId: year,
					defaultAcademicTermId: options.term ? id(8) : null,
					terms: options.term
						? [
								{
									id: id(8),
									academicYearId: year,
									blocksYearClosure: true,
									code: String(options.term === true ? 1 : options.term.sequence),
									includedInYearResult: true,
									isVisible: true,
									schoolDays: [],
									status: 'active',
									...(options.term === true
										? { name: 'ภาคเรียนที่ 1', sequence: 1, termType: 'regular' }
										: options.term)
								} satisfies AcademicTermOption
							]
						: [],
					years: [year, nextYear].map((value, index) => ({
						isVisible: true,
						id: value,
						year: 2569 + index,
						name: String(2569 + index),
						status: 'open'
					}))
				});
			if (resource === '/api/menu/user') return reply(route, { groups: [] });
			const cycles = () =>
				[firstCycle, secondCycle].map((value, index) => ({
					id: value,
					academicYearId: url.searchParams.get('academicYearId') ?? year,
					academicTermId: options.term ? id(8) : null,
					templateId,
					title: index
						? 'รอบที่สอง'
						: url.searchParams.get('academicYearId') === nextYear
							? 'รอบปีใหม่'
							: 'รอบแรก',
					status: cycleStatus,
					observationStartDate:
						options.observationStartDate === undefined
							? '2026-09-01'
							: options.observationStartDate,
					observationEndDate:
						options.observationEndDate === undefined ? '2026-12-31' : options.observationEndDate,
					createdAt: '2026-09-01T00:00:00Z',
					updatedAt: '2026-09-01T00:00:00Z',
					targets: [],
					...cyclePatches[value]
				}));
			const selectedId = resource.match(/\/observations\/([^/]+)$/)?.[1];
			const snapshot = {
				...observation(url.searchParams.get('academicYearId') ?? year),
				...(selectedId
					? {
							id: selectedId,
							observedDisplayName: selectedId === id(40) ? 'ครู รายการสอง' : 'ครู ในคิว'
						}
					: {})
			};
			const kind = resource.endsWith('/review')
				? 'review'
				: resource.endsWith('/timetable-options')
					? 'timetable-detail'
					: resource.endsWith('/summary')
						? 'summary'
						: resource === `/api/supervision/cycles/${firstCycle}`
							? 'selected-cycle'
							: selectedId
								? 'observation-detail'
								: resource === '/api/supervision/cycles'
									? 'cycles'
									: resource.endsWith('/teacher-status')
										? 'teacher-status'
										: resource === `/api/supervision/templates/${templateId}`
											? 'detail'
											: resource === '/api/supervision/observations'
												? 'observations'
												: resource.endsWith('/summaries')
													? 'summaries'
													: resource === '/api/supervision/booking-options'
														? 'timetable'
														: resource.endsWith('/evaluator-availability')
															? 'evaluator'
															: resource.endsWith('/progress')
																? 'progress'
																: '';
			if (method === 'GET' && kind) {
				if (
					options.hold === kind &&
					(!options.holdAt || counts.get(resource) === options.holdAt) &&
					(!['cycles', 'observations'].includes(kind) ||
						url.searchParams.get('academicYearId') === year) &&
					(kind !== 'teacher-status' || resource.includes(secondCycle)) &&
					(kind !== 'observation-detail' || selectedId === id(30))
				)
					await held.promise;
				if (options.fail === kind && counts.get(resource) === (options.failAt ?? 1))
					return reply(route, 'region ไม่พร้อม', options.failStatus ?? 503);
			}
			if (selectedId && method === 'GET') return reply(route, snapshot);
			if (resource.endsWith('/review') && method === 'GET')
				return reply(route, {
					observation: observation(),
					template: detail(),
					evaluatorResults: [],
					itemSummaries: [{ templateItemId: id(7), averageRating: 4, responseCount: 1 }],
					averageRating: 4
				});
			if (
				resource.endsWith('/timetable-options') ||
				resource === '/api/supervision/booking-options'
			)
				return reply(route, {
					sourceDigest: 'source-1',
					weekStartDate: url.searchParams.get('weekStartDate'),
					weekEndDate: url.searchParams.get('weekStartDate'),
					items: options.bookingItems ?? []
				});
			if (resource === `/api/supervision/cycles/${firstCycle}` && method === 'GET')
				return reply(route, cycles()[0]);
			if (resource.endsWith('/summary'))
				return reply(route, {
					id: templateId,
					title: templateTitle,
					status: 'active',
					ratingMin: 1,
					ratingMax: 5,
					sectionCount: 1,
					itemCount: 1
				});
			if (resource === '/api/supervision/observations' && method === 'GET')
				return reply(route, {
					items: [
						...(observationScope === 'hidden' ? [] : [snapshot]),
						...(options.additionalObservations ?? []).map((item) => ({ ...snapshot, ...item }))
					]
				});
			if (resource.endsWith('/evaluator-availability'))
				return reply(route, {
					items: evaluatorAvailability,
					overlap: { pendingCount: 0, confirmedCount: 0, blocked: false },
					withinCycleDates: true,
					overlappingBookings: []
				});
			if (resource.endsWith('/progress'))
				return reply(route, {
					cycleId: firstCycle,
					totalObservations: 1,
					completedCount: 0,
					underReviewCount: 0,
					approvedCount: 0,
					averageRating: null
				});
			if (method === 'POST' && resource.includes(`/observations/${id(30)}/`)) {
				if (options.hold === 'patch') await held.promise;
				if (resource.endsWith('/approve-request')) {
					periodsLocked = true;
					applyEvaluators(route);
				}
				if (resource.endsWith('/return-request')) {
					const { comment }: ReturnObservationRequest = route.request().postDataJSON();
					observationPatch = {
						...observationPatch,
						actions: [
							...observation().actions,
							{
								id: id(300),
								observationId: id(30),
								fromStatus: observationStatus,
								toStatus: 'returned',
								actionKind: 'request_returned',
								comment,
								createdAt: '2026-10-01T09:00:00Z'
							}
						]
					};
				}
				observationStatus = resource.endsWith('/resubmit-request')
					? 'requested'
					: resource.endsWith('/cancel')
						? 'cancelled'
						: resource.endsWith('/certify')
							? 'approved'
							: resource.endsWith('/approve')
								? 'published'
								: resource.endsWith('/return-request')
									? 'returned'
									: resource.includes('/evaluations/')
										? 'evaluators_submitted'
										: 'planned';
				if (resource.includes('/evaluations/'))
					observationPatch = {
						...observationPatch,
						evaluators: observation().evaluators.map((evaluator): SupervisionEvaluator =>
							evaluator.evaluatorUserId === id(20)
								? { ...evaluator, status: 'submitted' }
								: evaluator
						)
					};
				return reply(route, observation());
			}
			if (method === 'PUT' && resource === `/api/supervision/observations/${id(30)}/evaluators`) {
				if (options.hold === 'patch') await held.promise;
				applyEvaluators(route);
				return reply(route, observation());
			}
			if (method === 'DELETE' && resource === `/api/supervision/observations/${id(30)}/request`) {
				observationStatus = 'cancelled';
				return reply(route, { ...observation(), status: observationStatus });
			}
			if (
				method === 'PATCH' &&
				(resource === `/api/supervision/observations/${id(30)}/request` ||
					resource === `/api/supervision/observations/${id(30)}`)
			) {
				if (resource.endsWith('/request')) observationStatus = 'requested';
				const payload: UpdateRequestedObservationRequest = route.request().postDataJSON();
				const periods = payload.timetableBlockGroupIds.map((groupId, index) => {
					const item =
						options.bookingItems?.find((item) => item.blockGroupId === groupId) ??
						timetableOption(index);
					return savedPeriod({
						id: id(200 + index),
						position: index + 1,
						timetableBlockGroupId: groupId,
						homeroomId: item.homeroomId,
						learningGroupId: item.learningGroupId,
						observedAt: new Date(`${payload.observedDate}T${item.startTime}+07:00`).toISOString(),
						endsAt: new Date(`${payload.observedDate}T${item.endTime}+07:00`).toISOString(),
						lessonSnapshot: {
							source: 'timetable',
							subjectName: item.subjectName,
							classroomLabel: item.homeroomName,
							periodLabel: item.periodName,
							roomLabel: item.roomName
						}
					});
				});
				observationPatch = { ...observationPatch, periods, observedAt: periods[0].observedAt };
				return reply(route, { ...observation(), status: observationStatus });
			}
			if (resource === '/api/supervision/cycles' && method === 'POST')
				return reply(route, { ...cycles()[0], ...route.request().postDataJSON() });
			if (resource === '/api/supervision/cycles' && method === 'GET')
				return reply(route, { items: cycles() });
			if (resource === '/api/supervision/templates/summaries')
				return reply(route, {
					items: [
						{
							id: templateId,
							title: templateTitle,
							status: 'active',
							ratingMin: 1,
							ratingMax: 5,
							sectionCount: 1,
							itemCount: 1
						}
					]
				});
			if (resource === `/api/supervision/templates/${templateId}` && method === 'GET')
				return reply(route, detail());
			if (resource.endsWith('/teacher-status'))
				return reply(route, {
					items: [
						{
							teacherId: id(21),
							teacherDisplayName: resource.includes(secondCycle) ? 'ครู รอบสอง' : 'ครู รอบแรก',
							organizationUnitNames: [],
							evaluatorNames: [],
							nextStepLabel: 'จองคาบนิเทศ',
							averageRating: null
						}
					]
				});
			if (resource === `/api/supervision/cycles/${firstCycle}` && method === 'PATCH') {
				if (options.hold === 'patch') await held.promise;
				cyclePatches[firstCycle] = {
					...cyclePatches[firstCycle],
					...route.request().postDataJSON()
				};
				return reply(route, { ...cycles()[0], title: 'รอบแรก' });
			}
			if (resource === `/api/supervision/templates/${templateId}` && method === 'PATCH') {
				if (options.hold === 'patch') await held.promise;
				templateTitle = route.request().postDataJSON().title;
				return reply(route, detail());
			}
			if (resource.startsWith('/api/supervision/'))
				return reply(route, 'unmocked supervision read', 404);
			return reply(route, {});
		}
	);
	return {
		replaceIdentity: async (permissions: string[], sessionId = authSessionId) => {
			authPermissions = [...permissions];
			authSessionId = sessionId;
			await replaceFixtureIdentity(page, authPermissions, authSessionId);
		},
		setFirstName: (firstName: string) => {
			authFirstName = firstName;
		},
		setPermissions: (permissions: string[]) => {
			authPermissions = [...permissions];
		},
		setObservationScope: (scope: 'manage' | 'read' | 'hidden') => {
			observationScope = scope;
		},
		reads,
		writes,
		payloads,
		observation: () => structuredClone(observation()),
		release: held.release,
		count: (resource: string) => counts.get(resource) ?? 0
	};
}

export async function navigate(page: Page, href: string) {
	await page.evaluate((destination) => {
		const link = document.createElement('a');
		link.href = destination;
		link.textContent = 'test navigation';
		link.dataset.sveltekitPreloadData = 'off';
		link.style.cssText = 'position:fixed;right:10px;bottom:10px;z-index:9999;background:white';
		document.body.append(link);
	}, href);
	await page.getByRole('link', { name: 'test navigation' }).click();
	await page
		.getByRole('link', { name: 'test navigation', exact: true })
		.evaluateAll((links) => links.forEach((link) => link.remove()));
}

async function replaceFixtureIdentity(page: Page, permissions: string[], sessionId?: string) {
	await page.evaluate(
		async ({ permissions, sessionId }) => {
			const modulePath = '/src/lib/stores/auth.ts';
			const { authStore } = await import(/* @vite-ignore */ modulePath);
			let current: import('../../../src/lib/stores/auth.js').AuthState | undefined;
			const stop = authStore.subscribe(
				(state: import('../../../src/lib/stores/auth.js').AuthState) => {
					current = state;
				}
			);
			stop();
			if (!current?.user || !current.session)
				throw new Error('Expected authenticated fixture session');
			authStore.setUser(current.user, permissions, {
				...current.session,
				...(sessionId ? { sessionId } : {})
			});
		},
		{ permissions, sessionId }
	);
}

/** Select a displayed date through the shared DatePicker's real calendar controls. */
export async function selectSupervisionDate(page: Page, label: string, date: string) {
	await page.getByRole('button', { name: label, exact: true }).click();
	const accessibleDate = new Intl.DateTimeFormat('th-TH', {
		timeZone: 'Asia/Bangkok',
		weekday: 'long',
		day: 'numeric',
		month: 'long',
		year: 'numeric'
	}).format(new Date(`${date}T12:00:00+07:00`));
	await page.getByRole('button', { name: accessibleDate, exact: true }).click();
}
