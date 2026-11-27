import { expect, test, type Page, type Route } from '@playwright/test';
import { revalidatePage } from './fixtures/page-revalidation';
import { mock, id, path, savedPeriod, timetableOption } from './fixtures/supervision-route-data';
import type {
	RequestSupervisionObservationRequest,
	SupervisionTimetableOption
} from '../../src/lib/api/supervision';

test.use({ serviceWorkers: 'block' });
const permissions = ['supervision.request.own', 'supervision.read.own'];
const mine = path('mine');
const optionsPath = '/api/supervision/booking-options';
const observationsPath = '/api/supervision/observations';
const choices = () => [timetableOption(0), timetableOption(1), timetableOption(2)];
const reply = (route: Route, data: unknown, status = 200) =>
	route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});

async function openBooking(page: Page, items = choices()) {
	await page.clock.setFixedTime(new Date('2026-10-01T03:00:00Z'));
	const api = await mock(page, { permissions, term: true, bookingItems: items, canManage: false });
	await page.goto(mine);
	await expect(page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true })).toHaveCount(1);
	await expect(page.getByTestId('supervision-lesson-picker')).toHaveCount(0);
	expect(api.count(optionsPath)).toBe(0);
	await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).click();
	await expect(page.getByTestId('supervision-lesson-picker')).toBeVisible();
	await expect(
		page.getByRole('button', { name: new RegExp(`^เลือก ${items[0].periodName} `) })
	).toBeVisible();
	return api;
}

function savedSelection(
	payload: RequestSupervisionObservationRequest,
	items: SupervisionTimetableOption[]
) {
	return payload.timetableBlockGroupIds.map((groupId, index) => {
		const item = items.find((item) => item.blockGroupId === groupId)!;
		return savedPeriod({
			id: id(200 + index),
			position: index + 1,
			timetableBlockGroupId: groupId,
			homeroomId: item.homeroomId,
			observedAt: new Date(`${payload.observedDate}T${item.startTime}+07:00`).toISOString(),
			endsAt: new Date(`${payload.observedDate}T${item.endTime}+07:00`).toISOString(),
			lessonSnapshot: {
				source: 'timetable',
				subjectName: item.subjectName,
				classroomLabel: item.homeroomName,
				periodLabel: item.periodName
			}
		});
	});
}

for (const count of [1, 2, 3]) {
	test(`one request contains ${count} ordered consecutive periods and marks every saved cell`, async ({
		page
	}) => {
		const items = choices();
		const api = await openBooking(page, items);
		const payloads: RequestSupervisionObservationRequest[] = [];
		const initialListReads = api.count(observationsPath);
		await page.route(`**${observationsPath}/requests`, async (route) => {
			if (route.request().method() !== 'POST') return route.fallback();
			const payload: RequestSupervisionObservationRequest = route.request().postDataJSON();
			payloads.push(payload);
			await reply(route, {
				...api.observation(),
				status: 'requested',
				periods: savedSelection(payload, items)
			});
		});
		await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
		for (let index = 1; index < count; index++)
			await page
				.getByRole('button', { name: new RegExp(`^เลือก คาบ ${index + 1} ม\\.1/1`) })
				.click();
		await expect(page.getByTestId('supervision-booking-selection')).toContainText(
			`${count} คาบต่อเนื่อง`
		);
		await expect(page.locator('input[type="time"], input[id^="supervision-manual-"]')).toHaveCount(
			0
		);
		await page.getByRole('button', { name: 'ส่งคำขอจอง', exact: true }).click();
		await expect(page.getByTestId('supervision-booking-selection')).toContainText('ส่งคำขอสำเร็จ');
		expect(payloads).toEqual([
			{
				cycleId: api.observation().cycleId,
				academicTermId: id(8),
				observedDate: '2026-10-01',
				timetableBlockGroupIds: items.slice(0, count).map((item) => item.blockGroupId),
				expectedSourceDigest: 'source-1'
			}
		]);
		for (let index = 0; index < count; index++) {
			const savedCell = page.getByRole('button', {
				name: new RegExp(`^เลือก คาบ ${index + 1} ม\\.1/1`)
			});
			await expect(savedCell).toHaveJSProperty('disabled', true);
			await expect(savedCell).toHaveAttribute('aria-pressed', 'false');
		}
		expect(api.count(observationsPath)).toBe(initialListReads);
	});
}

