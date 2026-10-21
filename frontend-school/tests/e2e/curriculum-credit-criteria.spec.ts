import { expect, test, type Page } from '@playwright/test';
import type { CurriculumCreditCriterion } from '../../src/lib/api/academic-core.js';
import {
	curriculumCatalogFixture,
	curriculumCatalogPath,
	installCurriculumCatalog,
	openCurriculumEditor
} from './fixtures/curriculum-catalog';

test.use({ serviceWorkers: 'block', timezoneId: 'Asia/Bangkok' });
function creditFixture() {
	const fixture = curriculumCatalogFixture({ legacyDuplicates: true });
	const first = fixture.original;
	fixture.workspace.requirements = [
		{
			...first,
			id: 'basic-first',
			metrics: { ...first.metrics, credit: '10' },
			creditCategory: 'basic'
		},
		{
			...first,
			id: 'basic-second',
			catalogId: 'second-basic',
			catalogVersionId: 'second-basic',
			termSlotId: fixture.workspace.termSlots[1].id,
			metrics: { ...first.metrics, credit: '12' },
			creditCategory: 'basic',
			hasNewerCurrentVersion: false
		},
		{
			...first,
			id: 'additional',
			catalogId: 'additional',
			catalogVersionId: 'additional',
			name: 'รายวิชาเพิ่มเติม',
			section: 'additional_course',
			creditCategory: 'additional',
			metrics: { ...first.metrics, credit: '2' },
			hasNewerCurrentVersion: false
		},
		{
			...first,
			id: 'activity',
			catalogId: 'activity',
			catalogVersionId: 'activity',
			name: 'กิจกรรม',
			code: 'HIDDEN-ACTIVITY',
			resourceKind: 'activity',
			section: 'student_development',
			creditCategory: null,
			metrics: { credit: null, weeklyUnit: 'hour', weeklyValue: '3', totalHours: '60' },
			hasNewerCurrentVersion: false
		}
	];
	return fixture;
}
function criteria(
	fixture: ReturnType<typeof creditFixture>,
	scope: 'annual' | 'level',
	targetCredits: string
): CurriculumCreditCriterion {
	return {
		scope,
		targetCredits,
		comparison: 'equal',
		enabled: true,
		gradeLevelIds: [...fixture.workspace.level.gradeLevelIds]
	};
}
const editor = (page: Page) => page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
const settings = (page: Page) =>
	editor(page).getByRole('region', { name: 'เกณฑ์หน่วยกิตพื้นฐาน', exact: true });
const annual = (page: Page) =>
	page.getByRole('table', { name: 'สรุปทั้งปี มัธยมศึกษาปีที่ 1 · ทุกภาคเรียน', exact: true });

test('annual and whole-level summaries split pinned basic/additional credits and ignore filtered terms for criteria', async ({
	page
}) => {
	const fixture = creditFixture();
	fixture.workspace.level.creditCriteria = [
		criteria(fixture, 'annual', '22'),
		criteria(fixture, 'level', '22')
	];
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await expect(annual(page).locator('thead tr').last().getByRole('columnheader')).toHaveText([
		'หน่วยกิตพื้นฐาน',
		'หน่วยกิตเพิ่มเติม',
		'หน่วยกิตรวม',
		'ชั่วโมง/สัปดาห์',
		'ชั่วโมงรวม',
		'หน่วยกิตพื้นฐาน',
		'หน่วยกิตเพิ่มเติม',
		'หน่วยกิตรวม',
		'ชั่วโมง/สัปดาห์',
		'ชั่วโมงรวม'
	]);
	await expect(annual(page).locator('tbody td').nth(1)).toHaveText('22.00');
	await expect(annual(page).locator('tbody td').nth(2)).toHaveText('2.00');
	await expect(annual(page).locator('tbody td').nth(3)).toHaveText('24.00');
	await expect(annual(page).locator('[data-credit-status]')).toHaveCount(1); // empty second plan is a known zero, below target
	await page.getByRole('checkbox', { name: 'ภาคเรียนที่ 2', exact: true }).uncheck();
	await expect(annual(page).locator('tbody td').nth(1)).toHaveText('22.00');
	const selected = page
		.getByRole('table', { name: 'สรุปช่วงที่เลือกและทั้งระดับ', exact: true })
		.locator('tbody tr');
	await expect(selected.first().locator('td').nth(1)).toHaveText('10.00');
	await expect(selected.first().locator('[data-credit-status]')).toHaveCount(0);
	await expect(selected.nth(1).locator('td').nth(1)).toHaveText('22.00');
	await page.getByRole('tab', { name: 'เอกสารรายแผน', exact: true }).click();
	await expect(annual(page).locator('tbody td').nth(1)).toHaveText('22.00');
	await expect(annual(page).locator('[data-credit-status]')).toHaveCount(0);
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});

