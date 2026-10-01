import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id, firstStaff, secondStaff } from './staff-home-route-data';
export const firstUnit = id(90),
	secondUnit = id(91);
export const unitPath = (unit = firstUnit) => `/staff/organization/${unit}`;
export async function mockOrganization(
	page: Page,
	options: {
		hold?: string;
		fail?: string;
		failAt?: number;
		permissions?: string[];
	} = {}
) {
	const base = await mockStaffHome(page, {
		permissions: options.permissions ?? [
			'roles.read.all',
			'roles.create.all',
			'roles.update.all',
			'roles.assign.all',
			'roles.delete.all',
			'settings.read.all',
			'staff_profile.read.school'
		]
	});
	let units = [firstUnit, secondUnit].map((unitId, index) => ({
		id: unitId,
		code: index ? 'SECOND' : 'SCHOOL',
		name: index ? 'หน่วยงานสอง' : 'หน่วยงานแรก',
		name_en: null,
		parent_unit_id: index ? firstUnit : null,
		category: 'administrative',
		unit_type: index ? 'division' : 'school',
		is_active: true,
		is_system: !index,
		display_order: index,
		description: null,
		phone: null,
		email: null,
		location: null,
		subject_group_id: null,
		created_at: '2026-09-01T00:00:00Z',
		updated_at: '2026-09-01T00:00:00Z'
	}));
	let members = [firstUnit, secondUnit].map((unitId, index) => ({
		organization_unit_id: unitId,
		organization_unit_name: index ? 'หน่วยงานสอง' : 'หน่วยงานแรก',
		user_id: index ? secondStaff : firstStaff,
		name: index ? 'สมาชิกสอง' : 'สมาชิกแรก',
		title: '',
		position_code: 'head',
		position_title: null,
		is_primary: true,
		responsibilities: null,
		started_at: '2026-09-01'
	}));
	const permission = {
		id: id(92),
		code: 'staff_profile.read.own',
		name: 'ดูโปรไฟล์ตนเอง',
		description: null,
		module: 'staff_profile',
		action: 'read',
		scope: 'own',
		is_active: true
	};
	const grants = [{ permission_id: permission.id, position_code: 'head' }];
	const counts = new Map<string, number>(),
		reads: URL[] = [],
		writes: { method: string; path: string }[] = [];
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
			url.pathname.startsWith('/api/organization/') ||
			url.pathname === '/api/permissions/modules' ||
			url.pathname === '/api/lookup/staff',
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method();
			const kind =
				method !== 'GET'
					? 'mutation'
					: path === '/api/organization/units'
						? 'structure'
						: path.endsWith('/members')
							? 'members'
							: path.endsWith('/permissions')
								? 'grants'
								: path.endsWith('/modules')
									? 'catalog'
									: path.endsWith('/delegations')
										? 'delegations'
										: path.endsWith('/delegatable-permissions')
											? 'options'
											: path === '/api/lookup/staff'
												? 'staff'
												: 'unit';
			if (method === 'GET') {
				reads.push(url);
				counts.set(path, (counts.get(path) ?? 0) + 1);
			} else writes.push({ method, path });
			const selected = path.includes(secondUnit) ? secondUnit : firstUnit;
			const unitSnapshot = units.find((unit) => unit.id === selected);
			const memberSnapshot = members.filter(
				(member) =>
					member.organization_unit_id === selected ||
					(url.searchParams.get('include_children') === 'true' && selected === firstUnit)
			);
			if (
				options.hold === kind &&
				(kind === 'structure' ||
					kind === 'catalog' ||
					kind === 'mutation' ||
					!path.includes(secondUnit))
			)
				await held;
			if (options.fail === kind && counts.get(path) === (options.failAt ?? 1))
				return reply(route, `region ${kind} ไม่พร้อม`, 503);
			if (method !== 'GET') {
				if (path.endsWith('/members'))
					members = [
						...members,
						{ ...members[0], user_id: id(93), name: 'สมาชิกใหม่', organization_unit_id: selected }
					];
				else if (method === 'DELETE' && path.includes('/members/'))
					members = members.filter((member) => member.organization_unit_id !== selected);
				else if (kind === 'mutation' && method === 'PUT' && !path.endsWith('/permissions'))
					units = units.map((unit) =>
						unit.id === selected ? { ...unit, ...route.request().postDataJSON() } : unit
					);
				return reply(route, method === 'POST' && !path.endsWith('/members') ? { id: id(94) } : {});
			}
			if (kind === 'structure') return reply(route, units);
			if (kind === 'unit') return reply(route, unitSnapshot);
			if (kind === 'members') return reply(route, memberSnapshot);
			if (kind === 'catalog') return reply(route, { staff_profile: [permission] });
			if (kind === 'grants') return reply(route, grants);
			if (kind === 'delegations') return reply(route, []);
			if (kind === 'options') return reply(route, [permission]);
			return reply(route, [{ id: id(93), name: 'ตัวเลือก บุคลากร', title: '' }]);
		}
	);
	return { release, reads, writes, count: (path: string) => counts.get(path) ?? base.count(path) };
}
