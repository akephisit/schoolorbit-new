import { test, expect, type Page, type Route } from '@playwright/test';
import { mockStaffHome, id, year } from './fixtures/staff-home-route-data';
import type { HomeroomRoster } from '../../src/lib/api/homeroom-roster';
import type { StudentAcademicYear, HomeroomPlacement } from '../../src/lib/api/academic-core';

const room = id(80),
	target = id(81),
	path = `/staff/academic/homerooms/${room}/students?academicYearId=${year}`;
function fixture(): HomeroomRoster {
	return {
		homeroom: {
			id: room,
			code: 'M1-3',
			name: 'ม.1/3',
			academicYearId: year,
			gradeLevelId: id(82),
			roomNumber: '3',
			studyProgramId: id(83),
			capacity: 40,
			isActive: true,
			rowVersion: 1,
			migrated: false,
			createdAt: '2026-05-01T00:00:00Z',
			updatedAt: '2026-05-01T00:00:00Z',
			studentCount: 2
		},
		yearStatus: 'active',
		revision: 'fixture-revision-1',
		students: [
			{
				placementId: id(90),
				studentAcademicYearId: id(91),
				studentId: id(92),
				studentCode: '69001',
				title: 'เด็กหญิง',
				firstName: 'ขวัญ',
				lastName: 'ทดสอบ',
				gender: 'female',
				classNumber: 1,
				status: 'current',
				startDate: '2026-05-01',
				rowVersion: 1,
				studentYearRowVersion: 1
			},
			{
				placementId: id(93),
				studentAcademicYearId: id(94),
				studentId: id(95),
				studentCode: '69002',
				title: 'เด็กชาย',
				firstName: 'เกียรติ',
				lastName: 'ทดสอบ',
				gender: 'male',
				classNumber: 2,
				status: 'current',
				startDate: '2026-05-01',
				rowVersion: 1,
				studentYearRowVersion: 1
			}
		]
	};
}
async function mock(
	page: Page,
	options: {
		reader?: boolean;
		closed?: boolean;
		hold?: boolean;
		fail?: boolean;
		failMutation?: boolean;
		permissions?: string[];
	} = {}
) {
	await mockStaffHome(page, {
		permissions:
			options.permissions ??
			(options.reader
				? ['student_academic_year.read.school']
				: ['student_academic_year.read.school', 'student_academic_year.manage.school'])
	});
	let roster = fixture();
	if (options.closed) roster.yearStatus = 'closed';
	const writes: { path: string; body: Record<string, unknown> }[] = [],
		reads: string[] = [];
	let release = () => {};
	const pending = new Promise<void>((resolve) => (release = resolve));
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route(
		(url) => url.pathname.startsWith(`/api/academic/homerooms/${room}`),
		async (route) => {
			const url = new URL(route.request().url()),
				method = route.request().method();
			if (method === 'GET') reads.push(url.pathname);
			else writes.push({ path: url.pathname, body: route.request().postDataJSON() });
			if (url.pathname.endsWith('/students') && method === 'GET') {
				if (options.hold) await pending;
				if (options.fail) return reply(route, 'โหลดรายชื่อไม่สำเร็จ', 503);
				return reply(route, roster);
			}
			if (url.pathname.endsWith('/numbering-preview'))
				return reply(route, {
					roster,
					numbers: [
						{ placementId: id(93), classNumber: 1 },
						{ placementId: id(90), classNumber: 2 }
					]
				});
			if (url.pathname.endsWith('/numbers')) {
				if (options.failMutation) return reply(route, 'รายชื่อเปลี่ยนแล้ว กรุณาโหลดใหม่', 409);
				roster = {
					...roster,
					revision: 'fixture-revision-2',
					students: roster.students
						.map((student) => {
							const numbers = route.request().postDataJSON().numbers as {
								placementId: string;
								classNumber: number;
							}[];
							return {
								...student,
								classNumber:
									numbers.find((number) => number.placementId === student.placementId)
										?.classNumber ?? student.classNumber
							};
						})
						.sort((a, b) => (a.classNumber ?? 0) - (b.classNumber ?? 0))
				};
				return reply(route, roster);
			}
			if (url.pathname.endsWith('/student-candidates'))
				return reply(route, [
					{
						studentAcademicYearId: id(96),
						studentId: id(97),
						studentCode: '69003',
						name: 'นักเรียนใหม่ ทดสอบ',
						rowVersion: 1
					}
				]);
			if (url.pathname.endsWith('/transfer-targets'))
				return reply(route, [{ ...roster.homeroom, id: target, name: 'ม.1/4', studentCount: 38 }]);
			if (url.pathname.endsWith('/students') && method === 'POST') {
				const body = route.request().postDataJSON();
				if (body.action === 'add')
					roster = {
						...roster,
						revision: 'after-add',
						students: [
							...roster.students,
							{
								...roster.students[0],
								placementId: id(98),
								studentAcademicYearId: id(96),
								studentId: id(97),
								firstName: 'นักเรียนใหม่',
								classNumber: 3
							}
						]
					};
				else roster = { ...roster, revision: 'after-transfer', students: [] };
				return reply(route, roster);
			}
			return reply(route, []);
		}
	);
	return { writes, reads, release };
}
for (const mobile of [false, true])
	for (const dark of [false, true]) {
		test(`room numbering previews all students and saves explicitly (${mobile ? 'mobile' : 'desktop'},${dark ? 'dark' : 'light'})`, async ({
			page
		}) => {
			await page.setViewportSize({ width: mobile ? 390 : 1440, height: 900 });
			const api = await mock(page);
			await page.goto(path);
			await expect(page.getByTestId('homeroom-roster-ready')).toBeVisible();
			if (dark) await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
			await expect(page.getByRole('heading', { name: 'นักเรียนในห้อง ม.1/3' })).toBeVisible();
			await expect
				.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth))
				.toBe(true);
			await page.screenshot({
				path: `test-results/homeroom-roster-page-${mobile ? 'mobile' : 'desktop'}-${dark ? 'dark' : 'light'}.png`
			});
			await page.getByRole('textbox', { name: 'ค้นหานักเรียนในห้อง' }).fill('ขวัญ');
			await expect(page.getByRole('row')).toHaveCount(2);
			await page.getByRole('button', { name: 'จัดเลขที่', exact: true }).click();
			const dialog = page.getByRole('dialog');
			await expect(dialog.getByRole('button', { name: 'บันทึกเลขที่ใหม่' })).toBeDisabled();
			await dialog.getByRole('button', { name: 'ดูตัวอย่างเลขที่ใหม่' }).click();
			await expect(dialog.getByText('เลขที่เปลี่ยน 2 คน จากทั้งหมด 2 คน')).toBeVisible();
			expect(api.writes).toHaveLength(0);
			await dialog.getByLabel('วิธีเรียง').click();
			await page.getByRole('option', { name: 'ตามรหัสนักเรียน', exact: true }).click();
			await expect(dialog.getByRole('button', { name: 'บันทึกเลขที่ใหม่' })).toBeDisabled();
			await expect(dialog.getByText('เลขที่เปลี่ยน 2 คน จากทั้งหมด 2 คน')).toHaveCount(0);
			await dialog.getByRole('button', { name: 'ดูตัวอย่างเลขที่ใหม่' }).click();
			await expect(dialog.getByText('เลขที่เปลี่ยน 2 คน จากทั้งหมด 2 คน')).toBeVisible();
			await expect.poll(() => dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
			await page.screenshot({
				path: `test-results/homeroom-roster-${mobile ? 'mobile' : 'desktop'}-${dark ? 'dark' : 'light'}.png`
			});
			await dialog.getByRole('button', { name: 'บันทึกเลขที่ใหม่' }).click();
			await expect(dialog).toBeHidden();
			expect(api.writes).toHaveLength(1);
			expect(api.writes[0].body.numbers).toHaveLength(2);
		});
	}
