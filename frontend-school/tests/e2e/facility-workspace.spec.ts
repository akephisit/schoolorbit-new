import { expect, test, type Page } from '@playwright/test';
import type { Building, Room } from '../../src/lib/api/facility';
import { mockFacility, buildingsPath, roomsPath } from './fixtures/facility-route-data';
import { id } from './fixtures/staff-home-route-data';

test.use({ serviceWorkers: 'block', hasTouch: true });
const path = '/staff/facility/buildings';
const longName = 'อาคารศูนย์การเรียนรู้วิทยาศาสตร์และเทคโนโลยีเพื่อการพัฒนาชุมชนอย่างยั่งยืน';
const longCode = 'BUILDING-CODE-WITH-A-LONG-UNBROKEN-IDENTIFIER-1234567890';
const timestamp = '2026-10-01T00:00:00Z';

function building(index: number): Building {
	return {
		id: id(300 + index),
		name_th: index === 0 ? longName : `อาคารทดสอบ ${index}`,
		name_en: `Learning building ${index}`,
		code: index === 0 ? longCode : `building-${index}`,
		description:
			'พื้นที่สำหรับการเรียนรู้และกิจกรรมของนักเรียน พร้อมข้อมูลรายละเอียดที่อ่านได้ครบถ้วน',
		created_at: timestamp,
		updated_at: timestamp
	};
}
function room(index: number): Room {
	return {
		id: id(400 + index),
		building_id: id(300),
		building_name: longName,
		name_th:
			index === 0
				? 'ห้องปฏิบัติการวิทยาศาสตร์และเทคโนโลยีบูรณาการสำหรับนักเรียน'
				: `ห้องทดสอบ ${index}`,
		name_en: `Learning room ${index}`,
		code: index === 0 ? `ROOM-${longCode}` : `room-${index}`,
		room_type: 'LAB',
		capacity: 0,
		floor: 0,
		status: 'active',
		description: 'รายละเอียดห้องทดสอบ',
		created_at: timestamp,
		updated_at: timestamp
	};
}
async function openRooms(page: Page, name = 'อาคารแรก') {
	await page.getByRole('button', { name, exact: true }).click();
	await expect(page.getByTestId('facility-rooms')).toContainText('ห้อง');
}
async function noOverflow(page: Page) {
	expect(
		await page.evaluate(
			() => document.documentElement.scrollWidth - document.documentElement.clientWidth
		)
	).toBeLessThanOrEqual(1);
}
async function screenshot(page: Page, part: string, width: number, theme: string) {
	await noOverflow(page);
	await page.screenshot({
		path: `/workspace/artifacts/facility-room-names/facility-${part}-${width}-${theme}.png`,
		fullPage: true,
		animations: 'disabled'
	});
}

