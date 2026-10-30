import type { BrowserContext, Page, Route } from '@playwright/test';
import type { SessionState, CurrentUserDto } from '../../../src/lib/api/auth.js';
import { sessionState } from './session-state';
import { academicOptions, term, year } from './student-schedules-route-data';
import { id } from './staff-home-route-data';

export const activityStart = Date.parse('2026-10-29T00:00:00Z');
export const childId = id(501);
export const activityRoute = (role: 'staff' | 'student' | 'parent') =>
	`${role === 'parent' ? `/parent/student/${childId}` : `/${role}`}/timetable?academicYearId=${year}&academicTermId=${term}`;

export async function mockSessionActivity(
	context: BrowserContext,
	role: 'staff' | 'student' | 'parent' = 'staff',
	overrides: Partial<SessionState> = {}
) {
	let now = activityStart;
	let session = sessionState(now, overrides);
	let activityStatus = 200;
	let release: (() => void) | null = null;
	let hold = false;
	let clockInstalled = false;
	const loginPages = new WeakSet<Page>();
	const requests: { path: string; method: string; page: Page; body: string | null }[] = [];
	const user = (): CurrentUserDto => ({
		id: id(10),
		username: 'session-fixture',
		firstName: 'ผู้ใช้',
		lastName: 'ทดสอบ',
		displayName: 'ผู้ใช้ ทดสอบ',
		userType: role,
		status: 'active',
		primaryRoleName: null,
		profileImageFileId: null,
		permissions: [],
		session
	});
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			headers: { 'X-CSRF-Token': 'synthetic-session-csrf', 'Retry-After': '20' },
			json:
				status < 400 ? { success: true, data } : { success: false, error: 'fixture unavailable' }
		});
	await context.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
	await context.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const request = route.request(),
				path = new URL(request.url()).pathname,
				page = request.frame().page();
			requests.push({ path, method: request.method(), page, body: request.postData() });
			if (path === '/api/auth/login') {
				loginPages.delete(page);
				session = sessionState(now, { sessionId: '01900000-0000-7000-8000-000000000002' });
				return reply(route, { user: user() });
			}
			if (path === '/api/auth/me') return reply(route, user(), loginPages.has(page) ? 401 : 200);
			if (path === '/api/auth/activity') {
				if (hold)
					await new Promise<void>((resolve) => {
						release = resolve;
					});
				if (activityStatus !== 200) return reply(route, null, activityStatus);
				if (request.postDataJSON().expectedSessionId !== session.sessionId)
					return reply(route, null, 409);
				session = sessionState(now, {
					sessionId: session.sessionId,
					absoluteExpiresAt: session.absoluteExpiresAt
				});
				return reply(route, session);
			}
			if (path === '/api/notifications/stream')
				return route.fulfill({
					contentType: 'text/event-stream',
					body: ': connected\nretry: 3600000\n\n'
				});
			if (path.endsWith('/academic-context/options') || path === '/api/academic/context/options')
				return reply(route, academicOptions);
			if (path.endsWith('/timetable')) return reply(route, []);
			if (path === `/api/parent/students/${childId}`)
				return reply(route, {
					id: childId,
					first_name: 'นักเรียน',
					last_name: 'ทดสอบ',
					status: 'active',
					student_id: 'TEST-01',
					student_number: 1,
					grade_level: 'ม.1',
					homeroom: 'ห้องหนึ่ง',
					parents: []
				});
			if (path === '/api/menu/user') return reply(route, { groups: [] });
			if (path === '/api/me/work-items/counts')
				return reply(route, { open: 0, dueSoon: 0, overdue: 0, submitted: 0, closed: 0, total: 0 });
			if (path === '/api/notifications') return reply(route, { items: [], unread_count: 0 });
			if (path === '/api/school/settings') return reply(route, null, 403);
			return reply(route, {});
		}
	);
	return {
		requests,
		count(path: string, page?: Page) {
			return requests.filter((request) => request.path === path && (!page || request.page === page))
				.length;
		},
		setNow(value: number) {
			now = activityStart + value;
		},
		status(value: number) {
			activityStatus = value;
		},
		hold() {
			hold = true;
		},
		release() {
			hold = false;
			release?.();
			release = null;
		},
		requireLogin(page: Page) {
			loginPages.add(page);
		},
		async open(page: Page) {
			if (!clockInstalled) {
				await page.clock.install({ time: activityStart });
				await page.clock.pauseAt(activityStart);
				clockInstalled = true;
			}
			await page.goto(activityRoute(role));
			await page.clock.runFor(100);
			await page.getByRole('heading', { name: role === 'staff' ? 'ยังไม่มีตารางสอน' : 'ยังไม่มีตารางเรียน', exact: true }).waitFor();
		}
	};
}
