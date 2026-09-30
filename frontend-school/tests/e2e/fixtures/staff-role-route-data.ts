import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id } from './staff-home-route-data';
export const firstRole = id(40),
	secondRole = id(41);
export const rolePath = (roleId = firstRole) => `/staff/roles/${roleId}`;
export async function mockStaffRoles(
	page: Page,
	options: {
		hold?: 'list' | 'role' | 'catalog' | 'mutation';
		holdAt?: number;
		fail?: 'list' | 'role' | 'catalog';
		failAt?: number;
		permissions?: string[];
		inactive?: boolean;
		signalPermissionChange?: boolean;
	} = {}
) {
	const permissionCodes = options.permissions ?? [
		'roles.read.all',
		'roles.create.all',
		'roles.update.all',
		'roles.delete.all',
		'settings.read.all'
	];
	const base = await mockStaffHome(page, { permissions: permissionCodes });
	let releasePermissions = () => {};
	const permissionSignal = new Promise<void>((resolve) => (releasePermissions = resolve));
	if (options.signalPermissionChange)
		await page.route(
			(url) => url.pathname === '/api/notifications/stream',
			async (route) => {
				await permissionSignal;
				return route.fulfill({
					status: 200,
					contentType: 'text/event-stream',
					body: 'event: permission_changed\ndata: {}\n\nretry: 3600000\n\n'
				});
			}
		);

	const counts = new Map<string, number>(),
		writes: { method: string; resource: string; payload: unknown }[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	const records = new Map(
		[firstRole, secondRole].map((key, index) => [
			key,
			{
				id: key,
				code: `ROLE_${index}`,
				name: index ? 'บทบาทที่สอง' : 'บทบาทแรก',
				name_en: '',
				description: '',
				user_type: 'staff',
				level: 10,
				is_active: !options.inactive,
				is_system: false,
				permissions: ['staff_profile.read.own'],
				created_at: '2026-09-01T00:00:00Z',
				updated_at: '2026-09-01T00:00:00Z'
			}
		])
	);
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route(
		(url) => url.pathname.startsWith('/api/roles') || url.pathname === '/api/permissions/modules',
		async (route) => {
			const resource = new URL(route.request().url()).pathname,
				method = route.request().method();
			const key = resource.split('/').at(-1)!;
			const kind =
				method !== 'GET'
					? 'mutation'
					: resource === '/api/roles'
						? 'list'
						: resource === '/api/permissions/modules'
							? 'catalog'
							: 'role';
			if (method === 'GET') counts.set(resource, (counts.get(resource) ?? 0) + 1);
			else writes.push({ method, resource, payload: route.request().postDataJSON() });
			const snapshot =
				resource === '/api/roles'
					? [...records.values()].map((record) => ({ ...record }))
					: records.get(key)
						? { ...records.get(key)! }
						: null;
			if (
				options.hold === kind &&
				(!options.holdAt || counts.get(resource) === options.holdAt) &&
				(kind !== 'role' || key === firstRole)
			)
				await held;
			if (options.fail === kind && counts.get(resource) === (options.failAt ?? 1))
				return reply(route, 'region บทบาทไม่พร้อม', 503);
			if (method !== 'GET') {
				if (method === 'POST') {
					records.set(
						id(42),
						Object.assign({}, records.get(firstRole)!, route.request().postDataJSON(), {
							id: id(42)
						})
					);
					return reply(route, { id: id(42) });
				}
				if (method === 'DELETE') records.get(key)!.is_active = false;
				else Object.assign(records.get(key)!, route.request().postDataJSON());
				return reply(route, {});
			}
			if (kind === 'catalog')
				return reply(route, {
					staff_profile: [
						{
							id: id(43),
							code: 'staff_profile.read.own',
							name: 'อ่านโปรไฟล์ตนเอง',
							module: 'staff_profile',
							action: 'read',
							scope: 'own',
							description: null,
							created_at: '2026-09-01T00:00:00Z'
						}
					]
				});
			return reply(route, snapshot);
		}
	);
	return {
		release,
		signalPermissions: (next: string[]) => {
			permissionCodes.splice(0, permissionCodes.length, ...next);
			releasePermissions();
		},
		writes,
		count: (resource: string) => counts.get(resource) ?? base.count(resource)
	};
}
