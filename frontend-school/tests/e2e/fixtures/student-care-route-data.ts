import type { Page, Route } from '@playwright/test';
import { mockBehavior, behaviorTerm } from './behavior-route-data';
import { id, year, actor } from './staff-home-route-data';
import { PERMISSIONS } from '../../../src/lib/permissions/registry';
import type {
	CareRespondent,
	CareInstrumentDefinition,
	CareInstrumentVersion,
	CareRound,
	CareAssignment,
	CareAssignmentDetail,
	CareVisitRecord,
	CareCaseDetail,
	CareCaseEvent,
	SaveCareResponse,
	CreateCareInstrumentVersion,
	CreateCareRound,
	CreateCareAssignments,
	SaveCareVisit,
	SaveCareCase,
	AddCareCaseEvent,
	UpdateCareRound,
	UpdateCareAssignmentReviewers,
	CareStudentOption,
	CareStaffOption
} from '../../../src/lib/api/student-care';
export const careLongStudentName = `นักเรียนสมมติ${'ชื่อยาวสำหรับตรวจสอบการจัดหน้า'.repeat(5)}`;
export const careLongStaffName = `ครูสมมติ${'ชื่อยาวสำหรับตรวจสอบการจัดหน้า'.repeat(5)}`;
export const careTerm = behaviorTerm,
	careStudent = actor,
	careVersionId = id(2000),
	careRoundId = id(2001),
	careAssignmentId = id(2002),
	careVisitId = id(2003),
	careCaseId = id(2004);
export const carePath = (suffix = '') =>
	`/staff/student-care${suffix}?academicYearId=${year}&academicTermId=${careTerm}`;
