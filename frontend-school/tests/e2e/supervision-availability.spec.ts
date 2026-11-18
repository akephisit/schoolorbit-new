import { expect, test, type Page, type Route } from '@playwright/test';
import {
	id,
	year,
	firstCycle,
	mock,
	path,
	savedPeriod,
	timetableOption,
	selectSupervisionDate
} from './fixtures/supervision-route-data';
import type {
	SupervisionEvaluatorAvailabilityResponse,
	SupervisionTimetableOption
} from '../../src/lib/api/supervision';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-01T03:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
const permissions = ['supervision.read.own', 'supervision.request.own'];
const root = `/staff/academic/supervision?academicYearId=${year}`;
const detail = `/staff/academic/supervision/${id(30)}?academicYearId=${year}`;
const optionsPath = '/api/supervision/booking-options';
const requestPath = '/api/supervision/observations/requests';
const reply = (route: Route, data: unknown, status = 200) =>
	route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
const option = (index: number, patch: Partial<SupervisionTimetableOption> = {}) =>
	timetableOption(index, patch);
async function open(page: Page) {
	await page.goto(root);
	await page.getByRole('button', { name: 'เปิดจองคาบนิเทศ', exact: true }).click();
}

test('an open legacy round without dates remains readable but cannot load or select new lessons', async ({
	page
}) => {
	const api = await mock(page, {
		permissions,
		term: true,
		observationStartDate: null,
		observationEndDate: null
	});
	await open(page);
	await expect(page.getByText('รอผู้ดูแลกำหนดช่วงวันที่นิเทศ', { exact: true })).toBeVisible();
	await expect(page.getByTestId('supervision-observations')).toContainText('วิชาทดสอบ');
	await expect(page.getByTestId('supervision-lesson-picker')).toHaveCount(0);
	expect(api.count(optionsPath)).toBe(0);
	await expect(page.getByRole('button', { name: /กำหนดช่วงวันที่/ })).toHaveCount(0);
});

test('week defaults clamp to the window and partial first/last weeks include only allowed dates', async ({
	page
}) => {
	const queries: URL[] = [];
	await mock(page, {
		permissions,
		term: true,
		observationStartDate: '2026-10-07',
		observationEndDate: '2026-10-13'
	});
	await page.route(`**${optionsPath}?*`, async (route) => {
		const query = new URL(route.request().url());
		queries.push(query);
		const week = query.searchParams.get('weekStartDate')!;
		return reply(route, {
			sourceDigest: week,
			weekStartDate: week,
			weekEndDate: week === '2026-10-05' ? '2026-10-11' : '2026-10-18',
			items:
				week === '2026-10-05'
					? [option(0, { observedDate: '2026-10-06' }), option(0, { observedDate: '2026-10-07' })]
					: [option(0, { observedDate: '2026-10-13' }), option(0, { observedDate: '2026-10-14' })]
		});
	});
	await open(page);
	const picker = page.getByTestId('supervision-lesson-picker');
	await expect(picker.getByRole('button', { name: 'สัปดาห์ก่อน', exact: true })).toBeDisabled();
	await expect(picker.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1.*6 ต.ค./ })).toBeDisabled();
	await picker.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1.*7 ต.ค./ }).click();
	await expect(page.getByTestId('supervision-booking-selection')).toContainText('1 คาบต่อเนื่อง');
	await picker.getByRole('button', { name: 'สัปดาห์ถัดไป', exact: true }).click();
	await expect(picker.getByRole('button', { name: 'สัปดาห์ถัดไป', exact: true })).toBeDisabled();
	await expect(picker.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1.*14 ต.ค./ })).toBeDisabled();
	await picker.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1.*13 ต.ค./ }).click();
	await expect(page.getByTestId('supervision-booking-selection')).toContainText('1 คาบต่อเนื่อง');
	for (const query of queries) {
		expect(query.searchParams.get('cycleId')).toBe(firstCycle);
		expect(query.searchParams.get('academicYearId')).toBe(year);
		expect(query.searchParams.get('academicTermId')).toBe(id(8));
	}
	expect(queries.map((query) => query.searchParams.get('weekStartDate'))).toEqual([
		'2026-10-05',
		'2026-10-12'
	]);
});

