import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id, year, nextYear } from './staff-home-route-data';
import type {
	CalendarEvent,
	CalendarCategory,
	CalendarTag,
	CreateCalendarEventRequest
} from '../../../src/lib/api/calendar';
export { year, nextYear };
export const categoryId = id(110),
	tagId = id(111),
	eventId = id(112);
export const calendarPath = (academicYearId = year, month = '2026-10') =>
	`/staff/calendar?academicYearId=${academicYearId}&month=${month}`;
export const eventsPath = '/api/calendar/events',
	categoriesPath = '/api/calendar/categories',
	tagsPath = '/api/calendar/tags';
export async function mockCalendar(
	page: Page,
	options: {
		hold?: string;
		holdAt?: number;
		fail?: string;
		failAt?: number;
		permissions?: string[];
	} = {}
) {
	const base = await mockStaffHome(page, {
		permissions: options.permissions ?? ['calendar.read.school', 'calendar.manage.school']
	});
	const timestamp = '2026-10-01T00:00:00Z';
	let categories: CalendarCategory[] = [
		{
			id: categoryId,
			name: 'หมวดแรก',
			color: '#2563eb',
			orderIndex: 0,
			isActive: true,
			createdAt: timestamp,
			updatedAt: timestamp
		}
	];
	let tags: CalendarTag[] = [
		{ id: tagId, name: 'แท็กแรก', createdAt: timestamp, updatedAt: timestamp }
	];
	const makeEvent = (selectedYear: string, november = false): CalendarEvent => ({
		id: eventId,
		categoryId: categories[0]?.id ?? null,
		categoryName: categories[0]?.name ?? null,
		categoryColor: categories[0]?.color ?? null,
		title:
			selectedYear === nextYear ? 'กิจกรรมปีใหม่' : november ? 'กิจกรรมพฤศจิกายน' : 'กิจกรรมแรก',
		description: null,
		location: null,
		startDate: november ? '2026-11-01' : '2026-10-01',
		endDate: november ? '2026-11-01' : '2026-10-01',
		startTime: null,
		endTime: null,
		allDay: true,
		isPublic: true,
		targets: [{ id: id(113), audienceType: 'all', gradeLevelId: null, homeroomId: null }],
		tags: tags.map((tag) => ({ id: tag.id, name: tag.name })),
		reminders: [],
		createdBy: null,
		updatedBy: null,
		createdAt: timestamp,
		updatedAt: timestamp
	});
	const counts = new Map<string, number>(),
		reads: URL[] = [],
		writes: { path: string; method: string }[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	let deleted = false,
		savedTitle = '';
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route(
		(url) =>
			url.pathname.startsWith('/api/calendar/') ||
			url.pathname === '/api/lookup/grade-levels' ||
			url.pathname === '/api/academic/homerooms',
		async (route) => {
			const request = route.request(),
				url = new URL(request.url()),
				path = url.pathname;
			const kind =
				request.method() !== 'GET'
					? 'mutation'
					: path === eventsPath
						? 'events'
						: path === categoriesPath
							? 'categories'
							: path === tagsPath
								? 'tags'
								: 'options';
			if (request.method() === 'GET') {
				counts.set(path, (counts.get(path) ?? 0) + 1);
				reads.push(url);
			} else writes.push({ path, method: request.method() });
			const ordinal = counts.get(path) ?? 1;
			const selectedYear = url.searchParams.get('academicYearId') ?? year;
			const eventSnapshot = makeEvent(
				selectedYear,
				(url.searchParams.get('from') ?? '').startsWith('2026-11')
			);
			if (savedTitle) eventSnapshot.title = savedTitle;
			if (options.hold === kind && ordinal === (options.holdAt ?? 1) && selectedYear !== nextYear)
				await held;
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, `region ${kind} ไม่พร้อม`, 503);
			if (kind === 'events')
				return reply(
					route,
					deleted ||
						((url.searchParams.get('q') ?? '') &&
							![eventSnapshot.title, ...eventSnapshot.tags.map((tag) => tag.name)]
								.join(' ')
								.includes(url.searchParams.get('q')!))
						? []
						: [eventSnapshot]
				);
			if (kind === 'categories') return reply(route, categories);
			if (kind === 'tags') return reply(route, tags);
			if (kind === 'options')
				return reply(
					route,
					path === '/api/calendar/target-options' ? { gradeLevels: [], homerooms: [] } : []
				);
			if (path.startsWith(eventsPath)) {
				if (request.method() === 'DELETE') {
					deleted = true;
					return reply(route, {});
				}
				const payload = request.postDataJSON();
				savedTitle = payload.title;
				return reply(route, {
					...makeEvent(payload.academicYearId),
					...payload,
					title: savedTitle,
					targets: payload.targets.map((target: object) => ({ id: id(113), ...target }))
				});
			}
			if (path.startsWith(categoriesPath)) {
				if (request.method() === 'DELETE') {
					categories = [];
					return reply(route, {});
				}
				categories = [{ ...categories[0], ...request.postDataJSON() }];
				return reply(route, categories[0]);
			}
			if (path.startsWith(tagsPath)) {
				if (request.method() === 'DELETE') {
					tags = [];
					return reply(route, {});
				}
				tags = [{ ...tags[0], ...request.postDataJSON() }];
				return reply(route, tags[0]);
			}
			return reply(route, {});
		}
	);
	return { ...base, release, reads, writes, count: (path: string) => counts.get(path) ?? 0 };
}

export function makeApprovedCalendarEvent(payload: CreateCalendarEventRequest): CalendarEvent {
	const timestamp = '2026-10-01T02:00:00Z';
	return {
		id: id(145),
		categoryId: payload.categoryId ?? null,
		categoryName: payload.categoryId ? 'หมวดแรก' : null,
		categoryColor: payload.categoryId ? '#2563eb' : null,
		title: payload.title,
		description: payload.description ?? null,
		location: payload.location ?? null,
		startDate: payload.startDate,
		endDate: payload.endDate,
		startTime: payload.startTime ?? null,
		endTime: payload.endTime ?? null,
		allDay: payload.allDay,
		isPublic: payload.isPublic,
		targets: payload.targets.map((target, index) => ({
			id: id(150 + index),
			audienceType: target.audienceType,
			gradeLevelId: target.gradeLevelId ?? null,
			homeroomId: target.homeroomId ?? null
		})),
		tags: (payload.tagIds ?? []).map((tagId) => ({ id: tagId, name: 'แท็กแรก' })),
		reminders: [],
		createdBy: null,
		updatedBy: null,
		createdAt: timestamp,
		updatedAt: timestamp
	};
}
