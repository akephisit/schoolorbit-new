import { expect, test, type Page, type Route } from '@playwright/test';
import {
	mockParent,
	child,
	otherChild,
	deniedChild,
	childRoute
} from './fixtures/parent-route-data';
import { mockStudentSchedules, year, term } from './fixtures/student-schedules-route-data';
import { actor, id } from './fixtures/staff-home-route-data';
import type { Student } from '../../src/lib/api/students';
import type { TimetableBlock } from '../../src/lib/api/timetable';
import type { AttendanceReport } from '../../src/lib/api/attendance';

test.use({ serviceWorkers: 'block' });
test.setTimeout(60_000);
const familyExpect = expect.configure({ timeout: 30_000 });
const reply = (route: Route, data: unknown, status = 200) =>
	route.fulfill({
		status,
		json: status < 400 ? { success: true, data } : { success: false, error: data }
	});
const profile = (studentId: string): Student => ({
	id: studentId,
	username: 'synthetic-child',
	first_name: studentId === otherChild ? 'นักเรียนคนสอง' : 'นักเรียนคนแรก',
	last_name: 'ทดสอบ',
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
	status: 'active',
	student_id: '5471',
	student_number: 11,
	grade_level: 'ม.1',
	homeroom: '1',
	parents: []
});
async function ownPermission(
	page: Page,
	userType: 'parent' | 'student',
	permissions = ['attendance.read.own']
) {
	await page.route('**/api/auth/me', (route) =>
		reply(route, {
			id: actor,
			username: 'synthetic-family',
			firstName: 'ผู้ใช้',
			lastName: 'ทดสอบ',
			userType,
			status: 'active',
			permissions
		})
	);
}
function blocks(): TimetableBlock[] {
	const kinds: TimetableBlock['blockKind'][] = ['course', 'structural', 'activity', 'activity'];
	return kinds.map((kind, index) => ({
		id: id(800 + index),
		academicYearId: year,
		academicTermId: term,
		bellScheduleId: id(810),
		bellSchedulePeriodId: id(811 + index),
		blockKind: kind,
		createdAt: '2026-10-01T00:00:00Z',
		updatedAt: '2026-10-01T00:00:00Z',
		dayOfWeek: 'MON',
		startTime: `${String(8 + index).padStart(2, '0')}:00:00`,
		endTime: `${9 + index}:00:00`,
		groups: [],
		homerooms: [
			{
				id: id(820),
				homeroomId: id(821),
				code: 'M1-1',
				name: 'ชั้นที่ต้องซ่อน',
				roomId: id(822),
				roomCode: 'ห้องที่ต้องซ่อน',
				isActive: true,
				rowVersion: 1
			}
		],
		teachers: [],
		syncStates: [],
		isActive: true,
		learningOfferingId: index === 1 ? null : id(823),
		offeringCode: index === 0 ? 'COURSE-CODE' : 'ACTIVITY-CODE',
		offeringName: ['วิชาเรียน', null, 'กิจกรรมพร้อมกันทดสอบ', 'กิจกรรมอิสระทดสอบ'][index],
		periodName: `คาบ ${index + 1}`,
		rowVersion: 1,
		schedulingMode: index === 2 ? 'synchronized' : index === 3 ? 'independent' : null,
		seriesId: null,
		structuralKind: index === 1 ? 'flag_ceremony' : null,
		timetableVersionId: id(824),
		title: index === 1 ? 'กิจกรรมพิเศษทดสอบ' : null,
		note: null
	}));
}
const attendance = (studentId: string): AttendanceReport => ({
	archived: false,
	page: 1,
	pageSize: 50,
	total: 1,
	activityCountsAsPresent: true,
	summaries: [
		{
			studentId,
			academicTermId: term,
			displayName: 'นักเรียนทดสอบ',
			category: 'school',
			scopeKey: '',
			scopeLabel: 'มาโรงเรียน',
			present: 7,
			late: 1,
			absent: 2,
			leave: 0,
			activity: 0,
			unchecked: 0,
			expected: 10
		}
	]
});
const attendancePath = (studentId = child) =>
	`/parent/student/${studentId}/attendance?academicYearId=${year}&academicTermId=${term}`;

