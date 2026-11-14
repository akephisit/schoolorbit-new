import { expect, test, type Page } from '@playwright/test';
import {
	mockDeliveryNavigation,
	navigationIds as ids,
	navigationOfferingIds as offerings,
	navigationOverviewPath
} from './fixtures/delivery-navigation-route-data';

test.use({ serviceWorkers: 'block' });

async function expectRequestedContext(page: Page) {
	await expect(page).toHaveURL(
		(url) =>
			url.searchParams.get('academicYearId') === ids.year &&
			url.searchParams.get('academicTermId') === ids.term
	);
	// AcademicContextSwitcher uses Tailwind's lg breakpoint for its desktop controls.
	if (!(await page.evaluate(() => matchMedia('(min-width: 64rem)').matches))) {
		const context = page.getByRole('banner').getByRole('button', {
			name: 'ปีการศึกษาและภาคเรียน 2570 · ภาคเรียนที่ 2',
			exact: true
		});
		await expect(context).toBeVisible();
		await expect(context).toHaveText('2570 · ภาคเรียนที่ 2');
		return;
	}
	await expect(page.getByRole('combobox', { name: 'เลือกปีการศึกษา', exact: true })).toContainText(
		'2570'
	);
	await expect(page.getByRole('combobox', { name: 'เลือกภาคเรียน', exact: true })).toContainText(
		'ภาคเรียนที่ 2'
	);
}

async function choose(page: Page, control: string, option: string) {
	await page.getByRole('combobox', { name: control, exact: true }).click();
	await page.getByRole('option', { name: option, exact: true }).click();
}

async function expectRoomContext(page: Page) {
	await expect(page.getByRole('tab', { name: 'มุมมองรายห้อง', exact: true })).toHaveAttribute(
		'aria-selected',
		'true'
	);
	await expect(
		page.getByRole('textbox', { name: 'ค้นหาห้อง ระดับชั้น หรือแผนการเรียน', exact: true })
	).toHaveValue('ม.1/');
	await expect(
		page.getByRole('combobox', { name: 'กรองความพร้อมของห้อง', exact: true })
	).toHaveText('ต้องตรวจเพิ่มเติม');
	await expect(
		page
			.locator('details')
			.filter({ has: page.getByRole('heading', { name: 'ม.1/1', exact: true }) })
	).not.toHaveAttribute('open');
	await expect(
		page
			.locator('details')
			.filter({ has: page.getByRole('heading', { name: 'ม.1/2', exact: true }) })
	).toHaveAttribute('open', '');
}

