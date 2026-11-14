import { expect, test, type Page, type Route } from '@playwright/test';
import { installTimetableMock, makeTimetableBlock, timetableIds } from './timetable-test-harness';
import { createDraftRelay } from './timetable-collaboration-relay';
import { revalidatePage } from './fixtures/page-revalidation';

test.use({ serviceWorkers: 'block' });
const secondClass = '81000000-0000-4000-8000-000000000212';
const secondUser = '51000000-0000-4000-8000-000000000212';
const errorToasts = (page: Page, message: string) =>
	page.locator('[data-sonner-toast][data-type="error"]').filter({ hasText: message });
function url() {
	return `/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableId=${timetableIds.timetable}&view=homeroom&ownerId=${secondClass}`;
}
async function fail(route: Route, message: string) {
	await route.fulfill({ status: 500, json: { success: false, error: message } });
}
async function chooseOwner(page: Page, name: string) {
	await page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' }).click();
	await page.getByRole('option', { name, exact: true }).click();
}
async function stableBoard(page: Page, top: number) {
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	await expect
		.poll(async () =>
			Math.abs((await page.getByTestId('timetable-board-ready').boundingBox())!.y - top)
		)
		.toBeLessThanOrEqual(1);
	await expect(page.getByRole('alert').filter({ hasText: /synthetic-/ })).toHaveCount(0);
}

test('independent owner selections survive view/day roundtrips, same-current-board reload and authorized scope loss', async ({
	page
}) => {
	const mock = await installTimetableMock(page);
	const initial = mock.workspace();
	initial.homerooms.push({
		...initial.homerooms[0],
		id: secondClass,
		name: 'ม.1/2',
		roomNumber: '2'
	});
	mock.publishWorkspace(initial);
	await page.goto(url());
	const selector = page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' });
	await expect(selector).toHaveText('ม.1/2'); // explicit deep link wins
	await page.getByRole('button', { name: 'ครู', exact: true }).click();
	await chooseOwner(page, 'ครูคณิตศาสตร์ B');
	await page.getByRole('button', { name: 'กลุ่มเรียน', exact: true }).click();
	await chooseOwner(page, 'ม.1/1 วิทยาศาสตร์');
	await page.getByRole('button', { name: 'โรงเรียน', exact: true }).click();
	await page.getByRole('combobox', { name: 'เลือกวันดูภาพรวม' }).click();
	await page.getByRole('option', { name: 'วันอังคาร', exact: true }).click();
	for (const [view, label, owner] of [
		['ชั้น', 'ม.1/2', secondClass],
		['ครู', 'ครูคณิตศาสตร์ B', timetableIds.teacherB],
		['กลุ่มเรียน', 'ม.1/1 วิทยาศาสตร์', timetableIds.groupB]
	] as const) {
		await page.getByRole('button', { name: view, exact: true }).click();
		await expect(selector).toHaveText(label);
		await expect(page).toHaveURL(new RegExp(`ownerId=${owner}`));
		await page.getByRole('button', { name: view, exact: true }).click();
		await expect(selector).toHaveText(label);
	}
	expect(mock.workspaceRequestCount()).toBe(1);
	await revalidatePage(page);
	await expect.poll(mock.workspaceRequestCount).toBe(2);
	await expect(selector).toHaveText('ม.1/1 วิทยาศาสตร์');
	await page.getByRole('button', { name: 'ครู', exact: true }).click();
	await expect(selector).toHaveText('ครูคณิตศาสตร์ B');
	await page.getByRole('button', { name: 'ชั้น', exact: true }).click();
	await expect(selector).toHaveText('ม.1/2');
	await page.getByRole('button', { name: 'โรงเรียน', exact: true }).click();
	await expect(page.getByRole('combobox', { name: 'เลือกวันดูภาพรวม' })).toHaveText('อังคาร');
	await page.getByRole('button', { name: 'ครู', exact: true }).click();
	const restricted = mock.workspace();
	restricted.staff = restricted.staff.filter((row) => row.id !== timetableIds.teacherB);
	restricted.homerooms = restricted.homerooms.filter((row) => row.id !== secondClass);
	mock.publishWorkspace(restricted);
	await expect(selector).toHaveText('ครูคณิตศาสตร์ A');
	await page.getByRole('button', { name: 'ชั้น', exact: true }).click();
	await expect(selector).toHaveText('ม.1/1');
	await page.getByRole('button', { name: 'กลุ่มเรียน', exact: true }).click();
	await expect(selector).toHaveText('ม.1/1 วิทยาศาสตร์');
	expect(mock.workspaceRequestCount()).toBe(2);
});