test('reader and closed years never fetch action data or show mutation controls', async ({
	page
}) => {
	const api = await mock(page, { reader: true });
	await page.goto(path);
	await expect(page.getByTestId('homeroom-roster-ready')).toBeVisible();
	await expect(page.getByRole('button', { name: 'จัดเลขที่', exact: true })).toHaveCount(0);
	await expect(
		page.getByRole('button', { name: 'เพิ่มนักเรียนเข้าห้อง', exact: true })
	).toHaveCount(0);
	await expect(page.getByRole('link', { name: 'ข้อมูลนักเรียน', exact: true })).toHaveCount(0);
	expect(api.reads).toEqual([`/api/academic/homerooms/${room}/students`]);
	await page.unroute((url) => url.pathname.startsWith(`/api/academic/homerooms/${room}`));
	await mock(page, { closed: true });
	await page.reload();
	await expect(page.getByText('ปีการศึกษานี้ปิดแล้ว ดูข้อมูลย้อนหลังได้')).toBeVisible();
	await expect(page.getByRole('button', { name: 'จัดเลขที่', exact: true })).toHaveCount(0);
});
for (const entry of ['homeroom', 'annual'] as const)
	test(`${entry} information links open the profile and selected-year history directly`, async ({
		page
	}) => {
		const entryPath =
			entry === 'homeroom' ? path : `/staff/academic/student-years?academicYearId=${year}`;
		await mock(page, {
			reader: true,
			permissions: [
				'student_academic_year.read.school',
				'student_academic_year.manage.school',
				'homeroom.read.school',
				'student.read.school'
			]
		});
		const student: StudentAcademicYear = {
			id: id(91),
			studentId: id(92),
			academicYearId: year,
			gradeLevelId: id(82),
			gradeLevelName: 'มัธยมศึกษาปีที่ 1',
			studyProgramId: id(83),
			studyProgramName: 'แผนหลัก',
			studentCode: '69001',
			studentName: 'ขวัญ ทดสอบ',
			status: 'active',
			rowVersion: 1,
			migrated: false,
			createdAt: '2026-05-01T00:00:00Z',
			updatedAt: '2026-05-01T00:00:00Z'
		};
		const placement: HomeroomPlacement = {
			id: id(90),
			studentAcademicYearId: student.id,
			academicYearId: year,
			homeroomId: room,
			classNumber: 7,
			status: 'cancelled',
			startDate: '2026-05-01',
			endDate: null,
			enrollmentType: 'regular',
			rowVersion: 1,
			migrated: false,
			createdAt: '2026-05-01T00:00:00Z',
			updatedAt: '2026-05-01T00:00:00Z'
		};
		await page.route(
			(url) =>
				url.pathname === '/api/academic/homerooms' ||
				url.pathname.startsWith('/api/academic/student-years') ||
				url.pathname === '/api/academic/placements',
			async (route) => {
				const resource = new URL(route.request().url()).pathname;
				const data =
					resource === '/api/academic/student-years'
						? [student]
						: resource === `/api/academic/student-years/${student.id}`
							? student
							: resource === `/api/academic/student-years/${student.id}/placements`
								? [placement]
								: resource === '/api/academic/homerooms'
									? [fixture().homeroom]
									: resource === '/api/academic/placements'
										? [placement]
										: [];
				await route.fulfill({
					contentType: 'application/json',
					body: JSON.stringify({ success: true, data })
				});
			}
		);
		await page.route(
			(url) => url.pathname === `/api/students/${student.studentId}`,
			(route) =>
				route.fulfill({
					contentType: 'application/json',
					body: JSON.stringify({
						success: true,
						data: {
							id: student.studentId,
							first_name: 'ขวัญ',
							last_name: 'ทดสอบ',
							title: 'เด็กหญิง',
							student_id: '69001',
							status: 'active',
							parents: []
						}
					})
				})
		);
		await page.goto(entryPath);
		if (entry === 'homeroom') await expect(page.getByTestId('homeroom-roster-ready')).toBeVisible();
		else await expect(page.getByRole('button', { name: 'จัดห้อง', exact: true })).toBeVisible();
		await page
			.getByRole('row')
			.filter({ hasText: 'ขวัญ' })
			.getByRole('link', { name: 'ข้อมูลนักเรียน' })
			.click();
		await expect(page).toHaveURL(
			new RegExp(`/staff/students/${student.studentId}\\?academicYearId=${year}`)
		);
		await expect(page.getByRole('dialog')).toHaveCount(0);
		await expect(page.getByTestId('student-profile')).toContainText('เด็กหญิงขวัญ ทดสอบ');
		await expect(page.getByTestId('student-placement-history')).toContainText('ยกเลิก');
		await expect(page.getByRole('link', { name: 'ย้อนกลับ', exact: true })).toHaveAttribute(
			'href',
			entryPath
		);
		await page.goto(
			`/staff/students/${student.studentId}?academicYearId=${year}&returnTo=${encodeURIComponent('//example.com/staff/students')}`
		);
		await expect(page.getByRole('link', { name: 'ย้อนกลับ', exact: true })).toHaveAttribute(
			'href',
			`/staff/students?academicYearId=${year}`
		);
	});
