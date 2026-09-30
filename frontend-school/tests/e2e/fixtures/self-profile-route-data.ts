import type { Page, Route } from '@playwright/test';
import type { Student } from '../../../src/lib/api/students';
import { mockStaffHome, year, nextYear, actor } from './staff-home-route-data';
export { year, nextYear };
export const contextPath = '/api/me/academic-context/options',
	profilePath = '/api/student/profile';
export const studentRoute = (route = '/student', selected = year) =>
	`${route}?academicYearId=${selected}`;
export async function mockSelfProfile(
	page: Page,
	options: {
		hold?: 'context' | 'profile' | 'save';
		holdAt?: number;
		fail?: 'context' | 'profile' | 'save';
		failAt?: number;
		empty?: boolean;
	} = {}
) {
	const base = await mockStaffHome(page, { userType: 'student', permissions: [] });
	const reads: URL[] = [],
		writes: string[] = [],
		counts = new Map<string, number>();
	let release = () => {};
	const held = new Promise<void>((r) => (release = r));
	let nickname = 'ชื่อเล่นเดิม';
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			json: status < 400 ? { success: true, data } : { success: false, error: data }
		});
	let loggedOut = false;
	await page.route(
		(url) => ['/api/auth/me', '/api/auth/logout'].includes(url.pathname),
		async (route) => {
			if (new URL(route.request().url()).pathname === '/api/auth/logout') {
				loggedOut = true;
				return reply(route, null);
			}
			if (loggedOut) return reply(route, 'เข้าสู่ระบบใหม่', 401);
			return route.fallback();
		}
	);
	await page.route(
		(url) => url.pathname === contextPath || url.pathname === profilePath,
		async (route) => {
			const url = new URL(route.request().url()),
				method = route.request().method();
			const kind = method !== 'GET' ? 'save' : url.pathname === contextPath ? 'context' : 'profile';
			const ordinal = (counts.get(kind) ?? 0) + 1;
			counts.set(kind, ordinal);
			if (method === 'GET') reads.push(url);
			else writes.push(url.pathname);
			const student: Student = {
				id: actor,
				first_name: 'นักเรียน',
				last_name: 'ทดสอบ',
				username: 'synthetic-self',
				title: null,
				email: null,
				phone: null,
				address: null,
				gender: null,
				date_of_birth: null,
				blood_type: null,
				allergies: null,
				medical_conditions: null,
				nickname,
				national_id: null,
				profile_image_file_id: null,
				status: 'active',
				student_id: 'TEST-01',
				student_number: 1,
				grade_level: 'ม.1',
				homeroom:
					url.searchParams.get('academicYearId') === nextYear ? 'ห้องปีถัดไป' : 'ห้องปีเดิม',
				parents: []
			};
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, 'ส่วนนี้ไม่พร้อม', 503);
			if (kind === 'context')
				return reply(route, {
					activeAcademicYearId: year,
					activeAcademicTermId: null,
					years: options.empty
						? []
						: [year, nextYear].map((id, i) => ({
								id,
								year: 2569 + i,
								name: String(2569 + i),
								status: 'open',
								startDate: '2026-05-01',
								plannedEndDate: '2027-04-30',
								closedOn: null
							})),
					terms: []
				});
			if (kind === 'save') {
				nickname = route.request().postDataJSON().nickname;
				return reply(route, null);
			}
			return reply(route, student);
		}
	);
	return {
		...base,
		identityCount: () => base.count('/api/auth/me'),
		reads,
		writes,
		release,
		count: (kind: string) => counts.get(kind) ?? 0
	};
}