for (const width of [1440, 390]) {
	test(`header places populated avatars after badge and collapses empty roster at ${width}px`, async ({
		page,
		context
	}) => {
		await page.setViewportSize({ width, height: 900 });
		const relay = createDraftRelay();
		await installTimetableMock(page, { relay });
		await page.goto(url());
		const header = page.locator('header[aria-label="บริบทตารางสอน"]');
		await expect(header.locator('[data-timetable-roster]')).toHaveCount(0);
		expect(
			await header
				.locator('h2')
				.evaluate((element) =>
					Boolean(
						element.compareDocumentPosition(
							element.closest('header')!.querySelector('[data-slot="badge"]')!
						) & Node.DOCUMENT_POSITION_FOLLOWING
					)
				)
		).toBe(true);
		const peer = await context.newPage();
		try {
			await installTimetableMock(peer, { relay, userId: secondUser, userName: 'ผู้ร่วมทดสอบ' });
			await peer.goto(url());
			const roster = header.locator('[data-timetable-roster]');
			await expect(roster.getByRole('button')).toHaveCount(1);
			expect(
				await roster.evaluate((element) =>
					element.previousElementSibling?.getAttribute('data-slot')
				)
			).toBe('badge');
			await roster.getByRole('button').focus();
			await expect(roster.getByRole('button')).toBeFocused();
			await expect(page.locator('[data-slot="tooltip-content"]')).toContainText('ผู้ร่วมทดสอบ');
			await page.keyboard.press('Tab');
			await expect(page.locator('[data-slot="tooltip-content"]')).toHaveCount(0);
			expect((await roster.boundingBox())!.width).toBeLessThanOrEqual(34);
			const badge = header.locator('[data-slot="badge"]');
			for (const editing of [false, true]) {
				if (editing) await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
				await expect(badge).toContainText(editing ? 'กำลังแก้ไข' : 'โหมดดู');
				for (const theme of ['light', 'dark'] as const) {
					await page.evaluate(
						(theme) => document.documentElement.classList.toggle('dark', theme === 'dark'),
						theme
					);
					await page.screenshot({
						path: `/workspace/artifacts/timetable-header-populated-${width}-${editing ? 'editing' : 'view'}-${theme}.png`,
						animations: 'disabled',
						fullPage: true
					});
				}
				const rosterBox = (await roster.boundingBox())!,
					badgeBox = (await badge.boundingBox())!;
				expect(
					Math.abs(rosterBox.y + rosterBox.height / 2 - badgeBox.y - badgeBox.height / 2)
				).toBeLessThanOrEqual(1);
				expect(rosterBox.x - badgeBox.x - badgeBox.width).toBeGreaterThanOrEqual(0);
				expect(rosterBox.x - badgeBox.x - badgeBox.width).toBeLessThanOrEqual(8);
				expect(
					await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)
				).toBe(true);
			}

			// Leaving the current resource runs the real application-owned socket cleanup.
			await peer.goto('/staff');
			await expect.poll(() => relay.peers.size).toBe(1);
			await expect(header.locator('[data-timetable-roster]')).toHaveCount(0);
		} finally {
			await peer.close();
		}
		await expect(header.locator('[data-timetable-roster]')).toHaveCount(0);
	});
}