test('free, pending and parallel lessons stay selectable while a blocked edge cannot extend the range', async ({
	page
}) => {
	const api = await mock(page, {
		permissions,
		term: true,
		bookingItems: [
			option(0),
			option(1, { bookingAvailability: { pendingCount: 0, confirmedCount: 1, blocked: true } }),
			option(2, { bookingAvailability: { pendingCount: 2, confirmedCount: 1, blocked: false } })
		]
	});
	await open(page);
	const picker = page.getByTestId('supervision-lesson-picker');
	await expect(picker.getByText('ว่าง', { exact: true })).toBeVisible();
	await expect(
		picker.getByText('รออนุมัติ 2 · นัดหมาย 1 · จองขนานได้', { exact: true })
	).toBeVisible();
	await expect(picker.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ })).toBeDisabled();
	await picker.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
	await expect(picker.getByRole('button', { name: 'เพิ่ม คาบ 2 ม.1/1', exact: true })).toHaveCount(
		0
	);
	await picker.getByRole('button', { name: /^เลือก คาบ 3 ม.1\/1/ }).click();
	await expect(page.getByTestId('supervision-booking-selection')).toContainText('คาบ 3');
	expect(api.count(`/api/supervision/observations/${id(30)}/evaluator-availability`)).toBe(0);
});

for (const blocked of [false, true]) {
	test(`a refused save refresh ${blocked ? 'drops a newly blocked' : 'preserves a still eligible'} range without reporting success`, async ({
		page
	}) => {
		const items = [option(0), option(1)];
		const api = await mock(page, { permissions, term: true, bookingItems: items });
		let fresh = false;
		await page.route(`**${optionsPath}?*`, (route) =>
			reply(route, {
				sourceDigest: fresh ? 'fresh-source' : 'original-source',
				weekStartDate: '2026-09-28',
				weekEndDate: '2026-10-04',
				items: items.map((item, index) =>
					fresh && blocked && index === 1
						? {
								...item,
								bookingAvailability: { pendingCount: 0, confirmedCount: 1, blocked: true }
							}
						: item
				)
			})
		);
		let saves = 0;
		await page.route(`**${requestPath}`, (route) => {
			saves += 1;
			fresh = true;
			return reply(route, 'มีรายการยืนยันใหม่ กรุณาตรวจคาบอีกครั้ง', 409);
		});
		await open(page);
		await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
		await page.getByRole('button', { name: 'เพิ่ม คาบ 2 ม.1/1', exact: true }).click();
		await page.getByRole('button', { name: 'ส่งคำขอจอง', exact: true }).click();
		const selection = page.getByTestId('supervision-booking-selection');
		await expect(
			selection.getByText('มีรายการยืนยันใหม่ กรุณาตรวจคาบอีกครั้ง', { exact: true })
		).toBeVisible();
		await expect(
			selection.getByText('ส่งคำขอสำเร็จ ดูสถานะได้ในรายการของฉัน', { exact: true })
		).toHaveCount(0);
		await selection.getByRole('button', { name: 'ตรวจตารางสอนล่าสุด', exact: true }).click();
		if (blocked) {
			await expect(selection).toContainText('ช่วงคาบเดิมไม่พร้อมจอง');
			await expect(
				selection.getByRole('button', { name: 'ส่งคำขอจอง', exact: true })
			).toBeDisabled();
			await expect(page.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ })).toBeDisabled();
		} else {
			await expect(selection).toContainText('2 คาบต่อเนื่อง');
			await expect(
				selection.getByText('มีรายการยืนยันใหม่ กรุณาตรวจคาบอีกครั้ง', { exact: true })
			).toBeVisible();
			await expect(
				selection.getByRole('button', { name: 'ส่งคำขอจอง', exact: true })
			).toBeEnabled();
		}
		expect(saves).toBe(1);
		expect(api.count('/api/supervision/observations')).toBe(1);
	});
}