for (const role of ['student', 'parent'] as const) {
	for (const mobile of [false, true]) {
		test(`${role} timetable activities use shared display at ${mobile ? 'mobile' : 'desktop'} width`, async ({
			page
		}) => {
			if (role === 'student') await mockStudentSchedules(page);
			else await mockParent(page);
			await page.setViewportSize({ width: mobile ? 375 : 1280, height: 900 });
			await page.route(
				role === 'student'
					? '**/api/me/timetable?**'
					: `**/api/parent/students/${child}/timetable?**`,
				(route) => reply(route, blocks())
			);
			await page.goto(
				role === 'student'
					? `/student/timetable?academicYearId=${year}&academicTermId=${term}`
					: childRoute('timetable')
			);
			const workspace = page.getByTestId('personal-timetable');
			await familyExpect(workspace).toBeVisible();
			for (const index of [1, 2]) {
				const lesson = workspace.locator(`[data-personal-timetable-block="${id(800 + index)}"]`);
				await familyExpect(lesson).not.toContainText('ชั้นที่ต้องซ่อน');
				await familyExpect(lesson).not.toContainText('ห้องที่ต้องซ่อน');
			}
			await familyExpect(workspace).not.toContainText('ACTIVITY-CODE');
			await familyExpect(
				workspace.locator(`[data-personal-timetable-block="${id(803)}"]`)
			).toContainText('กิจกรรมอิสระทดสอบ');
			await familyExpect(workspace).toContainText('COURSE-CODE');
			await familyExpect
				.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
				.toBe(true);
		});
	}
}

for (const theme of ['light', 'dark'] as const) {
	for (const role of ['student', 'parent'] as const) {
		test(`${role} summary and real destinations remain usable on mobile in ${theme}`, async ({
			page
		}) => {
			if (role === 'student') await mockStudentSchedules(page);
			else await mockParent(page);
			await ownPermission(page, role);
			await page.route(
				role === 'student' ? '**/api/student/profile?**' : `**/api/parent/students/${child}?**`,
				(route) => reply(route, profile(role === 'student' ? actor : child))
			);
			await page.addInitScript((mode) => {
				localStorage.setItem('ui-preferences', JSON.stringify({ theme: mode }));
			}, theme);
			await page.setViewportSize({ width: 375, height: 844 });
			await page.goto(role === 'student' ? `/student?academicYearId=${year}` : childRoute());
			await familyExpect(page.getByText('5471', { exact: true })).toBeVisible();
			await familyExpect
				.poll(() => page.evaluate(() => document.documentElement.classList.contains('dark')))
				.toBe(theme === 'dark');
			await familyExpect(
				page.locator('dt').filter({ hasText: 'เลขที่' }).locator('..')
			).toContainText('11');
			await familyExpect(
				page.getByRole('link', { name: 'ดูการเช็คชื่อ', exact: true })
			).toHaveAttribute(
				'href',
				role === 'student'
					? `/student/attendance?academicYearId=${year}&academicTermId=${term}`
					: attendancePath()
			);
			await familyExpect(page.getByRole('link', { name: 'ดูปฏิทิน', exact: true })).toHaveAttribute(
				'href',
				new RegExp(role === 'student' ? '/student/calendar' : `/parent/student/${child}/calendar`)
			);
			await familyExpect(page.getByText('ยังไม่เปิดใช้งาน', { exact: true })).toHaveCount(0);
			await familyExpect
				.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
				.toBe(true);
			const mobileHeight = await page
				.getByRole('main')
				.evaluate((main) => Math.ceil(main.scrollHeight + innerHeight - main.clientHeight));
			await page.setViewportSize({ width: 375, height: mobileHeight });
			await page.screenshot({
				path: `/tmp/family-${role}-${theme}-mobile.png`,
				fullPage: true,
				animations: 'disabled'
			});
			await page.setViewportSize({ width: 1280, height: 900 });
			await familyExpect
				.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
				.toBe(true);
			await familyExpect(
				page.getByRole('link', { name: 'ดูการเช็คชื่อ', exact: true })
			).toBeVisible();
			await page.screenshot({
				path: `/tmp/family-${role}-${theme}-desktop.png`,
				fullPage: true,
				animations: 'disabled'
			});
		});
	}
}

test('child attendance report retries independently and only requests the selected child', async ({
	page
}) => {
	const api = await mockParent(page, { hold: 'profile' });
	await ownPermission(page, 'parent');
	const reads: URL[] = [];
	await page.route('**/api/attendance/report?**', (route) => {
		const url = new URL(route.request().url());
		reads.push(url);
		return reads.length === 1
			? reply(route, 'ผลเช็คชื่อไม่พร้อม', 503)
			: reply(route, attendance(child));
	});
	await page.goto(attendancePath());
	await page.getByRole('button', { name: 'ลองโหลดข้อมูลใหม่', exact: true }).click();
	await familyExpect(page.getByRole('button', { name: 'ของนักเรียน', exact: true })).toBeVisible();
	expect(reads).toHaveLength(2);
	expect(
		reads.every(
			(url) =>
				url.searchParams.get('studentId') === child &&
				url.searchParams.get('academicTermId') === term
		)
	).toBe(true);
	expect(api.count('profile')).toBe(1);
	expect(api.count('context')).toBe(1);
	api.release();
	await familyExpect(page.getByTestId('child-attendance-profile')).toContainText('นักเรียนคนแรก');
	await familyExpect(page.getByRole('heading', { name: 'ล้างรายละเอียดภาคเรียน' })).toHaveCount(0);
});