for (const width of [390, 1029]) {
	for (const theme of ['light', 'dark']) {
		test(`compact building-to-room navigation and full CRUD at ${width}px in ${theme}`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 888 });
			const api = await mockFacility(page, {
				buildings: [building(0), building(1)],
				rooms: [room(0), room(1)]
			});
			await page.addInitScript(
				(mode) => localStorage.setItem('ui-preferences', JSON.stringify({ theme: mode })),
				theme
			);
			await page.goto(path);
			await expect(page.getByTestId('facility-buildings')).toContainText(longName);
			await expect
				.poll(() => page.locator('html').evaluate((element) => element.classList.contains('dark')))
				.toBe(theme === 'dark');
			const buildings = page.getByTestId(
				width < 768 ? 'facility-buildings-list' : 'facility-buildings-table'
			);
			await expect(buildings).toBeVisible();
			await expect(buildings).not.toContainText(longCode);
			await expect(page.getByRole('tab')).toHaveCount(0);
			expect(api.count(roomsPath)).toBe(0);
			await screenshot(page, 'buildings', width, theme);
			await page.getByRole('button', { name: 'เพิ่มอาคาร', exact: true }).tap();
			let dialog = page.getByRole('dialog', { name: 'เพิ่มอาคารใหม่', exact: true });
			await expect(dialog.locator('input[name=code]')).toHaveCount(0);
			await dialog.getByLabel('ชื่ออาคาร (ไทย)').fill('อาคารเพิ่มใหม่');
			await dialog.getByLabel('ชื่ออาคาร (อังกฤษ)').fill('New building');
			await dialog.getByLabel('รายละเอียด', { exact: true }).fill('รายละเอียดอาคารใหม่');
			await screenshot(page, 'building-form', width, theme);
			await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			await expect(buildings).toContainText('อาคารเพิ่มใหม่');
			await buildings
				.getByRole('button', { name: 'แก้ไขอาคาร อาคารเพิ่มใหม่', exact: true })
				.click();
			dialog = page.getByRole('dialog', { name: 'แก้ไขอาคาร', exact: true });
			await dialog.getByLabel('ชื่ออาคาร (ไทย)').fill('อาคารแก้ไขใหม่');
			await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			await expect(buildings).toContainText('อาคารแก้ไขใหม่');
			await openRooms(page, longName);
			const rooms = page.getByTestId(width < 768 ? 'facility-rooms-list' : 'facility-rooms-table');
			await expect(rooms).toBeVisible();
			await expect(rooms).not.toContainText(`ROOM-${longCode}`);
			await expect(rooms).not.toContainText('Learning room');
			await expect(rooms).toContainText('รายละเอียดห้องทดสอบ');
			await expect(
				page.getByRole('heading', { level: 1, name: longName, exact: true })
			).toBeVisible();
			await screenshot(page, 'rooms', width, theme);
			await page.getByRole('button', { name: 'เพิ่มห้อง', exact: true }).click();
			dialog = page.getByRole('dialog', { name: 'เพิ่มห้องใหม่', exact: true });
			const buildingTrigger = dialog.getByLabel('อาคาร', { exact: true });
			await buildingTrigger.click();
			await expect(page.getByRole('option', { name: 'อาคารแก้ไขใหม่', exact: true })).toBeVisible();
			await page.getByRole('option', { name: longName, exact: true }).click();
			await expect(buildingTrigger).toContainText(longName);
			const selectedGeometry = await buildingTrigger.evaluate((element) => ({
				width: element.clientWidth,
				fullWidth: element.scrollWidth,
				labelWidth: element.firstElementChild!.clientWidth,
				labelFullWidth: element.firstElementChild!.scrollWidth
			}));
			expect(selectedGeometry.fullWidth).toBeLessThanOrEqual(selectedGeometry.width + 1);
			expect(selectedGeometry.labelFullWidth).toBeGreaterThan(selectedGeometry.labelWidth);
			const triggerBox = (await buildingTrigger.boundingBox())!;
			const dialogBox = (await dialog.boundingBox())!;
			expect(triggerBox.x).toBeGreaterThanOrEqual(dialogBox.x + 16);
			expect(triggerBox.x + triggerBox.width).toBeLessThanOrEqual(
				dialogBox.x + dialogBox.width - 16
			);
			await expect(
				dialog.locator('input[name=code], input[name=name_en], input[name=floor]')
			).toHaveCount(0);
			await dialog.getByLabel('ชื่อห้อง (ไทย)').fill('ห้องเพิ่มใหม่');
			await dialog.getByLabel('ความจุ (คน)', { exact: true }).fill('0');
			const box = (await dialog.boundingBox())!;
			expect(box.width).toBeLessThanOrEqual(width - 16);
			expect(box.height).toBeLessThanOrEqual(888 - 16);
			await screenshot(page, 'room-form', width, theme);
			await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			expect(api.writes.at(-1)?.payload).toMatchObject({
				capacity: 0,
				building_id: building(0).id
			});
			await rooms.getByRole('button', { name: 'แก้ไขห้อง ห้องเพิ่มใหม่', exact: true }).click();
			dialog = page.getByRole('dialog', { name: 'แก้ไขห้อง', exact: true });
			await expect(dialog.getByLabel('ความจุ (คน)', { exact: true })).toHaveValue('0');
			await dialog.getByLabel('ชื่อห้อง (ไทย)').fill('ห้องแก้ไขใหม่');
			await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
			await expect(dialog).toHaveCount(0);
			expect(api.writes.at(-1)?.payload).toMatchObject({
				capacity: 0,
				building_id: building(0).id
			});
			await rooms.getByRole('button', { name: 'ลบห้อง ห้องแก้ไขใหม่', exact: true }).click();
			let confirmation = page.getByRole('alertdialog', { name: 'ยืนยันการลบ', exact: true });
			await confirmation.getByRole('button', { name: 'ยืนยันลบ', exact: true }).click();
			await expect(confirmation).toHaveCount(0);
			await expect(rooms).not.toContainText('ห้องแก้ไขใหม่');
			await page.getByRole('link', { name: 'กลับรายการอาคาร', exact: true }).click();
			await buildings.getByRole('button', { name: 'ลบอาคาร อาคารแก้ไขใหม่', exact: true }).click();
			confirmation = page.getByRole('alertdialog', { name: 'ยืนยันการลบ', exact: true });
			await confirmation.getByRole('button', { name: 'ยืนยันลบ', exact: true }).click();
			await expect(confirmation).toHaveCount(0);
			await expect(buildings).not.toContainText('อาคารแก้ไขใหม่');
			expect(api.writes).toHaveLength(6);
			expect(api.count(buildingsPath)).toBe(1);
			expect(api.count(roomsPath)).toBe(1);
			await noOverflow(page);
		});
	}
}