for (const width of [390, 1280]) {
	test(`homeroom navigation preserves term 2, filters and expanded rooms through detail, Back and reload at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await mockDeliveryNavigation(page);
		await page.goto(navigationOverviewPath());
		await expectRequestedContext(page);
		await expect(page.getByTestId('delivery-homerooms-ready')).toBeVisible();
		const before = api.count('/api/academic/delivery/homerooms');
		const search = page.getByRole('textbox', {
			name: 'ค้นหาห้อง ระดับชั้น หรือแผนการเรียน',
			exact: true
		});
		await search.focus();
		await search.pressSequentially('ม.1/');
		await expect(search).toBeFocused();
		await expect(search).toHaveValue('ม.1/');
		await choose(page, 'กรองความพร้อมของห้อง', 'ต้องตรวจเพิ่มเติม');
		const roomA = page
			.locator('details')
			.filter({ has: page.getByRole('heading', { name: 'ม.1/1', exact: true }) });
		const roomB = page
			.locator('details')
			.filter({ has: page.getByRole('heading', { name: 'ม.1/2', exact: true }) });
		await roomA.locator('summary').click();
		await expect(roomA).not.toHaveAttribute('open');
		await roomB.locator('summary').click();
		await expectRoomContext(page);
		await search.fill('ม.1/2');
		await expect(roomB).toHaveAttribute('open', '');
		await search.fill('ม.1/');
		await expectRoomContext(page);
		expect(api.count('/api/academic/delivery/homerooms')).toBe(before);
		const expectedNames = [
			'คณิตศาสตร์ต้น',
			'คณิตศาสตร์ปลาย',
			'คณิตศาสตร์เพิ่มเติม',
			'ภาษาไทยเก่า',
			'ชุมนุม'
		];
		const rows = roomB.locator('tbody tr');
		await expect(rows).toHaveCount(expectedNames.length);
		for (const [index, name] of expectedNames.entries()) {
			await expect(
				rows.nth(index).getByRole('cell').first().getByText(name, { exact: true })
			).toBeVisible();
		}
		await expect(page.locator('body')).not.toContainText('ACT-TECHNICAL');
		const link = roomB.getByRole('link', { name: 'จัดการ คณิตศาสตร์ต้น', exact: true });
		const href = await link.getAttribute('href');
		const target = new URL(href!, page.url());
		expect(target.searchParams.get('academicYearId')).toBe(ids.year);
		expect(target.searchParams.get('academicTermId')).toBe(ids.term);
		expect(target.searchParams.get('deliveryVersionId')).toBeNull();
		expect(
			new URL(target.searchParams.get('returnTo')!, page.url()).searchParams.get('expandedRooms')
		).toBe(ids.roomB);
		await page.screenshot({
			path: testInfo.outputPath(`delivery-rooms-context-${width}.png`),
			animations: 'disabled'
		});
		await link.click();
		await expect(page.getByRole('heading',{name:'ภาษาไทยเก่า',level:1,exact:true})).toBeVisible();
		await expectRequestedContext(page);
		await expect(page.getByRole('button', { name: 'แก้ไขจำนวนคาบ', exact: true })).toHaveCount(0);
		await page.goBack();
		await expectRoomContext(page);
		await link.click();
		await page.getByRole('link', { name: 'ย้อนกลับ', exact: true }).click();
		await expectRoomContext(page);
		await page.reload();
		await expectRoomContext(page);
		await expectRequestedContext(page);
		expect(api.count('/api/academic/delivery/management-options')).toBe(0);
		expect(api.writes).toEqual([]);
	});
}

test('offering filters use the authorized current projection and preserve natural order and exact view through a new tab', async ({
	page,
	context
}, testInfo) => {
	const api = await mockDeliveryNavigation(page);
	await page.goto(`${navigationOverviewPath()}&view=offerings`);
	await expectRequestedContext(page);
	const region = page.getByTestId('delivery-offerings-ready');
	await expect(region).toBeVisible();
	await expect(region.locator('tbody tr td:first-child > p:first-child')).toHaveText([
		'ค20101 · คณิตศาสตร์เพิ่มเติม',
		'ค21101 · คณิตศาสตร์ต้น',
		'ค23101 · คณิตศาสตร์ปลาย',
		'ท11101 · ภาษาไทยเก่า',
		'ชุมนุม',
		'แนะแนว'
	]);
	await expect(region).not.toContainText('ACT-TECHNICAL');
	const workspaceReads = api.count('/api/academic/delivery/workspace');
	const roomReads = api.count('/api/academic/delivery/homerooms');
	await choose(page, 'กรองกลุ่มสาระรายการเปิดสอน', 'ภาษาไทย');
	await expect(region.locator('tbody tr')).toHaveCount(1);
	await expect(region).toContainText('ภาษาไทยเก่า');
	await choose(page, 'กรองกลุ่มสาระรายการเปิดสอน', 'คณิตศาสตร์');
	await choose(page, 'กรองประเภทรายการเปิดสอน', 'รายวิชา');
	const search = page.getByRole('textbox', { name: 'ค้นหารายการเปิดสอน', exact: true });
	await search.focus();
	await search.pressSequentially('ค21');
	await expect(search).toBeFocused();
	await expect(region.locator('tbody tr')).toHaveCount(1);
	await expect(region).toContainText('ค21101 · คณิตศาสตร์ต้น');
	expect(api.count('/api/academic/delivery/workspace')).toBe(workspaceReads);
	expect(api.count('/api/academic/delivery/homerooms')).toBe(roomReads);
	const link = region.getByRole('link', { name: 'รายละเอียด', exact: true });
	const target = new URL((await link.getAttribute('href'))!, page.url());
	const returnTo = new URL(target.searchParams.get('returnTo')!, page.url());
	expect(returnTo.searchParams.get('view')).toBe('offerings');
	expect(returnTo.searchParams.get('offeringSearch')).toBe('ค21');
	expect(returnTo.searchParams.get('kind')).toBe('course');
	expect(returnTo.searchParams.get('subjectGroupId')).toBe(ids.math);
	await page.screenshot({
		path: testInfo.outputPath('delivery-offering-filters.png'),
		animations: 'disabled'
	});
	const tab = await context.newPage();
	const tabApi = await mockDeliveryNavigation(tab);
	try {
		await tab.goto(target.href);
		await expectRequestedContext(tab);
		await expect(tab.getByRole('heading',{name:'คณิตศาสตร์ต้น',level:1,exact:true})).toBeVisible();
		await tab.getByRole('link', { name: 'ย้อนกลับ', exact: true }).click();
		await expect(
			tab.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม', exact: true })
		).toHaveAttribute('aria-selected', 'true');
		await expect(tab.getByRole('textbox', { name: 'ค้นหารายการเปิดสอน', exact: true })).toHaveValue(
			'ค21'
		);
		await expect(
			tab.getByRole('combobox', { name: 'กรองกลุ่มสาระรายการเปิดสอน', exact: true })
		).toHaveText('คณิตศาสตร์');
		await expect(
			tab.getByRole('combobox', { name: 'กรองประเภทรายการเปิดสอน', exact: true })
		).toHaveText('รายวิชา');
		await tab.reload();
		await expect(tab.getByTestId('delivery-offerings-ready').locator('tbody tr')).toHaveCount(1);
		await expectRequestedContext(tab);
		expect(tabApi.writes).toEqual([]);
	} finally {
		await tab.close();
	}
	expect(api.count('/api/academic/delivery/management-options')).toBe(0);
	expect(api.writes).toEqual([]);
});

test('activity details and labels hide technical activity codes while retaining the explicit term', async ({
	page
}) => {
	await mockDeliveryNavigation(page);
	await page.goto(`${navigationOverviewPath()}&view=offerings&kind=activity`);
	const rows = page.getByTestId('delivery-offerings-ready').locator('tbody tr');
	await expect(rows).toHaveCount(2);
	await expect(page.getByTestId('delivery-offerings-ready')).not.toContainText('ACT-TECHNICAL');
	await rows
		.filter({ hasText: 'ชุมนุม' })
		.getByRole('link', { name: 'รายละเอียด', exact: true })
		.click();
	await expect(page.getByRole('heading',{name:/คณิตศาสตร์ต้น|ชุมนุม/,level:1})).toBeVisible();
	await expectRequestedContext(page);
	await expect(page.locator('body')).not.toContainText('ACT-TECHNICAL');
	await page.getByRole('link', { name: 'ย้อนกลับ', exact: true }).click();
	await expect(
		page.getByRole('combobox', { name: 'กรองประเภทรายการเปิดสอน', exact: true })
	).toHaveText('กิจกรรม');
	await expect(rows).toHaveCount(2);
});

for (const missing of [true, false]) {
	test(`${missing ? 'legacy missing' : 'stale explicit'} academic context repairs to the offering owner and rejects an external return`, async ({
		page
	}) => {
		const api = await mockDeliveryNavigation(page);
		const query = new URLSearchParams({
			returnTo: 'https://outside.invalid/staff/academic/delivery'
		});
		if (!missing) {
			query.set('academicYearId', ids.activeYear);
			query.set('academicTermId', ids.activeTerm);
		}
		await page.goto(`/staff/academic/delivery/${offerings.basic21}?${query}`);
		await expectRequestedContext(page);
		await expect(page.getByRole('heading',{name:/คณิตศาสตร์ต้น|ชุมนุม/,level:1})).toBeVisible();
		const back = page.getByRole('link', { name: 'ย้อนกลับ', exact: true });
		const target = new URL((await back.getAttribute('href'))!, page.url());
		expect(target.origin).toBe(new URL(page.url()).origin);
		expect(target.pathname).toBe('/staff/academic/delivery');
		expect(target.searchParams.get('academicYearId')).toBe(ids.year);
		expect(target.searchParams.get('academicTermId')).toBe(ids.term);
		expect(target.searchParams.get('deliveryVersionId')).toBeNull();
		expect(api.writes).toEqual([]);
	});
}

for (const width of [390, 1280]) {
	for (const dark of [false, true]) {
		test(`offering subject-group filters fit ${width}px ${dark ? 'dark' : 'light'}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			const api = await mockDeliveryNavigation(page);
			await page.goto(`${navigationOverviewPath()}&view=offerings`);
			await expect(page.getByTestId('delivery-offerings-ready')).toBeVisible();
			if (dark) await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
			if (dark) await expect(page.locator('html')).toHaveClass(/\bdark\b/);
			else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/);
			await choose(page, 'กรองกลุ่มสาระรายการเปิดสอน', 'ภาษาไทย');
			const region = page.getByTestId('delivery-offerings-ready');
			await expect(region.locator('tbody tr')).toHaveCount(1);
			await expect(region).toContainText('ภาษาไทยเก่า');
			const search = region.getByRole('textbox', { name: 'ค้นหารายการเปิดสอน', exact: true });
			const controls = [
				search,
				region.getByRole('combobox', { name: 'กรองประเภทรายการเปิดสอน', exact: true }),
				region.getByRole('combobox', { name: 'กรองกลุ่มสาระรายการเปิดสอน', exact: true })
			];
			for (const control of controls) {
				await control.scrollIntoViewIfNeeded();
				const bounds = await control.boundingBox();
				expect(bounds!.width).toBeGreaterThan(0);
				expect(bounds!.x).toBeGreaterThanOrEqual(0);
				expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
			}
			expect(
				await search.locator('..').evaluate((element) => element.scrollWidth <= element.clientWidth)
			).toBe(true);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.evaluate(async () => {
				await document.fonts.ready;
			});
			await page.screenshot({
				path: testInfo.outputPath(`delivery-group-filters-${width}-${dark ? 'dark' : 'light'}.png`),
				animations: 'disabled'
			});
			expect(api.count('/api/academic/delivery/management-options')).toBe(0);
			expect(api.writes).toEqual([]);
		});
	}
}

