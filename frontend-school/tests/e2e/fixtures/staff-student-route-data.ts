import type { Page, Route } from '@playwright/test';
import type { StudentRegister, StudentListItem } from '../../../src/lib/api/students.js';
import { mockStaffHome, id, year, nextYear } from './staff-home-route-data';
export { year, nextYear };
export const firstStudent = id(60),
	secondStudent = id(61);
export const listPath = () => '/staff/students';
export const studentPath = (studentId = firstStudent, suffix = '', selectedYear?: string) => {
	const path = `/staff/students/${studentId}${suffix}`;
	if (!selectedYear) return path;
	return `${path}?${new URLSearchParams({ academicYearId: selectedYear, returnTo: `/staff/academic/student-years?academicYearId=${selectedYear}` })}`;
};
export async function mockStaffStudents(
	page: Page,
	options: {
		hold?: 'list' | 'profile' | 'mutation';
		holdAt?: number;
		fail?: 'list' | 'profile';
		failAt?: number;
		failMutation?: boolean;
		permissions?: string[];
		fullPage?: boolean;
		emptyPage?: number;
		listItem?: Partial<StudentListItem>;
		profile?: Partial<StudentRegister>;
	} = {}
) {
	const base = await mockStaffHome(page, {
		permissions: options.permissions ?? [
			'student.read.school',
			'student.update.all',
			'student.delete.all',
			'student.create.all'
		]
	});
	const reads: URL[] = [],
		writes: { method: string; path: string }[] = [];
	const counts = new Map<string, number>();
	const payloads: { method: string; path: string; data: Record<string, unknown> | null }[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	let firstName = 'นักเรียนแรก',
		parents = [
			{
				id: id(65),
				first_name: 'ผู้ปกครองเดิม',
				last_name: 'ทดสอบ',
				relationship: 'บิดา',
				is_primary: true,
				phone: null,
				username: 'synthetic-parent'
			}
		];
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route(
		(url) => url.pathname.startsWith('/api/students'),
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method();
			const kind = method !== 'GET' ? 'mutation' : path === '/api/students' ? 'list' : 'profile';
			if (method === 'GET') {
				reads.push(url);
				counts.set(path, (counts.get(path) ?? 0) + 1);
			} else {
				writes.push({ method, path });
				payloads.push({
					method,
					path,
					data: route.request().postData() ? route.request().postDataJSON() : null
				});
			}
			const selectedYear = url.searchParams.get('academicYearId');
			const name = path.includes(secondStudent) ? 'นักเรียนที่สอง' : firstName;
			const snapshot = {
				id: path.split('/').at(-1),
				first_name: name,
				last_name: 'ทดสอบ',
				username: 'synthetic-student',
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
				account_status: 'active',
				registration_status: 'studying',
				registration_status_updated_at: null,
				registration_status_updated_by: null,
				enrollment_date: '2023-05-16',
				student_id: 'TEST-01',
				student_number: selectedYear ? 1 : null,
				grade_level: selectedYear ? 'ม.1' : null,
				homeroom: selectedYear ? (selectedYear === nextYear ? 'ห้องปีถัดไป' : 'ห้องปีเดิม') : null,
				parents: [...parents]
			};
			if (
				options.hold === kind &&
				(!options.holdAt || counts.get(path) === options.holdAt) &&
				(kind === 'mutation' ||
					(kind === 'list' ? !url.searchParams.has('search') : path.includes(firstStudent)))
			)
				await held;
			if (options.fail === kind && counts.get(path) === (options.failAt ?? 1))
				return reply(route, 'region นักเรียนไม่พร้อม', 503);
			if (method !== 'GET') {
				if (options.failMutation) return reply(route, 'บันทึกตัวอย่างไม่สำเร็จ', 409);
				if (method === 'POST' && path === '/api/students')
					return reply(route, { id: firstStudent, username: 'synthetic-student' });
				if (path.endsWith('/parents')) {
					const payload = route.request().postDataJSON();
					parents.push({
						id: id(66),
						first_name: payload.first_name,
						last_name: payload.last_name,
						relationship: payload.relationship,
						is_primary: false,
						phone: payload.phone,
						username: 'synthetic-parent-new'
					});
				} else if (path.includes('/parents/'))
					parents = parents.filter((parent) => parent.id !== path.split('/').at(-1));
				else if (method === 'PUT') firstName = route.request().postDataJSON().first_name;
				return reply(route, {});
			}
			if (kind === 'profile') return reply(route, { ...snapshot, ...options.profile });
			const pageNumber = Number(url.searchParams.get('page') ?? 1),
				search = url.searchParams.get('search');
			const items = Array.from(
				{
					length:
						options.emptyPage === pageNumber ? 0 : options.fullPage && pageNumber === 1 ? 20 : 1
				},
				(_, index) => ({
					...snapshot,
					id: id(60 + index),
					first_name: search
						? `ค้นหา ${search}`
						: selectedYear === nextYear
							? 'นักเรียนปีถัดไป'
							: pageNumber > 1
								? 'นักเรียนหน้าสอง'
								: index
									? `นักเรียน ${index}`
									: firstName,
					...options.listItem
				})
			);
			return reply(route, { items, page: pageNumber, page_size: 20 });
		}
	);
	return {
		release,
		reads,
		writes,
		payloads,
		count: (path: string) => counts.get(path) ?? base.count(path)
	};
}