for (const kind of ['building', 'room', 'delete'] as const) {
	test(`${kind} failures stay visible with retained draft and focused retry`, async ({ page }) => {
		const api = await mockFacility(page, { fail: 'mutation' });
		await page.goto(path);
		await expect(page.getByTestId('facility-buildings')).toContainText('อาคารแรก');
		if (kind === 'room') await openRooms(page);
		await page
			.getByRole('button', {
				name:
					kind === 'room'
						? 'แก้ไขห้อง ห้องแรก'
						: kind === 'delete'
							? 'ลบอาคาร อาคารแรก'
							: 'แก้ไขอาคาร อาคารแรก',
				exact: true
			})
			.click();
		const dialog = page.getByRole(kind === 'delete' ? 'alertdialog' : 'dialog');
		if (kind !== 'delete') await dialog.locator('input[name=name_th]').fill('ร่างที่ต้องรักษา');
		const save = dialog.getByRole('button', {
			name: kind === 'delete' ? 'ยืนยันลบ' : 'บันทึก',
			exact: true
		});
		await save.click();
		await expect(dialog.getByRole('alert')).toContainText('ส่วนนี้ไม่พร้อม');
		if (kind !== 'delete')
			await expect(dialog.locator('input[name=name_th]')).toHaveValue('ร่างที่ต้องรักษา');
		await save.click();
		await expect(dialog).toHaveCount(0);
		expect(api.count('mutation')).toBe(2);
		expect(api.count(buildingsPath)).toBe(1);
		expect(api.count(roomsPath)).toBe(kind === 'room' ? 1 : 0);
	});
}

test('pending room save refuses duplicate writes and cannot close a newer room draft', async ({
	page
}) => {
	const api = await mockFacility(page, { hold: 'mutation' });
	await page.goto(path);
	await openRooms(page);
	await page.getByRole('button', { name: 'แก้ไขห้อง ห้องแรก', exact: true }).click();
	let dialog = page.getByRole('dialog');
	await dialog.locator('input[name=name_th]').fill('ห้องบันทึกก่อนหน้า');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog.getByRole('button', { name: 'กำลังบันทึก...' })).toBeDisabled();
	await dialog.locator('form').evaluate((element) => (element as HTMLFormElement).requestSubmit());
	expect(api.count('mutation')).toBe(1);
	await page.keyboard.press('Escape');
	await expect(dialog).toHaveCount(0);
	await page.getByRole('button', { name: 'เพิ่มห้อง', exact: true }).click();
	dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อห้อง (ไทย)').fill('ร่างใหม่ที่ต้องคงอยู่');
	api.release();
	await expect(page.getByTestId('facility-rooms')).toContainText('ห้องบันทึกก่อนหน้า');
	await expect(dialog.getByLabel('ชื่อห้อง (ไทย)')).toHaveValue('ร่างใหม่ที่ต้องคงอยู่');
	await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
	expect(api.count('mutation')).toBe(1);
});

