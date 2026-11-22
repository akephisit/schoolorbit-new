import { expect, test } from '@playwright/test';
import type { SupervisionEvaluator } from '../../src/lib/api/supervision';
import {
	id,
	mock,
	navigate,
	nextYear,
	path,
	savedPeriod,
	secondCycle
} from './fixtures/supervision-route-data';

const observationPath = '/api/supervision/observations';
const termQuery = `&academicTermId=${id(8)}`;
const evaluatePath = `${path('evaluate')}${termQuery}`;
const observedAt = '2026-10-31T17:20:00Z';
const endsAt = '2026-10-31T18:10:00Z';
const ownSubmitted: SupervisionEvaluator = {
	id: id(171),
	observationId: id(40),
	evaluatorUserId: id(20),
	evaluatorDisplayName: 'ผู้ประเมินทดสอบ',
	isRequired: true,
	status: 'submitted',
	createdAt: observedAt,
	updatedAt: observedAt,
	submittedAt: observedAt
};

test.use({ serviceWorkers: 'block', timezoneId: 'UTC' });

test('evaluators plan and complete only their assignments from the Bangkok calendar', async ({
	page
}) => {
	await page.clock.setFixedTime(new Date('2026-10-31T17:10:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
	const api = await mock(page, {
		permissions: ['supervision.read.school', 'supervision.evaluate.assigned'],
		term: true,
		observationStatus: 'planned',
		observation: {
			observedAt,
			periods: [savedPeriod({ observedAt, endsAt })]
		},
		additionalObservations: [
			{
				id: id(40),
				cycleId: secondCycle,
				observedDisplayName: 'ครู ส่งผลแล้ว',
				evaluators: [ownSubmitted]
			},
			{
				id: id(41),
				observedDisplayName: 'ครู ของผู้ประเมินอื่น',
				evaluators: [
					{ ...ownSubmitted, observationId: id(41), evaluatorUserId: id(191), status: 'assigned' }
				]
			},
			...(['returned', 'cancelled', 'requested'] as const).map((status, index) => ({
				id: id(42 + index),
				observedDisplayName: `ครู ที่ไม่นัด ${index}`,
				status,
				evaluators: [{ ...ownSubmitted, observationId: id(42 + index) }]
			}))
		]
	});
	await page.goto(evaluatePath);
	const listTab = page.getByRole('tab', { name: 'รายการ', exact: true });
	const monthTab = page.getByRole('tab', { name: 'ปฏิทิน', exact: true });
	const list = page.getByRole('tabpanel', { name: 'รายการ', exact: true });
	const month = page.getByRole('tabpanel', { name: 'ปฏิทิน', exact: true });
	const calendarHeading = month.getByRole('heading', { name: /^ปฏิทินนิเทศการสอน/ });
	const today = month.locator('[data-calendar-date="2026-11-01"]');
	const agenda = month.getByRole('region', { name: /1 พ.ย. 2569/ });
	const ownBooking = agenda.getByRole('listitem').filter({ hasText: 'ครู ในคิว' });
	const submittedBooking = agenda.getByRole('listitem').filter({ hasText: 'ครู ส่งผลแล้ว' });
	const openForm = { name: 'เปิดแบบประเมิน', exact: true };
	await expect(listTab).toHaveAttribute('aria-selected', 'true');
	await expect(list.getByRole('button', openForm)).toBeVisible();
	await expect(page.getByTestId('supervision-references')).toBeHidden();
	const initialReads = api.reads.filter((resource) => resource.startsWith('/api/supervision/'));
	await listTab.focus();
	await page.keyboard.press('ArrowRight');
	await expect(calendarHeading).toContainText('ภาคเรียนที่ 1 ปีการศึกษา 2569');
	await expect(month.getByText('รอบนิเทศ รอบแรก · รอบที่สอง', { exact: true })).toBeVisible();
	await expect(today).toHaveAttribute('aria-pressed', 'true');
	await expect(today).toHaveAccessibleName(/2 กิจกรรม$/);
	await expect(ownBooking.getByText('คาบ 1 · 00:20–01:10 น.')).toBeVisible();
	await expect(submittedBooking.getByText('นัดหมายแล้ว', { exact: true })).toBeVisible();
	await expect(submittedBooking.getByText('ส่งผลแล้ว', { exact: true })).toBeVisible();
	await expect(submittedBooking.getByRole('button', openForm)).toHaveCount(0);
	await expect(ownBooking.getByRole('button', openForm)).toBeVisible();
	await expect(month.getByText(/ครู (ของผู้ประเมินอื่น|ที่ไม่นัด)/)).toHaveCount(0);
	for (const width of [1280, 375]) {
		await page.setViewportSize({ width, height: 900 });
		for (const dark of [false, true]) {
			await page.evaluate(
				(value) => document.documentElement.classList.toggle('dark', value),
				dark
			);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await expect(ownBooking.getByRole('button', openForm)).toBeVisible();
		}
	}
	expect(api.reads.filter((resource) => resource.startsWith('/api/supervision/'))).toEqual(
		initialReads
	);

	const details = ownBooking.getByRole('link', { name: 'รายละเอียด ครู ในคิว', exact: true });
	const exactDetailPath = `${path(id(30))}${termQuery}`;
	await expect(details).toHaveAttribute('href', exactDetailPath);
	await details.click();
	await expect(page).toHaveURL(exactDetailPath);
	await expect(
		page.getByRole('heading', { name: 'ครู ในคิว', exact: true, level: 1 })
	).toBeVisible();
	await navigate(page, evaluatePath);
	await monthTab.click();
	await ownBooking.getByRole('button', openForm).click();
	const dialog = page.getByRole('dialog', { name: 'ทำแบบประเมินนิเทศ' });
	await expect(dialog.getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	const submit = dialog.getByRole('button', { name: 'ส่งผลประเมิน', exact: true });
	await expect(submit).toBeInViewport();
	await dialog.getByRole('button', { name: '3', exact: true }).click();
	await expect(submit).toBeInViewport();
	await submit.click();
	await expect(dialog).toHaveCount(0);
	await expect(ownBooking.getByText('ส่งผลแล้ว', { exact: true })).toBeVisible();
	await expect(ownBooking.getByRole('button', openForm)).toHaveCount(0);
	expect(api.writes).toEqual([`POST ${observationPath}/${id(30)}/evaluations/me/submit`]);
	await listTab.click();
	await expect(list.getByText('ยังไม่มีรายการที่ต้องประเมิน', { exact: true })).toBeVisible();
	await expect(
		list
			.locator('[data-supervision-submitted-assigned-list]')
			.getByRole('heading', { name: 'ครู ในคิว', exact: true })
	).toBeVisible();

	await monthTab.click();
	const tomorrow = month.locator('[data-calendar-date="2026-11-02"]');
	await tomorrow.click();
	await expect(tomorrow).toHaveAttribute('aria-pressed', 'true');
	await navigate(page, path('evaluate', nextYear));
	await expect(calendarHeading).toContainText('ตลอดปีการศึกษา 2570');
	await expect(today).toHaveAttribute('aria-pressed', 'true');
	await expect(agenda.getByText('ครู ปีใหม่', { exact: true })).toBeVisible();
	await expect(agenda.getByText('ครู ในคิว', { exact: true })).toHaveCount(0);
});