test('toggling a middle lesson keeps and submits the earlier consecutive range', async ({
	page
}) => {
	const items = [...choices(), timetableOption(3)];
	const api = await openBooking(page, items);
	const payloads: RequestSupervisionObservationRequest[] = [];
	await page.route(`**${observationsPath}/requests`, async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		const payload: RequestSupervisionObservationRequest = route.request().postDataJSON();
		payloads.push(payload);
		await reply(route, {
			...api.observation(),
			status: 'requested',
			periods: savedSelection(payload, items)
		});
	});
	const cards = items.map((item) =>
		page.getByRole('button', { name: new RegExp(`^เลือก ${item.periodName} `) })
	);
	for (const card of cards) await card.click();
	const selection = page.getByTestId('supervision-booking-selection');
	await expect(selection).toContainText('4 คาบต่อเนื่อง');
	await cards[2].click();
	await expect(selection).toContainText('2 คาบต่อเนื่อง · 08:00–10:00');
	for (const card of cards.slice(0, 2)) await expect(card).toHaveAttribute('aria-pressed', 'true');
	await expect(cards[2]).toHaveAttribute('aria-pressed', 'false');
	await expect(cards[2]).toBeEnabled();
	await expect(cards[3]).toHaveAttribute('aria-pressed', 'false');
	await expect(cards[3]).toBeDisabled();
	await selection.getByRole('button', { name: 'ส่งคำขอจอง', exact: true }).click();
	await expect(selection).toContainText('ส่งคำขอสำเร็จ');
	expect(payloads).toEqual([
		{
			cycleId: api.observation().cycleId,
			academicTermId: id(8),
			observedDate: '2026-10-01',
			timetableBlockGroupIds: items.slice(0, 2).map((item) => item.blockGroupId),
			expectedSourceDigest: 'source-1'
		}
	]);
});

test('only adjacent lessons can extend in either direction, and clearing allows a new first lesson', async ({
	page
}) => {
	const items = [
		...choices(),
		timetableOption(3, { homeroomId: id(121), homeroomName: 'ม.1/2' }),
		timetableOption(4, { startTime: '13:00:00', endTime: '14:00:00' }),
		timetableOption(5, { observedDate: '2026-10-02', startTime: '11:00:00', endTime: '12:00:00' })
	];
	await openBooking(page, items);
	await page.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ }).click();
	await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
	await page.getByRole('button', { name: /^เลือก คาบ 3 ม.1\/1/ }).click();
	const otherClass = page.getByRole('button', { name: /^เลือก คาบ 4 ม.1\/2/ });
	await expect(otherClass).toHaveJSProperty('disabled', true);
	await expect(page.getByRole('button', { name: /^เลือก คาบ 5 ม.1\/1/ })).toHaveJSProperty(
		'disabled',
		true
	);
	await expect(page.getByRole('button', { name: /^เลือก คาบ 6 ม.1\/1/ })).toHaveJSProperty(
		'disabled',
		true
	);
	await expect(page.getByRole('button', { name: 'นำ คาบ 2 ออกจากช่วง', exact: true })).toHaveCount(
		0
	);
	await page.getByRole('button', { name: 'นำ คาบ 1 ออกจากช่วง', exact: true }).click();
	await expect(page.getByTestId('supervision-booking-selection')).toContainText('2 คาบต่อเนื่อง');
	await expect(page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ })).toBeEnabled();
	await page.getByRole('button', { name: 'ล้างทั้งหมด', exact: true }).click();
	await otherClass.click();
	await expect(page.getByTestId('supervision-booking-selection')).toContainText('1 คาบต่อเนื่อง');
	await page.getByRole('button', { name: 'ล้างทั้งหมด', exact: true }).click();
	await expect(page.getByRole('button', { name: 'ส่งคำขอจอง', exact: true })).toBeDisabled();
});

