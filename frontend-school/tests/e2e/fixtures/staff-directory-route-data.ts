import type { Page, Route } from '@playwright/test';
import { id, mockStaffHome, firstStaff, secondStaff } from './staff-home-route-data';
export { firstStaff, secondStaff };
export const staffPath = (person = firstStaff, suffix = '') => `/staff/manage/${person}${suffix}`;
export const directoryPath = '/staff/manage';
export const roleId = id(80),
	organizationId = id(81);
export async function mockStaffDirectory(
	page: Page,
	options: {
		hold?: string;
		holdAt?: number;
		fail?: string;
		failAt?: number;
		permissions?: string[];
		fullPage?: boolean;
	} = {}
) {
	const base = await mockStaffHome(page, {
		permissions: options.permissions ?? [
			'staff_profile.read.school',
			'staff.create.all',
			'staff.update.all',
			'staff.delete.all',
			'roles.read.all',
			'roles.assign.all',
			'roles.remove.all',
			'achievement.read.all',
			'achievement.create.all',
			'achievement.update.all',
			'achievement.delete.all'
		]
	});
	const counts = new Map<string, number>(),
		reads: URL[] = [],
		writes: { method: string; path: string }[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	const role = {
		id: roleId,
		name: 'บทบาทตัวเลือก',
		name_en: null,
		code: 'SYNTHETIC_ROLE',
		description: null,
		user_type: 'staff',
		level: 10,
		is_active: true,
		is_system: false,
		permissions: ['staff_profile.read.own'],
		created_at: '2026-09-01T00:00:00Z',
		updated_at: '2026-09-01T00:00:00Z'
	};
	const newRole = { ...role, id: id(88), name: 'บทบาทใหม่', code: 'SYNTHETIC_NEW_ROLE' };
	const org = {
		id: organizationId,
		code: 'SYNTHETIC_UNIT',
		name: 'หน่วยงานตัวเลือก',
		parent_unit_id: null,
		category: 'administrative',
		unit_type: 'unit',
		subject_group_id: null,
		is_active: true,
		description: null,
		created_at: '2026-09-01T00:00:00Z',
		updated_at: '2026-09-01T00:00:00Z'
	};
	let assignments = [
		{
			id: id(82),
			user_id: firstStaff,
			role_id: roleId,
			is_primary: true,
			started_at: '2026-09-01',
			ended_at: null,
			role
		}
	];
	let firstName = 'บุคลากรแรก';
	let achievements = [
		{
			id: id(83),
			user_id: firstStaff,
			title: 'ผลงานที่พร้อม',
			description: null,
			achievement_date: '2026-09-01',
			image_file_id: null,
			created_at: '2026-09-01T00:00:00Z',
			updated_at: '2026-09-01T00:00:00Z'
		}
	];
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route(
		(url) =>
			url.pathname === '/api/staff' ||
			/^\/api\/staff\/[\da-f-]+$/.test(url.pathname) ||
			url.pathname === '/api/roles' ||
			url.pathname === '/api/organization/units' ||
			url.pathname.startsWith('/api/users/') ||
			url.pathname.startsWith('/api/achievements') ||
			url.pathname === '/api/files',
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method();
			const kind =
				method !== 'GET'
					? path === '/api/files'
						? 'upload'
						: 'mutation'
					: path === '/api/staff'
						? 'list'
						: path.startsWith('/api/staff/')
							? 'profile'
							: path === '/api/roles'
								? 'catalog'
								: path === '/api/organization/units'
									? 'organizations'
									: path.startsWith('/api/achievements')
										? 'achievements'
										: path.endsWith('/permissions')
											? 'permissions'
											: 'assignments';
			if (method === 'GET') {
				reads.push(url);
				counts.set(path, (counts.get(path) ?? 0) + 1);
			} else writes.push({ method, path });
			const profileSnapshot = {
				id: path.split('/').at(-1),
				username: 'synthetic-staff',
				title: null,
				first_name: path.includes(secondStaff) ? 'บุคลากรที่สอง' : firstName,
				last_name: 'ทดสอบ',
				nickname: null,
				email: null,
				national_id: null,
				phone: null,
				emergency_contact: null,
				line_id: null,
				date_of_birth: null,
				gender: 'male',
				address: null,
				hired_date: null,
				user_type: 'staff',
				status: 'active',
				profile_image_file_id: null,
				staff_info: null,
				roles: [{ ...role, is_primary: true }],
				organization_units: [
					{ ...org, position_code: 'member', is_primary: true, responsibilities: '' }
				],
				teaching_assignments: [],
				advisor_homerooms: []
			};
			const listSnapshot = path.includes(secondStaff) ? [] : [...assignments];
			const achievementSnapshot =
				url.searchParams.get('user_id') === secondStaff ? [] : [...achievements];
			if (
				options.hold === kind &&
				(!options.holdAt || counts.get(path) === options.holdAt) &&
				((kind !== 'profile' && kind !== 'assignments' && kind !== 'permissions') ||
					path.includes(firstStaff))
			)
				await held;
			if (options.fail === kind && counts.get(path) === (options.failAt ?? 1))
				return reply(route, `region ${kind} ไม่พร้อม`, 503);
			if (method !== 'GET') {
				if (kind === 'upload') return reply(route, { id: id(85) });
				if (path.startsWith('/api/achievements')) {
					if (method === 'DELETE') achievements = [];
					else {
						const payload = route.request().postDataJSON();
						const saved = { ...achievements[0], ...payload, id: id(84) };
						achievements = [saved];
						return reply(route, saved);
					}
				} else if (path.startsWith('/api/users/')) {
					if (method === 'DELETE') assignments = [];
					else if (method === 'POST') {
						const payload = route.request().postDataJSON();
						assignments.push({ ...assignments[0], ...payload, id: id(86), role: newRole });
					}
					return reply(route, method === 'POST' ? { id: id(86) } : {});
				} else if (method === 'POST') return reply(route, { id: id(87) });
				else if (method === 'PUT')
					firstName = route.request().postDataJSON().first_name ?? firstName;
				return reply(route, {});
			}
			if (kind === 'profile') return reply(route, profileSnapshot);
			if (kind === 'catalog') return reply(route, [role, newRole]);
			if (kind === 'organizations') return reply(route, [org]);
			if (kind === 'achievements') return reply(route, achievementSnapshot);
			if (kind === 'assignments') return reply(route, listSnapshot);
			if (kind === 'permissions') return reply(route, ['staff_profile.read.own']);
			const pageNumber = Number(url.searchParams.get('page') ?? 1),
				search = url.searchParams.get('search');
			return reply(route, {
				items: [
					{
						...profileSnapshot,
						id: firstStaff,
						first_name: search ? `ค้นหา ${search}` : pageNumber > 1 ? 'บุคลากรหน้าสอง' : firstName
					}
				],
				total: options.fullPage ? 21 : 1,
				page: pageNumber,
				page_size: 20,
				total_pages: options.fullPage ? 2 : 1
			});
		}
	);
	return { release, reads, writes, count: (path: string) => counts.get(path) ?? base.count(path) };
}