test('initial failed workspace reports one detailed toast and keeps an actionable no-data retry state', async ({
	page
}) => {
	await installTimetableMock(page);
	let broken = true;
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) =>
		broken ? fail(route, 'synthetic-initial-read') : route.fallback()
	);
	await page.goto(url());
	await expect(errorToasts(page, 'synthetic-initial-read')).toHaveCount(1);
	await expect(page.getByRole('alert').filter({ hasText: 'โหลดตารางสอนไม่สำเร็จ' })).toBeVisible();
	await expect(page.getByText('ลองอีกครั้งเพื่อโหลดข้อมูลตารางสอน', { exact: true })).toBeVisible();
	broken = false;
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	await expect(errorToasts(page, 'synthetic-initial-read')).toHaveCount(1);
});

test('a failed current workspace refresh keeps board geometry and retries only that read',async({page})=>{
 const mock=await installTimetableMock(page);await page.goto(url());await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
 const top=(await page.getByTestId('timetable-board-ready').boundingBox())!.y;let broken=true,workspaceReads=0;
 await page.route('**/api/academic/timetable-blocks/workspace?**',route=>{workspaceReads++;return broken?fail(route,'synthetic-stream-workspace'):route.fallback();});
 await revalidatePage(page);await expect(errorToasts(page,'synthetic-stream-workspace')).toHaveCount(1);await stableBoard(page,top);
 broken=false;await errorToasts(page,'synthetic-stream-workspace').getByRole('button',{name:'ลองใหม่',exact:true}).click();await expect.poll(()=>workspaceReads).toBe(2);await stableBoard(page,top);expect(mock.workspaceRequestCount()).toBe(2);expect(mock.retiredRequests()).toEqual([]);
});

test('lifecycle nested read failure reports once, retains editing and inserts no board or dialog alert', async ({
	page
}) => {
	await installTimetableMock(page, {
		blocks: [makeTimetableBlock(timetableIds.blockA, timetableIds.period1)]
	});
	await page.goto(url());
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const top = (await page.getByTestId('timetable-board-ready').boundingBox())!.y;
	await page.route('**/api/academic/timetable-blocks/workspace?**', (route) =>
		fail(route, 'synthetic-finish-reload')
	);
	await page.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).click();
	await expect(errorToasts(page, 'synthetic-finish-reload')).toHaveCount(1);
	await expect(page.locator('header[aria-label="บริบทตารางสอน"]').getByText('กำลังแก้ไข',{exact:true})).toBeVisible();
	await stableBoard(page, top);
});

test('a superseded failed workspace cannot toast or replace the newly selected empty term',async({page})=>{
 const nextTerm='b1000000-0000-4000-8000-000000000299';
 const terms=[timetableIds.term,nextTerm].map((id,index)=>({id,academicYearId:timetableIds.year,name:`ภาคเรียนที่ ${index+1}`,code:String(index+1),sequence:index+1,status:'active',termType:'regular',schoolDays:['MON','TUE','WED','THU','FRI'],isVisible:true}));
 await installTimetableMock(page,{terms});await page.route('**/api/academic/timetable?*',route=>new URL(route.request().url()).searchParams.get('academicTermId')===nextTerm?route.fulfill({json:{success:true,data:null}}):route.fallback());
 await page.goto(url());await expect(page.getByTestId('timetable-board-ready')).toBeVisible();let release=()=>{},completed=()=>{},started=false;const gate=new Promise<void>(resolve=>release=resolve),completion=new Promise<void>(resolve=>completed=resolve);
 await page.route('**/api/academic/timetable-blocks/workspace?**',async route=>{started=true;try{await gate;await fail(route,'synthetic-superseded-read');}finally{completed();}});
 try{await revalidatePage(page);await expect.poll(()=>started).toBe(true);await page.getByLabel('เลือกภาคเรียน',{exact:true}).click();await page.getByRole('option',{name:'ภาคเรียนที่ 2',exact:true}).click();await expect(page.getByText('ยังไม่มีตารางสอน',{exact:true})).toBeVisible();release();await completion;await page.evaluate(()=>new Promise<void>(resolve=>requestAnimationFrame(()=>resolve())));await expect(page).toHaveURL(new RegExp(`academicTermId=${nextTerm}`));await expect(errorToasts(page,'synthetic-superseded-read')).toHaveCount(0);await expect(page.getByTestId('timetable-board-ready')).toHaveCount(0);}finally{release();}
});