test('stale refusal keeps the selected draft and explicit refresh requires choosing from the new snapshot', async ({
	page
}) => {
	const items = choices();
	const refreshedItems = [
		timetableOption(0, { startTime: '08:15:00', endTime: '09:15:00' }),
		timetableOption(1, { startTime: '09:15:00', endTime: '10:15:00' }),
		timetableOption(2, { startTime: '10:15:00', endTime: '11:15:00' })
	];
	const api = await openBooking(page, items);
	const payloads: RequestSupervisionObservationRequest[] = [];
	await page.route(`**${observationsPath}/requests`, async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		const payload: RequestSupervisionObservationRequest = route.request().postDataJSON();
		payloads.push(payload);
		if (payloads.length === 1) return reply(route, 'ตารางสอนเปลี่ยนแล้ว', 409);
		return reply(route, { ...api.observation(), periods: savedSelection(payload, refreshedItems) });
	});
	await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
	await page.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ }).click();
	const selection = page.getByTestId('supervision-booking-selection');
	const submit = selection.getByRole('button', { name: 'ส่งคำขอจอง', exact: true });
	await submit.click();
	await expect(selection).toContainText('ตารางสอนเปลี่ยนแล้ว');
	await expect(selection).toContainText('2 คาบต่อเนื่อง · 08:00–10:00');
	await page.route(`**${optionsPath}*`, (route) =>
		reply(route, {
			sourceDigest: 'source-2',
			weekStartDate: '2026-09-28',
			weekEndDate: '2026-10-04',
			items: refreshedItems
		})
	);
	await page.getByRole('button', { name: 'ตรวจตารางสอนล่าสุด', exact: true }).click();
	await expect(selection).toContainText('ช่วงคาบเดิมไม่พร้อมจองหรือข้อมูลคาบเปลี่ยน');
	await expect(selection).toContainText('ยังไม่ได้เลือกคาบ');
	await expect(submit).toBeDisabled();
	expect(payloads).toHaveLength(1);
	await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
	await page.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ }).click();
	await expect(selection).toContainText('2 คาบต่อเนื่อง · 08:15–10:15');
	await submit.click();
	await expect(selection).toContainText('ส่งคำขอสำเร็จ');
	expect(payloads).toEqual(
		['source-1', 'source-2'].map((expectedSourceDigest) => ({
			cycleId: api.observation().cycleId,
			academicTermId: id(8),
			observedDate: '2026-10-01',
			timetableBlockGroupIds: refreshedItems.slice(0, 2).map((item) => item.blockGroupId),
			expectedSourceDigest
		}))
	);
});

test('late previous-week options cannot replace the newer week or its draft', async ({ page }) => {
	await openBooking(page);
	const gate = Promise.withResolvers<void>();
	await page.route(`**${optionsPath}*`, async (route) => {
		const week = new URL(route.request().url()).searchParams.get('weekStartDate')!;
		if (week === '2026-09-21') {
			await gate.promise;
			return reply(route, {
				sourceDigest: 'old',
				weekStartDate: week,
				weekEndDate: '2026-09-27',
				items: [
					timetableOption(0, { observedDate: '2026-09-24', subjectName: 'คำตอบเก่าที่ห้ามกลับมา' })
				]
			});
		}
		return reply(route, {
			sourceDigest: 'new',
			weekStartDate: week,
			weekEndDate: '2026-10-04',
			items: choices()
		});
	});
	await page.getByRole('button', { name: 'สัปดาห์ก่อน', exact: true }).click();
	await page.getByRole('button', { name: 'สัปดาห์ถัดไป', exact: true }).click();
	await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
	gate.resolve();
	await expect(page.getByTestId('supervision-booking-selection')).toContainText('1 คาบต่อเนื่อง');
	await expect(page.getByText('คำตอบเก่าที่ห้ามกลับมา', { exact: true })).toHaveCount(0);
});

