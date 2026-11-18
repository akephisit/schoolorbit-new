import { expect, test } from '@playwright/test';
import type { SupervisionEvaluator } from '../../src/lib/api/supervision';
import { revalidatePage } from './fixtures/page-revalidation';
import {
	id,
	mock,
	navigate,
	nextYear,
	path,
	savedPeriod,
	secondCycle,
	year
} from './fixtures/supervision-route-data';

const observationPath = '/api/supervision/observations';
const requestsPath = path('requests');
const bookingDay = '2026-11-01';
const crowdedDay = '2026-11-04';
const requestPeriods = [
	savedPeriod({
		observedAt: '2026-11-01T03:20:00Z',
		endsAt: '2026-11-01T04:10:00Z',
		lessonSnapshot: {
			periodLabel: 'คาบ 3',
			subjectName: 'คณิตศาสตร์',
			classroomLabel: 'ม.3/1',
			roomLabel: 'ห้อง 115'
		}
	}),
	savedPeriod({
		id: id(71),
		position: 2,
		observedAt: '2026-11-01T04:10:00Z',
		endsAt: '2026-11-01T05:00:00Z',
		lessonSnapshot: {
			periodLabel: 'คาบ 4',
			subjectName: 'วิทยาศาสตร์',
			classroomLabel: 'ม.3/1',
			roomLabel: 'ห้อง 115'
		}
	})
];

const irregularPeriods = [
	savedPeriod({
		id: id(72),
		observationId: id(40),
		observedAt: '2026-10-31T17:20:00Z',
		endsAt: '2026-10-31T18:10:00Z',
		lessonSnapshot: { periodLabel: 'คาบ 3' }
	}),
	savedPeriod({
		id: id(73),
		observationId: id(40),
		position: 2,
		observedAt: '2026-10-31T19:40:00Z',
		lessonSnapshot: { periodLabel: 'คาบ 5' }
	})
];

const crowdedBookings = Array.from({ length: 5 }, (_, index) => {
	const observationId = id(200 + index);
	const observedAt = `${crowdedDay}T02:00:00Z`;
	return {
		id: observationId,
		observedDisplayName: `ครู นัดที่ ${index + 1}`,
		observedAt,
		status: 'planned' as const,
		periods: [savedPeriod({ id: id(210 + index), observationId, observedAt })]
	};
});

const evaluators: SupervisionEvaluator[] = ['อาจารย์ มณี มีสุข', 'อาจารย์ ธนกฤต วัฒนกุล', null].map(
	(evaluatorDisplayName, index) => ({
		id: id(170 + index),
		observationId: id(40),
		evaluatorUserId: id(180 + index),
		evaluatorDisplayName,
		isRequired: index !== 1,
		status: 'submitted',
		createdAt: '2026-10-30T09:00:00Z',
		updatedAt: '2026-11-01T09:00:00Z'
	})
);

test.use({ serviceWorkers: 'block', timezoneId: 'UTC' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-31T17:10:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});