test('a failed direct removal has one toast and explicit unknown-result recovery without a dialog alert',async({page})=>{
 const block=makeTimetableBlock(timetableIds.blockA,timetableIds.period1),mock=await installTimetableMock(page,{blocks:[block]});
 await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();await page.route(`**/api/academic/timetable-blocks/${block.id}?*`,route=>route.request().method()==='DELETE'?fail(route,'synthetic-direct-remove'):route.fallback());
 await page.locator(`[data-block-id="${block.id}"]`).getByRole('button',{name:/นำ .* ออกจากตาราง/}).click();await page.getByRole('button',{name:'ยืนยันนำออก',exact:true}).click();await expect(errorToasts(page,'synthetic-direct-remove')).toHaveCount(1);await expect(page.locator(`[data-block-id="${block.id}"]`)).toBeVisible();await expect(page.getByText(/ยังยืนยันผลการบันทึกไม่ได้ การละทิ้ง/)).toBeVisible();await expect(page.getByRole('dialog').getByRole('alert')).toHaveCount(0);expect(mock.deleteRequestCount()).toBe(0);expect(mock.retiredRequests()).toEqual([]);
});

test('accepted lifecycle HTTP scope loss repairs the owner URL before regrant and same-current-board initialization', async ({
	page
}) => {
	let workspaceFrames = 0;
	const relay = createDraftRelay();
	const join = relay.join;
	// Observe the mocked transport hand-off; page websocket events cover native sockets only.
	relay.join = (workspace, output, identity) =>
		join(
			workspace,
			(event) => {
				if (event.type === 'TimetableStateSync' || event.type === 'TimetablePatched') workspaceFrames++;
				output(event);
			},
			identity
		);
	const mock = await installTimetableMock(page, { relay });
	await page.goto(
		url().replace('view=homeroom', 'view=teacher').replace(secondClass, timetableIds.teacherB)
	);
	const selector = page.getByRole('combobox', { name: 'เลือกรายการสำหรับจัดตาราง' });
	await expect(selector).toHaveText('ครูคณิตศาสตร์ B');
	// Presence can be sent only after the session has accepted its initial StateSync.
	await expect
		.poll(() =>
			mock.relay.received.some(
				({ message }) =>
					message.type === 'ClientPresenceUpdate' &&
					message.payload.context.viewMode === 'teacher' &&
					message.payload.context.viewId === timetableIds.teacherB
			)
		)
		.toBe(true);
	await expect(selector).toHaveText('ครูคณิตศาสตร์ B');
	await expect(page).toHaveURL(new RegExp(`view=teacher&ownerId=${timetableIds.teacherB}`));
	expect(workspaceFrames).toBe(1);
	const baselineFrames = workspaceFrames;
	await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
	const readsAfterEdit=mock.workspaceRequestCount();
	const complete = mock.workspace();
	const restricted = structuredClone(complete);
	restricted.staff = restricted.staff.filter((row) => row.id !== timetableIds.teacherB);
	// Model a scoped HTTP read without delivering a canonical WS patch first.
	mock.publishWorkspace(restricted, false);
	await page.getByRole('button', { name: 'เสร็จสิ้น', exact: true }).click();
	await expect(selector).toHaveText('ครูคณิตศาสตร์ A');
	await expect(page.locator('header[aria-label="บริบทตารางสอน"]').getByText('โหมดดู',{exact:true})).toBeVisible();
	expect(workspaceFrames).toBe(baselineFrames);
	await expect(page).toHaveURL(new RegExp(`view=teacher&ownerId=${timetableIds.teacherA}`));
	expect(mock.workspaceRequestCount()).toBe(readsAfterEdit+1);
	mock.publishWorkspace(complete, false);
	await revalidatePage(page);
	await expect.poll(mock.workspaceRequestCount).toBe(readsAfterEdit+2);
	await expect(selector).toHaveText('ครูคณิตศาสตร์ A');
	await expect(page).toHaveURL(new RegExp(`view=teacher&ownerId=${timetableIds.teacherA}`));
	await selector.click();
	await expect(page.getByRole('option', { name: 'ครูคณิตศาสตร์ B', exact: true })).toBeVisible();
});