test('a booking in another cycle leaves the same dated lesson available in the selected cycle', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-01T03:00:00Z'));
	await mock(page, {
		permissions,
		term: true,
		bookingItems: choices(),
		observation: {
			cycleId: id(4),
			periods: [
				savedPeriod({
					timetableBlockGroupId: timetableOption(0).blockGroupId,
					observedAt: '2026-10-01T01:00:00Z',
					endsAt: '2026-10-01T02:00:00Z'
				})
			]
		}
	});
	await page.goto(mine);
	await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).click();
	await expect(page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ })).toBeEnabled();
	await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
	await expect(page.getByRole('button', { name: 'ส่งคำขอจอง', exact: true })).toBeEnabled();
});

for (const colorScheme of ['light', 'dark'] as const) {
	for (const width of [390, 1280]) {
		test(`booking grid keeps readable non-overlapping cells (${colorScheme}, ${width})`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 900 });
			await page.emulateMedia({ colorScheme });
			await openBooking(page);
			const picker = page.getByTestId('supervision-lesson-picker');
			const mobile = width === 390;
			if (mobile) {
				const scrollport = await picker.locator('[data-slot="table-container"]').boundingBox();
				const secondPeriod = await picker
					.getByRole('columnheader', { name: /^คาบ 2 / })
					.boundingBox();
				expect(secondPeriod!.x + secondPeriod!.width).toBeLessThanOrEqual(
					scrollport!.x + scrollport!.width
				);
			}
			await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
			await page.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ }).click();
			await page.getByRole('button', { name: /^เลือก คาบ 3 ม.1\/1/ }).click();
			const dayRowHeights = await picker
				.getByRole('rowheader')
				.evaluateAll((headers) =>
					headers.map((header) => header.closest('tr')!.getBoundingClientRect().height)
				);
			expect(dayRowHeights).toHaveLength(5);
			expect(Math.max(...dayRowHeights) - Math.min(...dayRowHeights)).toBeLessThan(1);
			const bounds = await picker
				.getByRole('button', { name: /^เลือก คาบ/ })
				.evaluateAll((buttons) =>
					buttons.map((button) => {
						const rect = button.getBoundingClientRect();
						const cell = button.closest('td')!.getBoundingClientRect();
						return {
							width: rect.width,
							within: rect.left >= cell.left && rect.right <= cell.right
						};
					})
				);
			expect(bounds).toHaveLength(3);
			const rem = await page.evaluate(() =>
				Number.parseFloat(getComputedStyle(document.documentElement).fontSize)
			);
			for (const bound of bounds) {
				expect(bound.within).toBe(true);
				expect(bound.width).toBeGreaterThanOrEqual(mobile ? 5.5 * rem - 1 : 150);
				if (mobile) expect(bound.width).toBeLessThanOrEqual(6 * rem + 1);
			}
			expect(
				await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
			).toBe(true);
			await page.screenshot({
				path: test.info().outputPath(`booking-${colorScheme}-${width}.png`),
				fullPage: true
			});
		});
	}
}

test('long subject, period and room labels wrap within the mobile grid and selected chips', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 900 });
	const item = timetableOption(0, {
		subjectName: 'คณิตศาสตร์พื้นฐานและการแก้ปัญหาในชีวิตประจำวันสำหรับนักเรียนมัธยมศึกษา',
		periodName: 'คาบกิจกรรมการเรียนรู้ต่อเนื่องช่วงเช้า',
		roomName: 'ห้องปฏิบัติการวิทยาศาสตร์และคณิตศาสตร์อาคารเรียนกลาง'
	});
	await openBooking(page, [item]);
	await page.getByRole('button', { name: /^เลือก คาบกิจกรรมการเรียนรู้/ }).click();
	const selection = page.getByTestId('supervision-booking-selection');
	await expect(selection).toContainText(item.subjectName!);
	expect(await selection.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(
		true
	);
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
		true
	);
});

