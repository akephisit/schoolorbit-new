import type { Page, Route } from '@playwright/test';
export const id = (n: number) => `55000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
export const year = id(1),
	nextYear = id(2),
	actor = id(10),
	firstStaff = id(20),
	secondStaff = id(21);
export const homePath = (selectedYear = year) => `/staff?academicYearId=${selectedYear}`;
export const publicPath = (selectedStaff = firstStaff) => `/staff/view/${selectedStaff}`;
export async function mockStaffHome(
	page: Page,
	options: {
		hold?: string;
		holdAt?: number;
		fail?: string;
		failAt?: number;
		permissions?: string[];
		userType?: string;
	} = {}
) {
	const reads: string[] = [],
		writes: string[] = [];
	const counts = new Map<string, number>();
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	let nickname = 'ชื่อเล่นเดิม';
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			headers: { 'X-CSRF-Token': 'synthetic-csrf' },
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	const profile = () => ({
		id: actor,
		username: 'E2E-staff-home',
		firstName: 'เจ้าของ',
		lastName: 'โปรไฟล์',
		nickname,
		email: 'staff@example.invalid',
		phone: null,
		userType: 'staff',
		status: 'active',
		nationalId: null,
		title: null,
		dateOfBirth: null,
		gender: 'male',
		address: null,
		lineId: null,
		emergencyContact: null,
		profileImageFileId: null,
		primaryRoleName: 'บุคลากร',
		createdAt: '2026-09-01T00:00:00Z',
		updatedAt: '2026-09-01T00:00:00Z'
	});
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url()),
				resource = url.pathname,
				method = route.request().method();
			if (method === 'GET') {
				reads.push(resource);
				counts.set(resource, (counts.get(resource) ?? 0) + 1);
			} else writes.push(`${method} ${resource}`);
			if (resource === '/api/notifications/stream')
				return route.fulfill({
					status: 200,
					contentType: 'text/event-stream',
					body: ': connected\n\nretry: 3600000\n\n'
				});
			if (resource === '/api/auth/me')
				return reply(route, {
					...profile(),
					userType: options.userType ?? 'staff',
					permissions: options.permissions ?? ['achievement.read.all']
				});
			if (resource === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: year,
					activeAcademicTermId: null,
					years: [year, nextYear].map((value, index) => ({
						id: value,
						name: String(2569 + index),
						year: 2569 + index,
						status: 'open',
						startDate: '2026-05-01',
						plannedEndDate: '2027-04-30',
						closedOn: null
					})),
					terms: []
				});
			const kind =
				resource === '/api/menu/user'
					? 'menu'
					: resource.endsWith('/counts')
						? 'counts'
						: resource === '/api/staff/dashboard'
							? 'overview'
							: resource === '/api/auth/me/profile'
								? method === 'PUT'
									? 'save'
									: 'profile'
								: resource.endsWith('/public-profile')
									? 'public-profile'
									: resource === '/api/achievements'
										? 'achievements'
										: '';
			const profileSnapshot = profile();
			if (
				options.hold === kind &&
				(!options.holdAt || counts.get(resource) === options.holdAt) &&
				(kind !== 'overview' || url.searchParams.get('academicYearId') === year) &&
				(kind !== 'public-profile' || resource.includes(firstStaff))
			)
				await held;
			if (options.fail === kind && counts.get(resource) === (options.failAt ?? 1))
				return reply(route, 'region ไม่พร้อม', 503);
			if (resource === '/api/menu/user')
				return reply(route, {
					groups: [
						{
							code: 'test-services',
							name: 'บริการทดสอบ',
							icon: 'Circle',
							workspaceCode: 'home',
							workspaceName: 'งานของฉัน',
							workspaceIcon: 'Home',
							workspaceOrder: 1,
							displayOrder: 1,
							items: [
								{
									id: id(30),
									name: 'โปรไฟล์ทดสอบ',
									path: '/staff/profile',
									icon: 'User',
									displayOrder: 1,
									requiredPermission: null,
									userType: 'staff'
								}
							]
						}
					]
				});
			if (resource === '/api/me/work-items/counts')
				return reply(route, {
					open: 6 + (counts.get(resource) ?? 1),
					dueSoon: 2,
					overdue: 1,
					submitted: 0,
					closed: 0,
					total: 10
				});
			if (resource === '/api/staff/dashboard')
				return reply(route, {
					totalStaff: url.searchParams.get('academicYearId') === nextYear ? 42 : 18,
					totalStudents: 100,
					activeHomerooms: 4
				});
			if (resource === '/api/auth/me/profile' && method === 'PUT') {
				nickname = route.request().postDataJSON().nickname ?? nickname;
				return reply(route, profile());
			}
			if (resource === '/api/auth/me/profile') return reply(route, profileSnapshot);
			if (resource.endsWith('/public-profile'))
				return reply(route, {
					id: resource.includes(secondStaff) ? secondStaff : firstStaff,
					first_name: resource.includes(secondStaff) ? 'บุคลากรสอง' : 'บุคลากรแรก',
					last_name: 'ทดสอบ',
					username: 'E2E-public',
					email: null,
					title: null,
					nickname: null,
					phone: null,
					hired_date: null,
					profile_image_file_id: null,
					user_type: 'staff',
					status: 'active',
					roles: [],
					organization_units: []
				});
			if (resource === '/api/achievements')
				return reply(route, [
					{
						id: id(40),
						user_id: url.searchParams.get('user_id'),
						title:
							url.searchParams.get('user_id') === secondStaff
								? 'ผลงานคนที่สอง'
								: 'ผลงานที่พร้อมก่อน',
						description: 'ผลงานทดสอบ',
						achievement_date: '2026-09-01',
						image_file_id: null
					}
				]);
			return reply(route, {});
		}
	);
	return {
		reads,
		writes,
		release: () => release(),
		count: (resource: string) => counts.get(resource) ?? 0
	};
}
