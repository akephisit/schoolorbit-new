import { expect, test } from '@playwright/test';
import type { Locator, Page, TestInfo } from '@playwright/test';
import type { MenuGroup } from '../../src/lib/api/menu.js';
import { homePath, id, mockStaffHome } from './fixtures/staff-home-route-data';
import { mockStaffStudents, studentPath } from './fixtures/staff-student-route-data';

const workspaceNames = ['งานวิชาการ', 'งานบุคลากร', 'งานกิจการนักเรียน', 'งานบริหารทั่วไป'];
const menuGroups: MenuGroup[] = [9, 2, 6, 3].map((count, index) => ({
	code: `group-${index}`,
	name: 'บริการในกลุ่มงาน',
	icon: 'LayoutGrid',
	workspaceCode: `workspace-${index}`,
	workspaceName: workspaceNames[index],
	workspaceIcon: 'Building2',
	workspaceOrder: index,
	displayOrder: index,
	items: Array.from({ length: count }, (_, service) => ({
		code: `service-${index}-${service}`,
		id: id(100 + index * 20 + service),
		name: `บริการทดสอบ ${index + 1}.${service + 1}`,
		path: `/staff/profile?service=${index}-${service}`,
		icon: 'FileText'
	}))
}));

async function rect(locator: Locator) {
	const box = await locator.boundingBox();
	expect(box).not.toBeNull();
	return box!;
}
async function noPageOverflow(page: Page) {
	expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
		await page.evaluate(() => window.innerWidth)
	);
}
async function remSize(page: Page) {
	return page.evaluate(() =>
		Number.parseFloat(getComputedStyle(document.documentElement).fontSize)
	);
}

async function captureThemes(page: Page, testInfo: TestInfo, name: string) {
	const viewport = page.viewportSize()!;
	// The app scrolls inside main. Expand only the screenshot viewport to include every card.
	const height = await page
		.getByRole('main')
		.evaluate((main) => Math.ceil(main.scrollHeight + window.innerHeight - main.clientHeight));
	await page.setViewportSize({ width: viewport.width, height });
	await page.screenshot({ path: testInfo.outputPath(`${name}-light.png`), fullPage: true });
	await page.getByRole('button', { name: 'Toggle Dark Mode' }).click();
	await page.screenshot({ path: testInfo.outputPath(`${name}-dark.png`), fullPage: true });
}

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ contentType: 'text/css', body: '' })
	);
});

for (const width of [375, 1440]) {
	test(`unequal workspace cards fill vertical gaps at ${width}px`, async ({ page }, testInfo) => {
		await page.setViewportSize({ width, height: 1000 });
		await mockStaffHome(page, { menuGroups });
		await page.goto(homePath());
		const workspaces = page.getByTestId('staff-service-workspaces');
		const cards = workspaces.locator('[data-workspace]');
		await expect(cards).toHaveCount(4);
		await expect(workspaces.getByRole('link')).toHaveCount(20);
		const boxes = await Promise.all(Array.from({ length: 4 }, (_, i) => rect(cards.nth(i))));
		const spacing = await remSize(page);
		expect(boxes[0].height).toBeGreaterThan(boxes[1].height);
		const columns = new Map<number, typeof boxes>();
		for (const box of boxes) {
			const key = Math.round(box.x);
			columns.set(key, [...(columns.get(key) ?? []), box]);
		}
		expect(columns.size).toBe(width < 1280 ? 1 : 2);
		for (const column of columns.values()) {
			for (let i = 1; i < column.length; i++) {
				const gap = column[i].y - (column[i - 1].y + column[i - 1].height);
				expect(gap).toBeCloseTo(spacing, 0);
			}
		}
		// Every service remains inside its natural-height card, including the final link.
		for (let i = 0; i < boxes.length; i++) {
			const lastLink = await rect(cards.nth(i).getByRole('link').last());
			expect(lastLink.y + lastLink.height).toBeLessThanOrEqual(boxes[i].y + boxes[i].height);
		}
		await noPageOverflow(page);
		await captureThemes(page, testInfo, `dashboard-${width}`);
	});

	test(`student sections remain separated and long values wrap at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 1000 });
		await mockStaffStudents(page, {
			permissions: ['student.read.school'],
			profile: {
				first_name: 'นักเรียนตัวอย่าง',
				last_name: 'สำหรับทดสอบการแสดงผล',
				email: `${'long-contact-'.repeat(8)}@example.invalid`,
				address: 'ที่อยู่ตัวอย่างสำหรับทดสอบการตัดบรรทัดและระยะห่างระหว่างส่วนข้อมูล '.repeat(4),
				date_of_birth: '2012-05-15',
				blood_type: 'O',
				allergies: 'ข้อมูลอาการแพ้ตัวอย่าง',
				medical_conditions: 'ข้อมูลสุขภาพตัวอย่าง'
			}
		});
		await page.goto(studentPath());
		const profile = page.getByTestId('student-profile');
		await expect(profile).toContainText('นักเรียนตัวอย่าง');
		await expect(profile).toContainText('15 พ.ค. 2555');
		await expect(profile).toContainText('ไม่มีสิทธิ์ดูข้อมูลส่วนบุคคล');
		await expect(page.getByRole('link', { name: 'แก้ไข', exact: true })).toHaveCount(0);
		const ids = [
			'student-summary',
			'student-basic',
			'student-contact',
			'student-academic',
			'student-health'
		];
		const boxes = await Promise.all(ids.map((name) => rect(page.getByTestId(name))));
		const minimumGap = (await remSize(page)) * 1.5 - 0.5;
		for (let a = 0; a < boxes.length; a++) {
			for (let b = a + 1; b < boxes.length; b++) {
				const first = boxes[a],
					second = boxes[b];
				const horizontalGap = Math.max(
					second.x - first.x - first.width,
					first.x - second.x - second.width
				);
				const verticalGap = Math.max(
					second.y - first.y - first.height,
					first.y - second.y - second.height
				);
				expect(Math.max(horizontalGap, verticalGap)).toBeGreaterThanOrEqual(minimumGap);
			}
		}
		const history = await rect(page.getByTestId('student-placement-history'));
		expect(history.y - Math.max(...boxes.map((box) => box.y + box.height))).toBeGreaterThanOrEqual(
			minimumGap
		);
		await noPageOverflow(page);
		await captureThemes(page, testInfo, `student-${width}`);
	});
}