test('a replaced session discards pending old-session options and the old selected draft', async ({
	page
}) => {
	const api = await openBooking(page);
	await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
	const gate = Promise.withResolvers<void>();
	let held = false;
	await page.route(`**${optionsPath}*`, async (route) => {
		const week = new URL(route.request().url()).searchParams.get('weekStartDate')!;
		if (week === '2026-09-21') {
			held = true;
			await gate.promise;
			return reply(route, {
				sourceDigest: 'old-session',
				weekStartDate: week,
				weekEndDate: '2026-09-27',
				items: [
					timetableOption(0, { observedDate: '2026-09-24', subjectName: 'คำตอบจากเซสชันเดิม' })
				]
			});
		}
		return reply(route, {
			sourceDigest: 'new-session',
			weekStartDate: week,
			weekEndDate: '2026-10-04',
			items: choices()
		});
	});
	await page.getByRole('button', { name: 'สัปดาห์ก่อน', exact: true }).click();
	await expect.poll(() => held).toBe(true);
	await api.replaceIdentity(permissions, '01900000-0000-7000-8000-000000000002');
	await expect(page.getByTestId('supervision-lesson-picker')).toHaveCount(0);
	gate.resolve();
	await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).click();
	await expect(page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ })).toBeVisible();
	await expect(page.getByText('คำตอบจากเซสชันเดิม', { exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'ส่งคำขอจอง', exact: true })).toBeDisabled();
});

for (const change of ['permissions', 'session'] as const) {
	test(`${change} replacement clears visible old data and rejects an older held primary response`, async ({
		page
	}) => {
		await page.clock.setFixedTime(new Date('2026-10-01T03:00:00Z'));
		const api = await mock(page, { permissions, term: true });
		const oldGate = Promise.withResolvers<void>();
		const freshGate = Promise.withResolvers<void>();
		let reads = 0;
		await page.route(`**${observationsPath}?*`, async (route) => {
			const currentRead = ++reads;
			if (currentRead === 2) await oldGate.promise;
			if (currentRead === 3) await freshGate.promise;
			const subjectName =
				currentRead < 3 ? 'ข้อมูลคำขอจากสิทธิ์เดิม' : 'ข้อมูลคำขอหลังเปลี่ยนสิทธิ์';
			await reply(route, {
				items: [
					{ ...api.observation(), periods: [savedPeriod({ lessonSnapshot: { subjectName } })] }
				]
			});
		});
		await page.goto(mine);
		const region = page.getByTestId('supervision-observations');
		await expect(
			region.getByRole('heading', { name: 'ข้อมูลคำขอจากสิทธิ์เดิม', exact: true })
		).toBeVisible();
		await revalidatePage(page);
		await expect.poll(() => reads).toBe(2);
		await expect(
			region.getByRole('heading', { name: 'ข้อมูลคำขอจากสิทธิ์เดิม', exact: true })
		).toBeVisible();
		await api.replaceIdentity(
			change === 'permissions' ? ['supervision.read.own'] : permissions,
			change === 'session' ? '01900000-0000-7000-8000-000000000002' : undefined
		);
		await expect.poll(() => reads).toBe(3);
		// A real auth reconciliation must preserve the newly authoritative fixture identity.
		await page.evaluate(async () => {
			const modulePath = '/src/lib/api/auth.ts';
			const { authAPI } = await import(/* @vite-ignore */ modulePath);
			if (
				(await authAPI.refreshCurrentUser({ silent: true, invalidate: true })) !== 'authenticated'
			)
				throw new Error('Expected replacement identity to remain authenticated');
			await new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			);
		});
		expect(reads).toBe(3);
		await expect(
			region.getByRole('heading', { name: 'ข้อมูลคำขอจากสิทธิ์เดิม', exact: true })
		).toHaveCount(0);
		freshGate.resolve();
		await expect(
			region.getByRole('heading', { name: 'ข้อมูลคำขอหลังเปลี่ยนสิทธิ์', exact: true })
		).toBeVisible();
		oldGate.resolve();
		await expect(
			region.getByRole('heading', { name: 'ข้อมูลคำขอจากสิทธิ์เดิม', exact: true })
		).toHaveCount(0);
	});
}

test('a null subject keeps its learning-group label in the selected range', async ({ page }) => {
	await openBooking(page, [
		timetableOption(0, {
			subjectName: null,
			learningGroupName: 'กลุ่มการเรียนรู้ที่ไม่มีชื่อรายวิชา'
		})
	]);
	await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
	await expect(page.getByTestId('supervision-booking-selection')).toContainText(
		'กลุ่มการเรียนรู้ที่ไม่มีชื่อรายวิชา'
	);
});
