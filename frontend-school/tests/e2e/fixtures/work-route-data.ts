import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id, firstStaff } from './staff-home-route-data';
import type { WorkItem, WorkflowWindow } from '../../../src/lib/api/work';
export const windowId = id(100),
	secondWindowId = id(101);
export const managePermission = 'learning_offering.manage.school';
export const itemsPath = '/api/me/work-items',
	countsPath = `${itemsPath}/counts`,
	windowsPath = '/api/me/workflow-windows/manageable';
export async function mockWork(
	page: Page,
	options: {
		hold?: string;
		holdAt?: number;
		fail?: string;
		failAt?: number;
		permissions?: string[];
		event?: string;
	} = {}
) {
	const base = await mockStaffHome(page, {
		permissions: options.permissions ?? [managePermission]
	});
	let releaseEvent = () => {};
	const signal = new Promise<void>((resolve) => (releaseEvent = resolve));
	if (options.event)
		await page.route(
			(url) => url.pathname === '/api/notifications/stream',
			async (route) => {
				await signal;
				return route.fulfill({
					status: 200,
					contentType: 'text/event-stream',
					body: `event: ${options.event}\ndata: {}\n\nretry: 3600000\n\n`
				});
			}
		);
	const now = '2026-09-01T00:00:00Z';
	let windows: WorkflowWindow[] = [windowId, secondWindowId].map((key, index) => ({
		id: key,
		moduleCode: 'learning_offering',
		workflowCode: `code-${index}`,
		title: index ? 'รอบงานสอง' : 'รอบงานแรก',
		managedByPermission: managePermission,
		status: 'draft',
		timeState: 'draft',
		metadata: { tags: [] },
		createdAt: now,
		updatedAt: now
	}));
	const item = (title: string): WorkItem => ({
		id: id(102),
		workflowWindowId: windowId,
		moduleCode: 'learning_offering',
		workflowCode: 'code',
		sourceResourceType: 'manual',
		title,
		actionPath: '/staff/work',
		itemStatus: 'active',
		assigneeId: id(103),
		assigneeType: 'user',
		assigneeStatus: 'assigned',
		state: 'open',
		metadata: { tags: [] },
		createdAt: now,
		updatedAt: now
	});
	const counts = new Map<string, number>(),
		writes: { path: string; payload: unknown }[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route(
		(url) =>
			url.pathname.startsWith('/api/me/work') ||
			url.pathname.startsWith('/api/workflow-windows') ||
			url.pathname === '/api/work-items' ||
			url.pathname === '/api/lookup/staff' ||
			url.pathname === '/api/lookup/organization-units',
		async (route) => {
			const request = route.request(),
				url = new URL(request.url()),
				path = url.pathname;
			const kind =
				request.method() !== 'GET'
					? 'mutation'
					: path === itemsPath
						? 'items'
						: path === countsPath
							? 'counts'
							: path === windowsPath
								? 'windows'
								: path.endsWith('/staff')
									? 'staff'
									: 'units';
			if (request.method() === 'GET') counts.set(path, (counts.get(path) ?? 0) + 1);
			else writes.push({ path, payload: request.postDataJSON() });
			const ordinal = counts.get(path) ?? 1;
			const snapshot =
				kind === 'items'
					? [item(ordinal === 1 ? 'งานแรก' : 'งานปรับปรุง')]
					: windows.map((window) => ({ ...window }));
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, `region ${kind} ไม่พร้อม`, 503);
			if (kind === 'counts')
				return reply(route, { open: 1, dueSoon: 0, overdue: 0, submitted: 0, closed: 0, total: 1 });
			if (kind === 'items' || kind === 'windows') return reply(route, { items: snapshot });
			if (kind === 'staff')
				return reply(route, [{ id: firstStaff, name: 'ผู้รับงาน', title: null }]);
			if (kind === 'units')
				return reply(route, [
					{
						id: id(104),
						code: 'UNIT',
						name: 'ฝ่ายงาน',
						nameEn: null,
						description: null,
						category: 'academic',
						displayOrder: 0,
						isActive: true,
						parentUnitId: null,
						unitType: 'division',
						subjectGroupId: null
					}
				]);
			if (path === '/api/workflow-windows') {
				const payload = request.postDataJSON();
				const created: WorkflowWindow = { ...windows[0], ...payload, id: id(105) };
				windows = [created, ...windows];
				return reply(route, created);
			}
			if (path.startsWith('/api/workflow-windows/')) {
				const updated = {
					...windows.find((window) => path.endsWith(window.id))!,
					...request.postDataJSON()
				};
				windows = windows.map((window) => (window.id === updated.id ? updated : window));
				return reply(route, updated);
			}
			return reply(route, { id: id(106) });
		}
	);
	return { ...base, release, releaseEvent, writes, count: (path: string) => counts.get(path) ?? 0 };
}
