import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id, year, actor } from './staff-home-route-data';
import { PERMISSIONS } from '../../../src/lib/permissions/registry';
import type {
	BehaviorItem,
	BehaviorStudent,
	BehaviorReportEntry,
	BehaviorStudentSummary,
	BehaviorSettings,
	CreateBehaviorBatch,
	ExecuteBehaviorCleanup,
	CancelBehaviorEntries
} from '../../../src/lib/api/behavior';
export const behaviorTerm = id(800),
	behaviorStudent = id(801),
	secondStudent = id(802),
	otherStudent = id(803),
	roomA = id(804),
	roomB = id(805),
	meritItem = id(810);
export const behaviorDate = '2026-10-14';
export const behaviorPath = (suffix = '') =>
	`/staff/behavior${suffix}?academicYearId=${year}&academicTermId=${behaviorTerm}`;
export const students: BehaviorStudent[] = [
	{
		id: behaviorStudent,
		studentAcademicYearId: id(820),
		name: 'นักเรียน หนึ่ง',
		studentCode: '0001',
		gradeLevelId: id(830),
		gradeLevel: 'ม.1',
		homeroomId: roomA,
		homeroom: 'ม.1/1',
		classNumber: 1
	},
	{
		id: secondStudent,
		studentAcademicYearId: id(821),
		name: 'นักเรียน สอง',
		studentCode: '0002',
		gradeLevelId: id(830),
		gradeLevel: 'ม.1',
		homeroomId: roomA,
		homeroom: 'ม.1/1',
		classNumber: 2
	},
	{
		id: otherStudent,
		studentAcademicYearId: id(822),
		name: 'นักเรียน สาม',
		studentCode: '0003',
		gradeLevelId: id(830),
		gradeLevel: 'ม.1',
		homeroomId: roomB,
		homeroom: 'ม.1/2',
		classNumber: 1
	}
];
export const item: BehaviorItem = {
	id: meritItem,
	name: 'ช่วยงานโรงเรียน',
	side: 'merit',
	enabled: true,
	rowVersion: 1,
	versionId: id(811),
	effectiveFrom: '2026-01-01',
	points: 5
};
const cycle = (side: 'merit' | 'deduction') => ({
	id: id(side === 'merit' ? 840 : 841),
	side,
	startsOn: '2026-10-01',
	startsAt: '2026-09-30T17:00:00Z',
	endsOn: null,
	endsAt: null,
	state: 'current',
	label: 'ตุลาคม'
});
export const summary: BehaviorStudentSummary = {
	studentId: behaviorStudent,
	current: { merit: 20, deduction: 8, net: 12 },
	history: { merit: 10, deduction: 20, net: -10 },
	lifetime: { merit: 30, deduction: 28, net: 2 },
	comparableCurrent: { merit: 20, deduction: 8, net: 12 },
	comparisonExact: true,
	comparisonStart: '2026-10-01',
	comparisonEnd: behaviorDate,
	meritCycle: cycle('merit'),
	deductionCycle: cycle('deduction'),
	historyTotals: []
};
export const entries: BehaviorReportEntry[] = [
	{
		id: id(850),
		studentId: behaviorStudent,
		studentName: students[0].name,
		studentCode: '0001',
		homeroomId: roomA,
		homeroom: 'ม.1/1',
		gradeLevelId: id(830),
		date: behaviorDate,
		academicTermId: behaviorTerm,
		side: 'merit',
		source: 'manual',
		points: 5,
		itemName: item.name,
		detail: 'ช่วยจัดห้อง',
		occurredAt: '2026-10-14T02:00:00Z',
		batchId: id(851),
		sourceKey: null,
		sourceSessionId: null,
		createdBy: actor,
		creatorName: 'ครูผู้บันทึก',
		createdAt: '2026-10-14T02:01:00Z',
		cancelledAt: null,
		cancellationReason: null,
		canCancel: true
	},
	{
		id: id(852),
		studentId: behaviorStudent,
		studentName: students[0].name,
		studentCode: '0001',
		homeroomId: roomA,
		homeroom: 'ม.1/1',
		gradeLevelId: id(830),
		date: behaviorDate,
		academicTermId: behaviorTerm,
		side: 'deduction',
		source: 'lesson',
		points: 2,
		itemName: 'ขาดคาบ',
		detail: null,
		occurredAt: null,
		batchId: null,
		sourceKey: 'lesson-key',
		sourceSessionId: id(853),
		createdBy: null,
		creatorName: null,
		createdAt: '2026-10-14T03:01:00Z',
		cancelledAt: null,
		cancellationReason: null,
		canCancel: false
	}
];
export const settings: BehaviorSettings = {
	enabled: true,
	countAbsentLessons: false,
	countSpecial: true,
	dailyMeritCap: null,
	dailyDeductionCap: null,
	meritReset: 'monthly',
	deductionReset: 'monthly',
	rowVersion: 1
};
export async function mockBehavior(
	page: Page,
	options: {
		permissions?: string[];
		failBatch?: boolean;
		pendingCleanup?: boolean;
		splitCycles?: boolean;
	} = {}
) {
	await mockStaffHome(page, {
		permissions: options.permissions ?? [
			PERMISSIONS.BEHAVIOR_READ_SCHOOL,
			PERMISSIONS.BEHAVIOR_CREATE_SCHOOL,
			PERMISSIONS.BEHAVIOR_DELETE_OWN,
			PERMISSIONS.BEHAVIOR_MANAGE_SCHOOL,
			PERMISSIONS.BEHAVIOR_CLEANUP_EXECUTE_SCHOOL,
			PERMISSIONS.ATTENDANCE_READ_SCHOOL
		]
	});
	const reads: string[] = [],
		batches: CreateBehaviorBatch[] = [],
		cleanups: ExecuteBehaviorCleanup[] = [],
		cancellations: CancelBehaviorEntries[] = [];
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			headers: { 'X-CSRF-Token': 'synthetic-csrf' },
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route('**/api/academic/context/options', (route) =>
		reply(route, {
			activeAcademicYearId: year,
			activeAcademicTermId: behaviorTerm,
			years: [
				{
					id: year,
					name: '2569',
					year: 2569,
					status: 'open',
					startDate: '2026-05-01',
					plannedEndDate: '2027-04-30',
					closedOn: null
				}
			],
			terms: [
				{
					id: behaviorTerm,
					academicYearId: year,
					name: '1',
					code: '1',
					sequence: 1,
					status: 'open',
					startDate: '2026-05-01',
					plannedEndDate: '2027-04-30',
					closedOn: null,
					includedInYearResult: true,
					blocksYearClosure: true
				}
			]
		})
	);
	const selector = {
		gradeLevels: [{ id: id(830), label: 'ม.1' }],
		homerooms: [
			{ id: roomA, label: 'ม.1/1' },
			{ id: roomB, label: 'ม.1/2' }
		]
	};
	let fail = options.failBatch ?? false,
		polls = 0;
	await page.route('**/api/behavior/**', async (route) => {
		const url = new URL(route.request().url());
		const path = url.pathname;
		if (route.request().method() === 'GET') reads.push(path);
		if (path.endsWith('/overview'))
			return reply(route, {
				cycles: [cycle('merit'), cycle('deduction')],
				activationDate: '2026-10-01'
			});
		if (path.endsWith('/items')) return reply(route, [item]);
		if (path.endsWith('/students')) {
			const room = url.searchParams.get('homeroomId'),
				search = url.searchParams.get('search') ?? '';
			const filtered = students.filter(
				(s) => (!room || s.homeroomId === room) && `${s.name} ${s.studentCode}`.includes(search)
			);
			return reply(route, {
				items: filtered,
				total: filtered.length,
				rosterToken: 'stable-roster-token',
				options: selector
			});
		}
		if (path.endsWith('/options')) return reply(route, selector);
		if (path.endsWith('/entries')) {
			const batch = url.searchParams.get('batchId');
			const rows = batch ? entries.filter((e) => e.batchId === batch) : entries;
			return reply(route, { items: rows, total: rows.length });
		}
		if (path.endsWith('/summary'))
			return reply(
				route,
				options.splitCycles
					? {
							...summary,
							current: { merit: 20, deduction: 8, net: null },
							deductionCycle: { ...cycle('deduction'), startsOn: '2026-10-10' }
						}
					: summary
			);
		if (path.endsWith('/settings')) return reply(route, settings);
		if (path.endsWith('/rules')) return reply(route, []);
		if (path.endsWith('/cycles')) return reply(route, [cycle('merit'), cycle('deduction')]);
		if (path.endsWith('/batches')) {
			const payload: CreateBehaviorBatch = route.request().postDataJSON();
			batches.push(payload);
			if (fail) {
				fail = false;
				return reply(route, 'รายชื่อเปลี่ยน กรุณาตรวจใหม่', 409);
			}
			return reply(route, {
				id: id(860),
				affectedStudents: payload.studentIds.length,
				side: 'merit',
				pointsPerStudent: 5,
				replayed: false,
				detailsAvailable: true
			});
		}
		if (path.endsWith('/entries/cancel')) {
			cancellations.push(route.request().postDataJSON());
			return reply(route, { cancelled: 1, alreadyCancelled: 0 });
		}
		if (path.endsWith('/preview'))
			return reply(route, {
				token: id(870),
				revision: 1,
				students: 2,
				entries: 3,
				merit: 10,
				deduction: 20,
				current: { merit: 10, deduction: 20, net: -10 },
				history: { merit: 0, deduction: 0, net: 0 },
				lifetime: { merit: 10, deduction: 20, net: -10 }
			});
		if (path.endsWith('/cleanup')) {
			cleanups.push(route.request().postDataJSON());
			return reply(route, {
				id: id(871),
				status: options.pendingCleanup ? 'pending' : 'completed',
				students: 2,
				entriesDeleted: options.pendingCleanup ? 0 : 3,
				merit: 10,
				deduction: 20
			});
		}
		if (path.includes('/cleanup/')) {
			polls++;
			return reply(route, {
				id: id(871),
				status: polls < 2 ? 'pending' : 'completed',
				students: 2,
				entriesDeleted: polls < 2 ? 1 : 3,
				merit: 10,
				deduction: 20
			});
		}
		return reply(route, 'unexpected Behavior endpoint', 404);
	});
	return { reads, batches, cleanups, cancellations };
}