for (const width of [390, 1543])
	for (const mode of ['light', 'dark'] as const) {
		test(`credit criteria warning and settings at ${width}px ${mode}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 1000 });
			const fixture = creditFixture();
			fixture.workspace.level.creditCriteria = [
				criteria(fixture, 'annual', '23'),
				criteria(fixture, 'level', '21')
			];
			const state = await installCurriculumCatalog(page, fixture, { mode });
			await page.goto(curriculumCatalogPath);
			const deficit = annual(page).locator('[data-credit-status="deficit"]').first();
			await deficit.click();
			await expect(page.locator('[role="tooltip"]:not([data-state="closed"])')).toContainText(
				'ขาด 1.00 หน่วยกิต'
			);
			await page.keyboard.press('Escape');
			const excess = page
				.getByRole('table', { name: 'สรุปช่วงที่เลือกและทั้งระดับ', exact: true })
				.locator('[data-credit-status="excess"]')
				.first();
			await excess.focus();
			await expect(page.locator('[role="tooltip"]:not([data-state="closed"])')).toContainText(
				'เกิน 1.00 หน่วยกิต'
			);
			await page.keyboard.press('Escape');
			await annual(page).scrollIntoViewIfNeeded();
			await page.screenshot({
				path: testInfo.outputPath(`credit-summary-${width}-${mode}.png`),
				animations: 'disabled',
				fullPage: true
			});
			await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
			await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
			await settings(page).scrollIntoViewIfNeeded();
			await expect(settings(page).getByTestId('current-credit-criteria')).toContainText(
				'รายปี เท่ากับ 23'
			);
			const geometry = await settings(page).evaluate((node) => ({
				left: node.getBoundingClientRect().left,
				right: node.getBoundingClientRect().right,
				viewport: innerWidth
			}));
			expect(geometry.left).toBeGreaterThanOrEqual(0);
			expect(geometry.right).toBeLessThanOrEqual(geometry.viewport);
			await page.screenshot({
				path: testInfo.outputPath(`credit-settings-${width}-${mode}.png`),
				animations: 'disabled',
				fullPage: true
			});
			expect(state.writes).toEqual([]);
			expect(state.errors).toEqual([]);
		});
	}

test('unknown category or credit produces a yellow unknown instead of a deficit and keeps raw credits', async ({
	page
}) => {
	const fixture = creditFixture();
	fixture.workspace.requirements.push({
		...fixture.original,
		id: 'unclassified',
		creditCategory: null,
		metrics: { ...fixture.original.metrics, credit: '3' }
	});
	fixture.workspace.level.creditCriteria = [criteria(fixture, 'annual', '22')];
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await expect(annual(page).locator('tbody td').nth(3)).toHaveText('27.00');
	const basic = annual(page).locator('tbody td').nth(1);
	await expect(basic.locator('[data-credit-status="unknown"]')).toHaveCount(1);
	await expect(basic.locator('[data-credit-status="deficit"]')).toHaveCount(0);
	await basic.getByRole('button').click();
	await expect(page.locator('[role="tooltip"]:not([data-state="closed"])')).toContainText(
		'ประเภทวิชาบางรายการไม่ครบ'
	);
	expect(state.errors).toEqual([]);
});

test('manual template and criteria save preserve every other settings and structure draft and independent revision', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ legacyDuplicates: true });
	const levelRevision = fixture.workspace.level.rowVersion;
	const programRevision = fixture.workspace.programs[0].rowVersion;
	fixture.workspace.level.creditCriteria = [
		{
			scope: 'level',
			comparison: 'minimum',
			targetCredits: '1',
			enabled: false,
			gradeLevelIds: [...fixture.workspace.level.gradeLevelIds]
		}
	];
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	const oldRow = editor(page)
		.getByRole('region', { name: 'รายการในหลักสูตร' })
		.getByRole('row')
		.filter({ has: page.getByRole('button', { name: 'เก่า', exact: true }) });
	await oldRow.getByRole('button', { name: 'บังคับ', exact: true }).click();
	await page.getByRole('option', { name: 'เลือก', exact: true }).click();
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor(page)
		.getByLabel('ชื่อแผนการเรียนใหม่', { exact: true })
		.fill('ชื่อแผนที่ยังไม่บันทึก');
	await editor(page).getByLabel('ชื่อภาคเรียน', { exact: true }).first().fill('ชื่อภาคฉบับร่าง');
	await editor(page).getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	await settings(page)
		.getByRole('button', { name: /ใช้ตัวอย่าง มัธยมศึกษาตอนต้น/ })
		.click();
	await expect(settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true })).toHaveValue(
		'22.00'
	);
	await expect(settings(page).getByLabel('หน่วยกิตพื้นฐานทั้งระดับ', { exact: true })).toHaveValue(
		'1'
	);
	await expect(
		settings(page).getByRole('checkbox', { name: 'เปิดตรวจเกณฑ์ทั้งระดับ', exact: true })
	).not.toBeChecked();
	expect(state.writes).toEqual([]);
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	await expect(editor(page)).toBeVisible();
	await expect(editor(page).getByLabel('ชื่อแผนการเรียนใหม่', { exact: true })).toHaveValue(
		'ชื่อแผนที่ยังไม่บันทึก'
	);
	await expect(editor(page).getByLabel('ชื่อภาคเรียน', { exact: true }).first()).toHaveValue(
		'ชื่อภาคฉบับร่าง'
	);
	expect(state.structureReads()).toBe(1);
	expect(fixture.workspace.level.rowVersion).toBe(levelRevision);
	expect(fixture.workspace.programs[0].rowVersion).toBe(programRevision);
	expect(state.writes[0].body).toMatchObject({
		rowVersion: 1,
		criteria: expect.arrayContaining([
			{
				scope: 'annual',
				comparison: 'equal',
				enabled: true,
				targetCredits: '22.00',
				gradeLevelIds: fixture.workspace.level.gradeLevelIds
			}
		])
	});
	await editor(page).getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(
		editor(page).getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })
	).toBeDisabled();
	expect(state.writes[1].body).toMatchObject({ rowVersion: levelRevision });
	await editor(page).getByRole('button', { name: 'กลับไปจัดรายวิชา', exact: true }).click();
	await expect(oldRow.getByRole('button', { name: 'เลือก', exact: true })).toBeVisible();
	expect(state.errors).toEqual([]);
});

test('criteria conflict preserves draft and exposes latest server values only after explicit review', async ({
	page
}) => {
	const fixture = creditFixture();
	const initialSlotRevision = fixture.workspace.level.rowVersion;
	const state = await installCurriculumCatalog(page, fixture, { failCriteriaOnce: true });
	await openCurriculumEditor(page);
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await settings(page).getByRole('button', { name: 'เพิ่มเกณฑ์รายปี', exact: true }).click();
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('22');
	await editor(page).getByLabel('ชื่อภาคเรียน', { exact: true }).first().fill('ภาคฉบับร่าง');
	fixture.workspace.level.creditCriteria = [criteria(fixture, 'annual', '25')];
	fixture.workspace.level.creditCriteriaRowVersion = 2;
	fixture.workspace.level.rowVersion++;
	fixture.workspace.termSlots[0].name = 'ภาคของผู้แก้ไขอีกคน';
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(settings(page).getByRole('alert')).toContainText('เปลี่ยนไปแล้ว');
	await expect(settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true })).toHaveValue(
		'22'
	);
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	await settings(page)
		.getByRole('button', { name: 'โหลดเกณฑ์ล่าสุดเพื่อตรวจสอบ', exact: true })
		.click();
	await expect(settings(page).getByTestId('current-credit-criteria')).toContainText(
		'รายปี เท่ากับ 25'
	);
	await expect(settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true })).toHaveValue(
		'22'
	);
	await expect(editor(page).getByLabel('ชื่อภาคเรียน', { exact: true }).first()).toHaveValue(
		'ภาคฉบับร่าง'
	);
	expect(state.writes).toHaveLength(1);
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	expect(state.writes[1].body).toMatchObject({ rowVersion: 2 });
	await editor(page).getByRole('button', { name: 'บันทึกภาคเรียน', exact: true }).click();
	await expect(editor(page).getByRole('alert')).toContainText('ภาคเรียนเปลี่ยนไปแล้ว');
	expect(state.writes[2].body).toMatchObject({ rowVersion: initialSlotRevision });
	await expect(editor(page).getByLabel('ชื่อภาคเรียน', { exact: true }).first()).toHaveValue(
		'ภาคฉบับร่าง'
	);
	expect(state.errors).toEqual([]);
});

test('changed saved grade coverage stays unknown until explicit save and no-op numeric drafts can review coverage', async ({
	page
}) => {
	const fixture = creditFixture();
	fixture.workspace.level.creditCriteria = [
		{ ...criteria(fixture, 'annual', '22'), gradeLevelIds: [fixture.workspace.gradeLevels[0].id] }
	];
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	await expect(annual(page).locator('[data-credit-status="unknown"]').first()).toHaveCount(1);
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeEnabled();
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	expect(state.writes[0].body).toMatchObject({
		criteria: [{ gradeLevelIds: fixture.workspace.level.gradeLevelIds }]
	});
	expect(state.errors).toEqual([]);
});

test('invalid target never writes and an explicit zero target remains valid', async ({ page }) => {
	const fixture = creditFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await settings(page).getByRole('button', { name: 'เพิ่มเกณฑ์รายปี', exact: true }).click();
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('01.5');
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(settings(page).getByRole('alert')).toContainText('0 ถึง 9999.99');
	expect(state.writes).toEqual([]);
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('0');
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	expect(state.writes[0].body).toMatchObject({ criteria: [{ targetCredits: '0.00' }] });
	expect(state.errors).toEqual([]);
});

test('coverage changed on the server requires an explicit page review and never silently advances the criteria draft', async ({
	page
}) => {
	const fixture = creditFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await settings(page).getByRole('button', { name: 'เพิ่มเกณฑ์รายปี', exact: true }).click();
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('22');
	fixture.workspace.level.gradeLevelIds = fixture.workspace.level.gradeLevelIds.slice(0, 2);
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(settings(page).getByRole('alert')).toContainText('ชั้นที่ครอบคลุมเปลี่ยนไปแล้ว');
	await settings(page)
		.getByRole('button', { name: 'โหลดเกณฑ์ล่าสุดเพื่อตรวจสอบ', exact: true })
		.click();
	await expect(settings(page).getByRole('alert')).toContainText('ปิดตัวแก้ไขแล้วโหลดหน้าใหม่');
	await expect(settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true })).toHaveValue(
		'22'
	);
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	expect(state.writes).toHaveLength(1);
	expect(state.errors).toEqual([]);
});

test('a completed but delayed criteria review cannot regress a reopened editor after a newer criteria save', async ({
	page
}) => {
	const fixture = creditFixture();
	const state = await installCurriculumCatalog(page, fixture, { failCriteriaOnce: true });
	await page.addInitScript(() => {
		const requestFetch = window.fetch.bind(window);
		let reads = 0;
		window.fetch = async (input, init) => {
			const response = await requestFetch(input, init);
			const url = new URL(input instanceof Request ? input.url : String(input), location.href);
			if (
				!url.pathname.endsWith('/structure') ||
				(init?.method ?? 'GET') !== 'GET' ||
				++reads !== 2
			)
				return response;
			const body = await response.text();
			const headers = new Headers(response.headers);
			document.documentElement.dataset.criteriaReviewWaiting = 'true';
			await new Promise<void>((resolve) =>
				window.addEventListener('release-test-criteria-review', () => resolve(), { once: true })
			);
			const buffered = new Response(body, { status: response.status, headers });
			const read = buffered.text.bind(buffered);
			buffered.text = async () => {
				const text = await read();
				document.documentElement.dataset.criteriaReviewConsumed = 'true';
				return text;
			};
			return buffered;
		};
	});
	await openCurriculumEditor(page);
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await settings(page).getByRole('button', { name: 'เพิ่มเกณฑ์รายปี', exact: true }).click();
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('20');
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(settings(page).getByRole('alert')).toBeVisible();
	await settings(page)
		.getByRole('button', { name: 'โหลดเกณฑ์ล่าสุดเพื่อตรวจสอบ', exact: true })
		.click();
	await page.waitForFunction(
		() => document.documentElement.dataset.criteriaReviewWaiting === 'true'
	);
	await editor(page).getByRole('button', { name: 'Close', exact: true }).click();
	await expect(editor(page)).toBeHidden();
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await settings(page)
		.getByRole('button', { name: /ใช้ตัวอย่าง มัธยมศึกษาตอนต้น/ })
		.click();
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	await page.evaluate(() => window.dispatchEvent(new Event('release-test-criteria-review')));
	await page.waitForFunction(
		() => document.documentElement.dataset.criteriaReviewConsumed === 'true'
	);
	await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
	await editor(page).getByRole('button', { name: 'Close', exact: true }).click();
	await expect(editor(page)).toBeHidden();
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await expect(settings(page).getByTestId('current-credit-criteria')).toContainText(
		'รายปี เท่ากับ 22'
	);
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('23');
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	expect(state.writes[2].body).toMatchObject({
		rowVersion: 2,
		criteria: [{ targetCredits: '23.00' }]
	});
	expect(state.structureReads()).toBe(2);
	expect(state.errors).toEqual([]);
});

async function holdFirstMutationResponse(page: Page, endpoint: string) {
	await page.addInitScript((suffix) => {
		const requestFetch = window.fetch.bind(window);
		let held = false;
		window.fetch = async (input, init) => {
			const response = await requestFetch(input, init);
			const url = new URL(input instanceof Request ? input.url : String(input), location.href);
			if (held || !url.pathname.endsWith(suffix) || init?.method !== 'PUT') return response;
			held = true;
			const body = await response.text();
			const headers = new Headers(response.headers);
			document.documentElement.dataset.criteriaMutationWaiting = 'true';
			await new Promise<void>((resolve) =>
				window.addEventListener('release-test-criteria-mutation', () => resolve(), { once: true })
			);
			const buffered = new Response(body, { status: response.status, headers });
			const read = buffered.text.bind(buffered);
			buffered.text = async () => {
				const text = await read();
				document.documentElement.dataset.criteriaMutationConsumed = 'true';
				return text;
			};
			return buffered;
		};
	}, endpoint);
}
async function releaseMutationResponse(page: Page) {
	await page.evaluate(() => window.dispatchEvent(new Event('release-test-criteria-mutation')));
	await page.waitForFunction(
		() => document.documentElement.dataset.criteriaMutationConsumed === 'true'
	);
	await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
}

test('a delayed older criteria PUT cannot regress newer criteria saved by a reopened editor', async ({
	page
}) => {
	const fixture = creditFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await holdFirstMutationResponse(page, '/credit-criteria');
	await openCurriculumEditor(page);
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await settings(page).getByRole('button', { name: 'เพิ่มเกณฑ์รายปี', exact: true }).click();
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('22');
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await page.waitForFunction(
		() => document.documentElement.dataset.criteriaMutationWaiting === 'true'
	);
	await editor(page).getByRole('button', { name: 'Close', exact: true }).click();
	await expect(editor(page)).toBeHidden();
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await settings(page).getByRole('button', { name: 'เพิ่มเกณฑ์รายปี', exact: true }).click();
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('23');
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(settings(page).getByRole('alert')).toBeVisible();
	await settings(page)
		.getByRole('button', { name: 'โหลดเกณฑ์ล่าสุดเพื่อตรวจสอบ', exact: true })
		.click();
	await expect(settings(page).getByTestId('current-credit-criteria')).toContainText(
		'รายปี เท่ากับ 22'
	);
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	await releaseMutationResponse(page);
	await editor(page).getByRole('button', { name: 'Close', exact: true }).click();
	await expect(editor(page)).toBeHidden();
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await expect
		.soft(settings(page).getByTestId('current-credit-criteria'))
		.toContainText('รายปี เท่ากับ 23');
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('24');
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect.poll(() => state.writes.length).toBe(4);
	expect(state.writes[3].body).toMatchObject({
		rowVersion: 3,
		criteria: [{ targetCredits: '24.00' }]
	});
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	expect(state.errors).toEqual([]);
});

test('a delayed structure response preserves newer independent criteria while applying the structure result', async ({
	page
}) => {
	const fixture = creditFixture();
	fixture.workspace.level.creditCriteria = [criteria(fixture, 'annual', '20')];
	const state = await installCurriculumCatalog(page, fixture);
	await holdFirstMutationResponse(page, '/structure');
	await openCurriculumEditor(page);
	await editor(page).getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
	await page.waitForFunction(
		() => document.documentElement.dataset.criteriaMutationWaiting === 'true'
	);
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('22');
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	await releaseMutationResponse(page);
	await expect(editor(page)).toBeHidden();
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	await editor(page).getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await expect
		.soft(settings(page).getByTestId('current-credit-criteria'))
		.toContainText('รายปี เท่ากับ 22');
	await settings(page).getByLabel('หน่วยกิตพื้นฐานรายปี', { exact: true }).fill('23');
	await settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true }).click();
	await expect.poll(() => state.writes.length).toBe(3);
	expect(state.writes[2].body).toMatchObject({
		rowVersion: 2,
		criteria: [{ targetCredits: '23.00' }]
	});
	await expect(
		settings(page).getByRole('button', { name: 'บันทึกเกณฑ์หน่วยกิต', exact: true })
	).toBeDisabled();
	expect(state.errors).toEqual([]);
});
