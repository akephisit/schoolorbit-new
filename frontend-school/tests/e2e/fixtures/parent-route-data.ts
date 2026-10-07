import type { Page, Route } from '@playwright/test';
import type { Student } from '../../../src/lib/api/students';
import type { ParentProfile } from '../../../src/lib/api/parents';
import type { TimetableBlock } from '../../../src/lib/api/timetable';
import type { PersonalExamScheduleRound } from '../../../src/lib/api/examSchedule';
import type { CalendarViewerEvent } from '../../../src/lib/api/calendar';
import { mockStaffHome, id, actor, year, nextYear } from './staff-home-route-data';
import { academicOptions, term, secondTerm } from './student-schedules-route-data';
export { year, nextYear, term, secondTerm };
export const child = id(501),
	otherChild = id(502),
	deniedChild = id(503);
export type Region = 'context' | 'profile' | 'timetable' | 'calendar' | 'exams';
export const childRoute = (resource = '', studentId = child) =>
	`/parent/student/${studentId}${resource ? '/' + resource : ''}?academicYearId=${year}${['timetable', 'exams'].includes(resource) ? '&academicTermId=' + term : resource === 'calendar' ? '&month=2026-10' : ''}`;
export async function mockParent(
	page: Page,
	options: { hold?: Region; holdAt?: number; fail?: Region; failAt?: number; denied?: boolean } = {}
) {
	const base = await mockStaffHome(page, { userType: 'parent', permissions: [] });
	const reads: URL[] = [],
		writes: string[] = [],
		counts = new Map<string, number>();
	let release = () => {};
	const held = new Promise<void>((r) => (release = r));
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			json: status < 400 ? { success: true, data } : { success: false, error: data }
		});
	let loggedOut = false;
	await page.route(
		(url) => ['/api/auth/me', '/api/auth/logout'].includes(url.pathname),
		async (route) => {
			if (new URL(route.request().url()).pathname.endsWith('/logout')) {
				loggedOut = true;
				return reply(route, null);
			}
			if (loggedOut) return reply(route, 'เข้าสู่ระบบใหม่', 401);
			return route.fallback();
		}
	);
	await page.route(
		(url) => url.pathname.startsWith('/api/parent/'),
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname;
			const studentId = path.split('/')[4] ?? child;
			const kind: Region = path.endsWith('/options')
				? 'context'
				: path.endsWith('/timetable')
					? 'timetable'
					: path.endsWith('/calendar/events')
						? 'calendar'
						: path.endsWith('/exam-schedules')
							? 'exams'
							: 'profile';
			const ordinal = (counts.get(kind) ?? 0) + 1;
			counts.set(kind, ordinal);
			if (route.request().method() === 'GET') reads.push(url);
			else writes.push(path);
			const suffix = studentId === otherChild ? 'คนสอง' : 'คนแรก';
			const name = `นักเรียน${suffix}`,
				room = url.searchParams.get('academicYearId') === nextYear ? 'ห้องปีถัดไป' : 'ห้องปีเดิม';
			const selected = url.searchParams.get('academicTermId') ?? term;
			const label = selected === secondTerm ? 'ภาคสอง' : 'ภาคหนึ่ง';
			const from = new Date((url.searchParams.get('from') ?? '2026-10-01') + 'T00:00:00Z');
			from.setUTCDate(from.getUTCDate() + 15);
			const month = from.toISOString().slice(0, 7);
			const student: Student = {
				id: studentId,
				first_name: name,
				last_name: 'ทดสอบ',
				username: 'synthetic-child',
				title: null,
				email: null,
				phone: null,
				address: null,
				gender: null,
				date_of_birth: null,
				blood_type: null,
				allergies: null,
				medical_conditions: null,
				nickname: null,
				national_id: null,
				profile_image_file_id: null,
				status: 'active',
				student_id: 'TEST-CHILD',
				student_number: 1,
				grade_level: 'ม.1',
				homeroom: room,
				parents: []
			};
			const profile: ParentProfile = {
				id: actor,
				username: 'synthetic-parent',
				first_name: 'ผู้ปกครอง',
				last_name: 'ทดสอบ',
				title: null,
				phone: null,
				email: null,
				national_id: null,
				children: [child, otherChild].map((id, i) => ({
					id,
					first_name: i ? 'นักเรียนคนสอง' : 'นักเรียนคนแรก',
					last_name: 'ทดสอบ',
					student_id: 'TEST-CHILD',
					grade_level: 'ม.1',
					homeroom: room,
					profile_image_file_id: null,
					relationship: 'parent'
				}))
			};
			const blocks: TimetableBlock[] = [
				{
					id: id(510),
					academicYearId: year,
					academicTermId: selected,
					bellScheduleId: id(511),
					bellSchedulePeriodId: id(512),
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
					learningOfferingId: id(513),
					offeringCode: 'CHILD-01',
					offeringName: `วิชา${suffix}${label}`,
					periodName: 'คาบแรก',
					rowVersion: 1,
					schedulingMode: null,
					seriesId: null,
					structuralKind: null,
					timetableVersionId: id(514),
					title: null,
					note: null
				}
			];
			const rounds: PersonalExamScheduleRound[] = [
				{
					roundId: id(515),
					roundName: `รอบ${suffix}${label}`,
					academicTermId: selected,
					publishedAt: '2026-10-01T00:00:00Z',
					sessions: [
						{
							examDate: '2026-10-01',
							startsAt: '08:00:00',
							endsAt: '09:00:00',
							subjectName: 'วิชาลูก',
							assessmentPhaseName: 'ปลายภาค',
							homeroomName: room,
							roomName: 'ห้องสอบ',
							buildingName: null,
							seatNumber: '1'
						}
					]
				}
			];
			const events: CalendarViewerEvent[] = [
				{
					id: id(516),
					title: `กิจกรรม${suffix} ${month}`,
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
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.denied || studentId === deniedChild)
				return reply(route, 'ไม่มีสิทธิ์เข้าถึงลูกคนนี้', 403);
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, 'ส่วนนี้ไม่พร้อม', 503);
			return reply(
				route,
				kind === 'context'
					? academicOptions
					: kind === 'timetable'
						? blocks
						: kind === 'exams'
							? rounds
							: kind === 'calendar'
								? events
								: path === '/api/parent/profile'
									? profile
									: student
			);
		}
	);
	return { ...base, reads, writes, release, count: (kind: Region) => counts.get(kind) ?? 0 };
}