test('a failed primary read can retry without presenting an empty room', async ({ page }) => {
	const options = { fail: true };
	await mock(page, options);
	await page.goto(path);
	await expect(page.getByText('โหลดนักเรียนในห้องไม่สำเร็จ', { exact: true })).toBeVisible();
	await expect(page.getByText('ห้องนี้ยังไม่มีนักเรียน')).toHaveCount(0);
	options.fail = false;
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('homeroom-roster-ready')).toBeVisible();
});
test('individual numbering and removal preserve the chosen placement identities', async ({
	page
}) => {
	const api = await mock(page);
	await page.goto(path);
	await expect(page.getByTestId('homeroom-roster-ready')).toBeVisible();
	await page.getByRole('button', { name: /จัดการ.*ขวัญ/ }).click();
	await page.getByRole('menuitem', { name: 'แก้เลขที่' }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('เลขที่ใหม่').fill('8');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(api.writes[0].body.numbers).toEqual([{ placementId: id(90), classNumber: 8 }]);
	await page.getByRole('checkbox', { name: 'เลือกนักเรียนที่แสดงทั้งหมด' }).check();
	await page.getByRole('button', { name: 'นำออกจากห้อง', exact: true }).click();
	await dialog.getByLabel('เหตุผล').fill('ยืนยันจัดห้องใหม่');
	await dialog.getByRole('button', { name: 'ยืนยันนำออกจากห้อง', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(api.writes[1].body.action).toBe('remove');
	await expect(page.getByText('ห้องนี้ยังไม่มีนักเรียน')).toBeVisible();
});
test('primary read streams a skeleton; failed save retains numbering preview', async ({ page }) => {
	const api = await mock(page, { hold: true, failMutation: true });
	await page.goto(path);
	await expect(page.getByTestId('homeroom-roster-ready')).toHaveCount(0);
	api.release();
	await expect(page.getByTestId('homeroom-roster-ready')).toBeVisible();
	await page.getByRole('button', { name: 'จัดเลขที่', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'ดูตัวอย่างเลขที่ใหม่' }).click();
	await dialog.getByRole('button', { name: 'บันทึกเลขที่ใหม่' }).click();
	await expect(dialog.getByRole('alert')).toContainText('รายชื่อเปลี่ยนแล้ว');
	await expect(dialog.getByText('เลขที่เปลี่ยน 2 คน จากทั้งหมด 2 คน')).toBeVisible();
});
test('add and transfer selections use one atomic command and patch the roster', async ({
	page
}) => {
	const api = await mock(page);
	await page.goto(path);
	await expect(page.getByTestId('homeroom-roster-ready')).toBeVisible();
	await page.getByRole('button', { name: 'เพิ่มนักเรียนเข้าห้อง', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('checkbox', { name: /นักเรียนใหม่/ }).check();
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(api.writes[0].body.action).toBe('add');
	await expect(page.getByText('3 / 40 คน')).toBeVisible();
	await page.getByRole('checkbox', { name: 'เลือกนักเรียนที่แสดงทั้งหมด' }).check();
	await page.getByRole('button', { name: 'ย้ายไปห้องอื่น', exact: true }).click();
	await dialog.getByLabel('ห้องปลายทาง').click();
	await page.getByRole('option', { name: /ม.1\/4/ }).click();
	await dialog.getByLabel('เหตุผล').fill('ทดสอบย้ายพร้อมกัน');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(api.writes[1].body.selections).toHaveLength(3);
	await expect(page.getByText('ห้องนี้ยังไม่มีนักเรียน')).toBeVisible();
});
