import { sessionState } from './fixtures/session-state';
import { mkdir } from 'node:fs/promises';
import { expect, test, type Page, type Route } from '@playwright/test';
import type {
	StaffPublishedExamDay,
	StaffPublishedExamScheduleRound,
	StaffPublishedExamSession
} from '../../src/lib/api/examSchedule.js';
import {
	DEFAULT_APPEARANCE,
	schoolAppearanceCss,
	type SchoolAppearanceResponse
} from '../../src/lib/appearance/model.js';

test.use({ serviceWorkers: 'block', timezoneId: 'Asia/Bangkok' });

const year = '10000000-0000-4000-8000-000000000011';
const term = '20000000-0000-4000-8000-000000000011';
const staff = '90000000-0000-4000-8000-000000000011';
const url = `/staff/exams?academicYearId=${year}&academicTermId=${term}`;
const subject = 'คณิตศาสตร์พื้นฐานและกระบวนการแก้ปัญหาอย่างสร้างสรรค์';
const appearance: SchoolAppearanceResponse = {
	appearance: { ...DEFAULT_APPEARANCE, font: 'noto', primary: '#fff2c4' },
	revision: 4
};

function day(id: string, date: string, upper = false): StaffPublishedExamDay {
	const homeroomId = `class-${upper ? 'upper' : 'lower'}`;
	const homeroomName = upper ? 'ม.4/2' : 'ม.1/1';
	const assignmentId = `assignment-${id}`;
	const session = (index: number): StaffPublishedExamSession => ({
		sessionId: `${id}-session-${index}`,
		dayRoomAssignmentId: assignmentId,
		assessmentPhaseName: index === 1 ? 'สอบกลางภาค' : 'สอบเพิ่มเติม',
		subjectId: 'subject-1',
		subjectName: subject,
		subjectCode: 'ค21101',
		gradeLevelId: `grade-${upper ? 4 : 1}`,
		gradeLevelName: upper ? 'มัธยมศึกษาปีที่ 4' : 'มัธยมศึกษาปีที่ 1',
		gradeLevelType: 'secondary',
		gradeLevelYear: upper ? 4 : 1,
		homeroomId,
		homeroomName,
		learningGroupId: 'group-1',
		learningOfferingId: 'offering-1',
		roomId: 'room-1',
		roomName: 'ห้อง 101',
		buildingName: 'อาคารเรียนวิทยาศาสตร์และเทคโนโลยี',
		startsAt: index === 1 ? '09:00:00' : '13:00:00',
		endsAt: index === 1 ? '10:00:00' : '14:30:00',
		durationMinutes: index === 1 ? 60 : 90
	});
	return {
		examDayId: id,
		examDate: date,
		label: upper ? 'สอบช่วงชั้นปลาย' : 'สอบช่วงชั้นต้น',
		sessions: [session(1), session(2)],
		roomAssignments: [
			{
				assignmentId,
				homeroomId,
				homeroomName,
				roomId: 'room-1',
				roomName: 'ห้อง 101',
				buildingName: 'อาคารเรียนวิทยาศาสตร์และเทคโนโลยี',
				earliestStartsAt: '09:00:00',
				latestEndsAt: '14:30:00',
				sessionMinutes: 150,
				invigilators: [
					{ staffId: staff, displayName: 'นางสาวทดสอบ บุคลากรชื่อยาวสำหรับตรวจสอบการตัดบรรทัด' },
					{ staffId: 'staff-2', displayName: 'นายสมมติ ตัวอย่าง' }
				]
			}
		]
	};
}

function rounds(): StaffPublishedExamScheduleRound[] {
	return [
		{
			roundId: 'round-main',
			roundName: 'กลางภาค · รอบหลัก',
			academicTermId: term,
			publishedAt: '2026-10-01T00:00:00Z',
			days: [day('past', '2026-10-19'), day('next', '2026-10-21'), day('upper', '2026-10-22', true)]
		},
		{
			roundId: 'round-other',
			roundName: 'รอบไม่มีงานคุมของฉัน',
			academicTermId: term,
			publishedAt: '2026-10-01T00:00:00Z',
			days: [{ ...day('other', '2026-10-23'), roomAssignments: [] }]
		},
		{
			roundId: 'round-empty',
			roundName: 'รอบที่ยังไม่มีรายการ',
			academicTermId: term,
			publishedAt: '2026-10-01T00:00:00Z',
			days: []
		}
	];
}