export const careDefinition: CareInstrumentDefinition = {
	name: 'แบบทดสอบสมมติ',
	family: 'other',
	variant: 'สำหรับทดสอบระบบเท่านั้น',
	language: 'th',
	respondent: 'self_report',
	minimumAge: 6,
	maximumAge: 18,
	mode: 'digital',
	instructions: 'ข้อความสมมติ ไม่มีความหมายทางคลินิก',
	source: 'Synthetic nonclinical fixture',
	licenseNote: 'Synthetic fixture author permission',
	digitalRightsConfirmed: true,
	questions: [
		{
			key: 'q1',
			text: 'คำถามสมมติข้อหนึ่ง',
			scaleKey: 's',
			reverse: false,
			options: [
				{ key: 'a', label: 'ตัวเลือกสมมติ ก', points: 0, urgent: false },
				{ key: 'b', label: 'ตัวเลือกสมมติ ข', points: 1, urgent: false }
			]
		},
		{
			key: 'q2',
			text: 'คำถามสมมติข้อสอง',
			scaleKey: 's',
			reverse: false,
			options: [
				{ key: 'a', label: 'คำตอบสมมติ หนึ่ง', points: 0, urgent: false },
				{ key: 'b', label: 'คำตอบสมมติ สอง', points: 1, urgent: true }
			]
		}
	],
	scales: [
		{
			key: 's',
			label: 'มาตรสมมติ',
			sourceScaleKeys: [],
			bands: [{ key: 'synthetic', label: 'ผลสมมติเท่านั้น', minimum: 0, maximum: 2, urgent: false }]
		}
	]
};
export const careVersion: CareInstrumentVersion = {
	id: careVersionId,
	instrumentId: id(2010),
	version: 1,
	definition: careDefinition,
	createdAt: '2026-10-17T01:00:00Z'
};
export const careRound: CareRound = {
	id: careRoundId,
	name: 'รอบทดสอบการดูแล',
	purpose: 'วัตถุประสงค์สมมติเพื่อตรวจสอบระบบ',
	instrumentVersionIds: [careVersionId],
	academicTermId: careTerm,
	rosterDate: '2026-10-17',
	dueDate: '2026-10-31',
	status: 'open',
	rowVersion: 1,
	createdAt: '2026-10-17T01:00:00Z'
};
export const careAssignment: CareAssignment = {
	id: careAssignmentId,
	roundId: careRoundId,
	studentId: careStudent,
	studentName: 'นักเรียนทดสอบ หนึ่ง',
	respondentId: actor,
	respondent: 'self_report',
	instrumentVersionId: careVersionId,
	instrumentName: careDefinition.name,
	reviewerUserIds: [actor],
	createdBy: actor,
	status: 'assigned',
	rowVersion: 1,
	submittedAt: null,
	acknowledgedAt: null,
	urgent: false
};
export async function mockStudentCare(
	page: Page,
	options: {
		permissions?: string[];
		userType?: 'staff' | 'student' | 'parent';
		manual?: boolean;
		submitted?: boolean;
		failDraft?: boolean;
		holdDraft?: boolean;
		failOverview?: boolean;
		manyEvents?: boolean;
		unassigned?: boolean;
		respondent?: CareRespondent;
		manyAssignments?: boolean;
		roundStatus?: CareRound['status'];
		longNames?: boolean;
		manyPeople?: boolean;
		emptyLists?: boolean;
	} = {}
) {
	const permissions = options.permissions ?? [
		PERMISSIONS.STUDENT_CARE_READ_OWN,
		PERMISSIONS.STUDENT_CARE_READ_SCHOOL,
		PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL,
		PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL,
		PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_SCHOOL,
		PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL
	];
	await mockBehavior(page, { permissions, userType: options.userType });
	const version = structuredClone(careVersion);
	const studentName = options.longNames ? careLongStudentName : careAssignment.studentName;
	const staff: CareStaffOption[] = [
		{ id: actor, name: options.longNames ? careLongStaffName : 'ครูผู้รับผิดชอบ' },
		{ id: id(2011), name: 'ครูผู้ติดตาม' },
		...(options.manyPeople
			? Array.from({ length: 30 }, (_, index) => ({
					id: id(6000 + index),
					name: `ครูสมมติ ${String(index + 3).padStart(2, '0')}`
				}))
			: [])
	];
	const students: CareStudentOption[] = Array.from(
		{ length: options.manyPeople ? 51 : 1 },
		(_, index) => ({
			id: index === 0 ? careStudent : id(5000 + index),
			name: index === 0 ? studentName : `นักเรียนสมมติ ${String(index + 1).padStart(2, '0')}`,
			studentCode: String(index + 1).padStart(4, '0'),
			homeroom: 'ม.1/1',
			advisorIds: [actor],
			guardianIds: [id(2012)]
		})
	);
	if (options.respondent) version.definition.respondent = options.respondent;
	if (options.manual) {
		version.definition.mode = 'manual_result';
		version.definition.questions = [];
	}
	let round = { ...structuredClone(careRound), status: options.roundStatus ?? 'open' };
	let detail: CareAssignmentDetail = {
		assignment: {
			...careAssignment,
			studentName,
			reviewerUserIds: options.unassigned ? [id(2999)] : [actor],
			status: options.submitted ? 'submitted' : 'assigned',
			submittedAt: options.submitted ? '2026-10-17T02:00:00Z' : null
		},
		instrument: version,
		answers:
			options.submitted && !options.manual
				? [
						{ questionKey: 'q1', optionKey: 'b' },
						{ questionKey: 'q2', optionKey: 'a' }
					]
				: [],
		manualScores: options.submitted && options.manual ? [{ scaleKey: 's', score: 1 }] : [],
		results: options.submitted
			? [
					{
						scaleKey: 's',
						score: 1,
						bandKey: 'synthetic',
						bandLabel: 'ผลสมมติเท่านั้น',
						urgent: false
					}
				]
			: [],
		manualNote: options.submitted && options.manual ? 'เอกสารกระดาษสมมติ' : null,
		purpose: round.purpose,
		purposeAcknowledgedAt: options.submitted ? '2026-10-17T02:00:00Z' : null
	};
	let visit: CareVisitRecord = {
		visit: {
			id: careVisitId,
			studentId: careStudent,
			studentName,
			academicTermId: careTerm,
			plannedDate: '2026-10-17',
			visitorIds: options.unassigned ? [id(2999)] : [actor],
			status: 'planned',
			rowVersion: 1,
			completedAt: null
		},
		visitedDate: null,
		contactPerson: '',
		factualObservations: '',
		strengths: '',
		needs: '',
		agreedActions: '',
		nextFollowUp: null
	};
	const event = (index: number): CareCaseEvent => ({
		id: id(3000 + index),
		kind: 'note',
		note: `บันทึกสมมติ ${index}`,
		createdBy: actor,
		createdAt: '2026-10-17T02:00:00Z'
	});
	let allEvents = Array.from({ length: options.manyEvents ? 101 : 1 }, (_, index) =>
		event(index)
	).reverse();
	let support: CareCaseDetail = {
		case: {
			id: careCaseId,
			studentId: careStudent,
			studentName,
			title: 'การดูแลสมมติ',
			assignedUserIds: options.unassigned ? [id(2999)] : [actor],
			status: 'open',
			nextFollowUp: '2026-10-17',
			rowVersion: 1,
			createdAt: '2026-10-17T02:00:00Z',
			sourceAssignmentId: null,
			sourceVisitId: null
		},
		events: allEvents.slice(0, 100)
	};
	const roundChanges: UpdateCareRound[] = [];
	const reviewerChanges: UpdateCareAssignmentReviewers[] = [];
	const reads: string[] = [],
		queries: string[] = [],
		responses: SaveCareResponse[] = [],
		versions: CreateCareInstrumentVersion[] = [],
		rounds: CreateCareRound[] = [],
		assignments: CreateCareAssignments[] = [],
		visits: SaveCareVisit[] = [],
		cases: SaveCareCase[] = [],
		events: AddCareCaseEvent[] = [];
	let failDraft = options.failDraft,
		failOverview = options.failOverview;
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	const reply = <T>(route: Route, data: T, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			json: status < 400 ? { success: true, data } : { success: false, error: data },
			headers: { 'X-CSRF-Token': 'synthetic-csrf' }
		});
	await page.route('**/api/student-care/**', async (route) => {
		const url = new URL(route.request().url()),
			path = url.pathname,
			method = route.request().method();
		if (method === 'GET') {
			reads.push(path);
			queries.push(url.search);
		}
		if (path.endsWith('/overview')) {
			if (failOverview) {
				failOverview = false;
				return reply(route, 'ภาพรวมไม่พร้อม', 503);
			}
			return reply(route, {
				assignedCount:
					detail.assignment.status === 'assigned' || detail.assignment.status === 'in_progress'
						? options.manyAssignments
							? 51
							: 1
						: 0,
				submittedCount:
					detail.assignment.status === 'submitted' ? (options.manyAssignments ? 51 : 1) : 0,
				pendingVisitCount: 1,
				openCaseCount: permissions.some(
					(permission) =>
						permission === PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_SCHOOL ||
						permission === PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL
				)
					? 1
					: 0,
				followUpDueCount: permissions.some(
					(permission) =>
						permission === PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_SCHOOL ||
						permission === PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL
				)
					? 1
					: 0,
				urgentUnacknowledgedCount: 0
			});
		}
		if (path.endsWith('/staff')) return reply(route, staff);
		if (path.endsWith('/students')) {
			const search = url.searchParams.get('search')?.trim() ?? '';
			const after = url.searchParams.get('afterId');
			return reply(
				route,
				students
					.filter(
						(student) =>
							(!search || student.name.includes(search) || student.studentCode?.includes(search)) &&
							(!after || student.id > after)
					)
					.slice(0, Number(url.searchParams.get('limit') ?? 50))
			);
		}
		if (path.endsWith('/instruments')) {
			if (method === 'POST') {
				const payload: CreateCareInstrumentVersion = route.request().postDataJSON();
				versions.push(payload);
				version.definition = payload.definition;
				version.version = payload.expectedVersion ? payload.expectedVersion + 1 : 1;
				return reply(route, version);
			}
			return reply(route, options.emptyLists ? [] : [version]);
		}
		if (path.includes('/instruments/')) return reply(route, version);
		if (path.endsWith('/rounds')) {
			if (method === 'POST') {
				const payload: CreateCareRound = route.request().postDataJSON();
				rounds.push(payload);
				round = { ...round, ...payload };
				return reply(route, round);
			}
			return reply(route, options.emptyLists ? [] : [round]);
		}
		if (path.includes('/rounds/')) {
			if (method === 'PUT') {
				const payload: UpdateCareRound = route.request().postDataJSON();
				roundChanges.push(payload);
				round = { ...round, status: payload.status, rowVersion: round.rowVersion + 1 };
			}
			return reply(route, round);
		}
		if (path.endsWith('/assignments')) {
			if (method === 'POST') {
				const payload: CreateCareAssignments = route.request().postDataJSON();
				assignments.push(payload);
				return reply(route, [detail.assignment]);
			}
			const rows = options.emptyLists
				? []
				: options.manyAssignments
					? Array.from({ length: 51 }, (_, index) => ({
							...detail.assignment,
							id: id(4000 + index),
							instrumentName: `แบบประเมินประวัติ ${index + 1}`
						}))
					: [detail.assignment];
			const after = url.searchParams.get('afterId');
			return reply(
				route,
				rows
					.filter((row) => !after || row.id > after)
					.slice(0, Number(url.searchParams.get('limit') ?? 50))
			);
		}
		if (path.endsWith('/draft') || path.endsWith('/submit')) {
			const payload: SaveCareResponse = route.request().postDataJSON();
			responses.push(payload);
			if (options.holdDraft) await held;
			if (failDraft) {
				failDraft = false;
				return reply(route, 'บันทึกชั่วคราวไม่ได้ กรุณาลองอีกครั้ง', 503);
			}
			detail = {
				...detail,
				answers: payload.answers,
				manualScores: payload.manualScores,
				manualNote: payload.manualNote,
				purposeAcknowledgedAt: payload.acknowledgePurpose ? '2026-10-17T03:00:00Z' : null,
				assignment: {
					...detail.assignment,
					rowVersion: detail.assignment.rowVersion + 1,
					status: path.endsWith('/submit') ? 'submitted' : 'in_progress',
					submittedAt: path.endsWith('/submit') ? '2026-10-17T03:00:00Z' : null
				},
				results: path.endsWith('/submit')
					? [
							{
								scaleKey: 's',
								score: 1,
								bandKey: 'synthetic',
								bandLabel: 'ผลสมมติเท่านั้น',
								urgent: false
							}
						]
					: []
			};
			return reply(route, detail);
		}
		if (path.endsWith('/acknowledge')) {
			detail.assignment = {
				...detail.assignment,
				acknowledgedAt: '2026-10-17T04:00:00Z',
				rowVersion: detail.assignment.rowVersion + 1
			};
			return reply(route, detail.assignment);
		}
		if (path.endsWith('/cancel')) {
			detail.assignment = {
				...detail.assignment,
				status: 'cancelled',
				rowVersion: detail.assignment.rowVersion + 1
			};
			return reply(route, detail.assignment);
		}
		if (path.endsWith('/reviewers')) {
			const payload: UpdateCareAssignmentReviewers = route.request().postDataJSON();
			reviewerChanges.push(payload);
			detail.assignment = {
				...detail.assignment,
				reviewerUserIds: payload.reviewerUserIds,
				rowVersion: detail.assignment.rowVersion + 1
			};
			return reply(route, detail.assignment);
		}
		if (path.includes('/assignments/')) return reply(route, detail);
		if (path.endsWith('/visits') || path.includes('/visits/')) {
			if (method === 'POST' || method === 'PUT') {
				const payload: SaveCareVisit = route.request().postDataJSON();
				visits.push(payload);
				visit = {
					...payload,
					visit: {
						...visit.visit,
						...payload,
						rowVersion: visit.visit.rowVersion + 1,
						completedAt: payload.status === 'completed' ? '2026-10-17T03:00:00Z' : null
					}
				};
				return reply(route, visit);
			}
			return reply(
				route,
				path.endsWith('/visits') ? (options.emptyLists ? [] : [visit.visit]) : visit
			);
		}
		if (path.endsWith('/events')) {
			if (method === 'GET') {
				const after = url.searchParams.get('afterId');
				return reply(route, allEvents.filter((event) => !after || event.id < after).slice(0, 100));
			}
			const payload: AddCareCaseEvent = route.request().postDataJSON();
			events.push(payload);
			allEvents = [
				{ ...event(200 + events.length), kind: payload.kind, note: payload.note },
				...allEvents
			];
			support = {
				case: {
					...support.case,
					status: payload.status,
					nextFollowUp: payload.nextFollowUp,
					rowVersion: support.case.rowVersion + 1
				},
				events: allEvents.slice(0, 100)
			};
			return reply(route, support);
		}
		if (path.endsWith('/cases') || path.includes('/cases/')) {
			if (method === 'POST' || method === 'PUT') {
				const payload: SaveCareCase = route.request().postDataJSON();
				cases.push(payload);
				support = {
					...support,
					case: { ...support.case, ...payload, rowVersion: support.case.rowVersion + 1 }
				};
				return reply(route, support);
			}
			return reply(
				route,
				path.endsWith('/cases') ? (options.emptyLists ? [] : [support.case]) : support
			);
		}
		return reply(route, 'Unexpected synthetic endpoint', 404);
	});
	return {
		roundChanges,
		reviewerChanges,
		reads,
		queries,
		responses,
		versions,
		rounds,
		assignments,
		visits,
		cases,
		events,
		release: () => release()
	};
}