test('request cards show saved periods, time, class, and unique saved subjects before disclosure', async ({
	page
}) => {
	const api = await mock(page, {
		observation: { periods: [...requestPeriods].reverse(), evaluators: [] },
		additionalObservations: [
			{ id: id(40), observedDisplayName: 'ครู คาบแยก', periods: irregularPeriods },
			{
				id: id(41),
				observedDisplayName: 'ครู วิชาเดียว',
				periods: requestPeriods.map((period) => ({
					...period,
					lessonSnapshot: { ...period.lessonSnapshot, subjectName: 'ประวัติศาสตร์' }
				}))
			},
			{
				id: id(42),
				observedDisplayName: 'ครู หลักฐานเดิม',
				periods: [
					savedPeriod({
						id: id(74),
						observationId: id(42),
						position: 1,
						lessonSnapshot: { periodLabel: null, subjectName: 'วิชาจากหลักฐานเดิม' },
						manualLesson: { periodLabel: '  ' }
					})
				]
			}
		]
	});
	await page.goto(requestsPath);
	await expect(page.getByRole('tab', { name: 'คำขอจอง' })).toHaveAttribute('aria-selected', 'true');
	const queue = page.getByRole('tabpanel', { name: 'คำขอจอง' });
	const card = queue.locator('[data-slot="card"]').filter({ hasText: 'ครู ในคิว' });
	await expect(card.getByText('คาบ 3–4', { exact: true })).toBeVisible();
	await expect(card).toContainText('1 พ.ย. 2569 · 10:20–12:00 น.');
	await expect(
		card.getByText('ม.3/1 · ห้อง 115', { exact: true }).filter({ visible: true })
	).toBeVisible();
	await expect(card.getByText('คณิตศาสตร์ · วิทยาศาสตร์', { exact: true })).toBeVisible();
	await expect(card.getByTestId('supervision-saved-periods')).toBeHidden();
	const irregular = queue.locator('[data-slot="card"]').filter({ hasText: 'ครู คาบแยก' });
	await expect(irregular.getByText('คาบ 3 · คาบ 5', { exact: true })).toBeVisible();
	await expect(irregular.getByText('คาบ 3–5', { exact: true })).toHaveCount(0);
	await expect(irregular.getByText('คาบ 3 · 00:20–01:10 น.')).toBeVisible();
	await expect(irregular.getByText('คาบ 5 · 02:40 น. · ไม่ระบุเวลาสิ้นสุด')).toBeVisible();
	await expect(queue.getByText('ประวัติศาสตร์', { exact: true })).toBeVisible();
	const disclosure = card.getByRole('button', { name: 'ดูรายละเอียด 2 คาบ' });
	await disclosure.focus();
	await page.keyboard.press('Enter');
	await expect(disclosure).toHaveAttribute('aria-expanded', 'true');
	await expect(card.getByTestId('supervision-saved-periods')).toContainText('คาบ 4 · วิทยาศาสตร์');
	await expect(card.getByText('แบบประเมินทดสอบ', { exact: true })).toBeVisible();
	await expect(card.getByText('ผู้รับผิดชอบตรวจคำขอ', { exact: false })).toBeVisible();
	const legacy = queue.locator('[data-slot="card"]').filter({
		has: page.getByRole('heading', { name: 'ครู หลักฐานเดิม', exact: true })
	});
	await expect(legacy.getByText('ไม่ระบุคาบ', { exact: true })).toBeVisible();
	const legacyEvidence = legacy.getByTestId('supervision-saved-periods');
	await expect(legacyEvidence).toBeHidden();
	await legacy.getByRole('button', { name: 'ดูรายละเอียด 1 คาบ' }).click();
	await expect(legacyEvidence).toBeVisible();
	await expect(legacyEvidence).toContainText('ไม่ระบุคาบ · วิชาจากหลักฐานเดิม');
	await expect(legacy).not.toContainText('คาบ 1');
	expect(api.count(`/api/supervision/observations/${id(30)}/evaluator-availability`)).toBe(0);
});