test('touch users can filter offerings and return from detail with their group selection', async ({
	browser,
	baseURL
}, testInfo) => {
	const context = await browser.newContext({
		baseURL,
		hasTouch: true,
		isMobile: true,
		viewport: { width: 390, height: 844 },
		serviceWorkers: 'block'
	});
	try {
		const page = await context.newPage();
		const api = await mockDeliveryNavigation(page);
		await page.goto(`${navigationOverviewPath()}&view=offerings`);
		expect(await page.evaluate(() => navigator.maxTouchPoints)).toBeGreaterThan(0);
		expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
		const group = page.getByRole('combobox', { name: 'กรองกลุ่มสาระรายการเปิดสอน', exact: true });
		await group.tap();
		await page.getByRole('option', { name: 'ภาษาไทย', exact: true }).tap();
		const region = page.getByTestId('delivery-offerings-ready');
		await expect(region.locator('tbody tr')).toHaveCount(1);
		await expect(group).toHaveText('ภาษาไทย');
		await page.screenshot({
			path: testInfo.outputPath('delivery-group-filters-touch.png'),
			animations: 'disabled'
		});
		await region.getByRole('link', { name: 'รายละเอียด', exact: true }).tap();
		await expect(page.getByRole('heading',{name:'ภาษาไทยเก่า',level:1,exact:true})).toBeVisible();
		await expectRequestedContext(page);
		await page.getByRole('link', { name: 'ย้อนกลับ', exact: true }).tap();
		await expect(group).toHaveText('ภาษาไทย');
		await expect(
			page.getByRole('tab', { name: 'มุมมองรายวิชา/กิจกรรม', exact: true })
		).toHaveAttribute('aria-selected', 'true');
		await expect(region.locator('tbody tr')).toHaveCount(1);
		expect(api.count('/api/academic/delivery/management-options')).toBe(0);
		expect(api.writes).toEqual([]);
	} finally {
		await context.close();
	}
});