test('date edits retain the draft on conflict and retry patches only the affected cycle', async ({
	page
}) => {
	const api = await mock(page, { observationStartDate: null, observationEndDate: null });
	await page.goto(path('cycles'));
	const row = page.getByRole('row', { name: /รอบแรก/ });
	await row.getByRole('button', { name: 'กำหนดช่วงวันที่ รอบแรก', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await selectSupervisionDate(page, 'วันเริ่มนิเทศ', '2026-10-01');
	await selectSupervisionDate(page, 'วันสุดท้ายที่นิเทศ', '2026-10-15');
	let saves = 0;
	await page.route(`**/api/supervision/cycles/${firstCycle}`, async (route) => {
		if (route.request().method() !== 'PATCH') return route.fallback();
		saves += 1;
		if (saves === 1) return reply(route, 'มีคำขอที่ทับช่วงวันที่ กรุณาตรวจและบันทึกอีกครั้ง', 409);
		return route.fallback();
	});
	await dialog.getByRole('button', { name: 'บันทึกช่วงวันที่', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('มีคำขอที่ทับช่วงวันที่');
	await expect(
		dialog.getByRole('button', { name: 'วันสุดท้ายที่นิเทศ', exact: true })
	).toContainText('15 ต.ค. 2569');
	await dialog.getByRole('button', { name: 'ลองบันทึกอีกครั้ง', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	await expect(row).not.toContainText('ยังไม่กำหนดช่วงวันที่นิเทศ');
	expect(api.count('/api/supervision/cycles')).toBe(1);
	expect(api.payloads.at(-1)).toEqual({
		observationStartDate: '2026-10-01',
		observationEndDate: '2026-10-15'
	});
});

test('read-only staff see saved cycle dates but cannot enter cycle management or edit dates', async ({
	page
}) => {
	const api = await mock(page, { permissions: ['supervision.read.school'] });
	await page.goto(detail);
	await expect(page.getByTestId('supervision-detail')).toContainText('31 ธ.ค. 2569');
	await expect(page.getByRole('button', { name: /กำหนดช่วงวันที่/ })).toHaveCount(0);
	await page.goto(path('cycles'));
	await expect(page).toHaveURL(/\/403(?:\?|$)/);
	expect(api.count('/api/supervision/cycles')).toBe(0);
	await expect(page.getByRole('button', { name: /กำหนดช่วงวันที่/ })).toHaveCount(0);
});

test('date mutation cannot be closed or rebound by Escape while pending, and its late response cannot own a replaced session', async ({
	page
}) => {
	const api = await mock(page, { hold: 'patch' });
	await page.goto(path('cycles'));
	await page.getByRole('button', { name: 'กำหนดช่วงวันที่ รอบแรก', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await selectSupervisionDate(page, 'วันสุดท้ายที่นิเทศ', '2026-12-15');
	await dialog.getByRole('button', { name: 'บันทึกช่วงวันที่', exact: true }).click();
	await expect
		.poll(() => api.writes.includes(`PATCH /api/supervision/cycles/${firstCycle}`))
		.toBe(true);
	await page.keyboard.press('Escape');
	await expect(dialog).toBeVisible();
	const refreshedCycles = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname === '/api/supervision/cycles' &&
			response.request().method() === 'GET'
	);
	await api.replaceIdentity(
		['supervision.manage.school', 'supervision.read.school'],
		'01900000-0000-7000-8000-000000000002'
	);
	await expect(dialog).toHaveCount(0);
	await (await refreshedCycles).finished();
	const cycles = page.getByTestId('supervision-cycles');
	const row = cycles.getByRole('row', { name: /รอบแรก/ });
	await expect(cycles).toHaveAttribute('aria-busy', 'false');
	await expect(row).toContainText('31 ธ.ค. 2569');
	const response = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname === `/api/supervision/cycles/${firstCycle}` &&
			response.request().method() === 'PATCH'
	);
	api.release();
	await (await response).finished();
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
	await expect(row).toContainText('31 ธ.ค. 2569');
	await expect(
		page.getByRole('button', { name: 'กำหนดช่วงวันที่ รอบแรก', exact: true })
	).toBeEnabled();
	await expect(page.getByRole('alert')).toHaveCount(0);
});

test('a previously approved lesson is locked for managers while evaluator management remains available', async ({
	page
}) => {
	const api = await mock(page, { observationStatus: 'planned' });
	await page.goto(detail);
	await expect(page.getByTestId('supervision-periods-locked')).toBeVisible();
	await expect(page.getByRole('button', { name: 'แก้คาบ/วันเวลา', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'แก้ผู้ประเมิน', exact: true }).click();
	await expect(
		page.getByRole('dialog').getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน', exact: true })
	).toBeEnabled();
	expect(api.count(`/api/supervision/observations/${id(30)}/timetable-options`)).toBe(0);
});

test('a returned locked visit resubmits the exact saved periods with an empty payload and no timetable read', async ({
	page
}) => {
	const api = await mock(page, {
		permissions,
		observationStatus: 'returned',
		canManage: false,
		observation: {
			periodsLocked: true,
			periods: [
				savedPeriod({
					manualLesson: { subjectName: 'วิชาเดิมที่อนุมัติแล้ว', reason: 'หลักฐานเดิม' },
					lessonSnapshot: { source: 'manual' }
				})
			]
		}
	});
	const submissions: unknown[] = [];
	await page.route(`**/api/supervision/observations/${id(30)}/resubmit-request`, (route) => {
		submissions.push(route.request().postDataJSON());
		if (submissions.length === 1) return reply(route, 'ยังส่งใหม่ไม่ได้ กรุณาลองอีกครั้ง', 409);
		return route.fallback();
	});
	await page.goto(detail);
	await expect(page.getByTestId('supervision-saved-periods')).toContainText(
		'วิชาเดิมที่อนุมัติแล้ว'
	);
	await expect(page.getByRole('button', { name: 'แก้ไขและส่งใหม่', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'ส่งใหม่ด้วยคาบเดิม', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('ยังส่งใหม่ไม่ได้ กรุณาลองอีกครั้ง');
	await expect(page.getByTestId('supervision-saved-periods')).toContainText(
		'วิชาเดิมที่อนุมัติแล้ว'
	);
	await page.getByRole('button', { name: 'ส่งใหม่ด้วยคาบเดิม', exact: true }).click();
	await expect(page.getByText('ส่งคำขอใหม่โดยใช้ช่วงคาบเดิมแล้ว', { exact: true })).toBeVisible();
	expect(api.writes).toEqual([`POST /api/supervision/observations/${id(30)}/resubmit-request`]);
	expect(api.payloads).toEqual([{}]);
	expect(submissions).toEqual([{}, {}]);
	expect(api.count(`/api/supervision/observations/${id(30)}/timetable-options`)).toBe(0);
	await expect(page.getByTestId('supervision-saved-periods')).toContainText(
		'วิชาเดิมที่อนุมัติแล้ว'
	);
});

test('a returned locked visit stays readable when submitted evidence or missing cycle dates prevents resubmission', async ({
	page
}) => {
	for (const submitted of [true, false]) {
		await test.step(
			submitted ? 'with submitted evidence' : 'with missing cycle dates',
			async () => {
				const api = await mock(page, {
					permissions,
					observationStatus: 'returned',
					canManage: false,
					...(submitted ? {} : { observationStartDate: null, observationEndDate: null }),
					observation: {
						periodsLocked: true,
						evaluators: [
							{
								id: id(31),
								observationId: id(30),
								createdAt: '2026-09-30T09:00:00Z',
								updatedAt: '2026-10-01T09:00:00Z',
								evaluatorUserId: id(32),
								evaluatorDisplayName: 'ผู้ประเมินทดสอบ',
								roleLabel: 'ผู้ประเมิน',
								isRequired: true,
								status: submitted ? 'submitted' : 'assigned'
							}
						]
					}
				});
				await page.goto(detail);
				await expect(page.getByTestId('supervision-current-return-reason')).toBeVisible();
				await expect(page.getByTestId('supervision-saved-periods')).toContainText('วิชาทดสอบ');
				await expect(page.getByTestId('supervision-periods-locked')).toBeVisible();
				const resubmit = page.getByRole('button', { name: 'ส่งใหม่ด้วยคาบเดิม', exact: true });
				if (submitted) {
					await expect(resubmit).toHaveCount(0);
				} else {
					await expect(
						page.getByText('ส่งใหม่ได้เมื่อผู้ดูแลกำหนดช่วงวันที่นิเทศของรอบนี้', { exact: true })
					).toBeVisible();
					await expect(resubmit).toBeDisabled();
				}
				expect(api.writes).not.toContain(
					`POST /api/supervision/observations/${id(30)}/resubmit-request`
				);
				expect(api.count(`/api/supervision/observations/${id(30)}/timetable-options`)).toBe(0);
			}
		);
	}
});

for (const state of ['parallel', 'blocked', 'outside'] as const) {
	test(`request review displays full saved range and scoped overlap/evaluator groups for ${state}`, async ({
		page
	}) => {
		await mock(page, {
			observation: {
				periods: [
					savedPeriod({ endsAt: '2026-10-01T10:00:00Z' }),
					savedPeriod({
						id: id(71),
						position: 2,
						observedAt: '2026-10-01T10:00:00Z',
						endsAt: '2026-10-01T11:00:00Z',
						lessonSnapshot: { periodLabel: 'คาบ 2' }
					})
				]
			}
		});
		const result: SupervisionEvaluatorAvailabilityResponse = {
			items: [
				{
					id: id(32),
					name: 'ผู้ประเมินว่างตลอดช่วง',
					available: true,
					title: null,
					conflictReason: null,
					conflict: null
				},
				{
					id: id(33),
					name: 'ผู้ประเมินมีนัดนิเทศ',
					available: false,
					title: null,
					conflictReason: 'มีงานนิเทศ ครูที่มีสิทธิ์เห็น ในช่วงเวลาเดียวกัน',
					conflict: {
						observationId: id(34),
						observedDisplayName: 'ครูที่มีสิทธิ์เห็น',
						observedAt: '2026-10-01T10:00:00Z',
						lessonTitle: 'วิชาที่มีนัดนิเทศ / คาบ 2'
					}
				}
			],
			overlap: { pendingCount: 2, confirmedCount: 1, blocked: state === 'blocked' },
			withinCycleDates: state !== 'outside',
			overlappingBookings: [
				{
					observedDisplayName: 'ครูที่มีสิทธิ์เห็น',
					observedAt: '2026-10-01T10:00:00Z',
					endsAt: '2026-10-01T11:00:00Z',
					status: 'planned'
				}
			]
		};
		await page.route(`**/api/supervision/observations/${id(30)}/evaluator-availability`, (route) =>
			reply(route, result)
		);
		await page.goto(path('requests'));
		await page.getByRole('button', { name: 'ดูรายละเอียด 2 คาบ', exact: true }).click();
		await expect(page.getByTestId('supervision-saved-periods')).toContainText('คาบ 2');
		await page.getByRole('button', { name: 'ตรวจช่วงคาบและผู้ประเมิน', exact: true }).click();
		const review = page.getByTestId('supervision-request-availability');
		await expect(review).toContainText('รออนุมัติ 2 รายการ · ยืนยันแล้ว 1 รายการ');
		await expect(review).toContainText('ครูที่มีสิทธิ์เห็น');
		await expect(review).toContainText('ตรวจจากนัดนิเทศที่ยืนยันแล้วตลอดช่วงคาบที่เลือก');
		await page.getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน', exact: true }).click();
		await expect(page.getByText('พร้อมประเมินตลอดช่วงคาบ', { exact: true })).toBeVisible();
		await expect(page.getByText('ไม่ว่างในช่วงคาบนี้', { exact: true })).toBeVisible();
		const unavailable = page.getByRole('option', { name: /ผู้ประเมินมีนัดนิเทศ/ });
		await expect(unavailable).toHaveAttribute('aria-disabled', 'true');
		await expect(unavailable).toContainText('มีงานนิเทศ ครูที่มีสิทธิ์เห็น ในช่วงเวลาเดียวกัน');
		await expect(unavailable).toContainText('วิชาที่มีนัดนิเทศ / คาบ 2');
		await page.getByRole('option', { name: /ผู้ประเมินว่างตลอดช่วง/ }).click();
		await page.keyboard.press('Escape');
		if (state === 'parallel')
			await expect(
				page.getByRole('button', { name: 'อนุมัติและมอบหมาย', exact: true })
			).toBeEnabled();
		else
			await expect(
				page.getByRole('button', { name: 'อนุมัติและมอบหมาย', exact: true })
			).toBeDisabled();
	});
}

test('a unit manager can recheck blocked availability and approve with the still-eligible evaluator', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: ['supervision.manage.organization_unit', 'supervision.read.organization_unit']
	});
	const availabilityPath = `/api/supervision/observations/${id(30)}/evaluator-availability`;
	const initial: SupervisionEvaluatorAvailabilityResponse = {
		items: ['ยังว่าง', 'ติดนัดนิเทศใหม่'].map((name, index) => ({
			id: id(32 + index),
			name,
			available: true,
			title: null,
			conflictReason: null,
			conflict: null
		})),
		overlap: { pendingCount: 0, confirmedCount: 1, blocked: true },
		withinCycleDates: true,
		overlappingBookings: []
	};
	const updated: SupervisionEvaluatorAvailabilityResponse = {
		...initial,
		items: [
			initial.items[0]!,
			{ ...initial.items[1]!, available: false, conflictReason: 'มีงานนิเทศในช่วงเวลาเดียวกัน' }
		],
		overlap: { ...initial.overlap, confirmedCount: 1, blocked: false }
	};
	const refresh = Promise.withResolvers<void>();
	const failure = Promise.withResolvers<void>();
	let checks = 0;
	await page.route(`**${availabilityPath}`, async (route) => {
		const check = ++checks;
		if (check < 3) return reply(route, initial);
		if (check === 3) await refresh.promise;
		if (check === 4) {
			await failure.promise;
			return reply(route, 'ตรวจข้อมูลล่าสุดไม่สำเร็จ', 503);
		}
		return reply(route, updated);
	});
	await page.goto(path('requests'));
	const review = page.getByTestId('supervision-request-availability');
	const inspect = review.getByRole('button', { name: 'ตรวจช่วงคาบและผู้ประเมิน', exact: true });
	const recheck = review.getByRole('button', { name: 'ตรวจอีกครั้ง', exact: true });
	const picker = page.getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน', exact: true });
	const approve = page.getByRole('button', { name: 'อนุมัติและมอบหมาย', exact: true });
	await expect(inspect).toBeEnabled();
	expect(checks).toBe(0);
	await inspect.click();
	await expect(review).toContainText('มีนัดทับกัน อนุมัติไม่ได้');
	await picker.click();
	for (const name of ['ยังว่าง', 'ติดนัดนิเทศใหม่']) {
		await page.getByRole('option', { name, exact: true }).click();
		await expect(
			page.getByRole('button', { name: `ลบผู้ประเมิน ${name}`, exact: true })
		).toBeVisible();
	}
	await page.keyboard.press('Escape');
	await expect(picker).toHaveAttribute('aria-expanded', 'false');
	await expect(approve).toBeDisabled();
	await recheck.click();
	await expect(review).toHaveAttribute('aria-busy', 'true');
	await expect(recheck).toBeDisabled();
	await expect(review).toContainText('มีนัดทับกัน อนุมัติไม่ได้');
	refresh.resolve();
	await expect(review).toContainText('ไม่พบรายการที่กีดกันการอนุมัติ');
	await expect(
		page.getByRole('button', { name: 'ลบผู้ประเมิน ยังว่าง', exact: true })
	).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'ลบผู้ประเมิน ติดนัดนิเทศใหม่', exact: true })
	).toHaveCount(0);
	await expect(approve).toBeEnabled();

	await picker.click();
	await expect(review).toHaveAttribute('aria-busy', 'true');
	await expect(approve).toBeDisabled();
	await expect(page.getByRole('option', { name: 'ยังว่าง', exact: true })).toBeDisabled();
	await page.keyboard.press('Escape');
	await expect(picker).toHaveAttribute('aria-expanded', 'false');
	await expect(review).toHaveAttribute('aria-busy', 'true');
	failure.resolve();
	await expect(review.getByRole('alert')).toContainText('ตรวจข้อมูลล่าสุดไม่สำเร็จ');
	await expect(approve).toBeDisabled();
	await recheck.click();
	await expect(approve).toBeEnabled();
	await approve.click();
	await expect(page.getByText('ไม่มีคำขอจองที่รออนุมัติ')).toBeVisible();
	expect(checks).toBe(5);
	expect(api.payloads).toContainEqual({
		evaluators: [{ evaluatorUserId: id(32), isRequired: true }]
	});
});

test('a unit manager can read another unit’s request without checking its availability', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: ['supervision.manage.organization_unit', 'supervision.read.school'],
		canManage: false
	});
	await page.goto(path('requests'));
	await page.getByRole('button', { name: 'ดูรายละเอียด 1 คาบ', exact: true }).click();
	await expect(page.getByTestId('supervision-saved-periods')).toContainText('วิชาทดสอบ');
	await expect(page.getByTestId('supervision-request-availability')).toHaveCount(0);
	await expect(
		page.getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน', exact: true })
	).toBeDisabled();
	expect(api.count(`/api/supervision/observations/${id(30)}/evaluator-availability`)).toBe(0);
});
