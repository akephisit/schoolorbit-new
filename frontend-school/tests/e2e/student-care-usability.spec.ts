import { mkdir } from 'node:fs/promises';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { PERMISSIONS } from '../../src/lib/permissions/registry';
import {
	mockStudentCare,
	carePath,
	careStudent,
	careVersionId,
	careRoundId,
	careAssignmentId,
	careVisitId,
	careCaseId,
	careLongStudentName,
	careLongStaffName,
	careDefinition
} from './fixtures/student-care-route-data';
import { actor, id, year } from './fixtures/staff-home-route-data';

type Region =
	| { role: 'heading' | 'link' | 'button' | 'region'; name: string }
	| { text: string }
	| { label: string };
type CareRoute = {
	key: string;
	path: string;
	title: string;
	regions: Region[];
	current?: string;
	userType?: 'student' | 'parent';
	screenshot?: boolean;
};
const routes: CareRoute[] = [
	{
		key: 'overview',
		path: carePath(),
		title: 'ดูแลช่วยเหลือนักเรียน',
		current: 'ภาพรวม',
		screenshot: true,
		regions: [
			{ role: 'region', name: 'สรุปงานดูแล' },
			{ text: 'รอส่งแบบประเมิน' },
			{ text: 'การดูแลที่ยังเปิด' },
			{ text: 'แบบทดสอบสมมติ' }
		]
	},
	{
		key: 'library',
		path: carePath('/library'),
		title: 'คลังแบบประเมิน',
		current: 'คลังแบบประเมิน',
		regions: [{ text: 'แบบทดสอบสมมติ' }, { role: 'link', name: 'ดูแบบประเมิน' }]
	},
	{
		key: 'library-editor',
		path: carePath('/library/new'),
		title: 'เพิ่มรุ่นแบบประเมิน',
		screenshot: true,
		regions: [
			{ label: 'ชื่อแบบประเมิน *' },
			{ role: 'heading', name: '3. คะแนนและเกณฑ์แปลผล' },
			{ role: 'button', name: 'บันทึกรุ่นใหม่' }
		]
	},
	{
		key: 'library-detail',
		path: carePath(`/library/${careVersionId}`),
		title: 'รายละเอียดแบบประเมิน',
		regions: [
			{ role: 'heading', name: 'แบบทดสอบสมมติ' },
			{ text: 'Synthetic nonclinical fixture' },
			{ text: 'มาตรสมมติ' }
		]
	},
	{
		key: 'rounds',
		path: carePath('/rounds'),
		title: 'รอบประเมิน',
		current: 'รอบประเมิน',
		regions: [{ text: 'รอบทดสอบการดูแล' }, { role: 'link', name: 'ดูรอบประเมิน' }]
	},
	{
		key: 'round-new',
		path: carePath('/rounds/new'),
		title: 'สร้างรอบประเมิน',
		regions: [
			{ label: 'ชื่อรอบ *' },
			{ label: 'วัตถุประสงค์และการใช้ข้อมูล *' },
			{ text: 'แบบทดสอบสมมติ' }
		]
	},
	{
		key: 'round-detail',
		path: carePath(`/rounds/${careRoundId}`),
		title: 'รายละเอียดรอบประเมิน',
		regions: [
			{ role: 'heading', name: 'รอบทดสอบการดูแล' },
			{ role: 'link', name: 'มอบหมายผู้ตอบ' },
			{ text: 'แบบทดสอบสมมติ' }
		]
	},
	{
		key: 'assignment',
		path: carePath(`/rounds/${careRoundId}/assign`),
		title: 'มอบหมายแบบประเมิน',
		screenshot: true,
		regions: [
			{ role: 'button', name: 'รุ่นแบบประเมิน' },
			{ label: 'ค้นหานักเรียน' },
			{ text: careLongStudentName },
			{ text: careLongStaffName },
			{ role: 'button', name: 'มอบหมายแบบประเมิน' }
		]
	},
	{
		key: 'staff-inbox',
		path: carePath('/assignments'),
		title: 'แบบประเมินของฉัน',
		current: 'แบบประเมินของฉัน',
		regions: [{ text: 'แบบทดสอบสมมติ' }, { text: careLongStudentName }]
	},
	{
		key: 'student-history',
		path: carePath(`/students/${careStudent}`),
		title: 'ประวัติการดูแลรายบุคคล',
		regions: [
			{ text: 'แบบทดสอบสมมติ' },
			{ role: 'link', name: 'ดูบันทึกเยี่ยมบ้าน' },
			{ role: 'link', name: 'ดูการดูแล' }
		]
	},
	{
		key: 'assessment-history',
		path: carePath(`/students/${careStudent}/assessments`),
		title: 'ประวัติแบบประเมินรายบุคคล',
		regions: [
			{ role: 'button', name: 'สถานะ' },
			{ text: 'แบบทดสอบสมมติ' },
			{ text: careLongStudentName }
		]
	},
	{
		key: 'visits',
		path: carePath('/visits'),
		title: 'เยี่ยมบ้าน',
		current: 'เยี่ยมบ้าน',
		regions: [{ text: careLongStudentName }, { role: 'link', name: 'ดูบันทึกเยี่ยมบ้าน' }]
	},
	{
		key: 'visit-planning',
		path: carePath('/visits/new'),
		title: 'วางแผนเยี่ยมบ้าน',
		screenshot: true,
		regions: [
			{ label: 'ค้นหานักเรียน' },
			{ text: careLongStudentName },
			{ text: careLongStaffName },
			{ label: 'ข้อเท็จจริงที่สังเกต' },
			{ role: 'button', name: 'บันทึกเยี่ยมบ้าน' }
		]
	},
	{
		key: 'visit-detail',
		path: carePath(`/visits/${careVisitId}`),
		title: 'รายละเอียดเยี่ยมบ้าน',
		screenshot: true,
		regions: [
			{ role: 'heading', name: careLongStudentName },
			{ text: 'นัดหมาย' },
			{ role: 'heading', name: 'บันทึกการเยี่ยมบ้าน' },
			{ role: 'link', name: 'แก้ไขบันทึก' }
		]
	},
	{
		key: 'visit-edit',
		path: carePath(`/visits/${careVisitId}/edit`),
		title: 'บันทึกเยี่ยมบ้าน',
		regions: [
			{ text: careLongStudentName },
			{ text: careLongStaffName },
			{ label: 'ข้อเท็จจริงที่สังเกต' },
			{ role: 'button', name: 'บันทึกเยี่ยมบ้าน' }
		]
	},
	{
		key: 'cases',
		path: carePath('/cases'),
		title: 'การดูแลรายกรณี',
		current: 'การดูแลรายกรณี',
		regions: [
			{ text: 'การดูแลสมมติ' },
			{ text: careLongStudentName },
			{ role: 'link', name: 'ดูการดูแล' }
		]
	},
	{
		key: 'case-new',
		path: carePath('/cases/new'),
		title: 'เปิดการดูแลรายกรณี',
		regions: [
			{ label: 'เรื่องที่ดูแล *' },
			{ label: 'รายละเอียด / เหตุผล *' },
			{ text: careLongStudentName },
			{ text: careLongStaffName },
			{ role: 'button', name: 'เปิดการดูแล' }
		]
	},
	{
		key: 'case-detail',
		path: carePath(`/cases/${careCaseId}`),
		title: 'รายละเอียดการดูแล',
		screenshot: true,
		regions: [
			{ role: 'heading', name: 'การดูแลสมมติ' },
			{ text: careLongStudentName },
			{ text: 'บันทึกสมมติ 0' },
			{ label: 'รายละเอียดการดำเนินการ *' }
		]
	},
	{
		key: 'case-edit',
		path: carePath(`/cases/${careCaseId}/edit`),
		title: 'แก้ผู้รับผิดชอบและแผนการดูแล',
		regions: [
			{ label: 'เรื่องที่ดูแล *' },
			{ label: 'รายละเอียด / เหตุผล *' },
			{ text: careLongStudentName },
			{ text: careLongStaffName },
			{ role: 'button', name: 'บันทึกผู้รับผิดชอบและแผน' }
		]
	},
	{
		key: 'student-inbox',
		path: '/student/student-care',
		title: 'แบบประเมินของฉัน',
		userType: 'student',
		screenshot: true,
		regions: [
			{ text: 'แบบทดสอบสมมติ' },
			{ text: careLongStudentName },
			{ role: 'link', name: 'เปิดแบบประเมิน' }
		]
	},
	{
		key: 'parent-inbox',
		path: '/parent/student-care',
		title: 'แบบประเมินสำหรับผู้ปกครอง',
		userType: 'parent',
		screenshot: true,
		regions: [
			{ text: 'แบบทดสอบสมมติ' },
			{ text: careLongStudentName },
			{ role: 'link', name: 'เปิดแบบประเมิน' }
		]
	},
	{
		key: 'response',
		path: `/student-care/assignments/${careAssignmentId}`,
		title: 'แบบประเมินเพื่อการดูแล',
		userType: 'student',
		screenshot: true,
		regions: [
			{ role: 'heading', name: 'แบบทดสอบสมมติ' },
			{ text: careLongStudentName },
			{ text: 'คำถามสมมติข้อหนึ่ง' },
			{ text: 'คำถามสมมติข้อสอง' },
			{ role: 'button', name: 'บันทึกร่าง' }
		]
	}
];