for (const capability of ['read', 'create', 'update', 'delete']) {
	test(`facility ${capability} permission exposes only its authorized controls`, async ({
		page
	}) => {
		const api = await mockFacility(page, {
			permissions:
				capability === 'read'
					? ['facility.read.all']
					: ['facility.read.all', `facility.${capability}.all`]
		});
		await page.goto(path);
		await expect(page.getByTestId('facility-buildings')).toContainText('อาคารแรก');
		for (const kind of ['อาคาร', 'ห้อง']) {
			if (kind === 'ห้อง') await openRooms(page);
			await expect(page.getByRole('button', { name: `เพิ่ม${kind}`, exact: true })).toHaveCount(
				capability === 'create' ? 1 : 0
			);
			await expect(page.getByRole('button', { name: new RegExp(`^แก้ไข${kind} `) })).toHaveCount(
				capability === 'update' ? (kind === 'ห้อง' ? 1 : 2) : 0
			);
			await expect(page.getByRole('button', { name: new RegExp(`^ลบ${kind} `) })).toHaveCount(
				capability === 'delete' ? (kind === 'ห้อง' ? 1 : 2) : 0
			);
		}
		expect(api.writes).toHaveLength(0);
	});
}

test('client pages cover the complete filtered dataset without extra reads', async ({ page }) => {
	const api = await mockFacility(page, {
		buildings: Array.from({ length: 31 }, (_, index) => building(index)),
		rooms: Array.from({ length: 31 }, (_, index) => room(index))
	});
	await page.goto(path);
	const buildings = page.getByTestId('facility-buildings-table');
	await expect(buildings.getByRole('row')).toHaveCount(26);
	await page
		.getByRole('navigation', { name: 'หน้ารายการอาคาร' })
		.getByRole('button', { name: 'ถัดไป' })
		.click();
	await expect(buildings.getByRole('row')).toHaveCount(7);
	await page.getByRole('textbox', { name: 'ค้นหาอาคาร', exact: true }).fill('อาคารทดสอบ 30');
	await expect(buildings).toContainText('อาคารทดสอบ 30');
	await expect(buildings.getByRole('row')).toHaveCount(2);
	expect(api.count(buildingsPath)).toBe(1);
	await page.getByLabel('ค้นหาอาคาร', { exact: true }).fill('');
	await openRooms(page, longName);
	const rooms = page.getByTestId('facility-rooms-table');
	await expect(rooms.getByRole('row')).toHaveCount(26);
	await page
		.getByRole('navigation', { name: 'หน้ารายการห้อง' })
		.getByRole('button', { name: 'ถัดไป' })
		.click();
	await expect(rooms.getByRole('row')).toHaveCount(7);
	expect(api.count(roomsPath)).toBe(1);
	await page.getByRole('textbox', { name: 'ค้นหาห้อง', exact: true }).fill('ห้องทดสอบ 30');
	await expect(rooms).toContainText('ห้องทดสอบ 30');
	await expect(rooms.getByRole('row')).toHaveCount(2);
	expect(api.count(roomsPath)).toBe(1);
	await noOverflow(page);
});