test('denied child history never starts an attendance report', async ({ page }) => {
	await mockParent(page);
	await ownPermission(page, 'parent');
	let reports = 0;
	await page.route('**/api/attendance/report?**', (route) => {
		reports++;
		return reply(route, attendance(deniedChild));
	});
	await page.goto(attendancePath(deniedChild));
	await familyExpect(
		page.getByRole('button', { name: 'ลองบริบทอีกครั้ง', exact: true })
	).toBeVisible();
	expect(reports).toBe(0);
});

test('child change rejects late attendance responses from the previous child', async ({ page }) => {
	await mockParent(page);
	await ownPermission(page, 'parent');
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route('**/api/attendance/report?**', async (route) => {
		const selected = new URL(route.request().url()).searchParams.get('studentId')!;
		if (selected === child) await held;
		return reply(route, attendance(selected));
	});
	await page.goto(attendancePath());
	await familyExpect(page.getByTestId('child-attendance-profile')).toContainText('นักเรียนคนแรก');
	await page.evaluate((href) => {
		const link = document.createElement('a');
		link.href = href;
		link.textContent = 'เปลี่ยนนักเรียน';
		document.querySelector('main')?.append(link);
	}, attendancePath(otherChild));
	await page.getByRole('link', { name: 'เปลี่ยนนักเรียน', exact: true }).click();
	await familyExpect(page.getByTestId('child-attendance-profile')).toContainText('นักเรียนคนสอง');
	release();
	await familyExpect(page.getByRole('button', { name: 'ของนักเรียน', exact: true })).toBeVisible();
	await familyExpect(page.getByTestId('child-attendance-profile')).not.toContainText(
		'นักเรียนคนแรก'
	);
});

test('logout while child context is pending never starts dependent attendance reads', async ({
	page
}) => {
	const api = await mockParent(page, { hold: 'context' });
	await ownPermission(page, 'parent');
	let reports = 0;
	await page.route('**/api/attendance/report?**', (route) => {
		reports++;
		return reply(route, attendance(child));
	});
	await page.goto(attendancePath());
	await familyExpect.poll(() => api.count('context')).toBe(1);
	await page.getByRole('banner').getByRole('button').last().click();
	await page.getByRole('menuitem', { name: 'ออกจากระบบ' }).click();
	await familyExpect(page).toHaveURL(new URL('/', page.url()).href);
	api.release();
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
	expect(reports).toBe(0);
});

test('a parent without own attendance access cannot load child attendance regions', async ({
	page
}) => {
	const api = await mockParent(page);
	await ownPermission(page, 'parent', []);
	let reports = 0;
	await page.route('**/api/attendance/report?**', (route) => {
		reports++;
		return reply(route, attendance(child));
	});
	await page.goto(attendancePath());
	await familyExpect(page).toHaveURL(/\/403/);
	expect(api.count('context')).toBe(0);
	expect(api.count('profile')).toBe(0);
	expect(reports).toBe(0);
});

test('account replacement disposes the previous child report before its delayed response', async ({
	page
}) => {
	await mockParent(page);
	await ownPermission(page, 'parent');
	let contexts = 0;
	await page.route(`**/api/parent/students/${child}/academic-context/options`, (route) =>
		++contexts === 1
			? route.fallback()
			: reply(route, 'บัญชีใหม่ไม่มีสิทธิ์เข้าถึงนักเรียนคนนี้', 403)
	);
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	let reports = 0;
	await page.route('**/api/attendance/report?**', async (route) => {
		reports++;
		await held;
		const report = attendance(child);
		report.summaries[0].scopeKey = 'synthetic-previous-account';
		report.summaries[0].scopeLabel = 'ข้อมูลบัญชีเดิม';
		return reply(route, report);
	});
	await page.goto(attendancePath());
	await familyExpect.poll(() => reports).toBe(1);
	await page.evaluate(async (replacementId) => {
		const modulePath = '/src/lib/stores/auth.ts';
		const { authStore } = await import(modulePath);
		authStore.setUser(
			{
				id: replacementId,
				firstName: 'ผู้ปกครองใหม่',
				lastName: 'ทดสอบ',
				role: 'parent',
				user_type: 'parent',
				status: 'active'
			},
			['attendance.read.own']
		);
	}, id(999));
	await familyExpect.poll(() => contexts).toBe(2);
	const pending = page.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/attendance/report'
	);
	release();
	await pending;
	await familyExpect(
		page.getByText('บัญชีใหม่ไม่มีสิทธิ์เข้าถึงนักเรียนคนนี้', { exact: true })
	).toBeVisible();
	await familyExpect(page.getByText('ข้อมูลบัญชีเดิม', { exact: true })).toHaveCount(0);
	await familyExpect(page.getByRole('button', { name: 'ของนักเรียน', exact: true })).toHaveCount(0);
	expect(reports).toBe(1);
});