function feature(page: Page) {
	return page
		.getByRole('main')
		.getByRole('heading', { level: 1 })
		.locator('xpath=ancestor::header/..');
}

async function scrollableStudentList(page: Page) {
	const checkbox = page
		.getByRole('group', { name: 'นักเรียน *', exact: true })
		.getByRole('checkbox')
		.first();
	const handle = await checkbox.evaluateHandle((element) => {
		let ancestor = element.parentElement;
		while (ancestor) {
			if (
				/auto|scroll/.test(getComputedStyle(ancestor).overflowY) &&
				ancestor.scrollHeight > ancestor.clientHeight
			)
				return ancestor;
			ancestor = ancestor.parentElement;
		}
		throw new Error('Expected a bounded, natively scrollable student list');
	});
	const pane = handle.asElement();
	if (!pane) throw new Error('Scrollable student list is missing');
	const outerScroll = () =>
		pane.evaluate((element) => {
			let total = 0;
			let ancestor = element.parentElement;
			while (ancestor) {
				total += ancestor.scrollTop;
				ancestor = ancestor.parentElement;
			}
			return total;
		});
	return { pane, outerScroll };
}
async function noDocumentOverflow(page: Page) {
	await expect
		.poll(() => page.evaluate(() => document.documentElement.scrollWidth - innerWidth))
		.toBeLessThanOrEqual(1);
}
async function namedControls(content: Locator) {
	const snapshot = await content.ariaSnapshot();
	expect(snapshot, 'Every visible interactive control needs an accessible name').not.toMatch(
		/^\s*-\s+(?:button|link|textbox|combobox|spinbutton|checkbox|radio)(?=\s|:|$)(?!\s+")/m
	);
}
function regionLocator(content: Locator, region: Region) {
	const locator =
		'text' in region
			? content.getByText(region.text, { exact: false })
			: 'label' in region
				? content.getByLabel(region.label, { exact: true })
				: content.getByRole(region.role, { name: region.name, exact: true });
	return locator.first();
}
async function regionVisible(content: Locator, region: Region) {
	const locator = regionLocator(content, region);
	await expect(locator).toBeVisible();
	await locator.scrollIntoViewIfNeeded();
	await expect(locator).toBeInViewport();
}

for (const viewport of [
	{ name: 'desktop', width: 1280, height: 800 },
	{ name: 'mobile', width: 375, height: 812 }
]) {
	for (const theme of ['light', 'dark'] as const) {
		for (const route of routes) {
			test(`${route.key} renders usable content at ${viewport.name} in ${theme}`, async ({
				page
			}) => {
				await page.setViewportSize(viewport);
				await page.addInitScript(
					(value) => localStorage.setItem('ui-preferences', JSON.stringify({ theme: value })),
					theme
				);
				await mockStudentCare(page, {
					longNames: true,
					manyPeople: true,
					userType: route.userType,
					permissions: route.userType ? [PERMISSIONS.STUDENT_CARE_READ_OWN] : undefined
				});
				await page.goto(route.path);
				await expect(page.getByRole('main').getByRole('heading', { level: 1 })).toHaveText(
					route.title
				);
				await expect(page.locator('html')).toHaveClass(
					theme === 'dark' ? /dark/ : /^(?!.*\bdark\b)/
				);
				const content = feature(page);
				for (const region of route.regions) await regionVisible(content, region);
				await namedControls(content);
				const headerIcons = content.locator('header svg');
				expect(await headerIcons.count()).toBeGreaterThan(0);
				expect(
					await headerIcons.evaluateAll((icons) =>
						icons.every((icon) => !!icon.closest('[aria-hidden="true"]'))
					)
				).toBe(true);
				if (route.current) {
					const nav = content.getByRole('navigation', { name: 'งานดูแลช่วยเหลือนักเรียน' });
					await expect(nav.locator('[aria-current="page"]')).toHaveCount(1);
					await expect(nav.getByRole('link', { name: route.current, exact: true })).toHaveAttribute(
						'aria-current',
						'page'
					);
					for (const link of await nav.getByRole('link').all()) {
						expect((await link.innerText()).trim()).not.toBe('');
						await expect(link.locator('svg')).toHaveAttribute('aria-hidden', 'true');
					}
				}
				await noDocumentOverflow(page);
				if (route.screenshot) {
					await mkdir('/tmp/student-care-ux', { recursive: true });
					await page.getByRole('main').evaluate((element) => {
						element.scrollTop = 0;
					});
					await page.screenshot({
						path: `/tmp/student-care-ux/${route.key}-${viewport.name}-${theme}.png`,
						fullPage: true
					});
					for (const [index, region] of route.regions.entries()) {
						if (index < route.regions.length - 2) continue;
						const target = regionLocator(content, region);
						await target.scrollIntoViewIfNeeded();
						await expect(target).toBeInViewport();
						await page.screenshot({
							path: `/tmp/student-care-ux/${route.key}-${viewport.name}-${theme}-region-${index + 1}.png`
						});
					}
				}
			});
		}
	}
}

test('care navigation follows keyboard order and marks the destination current', async ({
	page
}) => {
	await mockStudentCare(page);
	await page.goto(carePath());
	const nav = page.getByRole('navigation', { name: 'งานดูแลช่วยเหลือนักเรียน' });
	await nav.getByRole('link', { name: 'ภาพรวม', exact: true }).focus();
	await page.keyboard.press('Tab');
	await expect(nav.getByRole('link', { name: 'แบบประเมินของฉัน', exact: true })).toBeFocused();
	await page.keyboard.press('Enter');
	await expect(page.getByRole('main').getByRole('heading', { level: 1 })).toHaveText(
		'แบบประเมินของฉัน'
	);
	await expect(nav.getByRole('link', { name: 'แบบประเมินของฉัน', exact: true })).toHaveAttribute(
		'aria-current',
		'page'
	);
	await nav.getByRole('link', { name: 'คลังแบบประเมิน', exact: true }).focus();
	await page.keyboard.press('Enter');
	await expect(page.getByRole('main').getByRole('heading', { level: 1 })).toHaveText(
		'คลังแบบประเมิน'
	);
	await expect(nav.getByRole('link', { name: 'คลังแบบประเมิน', exact: true })).toHaveAttribute(
		'aria-current',
		'page'
	);
});

test('student and staff selections survive search, empty results and student paging', async ({
	page
}) => {
	const api = await mockStudentCare(page, { manyPeople: true, longNames: true });
	await page.setViewportSize({ width: 375, height: 812 });
	await page.goto(carePath(`/rounds/${careRoundId}/assign`));
	await page.getByRole('button', { name: 'รุ่นแบบประเมิน', exact: true }).focus();
	await page.keyboard.press('Enter');
	await page.keyboard.press('ArrowDown');
	await page.keyboard.press('Enter');
	await expect(page.getByRole('button', { name: 'รุ่นแบบประเมิน', exact: true })).toContainText(
		'แบบทดสอบสมมติ'
	);
	const students = page.getByRole('group', { name: 'นักเรียน *', exact: true });
	await students.getByRole('checkbox', { name: new RegExp(careLongStudentName) }).click();
	await expect(
		students.getByRole('checkbox', { name: new RegExp(careLongStudentName) })
	).toBeChecked();
	await students.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(students.getByRole('checkbox', { name: /นักเรียนสมมติ 51/ })).toBeVisible();
	await expect(
		students.getByRole('button', { name: `นำ ${careLongStudentName} ออก`, exact: true })
	).toBeVisible();
	await students.getByRole('button', { name: 'หน้าแรก', exact: true }).click();
	await expect(
		students.getByRole('checkbox', { name: new RegExp(careLongStudentName) })
	).toBeChecked();
	await students.getByLabel('ค้นหานักเรียน', { exact: true }).fill('0002');
	await students.getByLabel('ค้นหานักเรียน', { exact: true }).press('Enter');
	const secondStudent = students.getByRole('checkbox', { name: /นักเรียนสมมติ 02/ });
	await secondStudent.focus();
	await page.keyboard.press('Space');
	await expect(secondStudent).toBeChecked();
	await students.getByLabel('ค้นหานักเรียน', { exact: true }).fill('ไม่ตรงกับรายชื่อสมมติ');
	await students.getByLabel('ค้นหานักเรียน', { exact: true }).press('Enter');
	await expect(students.getByText('ไม่พบรายชื่อตามคำค้น', { exact: true })).toBeVisible();
	await expect(students.getByRole('button', { name: /^นำ .+ ออก$/ })).toHaveCount(2);
	await students.getByLabel('ค้นหานักเรียน', { exact: true }).fill('');
	await students.getByLabel('ค้นหานักเรียน', { exact: true }).press('Enter');
	await expect(students.getByRole('checkbox', { name: /นักเรียนสมมติ 02/ })).toBeChecked();
	const staff = page.getByRole('group', { name: 'ผู้รับผิดชอบติดตามผล', exact: true });
	await staff.getByRole('checkbox', { name: careLongStaffName, exact: true }).focus();
	await page.keyboard.press('Space');
	await staff.getByLabel('ค้นหาผู้รับผิดชอบติดตามผล', { exact: true }).fill('ครูสมมติ 32');
	await expect(staff.getByRole('checkbox')).toHaveCount(1);
	await staff.getByRole('checkbox', { name: 'ครูสมมติ 32', exact: true }).click();
	await staff.getByLabel('ค้นหาผู้รับผิดชอบติดตามผล', { exact: true }).fill('');
	await expect(staff.getByRole('checkbox', { name: careLongStaffName, exact: true })).toBeChecked();
	await expect(staff.getByRole('checkbox', { name: 'ครูสมมติ 32', exact: true })).toBeChecked();
	await noDocumentOverflow(page);
	await page.getByRole('button', { name: 'มอบหมายแบบประเมิน', exact: true }).click();
	await expect(page.getByText('มอบหมายแล้ว 1 รายการ', { exact: true })).toBeVisible();
	expect(api.assignments[0].targets).toEqual([
		{ studentId: careStudent, respondentId: careStudent },
		{ studentId: id(5001), respondentId: id(5001) }
	]);
	expect(api.assignments[0].reviewerUserIds).toEqual([actor, id(6029)]);
});

for (const viewport of [
	{ width: 375, height: 812 },
	{ width: 1280, height: 800 }
]) {
	test(`bounded picker permits native page scrolling at both boundaries at ${viewport.width}px`, async ({
		page
	}) => {
		const api = await mockStudentCare(page, { manyPeople: true });
		await page.setViewportSize(viewport);
		await page.goto(carePath(`/rounds/${careRoundId}/assign`));
		const { pane, outerScroll } = await scrollableStudentList(page);
		await pane.evaluate((element) => {
			element.scrollIntoView({ block: 'center' });
			element.scrollTop = element.scrollHeight;
		});
		await pane.hover();
		const bottomBefore = await outerScroll();
		await page.mouse.wheel(0, 450);
		await expect.poll(outerScroll).toBeGreaterThan(bottomBefore);
		await pane.evaluate((element) => {
			element.scrollIntoView({ block: 'center' });
			element.scrollTop = 0;
		});
		await pane.hover();
		const topBefore = await outerScroll();
		expect(topBefore).toBeGreaterThan(0);
		await page.mouse.wheel(0, -450);
		await expect.poll(outerScroll).toBeLessThan(topBefore);
		expect(api.assignments).toHaveLength(0);
		expect(api.responses).toHaveLength(0);
	});
}

test.describe('emulated mobile touch', () => {
	test.use({ hasTouch: true, isMobile: true, viewport: { width: 375, height: 812 } });
	test('picker supports tap selection, clearing, retention and native touch scrolling at both boundaries', async ({
		page,
		context
	}) => {
		const api = await mockStudentCare(page, { manyPeople: true });
		await page.goto(carePath(`/rounds/${careRoundId}/assign`));
		const students = page.getByRole('group', { name: 'นักเรียน *', exact: true });
		const first = students.getByRole('checkbox', { name: /นักเรียนทดสอบ หนึ่ง/ });
		await first.tap();
		await expect(first).toBeChecked();
		await students.getByRole('button', { name: 'นำ นักเรียนทดสอบ หนึ่ง ออก', exact: true }).tap();
		await expect(first).not.toBeChecked();
		await first.tap();
		await students.getByLabel('ค้นหานักเรียน', { exact: true }).fill('0002');
		await students.getByRole('button', { name: 'ค้นหา', exact: true }).tap();
		const second = students.getByRole('checkbox', { name: /นักเรียนสมมติ 02/ });
		await second.tap();
		await expect(second).toBeChecked();
		await expect(students.getByRole('button', { name: /^นำ .+ ออก$/ })).toHaveCount(2);
		await students.getByLabel('ค้นหานักเรียน', { exact: true }).fill('');
		await students.getByRole('button', { name: 'ค้นหา', exact: true }).tap();
		await expect(first).toBeChecked();
		await expect(second).toBeChecked();
		const { pane, outerScroll } = await scrollableStudentList(page);
		const session = await context.newCDPSession(page);
		const settleFrames = () =>
			page.evaluate(
				() =>
					new Promise<void>((resolve) =>
						requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
					)
			);
		async function swipe(distance: number) {
			const bounds = await pane.boundingBox();
			if (!bounds) throw new Error('Touch scroll target is not rendered');
			const x = bounds.x + bounds.width / 2;
			const y = bounds.y + bounds.height / 2;
			await session.send('Input.dispatchTouchEvent', {
				type: 'touchStart',
				touchPoints: [{ x, y, id: 1, radiusX: 1, radiusY: 1, force: 1 }]
			});
			for (let step = 1; step <= 12; step++) {
				await settleFrames();
				await session.send('Input.dispatchTouchEvent', {
					type: 'touchMove',
					touchPoints: [
						{ x, y: y + (distance * step) / 12, id: 1, radiusX: 1, radiusY: 1, force: 1 }
					]
				});
			}
			await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
			await settleFrames();
		}
		try {
			await pane.evaluate((element) => {
				element.scrollIntoView({ block: 'center' });
				element.scrollTop = (element.scrollHeight - element.clientHeight) / 2;
			});
			await settleFrames();
			const innerBefore = await pane.evaluate((element) => element.scrollTop);
			await swipe(-180);
			await expect
				.poll(() => pane.evaluate((element) => element.scrollTop))
				.toBeGreaterThan(innerBefore);
			for (const boundary of ['bottom', 'top'] as const) {
				await pane.evaluate((element, edge) => {
					element.scrollIntoView({ block: 'center' });
					element.scrollTop = edge === 'bottom' ? element.scrollHeight : 0;
				}, boundary);
				await settleFrames();
				const before = await outerScroll();
				if (boundary === 'top') expect(before).toBeGreaterThan(0);
				await swipe(boundary === 'bottom' ? -180 : 180);
				if (boundary === 'bottom') await expect.poll(outerScroll).toBeGreaterThan(before);
				else await expect.poll(outerScroll).toBeLessThan(before);
			}
		} finally {
			await session.detach();
		}
		await expect(first).toBeChecked();
		await expect(second).toBeChecked();
		expect(api.assignments).toHaveLength(0);
		expect(api.responses).toHaveLength(0);
		expect(api.visits).toHaveLength(0);
		expect(api.cases).toHaveLength(0);
		await noDocumentOverflow(page);
	});
});

test('new case validation preserves the factual draft and selected student', async ({ page }) => {
	const api = await mockStudentCare(page);
	await page.goto(carePath('/cases/new'));
	await page.getByRole('checkbox', { name: /นักเรียนทดสอบ หนึ่ง/ }).check();
	await page.getByLabel('เรื่องที่ดูแล *', { exact: true }).fill('หัวข้อสมมติที่ต้องเก็บไว้');
	await page.getByLabel('รายละเอียด / เหตุผล *', { exact: true }).fill('ข้อเท็จจริงสมมติในแบบร่าง');
	await page.getByRole('button', { name: 'เปิดการดูแล', exact: true }).click();
	await expect(
		page.getByText('เลือกนักเรียนและระบุผู้รับผิดชอบอย่างน้อยหนึ่งคน', { exact: true })
	).toBeVisible();
	await expect(page.getByLabel('เรื่องที่ดูแล *', { exact: true })).toHaveValue(
		'หัวข้อสมมติที่ต้องเก็บไว้'
	);
	await expect(page.getByLabel('รายละเอียด / เหตุผล *', { exact: true })).toHaveValue(
		'ข้อเท็จจริงสมมติในแบบร่าง'
	);
	await expect(page.getByRole('checkbox', { name: /นักเรียนทดสอบ หนึ่ง/ })).toBeChecked();
	expect(api.cases).toHaveLength(0);
	await page.getByRole('checkbox', { name: 'ครูผู้รับผิดชอบ', exact: true }).check();
	await page.getByRole('button', { name: 'เปิดการดูแล', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/cases/${careCaseId}`));
	expect(api.cases[0]).toMatchObject({
		title: 'หัวข้อสมมติที่ต้องเก็บไว้',
		note: 'ข้อเท็จจริงสมมติในแบบร่าง',
		studentId: careStudent,
		assignedUserIds: [actor]
	});
});

test('editor uses named aggregate choices and exposes reference recovery without losing its draft', async ({
	page
}) => {
	const api = await mockStudentCare(page);
	const definition = structuredClone(careDefinition);
	definition.scales.push({
		...structuredClone(definition.scales[0]),
		key: 'total',
		label: 'คะแนนรวมสมมติ',
		sourceScaleKeys: ['s']
	});
	await page.goto(carePath('/library/new'));
	await page.getByText('นำเข้า / ส่งออกการตั้งค่า', { exact: true }).click();
	await page.getByLabel('ข้อมูลการตั้งค่า JSON', { exact: true }).fill(JSON.stringify(definition));
	await page.getByRole('button', { name: 'นำเข้าการตั้งค่า', exact: true }).click();
	await expect(page.getByLabel('ชื่อแบบประเมิน *', { exact: true })).toHaveValue(definition.name);
	const aggregate = page.getByRole('group', { name: 'มาตรวัด 2', exact: true });
	await expect(aggregate.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true })).toBeChecked();
	await aggregate.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true }).uncheck();
	await aggregate.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true }).focus();
	await page.keyboard.press('Space');
	await expect(aggregate.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true })).toBeChecked();
	await expect(page.getByLabel('รหัสมาตรวัด 1', { exact: true })).not.toBeVisible();
	await page.getByText('รหัสอ้างอิงมาตรวัด', { exact: true }).first().click();
	await page.getByLabel('รหัสมาตรวัด 1', { exact: true }).fill('renamed_s');
	await page.getByRole('button', { name: 'บันทึกรุ่นใหม่', exact: true }).click();
	const errors = page.getByRole('alert').filter({ hasText: 'ตรวจสอบการตั้งค่า' });
	await expect(errors).toBeVisible();
	await expect(errors.getByRole('link').first()).toBeVisible();
	await expect(page.getByLabel('ชื่อแบบประเมิน *', { exact: true })).toHaveValue(definition.name);
	await expect(page.getByLabel('คำถามข้อ 1', { exact: true })).toHaveValue(
		definition.questions[0].text
	);
	expect(api.versions).toHaveLength(0);
	await expect(page.getByLabel('รหัสมาตรวัดที่จะรวม 2', { exact: true })).toBeVisible();
	await page.getByLabel('รหัสมาตรวัดที่จะรวม 2', { exact: true }).fill('renamed_s');
	await expect(aggregate.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true })).toBeChecked();
	for (const number of [1, 2]) {
		await page.getByRole('button', { name: `มาตรวัดข้อ ${number}`, exact: true }).click();
		await page.getByRole('option', { name: 'มาตรสมมติ', exact: true }).click();
	}
	await page.getByRole('button', { name: 'บันทึกรุ่นใหม่', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/library/${careVersionId}`));
	expect(api.versions[0].definition.scales[1].sourceScaleKeys).toEqual(['renamed_s']);
	expect(api.versions[0].definition.questions.map((question) => question.scaleKey)).toEqual([
		'renamed_s',
		'renamed_s'
	]);
});

test('aggregate source choices offer base scales and exclude other totals', async ({ page }) => {
	const api = await mockStudentCare(page);
	const definition = structuredClone(careDefinition);
	for (const [key, label] of [
		['total_one', 'คะแนนรวมสมมติหนึ่ง'],
		['total_two', 'คะแนนรวมสมมติสอง']
	]) {
		definition.scales.push({
			...structuredClone(definition.scales[0]),
			key,
			label,
			sourceScaleKeys: ['s']
		});
	}
	await page.goto(carePath('/library/new'));
	await page.getByText('นำเข้า / ส่งออกการตั้งค่า', { exact: true }).click();
	await page.getByLabel('ข้อมูลการตั้งค่า JSON', { exact: true }).fill(JSON.stringify(definition));
	await page.getByRole('button', { name: 'นำเข้าการตั้งค่า', exact: true }).click();
	await expect(page.getByLabel('ชื่อแบบประเมิน *', { exact: true })).toHaveValue(definition.name);
	for (const number of [1, 2, 3]) {
		const scale = page.getByRole('group', { name: `มาตรวัด ${number}`, exact: true });
		const sources = scale.getByRole('group', { name: 'รวมคะแนนจากมาตรวัดพื้นฐาน', exact: true });
		await expect(sources).toBeVisible();
		await expect(
			sources.getByRole('checkbox', { name: 'คะแนนรวมสมมติหนึ่ง', exact: true })
		).toHaveCount(0);
		await expect(
			sources.getByRole('checkbox', { name: 'คะแนนรวมสมมติสอง', exact: true })
		).toHaveCount(0);
		if (number === 1) {
			await expect(sources.getByRole('checkbox')).toHaveCount(0);
		} else {
			await expect(sources.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true })).toBeChecked();
		}
	}
	const secondTotal = page.getByRole('group', { name: 'มาตรวัด 3', exact: true });
	await secondTotal.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true }).click();
	await expect(
		secondTotal.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true })
	).not.toBeChecked();
	await secondTotal.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true }).click();
	await expect(secondTotal.getByRole('checkbox', { name: 'มาตรสมมติ', exact: true })).toBeChecked();
	await page.getByRole('button', { name: 'บันทึกรุ่นใหม่', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/library/${careVersionId}`));
	expect(api.versions[0].definition.scales.map((scale) => scale.sourceScaleKeys)).toEqual([
		[],
		['s'],
		['s']
	]);
});

test('case action comes before long history and stays available after loading older notes', async ({
	page
}) => {
	const api = await mockStudentCare(page, { manyEvents: true });
	await page.setViewportSize({ width: 375, height: 812 });
	await page.goto(carePath(`/cases/${careCaseId}`));
	const action = page.getByRole('button', { name: 'บันทึกการดำเนินการ', exact: true });
	const history = page.getByRole('heading', { name: 'ประวัติการดูแล', exact: true });
	await expect(action).toBeVisible();
	const actionY = await action.evaluate((element) => element.getBoundingClientRect().top);
	const historyY = await history.evaluate((element) => element.getBoundingClientRect().top);
	expect(actionY).toBeLessThan(historyY);
	await page
		.getByLabel('รายละเอียดการดำเนินการ *', { exact: true })
		.fill('บันทึกสมมติระหว่างอ่านประวัติ');
	await page.getByRole('button', { name: 'โหลดประวัติก่อนหน้า', exact: true }).click();
	await expect(page.getByText('บันทึกสมมติ 0', { exact: true })).toBeVisible();
	await expect(page.getByLabel('รายละเอียดการดำเนินการ *', { exact: true })).toHaveValue(
		'บันทึกสมมติระหว่างอ่านประวัติ'
	);
	await action.click();
	await expect(page.getByText('บันทึกสมมติระหว่างอ่านประวัติ', { exact: true })).toBeVisible();
	await expect(page.getByText('บันทึกสมมติ 0', { exact: true })).toBeVisible();
	expect(api.events[0].note).toBe('บันทึกสมมติระหว่างอ่านประวัติ');
});

test('clearing case filters preserves student and academic context and removes paging', async ({
	page
}) => {
	await mockStudentCare(page);
	await page.goto(
		`${carePath('/cases')}&studentId=${careStudent}&status=monitoring&followUpBefore=2026-10-18&afterId=${id(9000)}`
	);
	await page
		.getByRole('form', { name: 'ตัวกรองรายการ', exact: true })
		.getByRole('link', { name: 'ล้างตัวกรอง', exact: true })
		.click();
	await expect(page).not.toHaveURL(/status=|followUpBefore=|afterId=/);
	const url = new URL(page.url());
	expect(url.searchParams.get('studentId')).toBe(careStudent);
	expect(url.searchParams.get('academicYearId')).toBe(year);
	expect(url.searchParams.get('academicTermId')).toBeTruthy();
	await expect(page.getByText('การดูแลสมมติ', { exact: true })).toBeVisible();
});

for (const list of [
	{ path: '/library', heading: 'ยังไม่มีแบบประเมิน', action: 'เพิ่มแบบประเมิน' },
	{ path: '/rounds', heading: 'ยังไม่มีรอบประเมิน', action: 'สร้างรอบประเมิน' },
	{ path: '/visits', heading: 'ยังไม่มีรายการเยี่ยมบ้าน', action: 'วางแผนเยี่ยมบ้าน' },
	{ path: '/cases', heading: 'ยังไม่มีการดูแลรายกรณี', action: 'เปิดการดูแล' }
]) {
	test(`empty ${list.path} explains the result and provides its permitted next action`, async ({
		page
	}) => {
		await mockStudentCare(page, { emptyLists: true });
		await page.goto(carePath(list.path));
		await expect(page.getByRole('heading', { name: list.heading, exact: true })).toBeVisible();
		await expect(
			feature(page).getByRole('link', { name: list.action, exact: true }).first()
		).toBeVisible();
		await namedControls(feature(page));
	});
}

test('response progress and validation retain the selected answer until final confirmation', async ({
	page
}) => {
	const api = await mockStudentCare(page, {
		userType: 'student',
		permissions: [PERMISSIONS.STUDENT_CARE_READ_OWN]
	});
	await page.goto(`/student-care/assignments/${careAssignmentId}`);
	await expect(page.getByText(/ตอบแล้ว 0 จาก 2 ข้อ/)).toBeVisible();
	await expect(
		page.getByRole('progressbar', { name: 'ความคืบหน้าการตอบแบบประเมิน', exact: true })
	).toHaveAttribute('aria-valuenow', '0');
	await page
		.getByRole('checkbox', { name: 'อ่านและรับทราบวัตถุประสงค์การประเมินแล้ว', exact: true })
		.check();
	await page.getByRole('radio', { name: 'ตัวเลือกสมมติ ข', exact: true }).focus();
	await page.keyboard.press('Space');
	await expect(page.getByText(/ตอบแล้ว 1 จาก 2 ข้อ/)).toBeVisible();
	await expect(
		page.getByRole('progressbar', { name: 'ความคืบหน้าการตอบแบบประเมิน', exact: true })
	).toHaveAttribute('aria-valuenow', '1');
	await page.getByRole('button', { name: 'ตรวจและส่งแบบประเมิน', exact: true }).click();
	await expect(page.getByRole('alert')).toBeVisible();
	await expect(page.getByRole('radio', { name: 'ตัวเลือกสมมติ ข', exact: true })).toBeChecked();
	expect(api.responses).toHaveLength(0);
	await page.getByRole('radio', { name: 'คำตอบสมมติ หนึ่ง', exact: true }).check();
	await expect(page.getByText(/ตอบแล้ว 2 จาก 2 ข้อ/)).toBeVisible();
	await page.getByRole('button', { name: 'ตรวจและส่งแบบประเมิน', exact: true }).click();
	await page.getByRole('button', { name: 'กลับไปตรวจ', exact: true }).click();
	await expect(page.getByRole('radio', { name: 'ตัวเลือกสมมติ ข', exact: true })).toBeChecked();
	expect(api.responses).toHaveLength(0);
	await page.getByRole('button', { name: 'ล้างคำตอบข้อ 1', exact: true }).click();
	await expect(page.getByRole('radio', { name: 'ตัวเลือกสมมติ ข', exact: true })).not.toBeChecked();
	await expect(page.getByText(/ตอบแล้ว 1 จาก 2 ข้อ/)).toBeVisible();
});

for (const state of [
	{
		key: 'closed round',
		options: { roundStatus: 'closed' as const },
		heading: 'รอบนี้ปิดรับคำตอบแล้ว'
	},
	{ key: 'guardian paper response', options: { manual: true }, heading: 'แบบประเมินกระดาษ' },
	{ key: 'submitted response', options: { submitted: true }, heading: 'คำตอบที่ส่งแล้ว' }
]) {
	test(`${state.key} explains the next step without editable answer controls`, async ({ page }) => {
		const api = await mockStudentCare(page, {
			...state.options,
			userType: 'parent',
			permissions: [PERMISSIONS.STUDENT_CARE_READ_OWN]
		});
		await page.goto(`/student-care/assignments/${careAssignmentId}`);
		await expect(page.getByRole('heading', { name: state.heading, exact: true })).toBeVisible();
		await expect(page.getByRole('radio')).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'บันทึกร่าง', exact: true })).toHaveCount(0);
		await expect(
			page.getByRole('button', { name: 'ตรวจและส่งแบบประเมิน', exact: true })
		).toHaveCount(0);
		expect(api.responses).toHaveLength(0);
	});
}
