import type { Page, Route } from '@playwright/test';
import type { AcademicContextOptionsResponse } from '../../../src/lib/api/academic-context';
import type { TimetableBlock } from '../../../src/lib/api/timetable';
import type { PersonalExamScheduleRound } from '../../../src/lib/api/examSchedule';
import type { CalendarViewerEvent } from '../../../src/lib/api/calendar';
import type { StudentActivityOffering } from '../../../src/lib/api/student-activities';
import { mockSelfProfile, contextPath, year, nextYear } from './self-profile-route-data';
import { id } from './staff-home-route-data';
export { year, nextYear, contextPath };
export const term = id(401),
	secondTerm = id(402),
	nextYearTerm = id(403),
	group = id(404),
	offering = id(405);
export const resources = {
	timetable: '/api/me/timetable',
	exams: '/api/me/exam-schedules',
	calendar: '/api/me/calendar/events',
	activities: '/api/me/activity-registrations'
};
export type Resource = keyof typeof resources;
export const scheduleRoute = (resource: Resource, selected = term) =>
	`/student/${resource}?academicYearId=${year}${resource === 'calendar' ? '&month=2026-10' : `&academicTermId=${selected}`}`;
export const academicOptions: AcademicContextOptionsResponse = {
	activeAcademicYearId: year,
	activeAcademicTermId: term,
	years: [year, nextYear].map((id, i) => ({
		id,
		year: 2569 + i,
		name: String(2569 + i),
		status: 'active',
		startDate: '2026-05-01',
		endDate: '2027-04-30'
	})),
	terms: [term, secondTerm, nextYearTerm].map((id, i) => ({
		id,
		academicYearId: i === 2 ? nextYear : year,
		name: i === 0 ? 'ภาคหนึ่ง' : i === 1 ? 'ภาคสอง' : 'ภาคปีใหม่',
		code: String(i + 1),
		sequence: i + 1,
		status: 'active',
		termType: 'regular',
		startDate: '2026-05-01',
		plannedEndDate: '2027-04-30',
		closedOn: null,
		includedInYearResult: true,
		blocksYearClosure: true
	}))
};
export async function mockStudentSchedules(
	page: Page,
	options: {
		hold?: Resource | 'context' | 'save';
		holdAt?: number;
		fail?: Resource | 'context';
		failAt?: number;
		emptyTerms?: boolean;
	} = {}
) {
	const base = await mockSelfProfile(page);
	const reads: URL[] = [],
		writes: string[] = [],
		counts = new Map<string, number>();
	let release = () => {};
	const held = new Promise<void>((r) => (release = r));
	const enrolled = new Set<string>();
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			json: status < 400 ? { success: true, data } : { success: false, error: data }
		});
	await page.route(
		(url) =>
			url.pathname === contextPath ||
			Object.values(resources).some(
				(resource) => url.pathname === resource || url.pathname.startsWith(resource + '/')
			),
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method();
			const resource = Object.keys(resources).find((k) =>
				path.startsWith(resources[k as Resource])
			) as Resource | undefined;
			const kind =
				method !== 'GET' ? 'save' : path === contextPath ? 'context' : (resource ?? 'unknown');
			const ordinal = (counts.get(kind) ?? 0) + 1;
			counts.set(kind, ordinal);
			if (method === 'GET') reads.push(url);
			else writes.push(`${method} ${path}`);
			const selected = url.searchParams.get('academicTermId') ?? term,
				label = selected === secondTerm ? 'ภาคสอง' : 'ภาคหนึ่ง';
			const dateFrom = url.searchParams.get('from') ?? '2026-10-01',
				middle = new Date(`${dateFrom}T00:00:00Z`);
			middle.setUTCDate(middle.getUTCDate() + 15);
			const month = middle.toISOString().slice(0, 7);
			const blocks: TimetableBlock[] = [
				{
					id: id(410),
					academicYearId: year,
					academicTermId: selected,
					bellScheduleId: id(411),
					bellSchedulePeriodId: id(412),
					blockKind: 'course',
					createdAt: '2026-10-01T00:00:00Z',
					updatedAt: '2026-10-01T00:00:00Z',
					dayOfWeek: 'MON',
					startTime: '08:00:00',
					endTime: '09:00:00',
					groups: [],
					homerooms: [],
					teachers: [],
					syncStates: [],
					isActive: true,
					learningOfferingId: offering,
					offeringCode: 'SELF-01',
					offeringName: `วิชา${label}`,
					periodName: 'คาบแรก',
					rowVersion: 1,
					schedulingMode: null,
					seriesId: null,
					structuralKind: null,
					timetableVersionId: id(413),
					title: null,
					note: null
				}
			];
			const rounds: PersonalExamScheduleRound[] = [
				{
					roundId: id(414),
					roundName: `รอบ${label}`,
					academicTermId: selected,
					publishedAt: '2026-10-01T00:00:00Z',
					sessions: [
						{
							examDate: '2026-10-01',
							startsAt: '08:00:00',
							endsAt: '09:00:00',
							subjectName: `วิชา${label}`,
							assessmentPhaseName: 'ปลายภาค',
							homeroomName: 'ห้องแรก',
							roomName: 'ห้องสอบ',
							buildingName: null,
							seatNumber: '1'
						}
					]
				}
			];
			const events: CalendarViewerEvent[] = [
				{
					id: id(415),
					title: `กิจกรรม ${month}`,
					allDay: true,
					categoryColor: null,
					categoryId: null,
					categoryName: null,
					createdAt: '2026-10-01T00:00:00Z',
					updatedAt: '2026-10-01T00:00:00Z',
					description: null,
					startDate: `${month}-01`,
					endDate: `${month}-01`,
					startTime: null,
					endTime: null,
					isPublic: false,
					location: null,
					tags: []
				}
			];
			const offerings: StudentActivityOffering[] = [
				{
					id: offering,
					academicYearId: year,
					academicTermId: selected,
					activityType: 'club',
					code: 'CLUB',
					name: `กิจกรรม${label}`,
					enrolledGroupId: enrolled.has(selected) ? group : null,
					groups: [
						{
							id: group,
							code: 'G1',
							name: 'กลุ่มแรก',
							capacity: 30,
							description: null,
							enrolled: enrolled.has(selected),
							memberCount: enrolled.has(selected) ? 1 : 0,
							registrationOpen: true,
							teacherNames: ['ครูทดสอบ']
						}
					]
				}
			];
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, 'ส่วนนี้ไม่พร้อม', 503);
			if (kind === 'context')
				return reply(route, {
					...academicOptions,
					terms: options.emptyTerms ? [] : academicOptions.terms
				});
			if (kind === 'save') {
				if (method === 'POST') enrolled.add(selected);
				else enrolled.delete(selected);
				return reply(route, {
					enrolled: method === 'POST',
					learningGroupId: group,
					learningOfferingId: offering,
					revision: 2,
					studentAcademicYearId: id(416)
				});
			}
			return reply(
				route,
				resource === 'timetable'
					? blocks
					: resource === 'exams'
						? rounds
						: resource === 'calendar'
							? events
							: offerings
			);
		}
	);
	return {
		...base,
		reads,
		writes,
		release,
		count: (kind: string) => counts.get(kind) ?? 0,
		selfProfileCount: () => base.count('profile')
	};
}
