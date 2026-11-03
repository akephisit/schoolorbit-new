import { expect, test } from '@playwright/test';
import { mockYearManagement, yearUrl, id, selectOption } from './fixtures/year-management';
test.use({ serviceWorkers: 'block' });
test('direct term close and activate use the same annual roster without result preparation', async ({
	page
}) => {
	const observed = await mockYearManagement(page, { active: true });
	await page.goto(yearUrl);
	await page.getByRole('button', { name: 'ปิดภาคเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันปิดภาคเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'เปลี่ยนภาคเรียน', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText(
		'ภายในปีเดียวกันนักเรียนยังอยู่ชั้นและห้องเดิม'
	);
	await page.getByRole('button', { name: 'ยืนยันเปลี่ยนภาคเรียน', exact: true }).click();
	expect(observed.transitions.map((request) => request.action)).toEqual(['close', 'activate']);
	expect(observed.transitions.every((request) => request.academicYearId === id(1))).toBe(true);
	expect(observed.confirms).toEqual([]);
	expect(observed.reads.some((path) => /result|aggregate/.test(path))).toBe(false);
});
test('legacy term link preserves the year and leads to one canonical workflow', async ({
	page
}) => {
	await mockYearManagement(page);
	await page.goto(`/staff/academic/term-lifecycle?academicYearId=${id(1)}&academicTermId=${id(3)}`);
	await expect(page).toHaveURL(yearUrl);
	await expect(page.getByRole('heading', { name: 'จัดการปีการศึกษา', exact: true })).toBeVisible();
});

test('activation names the accepted period and keeps settings explicit and drafts intact', async ({
	page
}) => {
	const observed = await mockYearManagement(page, {
		active: true,
		readyToActivate: true,
		setupRecords: true
	});
	await page.goto(yearUrl);
	const setup = page.getByRole('region', { name: 'ตั้งค่าปีและภาคเรียน', exact: true });
	await expect(setup.getByLabel('ปีที่ต้องการตั้งค่า', { exact: true })).toContainText(
		'ปีการศึกษา 2570'
	);
	await setup.getByRole('button', { name: 'แก้ไขปีการศึกษา', exact: true }).click();
	await page.getByLabel('ปีการศึกษา (พ.ศ.)', { exact: true }).fill('2575');
	await page.getByRole('button', { name: 'เปลี่ยนภาคเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันเปลี่ยนภาคเรียน', exact: true }).click();
	const guidance = page.locator('[aria-label="ตั้งค่าหลังเริ่มใช้รอบใหม่"]');
	await expect(guidance).toContainText('เริ่มใช้ ปีการศึกษา 2569 · ภาคเรียนที่ 2 แล้ว');
	await expect(guidance).toContainText('เลือก ปีการศึกษา 2569 ด้วยช่อง “ปีที่ต้องการตั้งค่า”');
	await expect(
		guidance.getByRole('link', { name: 'ไปที่ตัวเลือกและค่าเริ่มต้น', exact: true })
	).toHaveAttribute('href', '#academic-year-settings');
	await expect
		.poll(() => observed.reads.filter((path) => path === '/api/academic/setup/workspace').length)
		.toBe(2);
	await expect(page.locator('[data-testid="academic-setup-ready"]')).toHaveAttribute(
		'aria-busy',
		'false'
	);
	expect(observed.settingsWrites).toEqual([]);
	await guidance.getByRole('link', { name: 'ไปที่ตัวเลือกและค่าเริ่มต้น', exact: true }).click();
	await expect(setup.getByLabel('ปีที่ต้องการตั้งค่า', { exact: true })).toContainText(
		'ปีการศึกษา 2570'
	);
	await expect(page.getByLabel('ปีการศึกษา (พ.ศ.)', { exact: true })).toHaveValue('2575');
	await selectOption(page, 'ปีที่ต้องการตั้งค่า', 'ปีการศึกษา 2569 · ใช้งาน');
	const controls = page.getByRole('region', { name: 'ตัวเลือกและค่าเริ่มต้น', exact: true });
	const newTerm = controls.getByRole('group', { name: 'ภาคเรียนที่ 2', exact: true });
	await expect(
		newTerm.getByRole('button', { name: 'ตั้งเป็นค่าเริ่มต้น', exact: true })
	).toBeDisabled();
	await newTerm.getByRole('button', { name: 'แสดงในตัวเลือก', exact: true }).click();
	await newTerm.getByRole('button', { name: 'ตั้งเป็นค่าเริ่มต้น', exact: true }).click();
	expect(observed.settingsWrites).toHaveLength(2);
	expect(observed.settingsWrites[0]).toMatchObject({
		rowVersion: 1,
		defaultAcademicTermId: id(3),
		terms: [{ id: id(4), isVisible: true }]
	});
	expect(observed.settingsWrites[1]).toMatchObject({
		rowVersion: 2,
		defaultAcademicYearId: id(1),
		defaultAcademicTermId: id(4),
		years: [],
		terms: []
	});
});

test('activation-only staff see settings-manager guidance without a settings link or read', async ({
	page
}) => {
	const observed = await mockYearManagement(page, {
		active: true,
		readyToActivate: true,
		activationOnly: true
	});
	await page.goto(yearUrl);
	await page.getByRole('button', { name: 'เปลี่ยนภาคเรียน', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันเปลี่ยนภาคเรียน', exact: true }).click();
	const guidance = page.locator('[aria-label="ตั้งค่าหลังเริ่มใช้รอบใหม่"]');
	await expect(guidance).toContainText('เริ่มใช้ ปีการศึกษา 2569 · ภาคเรียนที่ 2 แล้ว');
	await expect(guidance).toContainText('ให้ผู้ดูแลที่มีสิทธิ์จัดการปีและภาคเรียน');
	await expect(guidance.getByRole('link')).toHaveCount(0);
	expect(observed.reads).not.toContain('/api/academic/setup/workspace');
	expect(observed.settingsWrites).toEqual([]);
});

test('activation groups exact target calendar and capacity guidance without merging findings', async ({
	page
}) => {
	const calendarUrl = `/staff/attendance/settings?academicYearId=${id(1)}&academicTermId=${id(4)}`;
	const capacityUrl = `/staff/academic/homerooms?academicYearId=${id(1)}`;
	await mockYearManagement(page, {
		active: true,
		readyToActivate: true,
		activationFindings: [
			{
				code: 'opening.calendar_settings',
				severity: 'blocking',
				count: 1,
				message: 'ยังไม่บันทึกวันนับเข้าเรียนของภาคเรียนที่ 2',
				resolutionUrl: calendarUrl
			},
			{
				code: 'opening.room_capacity',
				severity: 'blocking',
				count: 3,
				message: 'ม.1/1 มี 43 คน ความจุ 40 คน',
				resolutionUrl: capacityUrl
			},
			{
				code: 'opening.room_capacity',
				severity: 'blocking',
				count: 2,
				message: 'ม.2/1 มี 42 คน ความจุ 40 คน',
				resolutionUrl: capacityUrl
			}
		]
	});
	await page.goto(yearUrl);
	await page.getByRole('button', { name: 'เปลี่ยนภาคเรียน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	const calendar = dialog.getByRole('region', { name: 'ปฏิทินและตารางเวลา', exact: true });
	await expect(calendar).toContainText('ยังไม่บันทึกวันนับเข้าเรียนของภาคเรียนที่ 2');
	await expect(
		calendar.getByRole('link', { name: 'ไปตรวจและแก้ข้อมูล', exact: true })
	).toHaveAttribute('href', calendarUrl);
	const rooms = dialog.getByRole('region', { name: 'นักเรียนและห้องเรียน', exact: true });
	await expect(rooms).toContainText('ม.1/1 มี 43 คน ความจุ 40 คน');
	await expect(rooms).toContainText('ม.2/1 มี 42 คน ความจุ 40 คน');
	await expect(rooms.getByRole('link', { name: 'ไปตรวจและแก้ข้อมูล', exact: true })).toHaveCount(2);
	await expect(
		dialog.getByRole('button', { name: 'ยืนยันเปลี่ยนภาคเรียน', exact: true })
	).toBeDisabled();
});
