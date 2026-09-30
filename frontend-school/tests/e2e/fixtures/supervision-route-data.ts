import type { Page, Route } from '@playwright/test';
export const id = (n: number) => `44000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
export const year = id(1);
export const nextYear = id(2);
export const firstCycle = id(3);
export const secondCycle = id(4);
export const templateId = id(5);
export const path = (section: string, academicYearId = year, cycleId = '') =>
	`/staff/academic/supervision/${section}?academicYearId=${academicYearId}${cycleId ? `&cycleId=${cycleId}` : ''}`;

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
		hold?:
			| 'cycles'
			| 'teacher-status'
			| 'detail'
			| 'patch'
			| 'observations'
			| 'summaries'
			| 'timetable'
			| 'evaluator'
			| 'progress';
		fail?:
			| 'cycles'
			| 'teacher-status'
			| 'detail'
			| 'observations'
			| 'summaries'
			| 'timetable'
			| 'evaluator'
			| 'progress';
		failAt?: number;
		holdAt?: number;
		term?: boolean;
		observationStatus?: string;
	} = {}
) {
	const reads: string[] = [];
	const writes: string[] = [];
	const held = gate();
	const counts = new Map<string, number>();
	let cycleStatus = 'open';
	let observationStatus = options.observationStatus ?? 'requested';
	const observation = (academicYearId = year) => ({
		id: id(30),
		cycleId: firstCycle,
		academicYearId,
		academicTermId: options.term ? id(8) : null,
		templateId,
		observedUserId: id(20),
		observedDisplayName: academicYearId === nextYear ? 'ครู ปีใหม่' : 'ครู ในคิว',
		observedAt: '2026-10-01T09:00:00Z',
		requestedAt: '2026-09-30T09:00:00Z',
		status: observationStatus,
		lessonSnapshot: { subjectName: 'วิชาทดสอบ', classroomLabel: 'ม.1/1' },
		manualLesson: null,
		evaluators: [
			{
				id: id(31),
				evaluatorUserId: id(20),
				evaluatorDisplayName: 'ผู้ประเมินทดสอบ',
				roleLabel: 'ผู้ประเมิน',
				isRequired: true,
				status: observationStatus === 'evaluators_submitted' ? 'submitted' : 'assigned'
			}
		],
		actions: []
	});
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
			} else writes.push(`${method} ${resource}`);
			if (resource === '/api/auth/me')
				return reply(route, {
					id: id(20),
					username: 'E2E-supervision',
					firstName: 'นิเทศ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: options.permissions ?? [
						'supervision.manage.school',
						'supervision.read.school'
					]
				});
			if (resource === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: year,
					activeAcademicTermId: null,
					terms: options.term
						? [
								{
									id: id(8),
									academicYearId: year,
									term: 1,
									name: 'ภาคเรียน 1',
									status: 'open',
									startDate: '2026-05-01',
									plannedEndDate: '2026-12-01',
									closedOn: null
								}
							]
						: [],
					years: [year, nextYear].map((value, index) => ({
						id: value,
						year: 2569 + index,
						name: String(2569 + index),
						status: 'open',
						startDate: '2026-05-01',
						plannedEndDate: '2027-04-30',
						closedOn: null
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
					startsAt: '2026-09-01T00:00:00Z',
					endsAt: '2026-12-01T00:00:00Z',
					status: cycleStatus,
					createdAt: '2026-09-01T00:00:00Z',
					updatedAt: '2026-09-01T00:00:00Z',
					targets: []
				}));
			const snapshot = observation(url.searchParams.get('academicYearId') ?? year);
			const kind =
				resource === '/api/supervision/cycles'
					? 'cycles'
					: resource.endsWith('/teacher-status')
						? 'teacher-status'
						: resource === `/api/supervision/templates/${templateId}`
							? 'detail'
							: resource === '/api/supervision/observations'
								? 'observations'
								: resource.endsWith('/summaries')
									? 'summaries'
									: resource === '/api/me/timetable'
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
					(kind !== 'teacher-status' || resource.includes(secondCycle))
				)
					await held.promise;
				if (options.fail === kind && counts.get(resource) === (options.failAt ?? 1))
					return reply(route, 'region ไม่พร้อม', 503);
			}
			if (resource === '/api/supervision/observations' && method === 'GET')
				return reply(route, { items: [snapshot] });
			if (resource === '/api/me/timetable') return reply(route, []);
			if (resource.endsWith('/evaluator-availability'))
				return reply(route, {
					items: [{ id: id(32), name: 'ผู้ประเมินที่ว่าง', available: true, conflicts: [] }]
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
				observationStatus = resource.endsWith('/return-request')
					? 'returned'
					: resource.includes('/evaluations/')
						? 'evaluators_submitted'
						: 'scheduled';
				return reply(route, observation());
			}
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
				cycleStatus = route.request().postDataJSON().status;
				return reply(route, { ...cycles()[0], title: 'รอบแรก', status: cycleStatus });
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
		reads,
		writes,
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
}