function managerOrigin(
	view: 'homerooms' | 'offerings',
	course = false
): string {
	const url = new URL(navigationOverviewPath(), 'https://schoolorbit.invalid');
	url.searchParams.set('view', view);
	url.searchParams.set('homeroomSearch', 'ม.1/2');
	url.searchParams.set('readiness', 'attention');
	url.searchParams.set('expandedRooms', ids.roomB);
	url.searchParams.set('offeringSearch', course ? 'ค21' : 'ชุมนุม');
	url.searchParams.set('kind', course ? 'course' : 'activity');
	if (course) url.searchParams.set('subjectGroupId', ids.math);
	return url.pathname + url.search;
}

async function expectManagerOrigin(page: Page, view: 'homerooms' | 'offerings', course = false) {
	await expectRequestedContext(page);
	await expect(
		page.getByRole('tab', {
			name: view === 'homerooms' ? 'มุมมองรายห้อง' : 'มุมมองรายวิชา/กิจกรรม',
			exact: true
		})
	).toHaveAttribute('aria-selected', 'true');
	const current = new URL(page.url());
	expect(current.searchParams.get('homeroomSearch')).toBe('ม.1/2');
	expect(current.searchParams.get('readiness')).toBe('attention');
	expect(current.searchParams.get('expandedRooms')).toBe(ids.roomB);
	expect(current.searchParams.get('offeringSearch')).toBe(course ? 'ค21' : 'ชุมนุม');
	expect(current.searchParams.get('kind')).toBe(course ? 'course' : 'activity');
	if (course) expect(current.searchParams.get('subjectGroupId')).toBe(ids.math);
}