test('empty building and unassigned-room regions remain usable and malformed capacity cannot write', async ({
	page
}) => {
	const api = await mockFacility(page, { buildings: [], rooms: [] });
	await page.goto(path);
	await expect(page.getByText('ไม่พบข้อมูลอาคาร', { exact: true })).toBeVisible();
	await page.getByRole('link', { name: 'ห้องที่ยังไม่จัดอาคาร', exact: true }).click();
	await expect(page.getByText('ไม่พบข้อมูลห้อง', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'เพิ่มห้อง', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อห้อง (ไทย)').fill('ร่างห้อง');
	await expect(dialog.getByLabel('ความจุ (คน)', { exact: true })).toHaveValue('40');
	await dialog.getByLabel('ความจุ (คน)', { exact: true }).fill('1.5');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	expect(
		await dialog
			.getByLabel('ความจุ (คน)', { exact: true })
			.evaluate((element) => (element as HTMLInputElement).validity.stepMismatch)
	).toBe(true);
	await expect(dialog.getByLabel('ชื่อห้อง (ไทย)')).toHaveValue('ร่างห้อง');
	expect(api.writes).toHaveLength(0);
});

test('editing visible fields preserves stored codes, room English names, floor and zero capacity', async ({
	page
}) => {
	const originalBuilding = building(0),
		originalRoom = room(0);
	const api = await mockFacility(page, { buildings: [originalBuilding], rooms: [originalRoom] });
	await page.goto(path);
	await page.getByRole('button', { name: `แก้ไขอาคาร ${longName}`, exact: true }).click();
	let dialog = page.getByRole('dialog');
	await expect(dialog.locator('input[name=code]')).toHaveCount(0);
	await dialog.getByLabel('รายละเอียด', { exact: true }).fill('รายละเอียดใหม่');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(api.writes.at(-1)?.payload).not.toHaveProperty('code');
	expect(api.snapshot().buildings[0].code).toBe(originalBuilding.code);
	await openRooms(page, longName);
	await page
		.getByRole('button', { name: `แก้ไขห้อง ${originalRoom.name_th}`, exact: true })
		.click();
	dialog = page.getByRole('dialog');
	await expect(
		dialog.locator('input[name=code], input[name=name_en], input[name=floor]')
	).toHaveCount(0);
	await expect(dialog.getByLabel('ความจุ (คน)', { exact: true })).toHaveValue('0');
	await dialog.getByLabel('ชื่อห้อง (ไทย)').fill('ห้องเปลี่ยนชื่อ');
	await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	for (const hidden of ['code', 'name_en', 'floor'])
		expect(api.writes.at(-1)?.payload).not.toHaveProperty(hidden);
	expect(api.snapshot().rooms[0]).toMatchObject({
		id: originalRoom.id,
		name_th: 'ห้องเปลี่ยนชื่อ',
		code: originalRoom.code,
		name_en: originalRoom.name_en,
		floor: 0,
		capacity: 0,
		building_id: originalBuilding.id
	});
	expect(api.count(buildingsPath)).toBe(1);
	expect(api.count(roomsPath)).toBe(1);
});

test('unassigned rooms filter the complete authorized result before name search and pagination', async ({
	page
}) => {
	const assigned = Array.from({ length: 31 }, (_, index) => room(index));
	const unassigned = assigned.map((item, index) => ({
		...item,
		id: id(900 + index),
		building_id: null,
		building_name: null,
		name_th: `ห้องรอจัดอาคาร ${index}`
	}));
	const api = await mockFacility(page, {
		buildings: [building(0)],
		rooms: assigned.flatMap((item, index) => [item, unassigned[index]])
	});
	await page.goto(path);
	expect(api.count(roomsPath)).toBe(0);
	await page.getByRole('link', { name: 'ห้องที่ยังไม่จัดอาคาร', exact: true }).click();
	const rooms = page.getByTestId('facility-rooms-table');
	await expect(rooms.getByRole('row')).toHaveCount(26);
	await expect(rooms.locator('tbody tr td:first-child')).toHaveText(
		Array.from({ length: 25 }, (_, index) => `ห้องรอจัดอาคาร ${index}`)
	);
	await page
		.getByRole('navigation', { name: 'หน้ารายการห้อง' })
		.getByRole('button', { name: 'ถัดไป', exact: true })
		.click();
	await expect(rooms.getByRole('row')).toHaveCount(7);
	await expect(rooms.locator('tbody tr td:first-child')).toHaveText(
		Array.from({ length: 6 }, (_, index) => `ห้องรอจัดอาคาร ${25 + index}`)
	);
	await page.getByLabel('ค้นหาห้อง', { exact: true }).fill('ห้องรอจัดอาคาร 30');
	await expect(rooms.getByRole('row')).toHaveCount(2);
	await expect(rooms).toContainText('ห้องรอจัดอาคาร 30');
	await page.getByLabel('ค้นหาห้อง', { exact: true }).fill('room-30');
	await expect(page.getByText('ไม่พบห้องที่ค้นหา', { exact: true })).toBeVisible();
	expect(api.count(roomsPath)).toBe(1);
	expect(api.reads.find((url) => url.pathname === roomsPath)?.searchParams.has('building_id')).toBe(
		false
	);
	expect(api.reads.find((url) => url.pathname === roomsPath)?.searchParams.has('search')).toBe(
		false
	);
});

for (const kind of ['building', 'room'] as const) {
	test(`dismissed ${kind} creation does not block another draft or close it after reverse completion`, async ({
		page
	}) => {
		const api = await mockFacility(page, { hold: 'mutation' });
		const label = kind === 'building' ? 'อาคาร' : 'ห้อง';
		await page.goto(path);
		if (kind === 'room') await openRooms(page);
		await page.getByRole('button', { name: `เพิ่ม${label}`, exact: true }).click();
		let dialog = page.getByRole('dialog');
		await dialog.locator('input[name=name_th]').fill('ร่างสร้างก่อนหน้า');
		await dialog.locator('form').evaluate((element) => {
			const form = element as HTMLFormElement;
			form.requestSubmit();
			form.requestSubmit();
		});
		await expect.poll(() => api.count('mutation')).toBe(1);
		await expect(
			dialog.getByRole('button', { name: 'กำลังบันทึก...', exact: true })
		).toBeDisabled();
		await page.keyboard.press('Escape');
		await expect(dialog).toHaveCount(0);
		await page.getByRole('button', { name: `เพิ่ม${label}`, exact: true }).click();
		dialog = page.getByRole('dialog');
		await dialog.locator('input[name=name_th]').fill('ร่างสร้างถัดมา');
		await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
		await dialog.locator('form').evaluate((element) => {
			const form = element as HTMLFormElement;
			form.requestSubmit();
			form.requestSubmit();
		});
		await expect.poll(() => api.count('mutation')).toBe(2);
		await expect(dialog).toHaveCount(0);
		const region = page.getByTestId(kind === 'building' ? 'facility-buildings' : 'facility-rooms');
		await expect(region).toContainText('ร่างสร้างถัดมา');
		await expect(region).not.toContainText('ร่างสร้างก่อนหน้า');
		await page.getByRole('button', { name: `เพิ่ม${label}`, exact: true }).click();
		dialog = page.getByRole('dialog');
		await dialog.locator('input[name=name_th]').fill('ร่างล่าสุดที่ยังไม่บันทึก');
		api.release();
		await expect(region).toContainText('ร่างสร้างก่อนหน้า');
		await expect(region).toContainText('ร่างสร้างถัดมา');
		await expect(dialog.locator('input[name=name_th]')).toHaveValue('ร่างล่าสุดที่ยังไม่บันทึก');
		await expect(dialog.getByRole('button', { name: 'บันทึก', exact: true })).toBeEnabled();
		expect(api.count('mutation')).toBe(2);
		const items = kind === 'building' ? api.snapshot().buildings : api.snapshot().rooms;
		expect(
			items.filter((item) => ['ร่างสร้างก่อนหน้า', 'ร่างสร้างถัดมา'].includes(item.name_th))
		).toHaveLength(2);
	});
}

for (const action of ['update', 'delete'] as const) {
	test(`reopening the same building during a pending ${action} displays busy state and recovers`, async ({
		page
	}) => {
		const api = await mockFacility(page, { hold: 'mutation' });
		await page.goto(path);
		await page.getByRole('button', { name: 'แก้ไขอาคาร อาคารแรก', exact: true }).click();
		let dialog = page.getByRole('dialog');
		await dialog.getByRole('button', { name: 'บันทึก', exact: true }).click();
		await expect.poll(() => api.count('mutation')).toBe(1);
		await page.keyboard.press('Escape');
		await expect(dialog).toHaveCount(0);
		await page
			.getByRole('button', {
				name: `${action === 'update' ? 'แก้ไข' : 'ลบ'}อาคาร อาคารแรก`,
				exact: true
			})
			.click();
		if (action === 'update') {
			dialog = page.getByRole('dialog');
			await expect(
				dialog.getByRole('button', { name: 'กำลังบันทึก...', exact: true })
			).toBeDisabled();
			await dialog
				.locator('form')
				.evaluate((element) => (element as HTMLFormElement).requestSubmit());
		} else {
			dialog = page.getByRole('alertdialog');
			await expect(dialog.getByRole('button', { name: 'กำลังลบ...', exact: true })).toBeDisabled();
		}
		expect(api.count('mutation')).toBe(1);
		api.release();
		const save = dialog.getByRole('button', {
			name: action === 'update' ? 'บันทึก' : 'ยืนยันลบ',
			exact: true
		});
		await expect(save).toBeEnabled();
		await save.click();
		await expect(dialog).toHaveCount(0);
		expect(api.count('mutation')).toBe(2);
	});
}