for (const width of [1280, 375]) {
	test(`calendar uses Bangkok dates and saved evaluators at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await mock(page, {
			term:
				width >= 640
					? { name: 'ภาคเรียนที่ 2', sequence: 2, termType: 'regular' }
					: { name: 'ภาคเรียนเสริมทักษะ', sequence: 3, termType: 'custom' },
			observation: { periods: requestPeriods, evaluators: [] },
			additionalObservations: [
				{
					id: id(40),
					cycleId: secondCycle,
					observedDisplayName: 'ครู คาบแยก',
					status: 'completed',
					periods: irregularPeriods,
					evaluators
				},
				{ id: id(41), observedDisplayName: 'ครู ถูกส่งกลับ', status: 'returned' },
				{ id: id(42), observedDisplayName: 'ครู ถูกยกเลิก', status: 'cancelled' },
				...crowdedBookings
			]
		});
		const termPath = `${requestsPath}&academicTermId=${id(8)}`;
		await page.goto(termPath);
		await expect(page.getByRole('heading', { name: 'คำขอจองที่รออนุมัติ' })).toBeVisible();
		await expect(page.getByTestId('supervision-references')).toBeHidden();
		const originalReads = api.reads.filter((value) => value.startsWith('/api/supervision/'));

		await page.getByRole('tab', { name: 'คำขอจอง' }).focus();
		await page.keyboard.press('ArrowRight');
		const month = page.getByRole('tabpanel', { name: 'ปฏิทิน' });
		const heading = month.getByRole('heading', {
			name: `ปฏิทินนิเทศการสอน ${width >= 640 ? 'ภาคเรียนที่ 2' : 'ภาคเรียนเสริมทักษะ'} ปีการศึกษา 2569`,
			exact: true
		});
		await expect(heading).toBeVisible();
		const cycles = month.getByText('รอบนิเทศ รอบแรก · รอบที่สอง', { exact: true });
		await expect(cycles).toBeVisible();
		await expect(month.getByText('พฤศจิกายน 2569', { exact: true })).toBeVisible();
		const day = month.locator(`[data-calendar-date="${bookingDay}"]`);
		await expect(day).toHaveAttribute('aria-pressed', 'true');
		await expect(day.locator('span').first()).toHaveClass(/\bbg-primary\b/);
		await expect(day).toHaveAccessibleName(/1 กิจกรรม, 1 คำร้องรออนุมัติ/);
		const pending = month.getByTitle('10:20 ครู ในคิว', { exact: true });
		await expect(pending).toHaveCSS('outline-style', 'dashed');
		await expect(pending).toHaveAttribute('aria-label', /ครู ในคิว.*รออนุมัติ/);
		expect(await pending.innerText()).toBe(width >= 640 ? '10:20 ครู ในคิว' : 'ครู ในคิว');
		await expect(month.getByRole('group', { name: 'สถานะนัดนิเทศ' })).toContainText('รออนุมัติคำขอ');
		const agenda = month.getByRole('region', { name: /1 พ.ย. 2569/ });
		const pendingBooking = agenda.getByRole('listitem').filter({ hasText: 'ครู ในคิว' });
		await expect(pendingBooking.getByText('ครู ในคิว', { exact: true })).toBeVisible();
		await expect(pendingBooking.getByText('รอบแรก', { exact: true })).toBeVisible();
		await expect(pendingBooking.getByText('รออนุมัติ', { exact: true })).toHaveClass('sr-only');
		await expect(pendingBooking.locator('[aria-hidden="true"].border-dashed')).toHaveCSS(
			'border-top-style',
			'dashed'
		);
		await expect(agenda.getByText('ยังไม่มอบหมายผู้ประเมิน')).toBeVisible();
		await expect(agenda.getByText('คาบ 3–4 · 10:20–12:00 น.')).toBeVisible();
		await expect(agenda.getByText('ครู คาบแยก', { exact: true })).toBeVisible();
		await expect(agenda.getByText('รอบที่สอง', { exact: true })).toBeVisible();
		await expect(agenda.getByText('เสร็จสิ้น', { exact: true })).toBeVisible();
		await expect(agenda.getByText('คาบ 3 · 00:20–01:10 น.')).toBeVisible();
		await expect(agenda.getByText('คาบ 5 · 02:40 น. · ไม่ระบุเวลาสิ้นสุด')).toBeVisible();
		for (const name of [
			...evaluators.slice(0, 2).map((item) => item.evaluatorDisplayName!),
			'ไม่ทราบชื่อผู้ประเมิน'
		]) {
			await expect(agenda.getByText(name, { exact: true })).toBeVisible();
		}
		await expect(month.getByText(/ครู ถูก(ส่งกลับ|ยกเลิก)/)).toHaveCount(0);
		await expect(agenda.getByRole('link', { name: 'รายละเอียด ครู คาบแยก' })).toHaveAttribute(
			'href',
			new RegExp(`/supervision/${id(40)}\\?`)
		);

		await month.getByRole('button', { name: 'เดือนก่อนหน้า' }).click();
		await expect(month.getByText('ตุลาคม 2569', { exact: true })).toBeVisible();
		const emptyDay = month.locator('[data-calendar-date="2026-10-31"]');
		await emptyDay.focus();
		await page.keyboard.press('Space');
		await expect(emptyDay).toHaveAttribute('aria-pressed', 'true');
		await expect(emptyDay).toHaveAccessibleName(/0 กิจกรรม$/);
		await expect(month.getByText('ไม่มีรายการจองนิเทศในวันนี้')).toBeVisible();
		await month.getByRole('button', { name: 'เดือนถัดไป' }).click();
		await expect(month.getByText('พฤศจิกายน 2569', { exact: true })).toBeVisible();
		await month.locator('[data-calendar-date="2026-11-02"]').click();
		await expect(month.getByText('ไม่มีรายการจองนิเทศในวันนี้')).toBeVisible();

		const crowdedCell = month.locator(`[data-calendar-date="${crowdedDay}"]`);
		await expect(crowdedCell).toHaveAccessibleName(/5 กิจกรรม$/);
		const overflow = crowdedCell.getByText(/^\+\d+$/);
		await expect(overflow).toBeVisible();
		const visibleChips = crowdedCell.locator('..').locator('button[title]').filter({
			hasText: 'ครู นัดที่'
		});
		const chips = await visibleChips.all();
		expect(chips.length).toBeGreaterThan(0);
		expect(chips.length + Number(await overflow.innerText())).toBe(crowdedBookings.length);
		const overflowBounds = await overflow.boundingBox();
		expect(overflowBounds).not.toBeNull();
		for (const chip of chips) {
			const chipBounds = await chip.boundingBox();
			expect(chipBounds).not.toBeNull();
			expect(chipBounds!.y + chipBounds!.height).toBeLessThanOrEqual(overflowBounds!.y);
		}
		await crowdedCell.click();
		const crowdedAgenda = month.getByRole('region', { name: /4 พ.ย. 2569/ });
		for (const booking of crowdedBookings) {
			await expect(
				crowdedAgenda.getByText(booking.observedDisplayName, { exact: true })
			).toBeVisible();
		}
		const lastDetails = crowdedAgenda.getByRole('link', {
			name: 'รายละเอียด ครู นัดที่ 5',
			exact: true
		});
		await lastDetails.scrollIntoViewIfNeeded();
		await expect(lastDetails).toBeInViewport();
		await month.getByRole('button', { name: 'เดือนถัดไป' }).click();
		await expect(month.getByText('ธันวาคม 2569', { exact: true })).toBeVisible();
		await expect(cycles).toHaveCount(0);
		await expect(heading).toBeVisible();
		await month.getByRole('button', { name: 'วันนี้', exact: true }).click();
		await expect(day).toHaveAttribute('aria-pressed', 'true');
		await expect(agenda.getByText('อาจารย์ มณี มีสุข', { exact: true })).toBeVisible();

		await page.addStyleTag({
			content: '*,*::before,*::after{transition:none!important;animation:none!important}'
		});
		for (const theme of ['light', 'dark']) {
			await page.evaluate(
				(value) => document.documentElement.classList.toggle('dark', value === 'dark'),
				theme
			);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await page.screenshot({
				path: testInfo.outputPath(`supervision-calendar-${width}-${theme}.png`),
				fullPage: true
			});
			if (width < 640) {
				await agenda.screenshot({ path: testInfo.outputPath(`supervision-agenda-${theme}.png`) });
				await heading.scrollIntoViewIfNeeded();
			}
		}
		await expect(page).toHaveURL(termPath);
		expect(api.reads.filter((value) => value.startsWith('/api/supervision/'))).toEqual(
			originalReads
		);
		expect(api.writes).toEqual([]);
	});
}

test('switching preserves request drafts and calendar reflects the saved approval', async ({
	page
}) => {
	const api = await mock(page, { observation: { periods: requestPeriods, evaluators: [] } });
	await page.goto(requestsPath);
	const reason = page.getByRole('textbox', { name: 'ส่งกลับคำขอ', exact: true });
	await reason.fill('เหตุผลฉบับร่าง');
	const picker = page.getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน' });
	await picker.click();
	await page.getByRole('option', { name: /ผู้ประเมินที่ว่าง/ }).click();
	await expect(page.getByPlaceholder('ค้นหาครูผู้ประเมิน...')).toBeVisible();
	await page.getByRole('tab', { name: 'ปฏิทิน' }).click();
	await expect(page.getByPlaceholder('ค้นหาครูผู้ประเมิน...')).toHaveCount(0);
	const agenda = page.getByRole('region', { name: /1 พ.ย. 2569/ });
	await expect(agenda.getByText('ยังไม่มอบหมายผู้ประเมิน')).toBeVisible();
	await expect(agenda.getByText('ผู้ประเมินที่ว่าง')).toHaveCount(0);
	await page.getByRole('tab', { name: 'คำขอจอง' }).click();
	await expect(picker).toHaveAttribute('aria-expanded', 'false');
	await expect(reason).toHaveValue('เหตุผลฉบับร่าง');
	await expect(page.getByRole('button', { name: 'ลบผู้ประเมิน ผู้ประเมินที่ว่าง' })).toBeVisible();
	expect(api.count(`${observationPath}/${id(30)}/evaluator-availability`)).toBe(1);
	expect(api.writes).toEqual([]);
	await page.getByRole('button', { name: 'อนุมัติและมอบหมาย' }).click();
	await expect(page.getByText('ไม่มีคำขอจองที่รออนุมัติ')).toBeVisible();
	await page.getByRole('tab', { name: 'ปฏิทิน' }).click();
	await expect(agenda.getByText('ผู้ประเมินที่ว่าง', { exact: true })).toBeVisible();
	await expect(agenda.getByText('นัดหมายแล้ว', { exact: true })).toBeVisible();
	expect(api.count(observationPath)).toBe(1);
	expect(api.writes).toEqual([`POST ${observationPath}/${id(30)}/approve-request`]);
});

test('changing year replaces calendar data in the authorized academic context', async ({
	page
}) => {
	const api = await mock(page, {
		observation: { periods: requestPeriods },
		hold: 'observations',
		holdAt: 2
	});
	await page.goto(requestsPath);
	await page.getByRole('tab', { name: 'ปฏิทิน' }).click();
	await expect(
		page.getByRole('heading', { name: 'ปฏิทินนิเทศการสอน ตลอดปีการศึกษา 2569' })
	).toBeVisible();
	await expect(page.getByText('รอบนิเทศ รอบแรก', { exact: true })).toBeVisible();
	await expect(
		page.getByRole('region', { name: /1 พ.ย. 2569/ }).getByText('ครู ในคิว')
	).toBeVisible();
	const oldRefresh = page.waitForRequest((request) => {
		const url = new URL(request.url());
		return (
			request.method() === 'GET' &&
			url.pathname === observationPath &&
			url.searchParams.get('academicYearId') === year
		);
	});
	await revalidatePage(page);
	const oldRequest = await oldRefresh;
	await expect(page.getByTestId('supervision-observations')).toHaveAttribute('aria-busy', 'true');
	const nextPath = path('requests', nextYear);
	await navigate(page, nextPath);
	const calendar = page.getByRole('tabpanel', { name: 'ปฏิทิน' });
	const newTeacher = calendar.getByRole('region', { name: /1 พ.ย. 2569/ }).getByText('ครู ปีใหม่');
	await expect(newTeacher).toBeVisible();
	await expect(
		calendar.getByRole('heading', { name: 'ปฏิทินนิเทศการสอน ตลอดปีการศึกษา 2570' })
	).toBeVisible();
	await expect(calendar.getByText('รอบนิเทศ รอบปีใหม่', { exact: true })).toBeVisible();
	api.release();
	const oldResponse = await oldRequest.response();
	expect(oldResponse?.ok()).toBe(true);
	expect(await oldResponse?.json()).toMatchObject({
		data: { items: [{ academicYearId: year, observedDisplayName: 'ครู ในคิว' }] }
	});
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
	await expect(calendar.getByText('ครู ในคิว', { exact: true })).toHaveCount(0);
	await expect(calendar.getByText('รอบนิเทศ รอบแรก', { exact: true })).toHaveCount(0);
	await expect(newTeacher).toBeVisible();
	await expect(page).toHaveURL(nextPath);
	expect(api.count(observationPath)).toBe(3);
});