async function fulfill(route: Route, data: unknown) {
	await route.fulfill({
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

async function install(page: Page, mode: 'light' | 'dark' = 'light', values = rounds()) {
	const writes: string[] = [];
	const forbidden: string[] = [];
	let reads = 0;
	await page.clock.setFixedTime(new Date('2026-10-20T05:00:00Z'));
	await page.addInitScript((theme) => {
		localStorage.setItem('ui-preferences', JSON.stringify({ theme }));
	}, mode);
	await page.route('**/school-appearance.css', (route) =>
		route.fulfill({ contentType: 'text/css', body: schoolAppearanceCss(appearance) })
	);
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const request = route.request();
			const pathname = new URL(request.url()).pathname;
			if (request.method() !== 'GET') writes.push(`${request.method()} ${pathname}`);
			if (pathname === '/api/auth/me') {
				await fulfill(route, {
					id: staff,
					username: 'synthetic-exam-reader',
					firstName: 'ทดสอบ',
					lastName: 'ตารางสอบ',
					displayName: 'ทดสอบ ตารางสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					permissions: []
				});
			} else if (pathname === '/api/academic/context/options') {
				await fulfill(route, {
					defaultAcademicYearId: year,
					defaultAcademicTermId: term,
					years: [{
 isVisible: true, id: year, name: 'ปีการศึกษา 2569', year: 2569, status: 'active' }],
					terms: [
						{
 isVisible: true,
							id: term,
							academicYearId: year,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
			} else if (pathname === '/api/staff/exam-schedules') {
				reads += 1;
				await fulfill(route, values);
			} else if (pathname === '/api/menu/user') await fulfill(route, { groups: [] });
			else if (pathname === '/api/school/public/appearance') await fulfill(route, appearance);
			else if (pathname === '/api/school/public')
				await fulfill(route, { schoolName: 'โรงเรียนทดสอบ' });
			else if (pathname === '/api/notifications/stream')
				await route.fulfill({ contentType: 'text/event-stream', body: '' });
			else if (pathname === '/api/notifications')
				await fulfill(route, { items: [], unread_count: 0 });
			else if (pathname === '/api/me/work-items/counts')
				await fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
			else {
				if (/exam|staff\/manage|school\/settings/.test(pathname)) forbidden.push(pathname);
				await fulfill(route, []);
			}
		}
	);
	return { writes, forbidden, reads: () => reads };
}

async function selectRound(page: Page, name: string) {
	await page.getByRole('button', { name: 'เลือกรอบสอบ' }).click();
	await page.getByRole('option', { name, exact: true }).click();
}

async function noPageOverflow(page: Page) {
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
}

test('four tabs keep their own scope, clear dependent filters, and allow leaving an empty round', async ({
	page
}) => {
	await page.setViewportSize({ width: 1029, height: 888 });
	const requests = await install(page);
	await page.goto(url);
	await expect(page.getByRole('tab', { name: 'ภาพรวม', exact: true })).toHaveAttribute(
		'aria-selected',
		'true'
	);
	await expect(page.getByRole('region', { name: 'ตัวกรองตารางสอบ' })).toHaveCount(0);
	const toolbar = await page.getByTestId('exam-round-toolbar').boundingBox();
	const tabs = await page.getByRole('tablist').boundingBox();
	expect(toolbar && tabs && tabs.y - toolbar.y - toolbar.height).toBeLessThanOrEqual(20);
	await page.getByRole('tab', { name: 'ตารางสอบ', exact: true }).click();
	await expect(
		page.getByRole('table', { name: 'ตารางสอบ', exact: true }).getByRole('row')
	).toHaveCount(7);
	await page.getByRole('button', { name: 'กรองตามวันสอบ' }).click();
	await page.getByRole('option', { name: /21 ต.ค./ }).click();
	await expect(
		page.getByRole('table', { name: 'ตารางสอบ', exact: true }).getByRole('row')
	).toHaveCount(3);
	await page.getByRole('button', { name: 'ล้างตัวกรอง', exact: true }).click();
	await page.getByRole('radio', { name: 'ม.1–ม.3', exact: true }).click();
	await page.getByRole('radio', { name: 'ม.1–ม.3', exact: true }).click();
	await expect(page.getByRole('radio', { name: 'ม.1–ม.3', exact: true })).toHaveAttribute(
		'data-state',
		'on'
	);
	await page.getByRole('button', { name: 'กรองตามชั้นเรียน' }).click();
	await expect(page.getByRole('option', { name: 'ม.4/2', exact: true })).toHaveCount(0);
	await page.getByRole('option', { name: 'ม.1/1', exact: true }).click();
	await page.getByRole('radio', { name: 'ม.4–ม.6', exact: true }).click();
	await expect(page.getByRole('button', { name: 'กรองตามชั้นเรียน' })).toHaveText('ทุกชั้นเรียน');
	await expect(
		page.getByRole('table', { name: 'ตารางสอบ', exact: true }).getByRole('row')
	).toHaveCount(3);
	await page.getByRole('textbox', { name: 'ค้นหาวิชา ห้อง หรือกรรมการคุมสอบ' }).fill('ไม่พบแน่นอน');
	await expect(page.getByText('ไม่พบตารางสอบตามตัวกรอง', { exact: true })).toBeVisible();
	await page.getByRole('tab', { name: 'งานคุมของฉัน', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ตัวกรองตารางสอบ' })).toHaveCount(0);
	const duties = page.getByRole('list', { name: 'งานคุมสอบของฉัน', exact: true });
	const dutyRows = duties
		.getByRole('listitem')
		.filter({ has: page.getByRole('heading', { level: 2 }) });
	await expect(dutyRows).toHaveCount(3);
	await expect(dutyRows.first().getByRole('heading')).toContainText('21 ต.ค. 2569');
	await expect(dutyRows.first().getByText('กำลังมาถึง', { exact: true })).toBeVisible();
	await expect(dutyRows.last().getByRole('heading')).toContainText('19 ต.ค. 2569');
	await expect(dutyRows.last().getByText('เสร็จแล้ว', { exact: true })).toBeVisible();
	await expect(page.getByRole('definition').filter({ hasText: '7 ชม. 30 นาที' })).toBeVisible();
	await expect(duties.getByText('เสร็จแล้ว', { exact: true })).toBeVisible();
	await expect(duties.getByText('กำลังมาถึง', { exact: true })).toHaveCount(2);
	await expect(
		duties.getByText('09:00–14:30 · เวลาคุมจริง 2 ชม. 30 นาที', { exact: true })
	).toHaveCount(3);
	await page.getByRole('tab', { name: 'กรรมการคุมสอบ', exact: true }).click();
	await expect(page.getByText('ไม่พบกรรมการคุมสอบตามตัวกรอง', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ล้างตัวกรอง', exact: true }).first().click();
	await expect(
		page.getByRole('table', { name: 'กรรมการคุมสอบ', exact: true }).getByRole('row')
	).toHaveCount(4);
	await expect(
		page.getByRole('table', { name: 'กรรมการคุมสอบ', exact: true }).getByText('ผู้ใช้ปัจจุบัน')
	).toHaveCount(3);
	await page.getByRole('textbox', { name: 'ค้นหาวิชา ห้อง หรือกรรมการคุมสอบ' }).fill('ไม่พบแน่นอน');
	await selectRound(page, 'รอบที่ยังไม่มีรายการ');
	await expect(page.getByText('รอบสอบนี้ยังไม่มีรายการสอบ', { exact: true })).toBeVisible();
	await selectRound(page, 'กลางภาค · รอบหลัก');
	await expect(page.getByRole('textbox', { name: 'ค้นหาวิชา ห้อง หรือกรรมการคุมสอบ' })).toHaveValue(
		''
	);
	await selectRound(page, 'รอบไม่มีงานคุมของฉัน');
	await page.getByRole('tab', { name: 'งานคุมของฉัน', exact: true }).click();
	await expect(page.getByText('ยังไม่มีงานคุมสอบของฉัน', { exact: true })).toBeVisible();
	await noPageOverflow(page);
	expect(requests.reads()).toBe(1);
	expect(requests.writes).toEqual([]);
	expect(requests.forbidden).toEqual([]);
});

for (const width of [1029, 390]) {
	for (const mode of ['light', 'dark'] as const) {
		test(`${width}px ${mode}: compact navigation, readable phase and duties, school appearance`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 888 });
			const requests = await install(page, mode);
			await page.goto(url);
			const roundControl = page.getByRole('button', { name: 'เลือกรอบสอบ' });
			await expect(roundControl).toBeVisible();
			await expect(roundControl).toHaveCSS('font-family', /School Noto Sans Thai/);
			await expect(page.locator('html')).toHaveClass(mode === 'dark' ? /dark/ : /^(?!.*dark)/);
			await expect
				.poll(() =>
					page.evaluate(() =>
						getComputedStyle(document.documentElement).getPropertyValue('--primary-fill').trim()
					)
				)
				.toBe('#fff2c4');
			const overview = page.getByRole('tab', { name: 'ภาพรวม', exact: true });
			await overview.focus();
			await page.keyboard.press('ArrowRight');
			await page.keyboard.press('Enter');
			await expect(page.getByRole('tab', { name: 'ตารางสอบ', exact: true })).toHaveAttribute(
				'aria-selected',
				'true'
			);
			if (width === 390) {
				await expect(page.getByRole('button', { name: 'กรองตามวันสอบ' })).toBeHidden();
				await page.getByRole('button', { name: 'ตัวกรอง', exact: true }).click();
				await page.getByRole('radio', { name: 'ม.4–ม.6', exact: true }).click();
				await page.getByRole('button', { name: 'ตัวกรอง', exact: true }).click();
				await expect(
					page.getByText('ทุกวันสอบ · ม.4–ม.6 · ทุกชั้นเรียน', { exact: true })
				).toBeVisible();
				await expect(page.getByText('ประเภทการสอบ: สอบเพิ่มเติม', { exact: true })).toBeVisible();
				await expect(page.getByText('ประเภทการสอบ: สอบกลางภาค', { exact: true })).toBeVisible();
			} else {
				const table = page.getByRole('table', { name: 'ตารางสอบ', exact: true });
				const overflow = table.locator('..');
				expect(
					await overflow.evaluate((element) => element.scrollWidth > element.clientWidth)
				).toBe(true);
				await overflow.evaluate((element) => {
					element.scrollLeft = element.scrollWidth;
				});
				expect(await overflow.evaluate((element) => element.scrollLeft)).toBeGreaterThan(0);
			}
			await mkdir('/workspace/artifacts/staff-schedules', { recursive: true });
			await page.screenshot({
				path: `/workspace/artifacts/staff-schedules/exams-${width}-${mode}-schedule.png`,
				fullPage: true
			});
			await page.getByRole('tab', { name: 'กรรมการคุมสอบ', exact: true }).click();
			await expect(page.getByRole('tabpanel')).toContainText('ฉัน');
			await noPageOverflow(page);
			await page.screenshot({
				path: `/workspace/artifacts/staff-schedules/exams-${width}-${mode}-invigilators.png`,
				fullPage: true
			});
			await page.getByRole('tab', { name: 'งานคุมของฉัน', exact: true }).click();
			await expect(page.getByRole('list', { name: 'งานคุมสอบของฉัน', exact: true })).toBeVisible();
			await noPageOverflow(page);
			await page.screenshot({
				path: `/workspace/artifacts/staff-schedules/exams-${width}-${mode}-mine.png`,
				fullPage: true
			});
			await page.getByRole('tab', { name: 'ภาพรวม', exact: true }).click();
			await noPageOverflow(page);
			await page.screenshot({
				path: `/workspace/artifacts/staff-schedules/exams-${width}-${mode}-overview.png`,
				fullPage: true
			});
			expect(requests.reads()).toBe(1);
			expect(requests.writes).toEqual([]);
			expect(requests.forbidden).toEqual([]);
		});
	}
}
