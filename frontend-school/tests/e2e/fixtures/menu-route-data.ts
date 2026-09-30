import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id } from './staff-home-route-data';
import type {
	MenuWorkspace,
	MenuGroup,
	MenuItem,
	AcademicMenuTemplatePreview
} from '../../../src/lib/api/menu-admin';
export const workspacePath = '/api/admin/menu/workspaces',
	groupPath = '/api/admin/menu/groups',
	itemPath = '/api/admin/menu/items',
	previewPath = '/api/admin/menu/templates/academic/recommended';
export async function mockMenu(
	page: Page,
	options: { hold?: string; holdAt?: number; fail?: string; permissions?: string[] } = {}
) {
	const base = await mockStaffHome(page, {
		permissions: options.permissions ?? [
			'menu.read.all',
			'menu.create.all',
			'menu.update.all',
			'menu.delete.all'
		]
	});
	let workspaces: MenuWorkspace[] = [
		{
			id: id(220),
			code: 'operations',
			name: 'กลุ่มบริหารทั่วไป',
			name_en: null,
			icon: 'School',
			display_order: 1,
			is_active: true
		},
		{
			id: id(221),
			code: 'academic',
			name: 'กลุ่มวิชาการ',
			name_en: null,
			icon: 'School',
			display_order: 2,
			is_active: true
		}
	];
	let groups: MenuGroup[] = [
		{
			id: id(222),
			code: 'other',
			name: 'อื่น ๆ',
			name_en: null,
			icon: 'School',
			workspace_code: 'operations',
			display_order: 1,
			is_active: true
		},
		{
			id: id(223),
			code: 'teaching',
			name: 'งานสอน',
			name_en: null,
			icon: 'School',
			workspace_code: 'academic',
			display_order: 1,
			is_active: true
		}
	];
	let items: MenuItem[] = [
		{
			id: id(224),
			code: 'lesson',
			name: 'เมนูการสอน',
			name_en: null,
			icon: 'School',
			path: '/staff/academic/teaching',
			required_permission: null,
			user_type: 'staff',
			group_id: id(223),
			parent_id: null,
			display_order: 1,
			is_active: true
		},
		{
			id: id(225),
			code: 'activity',
			name: 'เมนูกิจกรรม',
			name_en: null,
			icon: 'School',
			path: '/student/activities',
			required_permission: null,
			user_type: 'student',
			group_id: id(222),
			parent_id: null,
			display_order: 1,
			is_active: true
		}
	];
	const counts = new Map<string, number>(),
		writes: string[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			json: status < 400 ? { success: true, data } : { success: false, error: data }
		});
	await page.route(
		(url) => url.pathname.startsWith('/api/admin/menu/'),
		async (route) => {
			const path = new URL(route.request().url()).pathname,
				method = route.request().method(),
				kind = method === 'GET' ? path : 'mutation',
				ordinal = (counts.get(kind) ?? 0) + 1;
			counts.set(kind, ordinal);
			if (method !== 'GET') writes.push(path);
			const snapshot =
				path === workspacePath
					? workspaces.map((x) => ({ ...x }))
					: path === groupPath
						? groups.map((x) => ({ ...x }))
						: items.map((x) => ({ ...x }));
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && ordinal === 1) return reply(route, 'เฉพาะส่วนนี้ไม่พร้อม', 503);
			if (path === previewPath) {
				const preview: AcademicMenuTemplatePreview = {
					revision: `revision-${ordinal}`,
					moves: [],
					sectionsToCreate: [
						{ code: 'new', name: `งานใหม่ ${ordinal}`, displayOrder: 1, workspaceCode: 'academic' }
					],
					recommendationsReady: true,
					untouchedCustomItemCount: 0,
					untouchedNonAcademicRouteCount: 0
				};
				return reply(route, preview);
			}
			if (path === previewPath + '/apply')
				return reply(route, { movedCount: 0, createdSectionCount: 1 });
			if (method === 'GET') return reply(route, snapshot);
			const resourceId = path.split('/').at(-1),
				payload = route.request().postDataJSON();
			if (path.endsWith('/reorder')) return reply(route, {});
			if (path.startsWith(workspacePath)) {
				if (method === 'DELETE') {
					const target = workspaces.find((w) => w.id === resourceId);
					workspaces = workspaces.filter((w) => w.id !== resourceId);
					groups = groups.map((g) =>
						g.workspace_code === target?.code ? { ...g, workspace_code: 'operations' } : g
					);
					return reply(route, { moved_count: 1 });
				}
				const saved = {
					...workspaces.find((w) => w.id === resourceId),
					id: method === 'POST' ? id(226) : resourceId,
					display_order: 1,
					is_active: true,
					...payload
				} as MenuWorkspace;
				workspaces = [...workspaces.filter((w) => w.id !== saved.id), saved];
				return reply(route, saved);
			}
			if (path.startsWith(groupPath)) {
				if (method === 'DELETE') {
					groups = groups.filter((g) => g.id !== resourceId);
					items = items.map((i) => (i.group_id === resourceId ? { ...i, group_id: id(222) } : i));
					return reply(route, { moved_count: 1 });
				}
				const saved = {
					...groups.find((g) => g.id === resourceId),
					id: method === 'POST' ? id(227) : resourceId,
					display_order: 1,
					is_active: true,
					...payload
				} as MenuGroup;
				groups = [...groups.filter((g) => g.id !== saved.id), saved];
				return reply(route, saved);
			}
			if (method === 'DELETE') {
				items = items.filter((i) => i.id !== resourceId);
				return reply(route, {});
			}
			const saved = { ...items.find((i) => i.id === resourceId), ...payload } as MenuItem;
			items = items.map((i) => (i.id === resourceId ? saved : i));
			return reply(route, saved);
		}
	);
	return { ...base, release, writes, count: (path: string) => counts.get(path) ?? 0 };
}