test('an existing current activity detail restores its originating room filters without preparing a duplicate',async({page})=>{
 const api=await mockDeliveryNavigation(page,{manager:true});await page.goto(managerOrigin('homerooms'));const room=page.locator('details').filter({has:page.getByRole('heading',{name:'ม.1/2',exact:true})});await expect(room).toHaveAttribute('open','');await room.getByRole('link',{name:'จัดการ ชุมนุม',exact:true}).click();await expect(page.getByRole('heading',{name:'ชุมนุม',level:1,exact:true})).toBeVisible();await expectRequestedContext(page);await page.getByRole('link',{name:'ย้อนกลับ',exact:true}).click();await expectManagerOrigin(page,'homerooms');expect(api.creations).toEqual([]);expect(api.reads.filter(url=>/delivery-versions|term-change-sets/.test(url.pathname))).toEqual([]);
});

test('creating an activity with groups navigates with the created offering context and restores the originating offering view', async ({
	page
}) => {
	const api = await mockDeliveryNavigation(page, { manager: true, groupedCreation: true });
	await page.goto(managerOrigin('offerings'));
	await page.getByRole('button', { name: 'เปิดการเรียนการสอน', exact: true }).click();
	await page.getByRole('button', { name: /เพิ่มรายการเอง/ }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('combobox', { name: 'ประเภท', exact: true }).click();
	await page.getByRole('option', { name: 'กิจกรรมพัฒนาผู้เรียน', exact: true }).click();
	for (const [control, option] of [
		['เลือกกิจกรรม', 'ชุมนุม (ฉบับ 3)'],
		['เลือกระดับชั้น', 'มัธยมศึกษาปีที่ 1'],
		['เลือกแผนการเรียน', 'แผนคณิตศาสตร์']
	]) {
		await dialog.getByRole('combobox', { name: control, exact: true }).click();
		const visibleOption = page
			.getByRole('option')
			.filter({ has: page.getByText(option, { exact: true }) });
		await expect(visibleOption).toBeVisible();
		await expect(page.locator('body')).not.toContainText('ACT-TECHNICAL');
		await visibleOption.click();
	}
	await dialog.getByLabel('คาบที่จัดจริงต่อสัปดาห์',{exact:true}).fill('2');
	await choose(page, 'รูปแบบการเปิด', 'เปิดพร้อมจัดกลุ่ม');
	await dialog.getByRole('button', { name: 'สร้างข้อมูลปัจจุบัน', exact: true }).click();
	await expect(page).toHaveURL((url) => url.pathname.endsWith(`/delivery/${offerings.club}`));
	await expectRequestedContext(page);
	await expect(page.getByRole('heading',{name:/คณิตศาสตร์ต้น|ชุมนุม/,level:1})).toBeVisible();
	expect(api.count(`/api/academic/offerings/${offerings.club}`)).toBe(1);
	expect(api.creations).toHaveLength(1);
	expect(api.creations[0]).toMatchObject({
		kind: 'activity',
		academicTermId: ids.term,
		activityVersionId: offerings.club,
		createHomeroomGroups: true,
		weeklyPeriodTarget: 2
	});
	await page.getByRole('link', { name: 'ย้อนกลับ', exact: true }).click();
	await expectManagerOrigin(page, 'offerings');
	await expect(page.getByTestId('delivery-offerings-ready').locator('tbody tr')).toHaveCount(1);
	await expect(page.getByTestId('delivery-offerings-ready')).toContainText('ชุมนุม');
});

for(const kind of ['course','activity'] as const)test(`${kind} current comparison detail keeps exact context and originating filters without a publication workflow`,async({page})=>{
 const api=await mockDeliveryNavigation(page,{manager:true});await page.goto(managerOrigin('offerings',kind==='course'));await expect(page.getByTestId('delivery-offerings-ready')).toBeVisible();const row=page.getByTestId('delivery-offerings-ready').locator('tbody tr').filter({hasText:kind==='course'?'คณิตศาสตร์ต้น':'ชุมนุม'});const link=row.getByRole('link',{name:'รายละเอียด',exact:true});const target=new URL((await link.getAttribute('href'))!,page.url());expect(target.searchParams.get('academicYearId')).toBe(ids.year);expect(target.searchParams.get('academicTermId')).toBe(ids.term);expect(target.searchParams.has('deliveryVersionId')).toBe(false);const origin=new URL(target.searchParams.get('returnTo')!,page.url());expect(origin.searchParams.get('expandedRooms')).toBe(ids.roomB);expect(origin.searchParams.get('kind')).toBe(kind);await link.click();await expectRequestedContext(page);await page.getByRole('link',{name:'ย้อนกลับ',exact:true}).click();await expectManagerOrigin(page,'offerings',kind==='course');expect(api.writes).toEqual([]);expect(api.reads.filter(url=>/delivery-versions|term-change-sets/.test(url.pathname))).toEqual([]);
});
