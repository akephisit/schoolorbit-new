import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import { ids, edition } from './helpers/curriculum-fixtures';
import {
	curriculumCatalogFixture,
	curriculumCatalogPath,
	currentSubjectVersionId,
	installCurriculumCatalog,
	openCurriculumEditor
} from './fixtures/curriculum-catalog';

test.use({ serviceWorkers: 'block', timezoneId: 'Asia/Bangkok', hasTouch: true });
const currentCheckbox = (page: Page) =>
	page.getByRole('checkbox', { name: /^เลือก ค21101 คณิตศาสตร์พื้นฐาน ·/ });
const editor = (page: Page) => page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
const picker = (page: Page) => editor(page).getByRole('region', { name: 'เลือกจากทะเบียน' });
const rows = (page: Page) =>
	editor(page)
		.getByRole('region', { name: 'รายการในหลักสูตร' })
		.getByRole('row')
		.filter({ hasText: 'ค21101' });
async function selectContext(
	page: Page,
	field: 'แผนการเรียน' | 'ระดับชั้น' | 'ภาคเรียน',
	option: string
) {
	await editor(page)
		.getByRole('button', { name: `${field}ที่จะจัดโครงสร้าง`, exact: true })
		.click();
	await page.getByRole('option', { name: option, exact: true }).click();
}
async function addCurrent(page: Page) {
	await currentCheckbox(page).check();
	await picker(page)
		.getByRole('button', { name: /เพิ่ม 1 รายการ/ })
		.click();
}

test('only current v2 is selectable for new requirements in M1 through M3, with dates and pinned read-only rows', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	await expect(picker(page).getByRole('checkbox', { name: /^เลือก ค21101 / })).toHaveCount(1);
	await expect(picker(page)).not.toContainText(/\bv\d+\b/);
	await expect(picker(page)).not.toContainText('วิชาที่หมดอายุ');
	await expect(
		picker(page)
			.locator('label')
			.filter({ has: currentCheckbox(page) })
	).toContainText('ตั้งแต่ 1 พ.ค. 2569');
	for (const grade of [1, 2, 3]) {
		if (grade > 1) await selectContext(page, 'ระดับชั้น', `มัธยมศึกษาปีที่ ${grade}`);
		await addCurrent(page);
		await expect(currentCheckbox(page)).toBeDisabled();
		await expect(picker(page)).toContainText('มีรายวิชานี้แล้ว');
	}
	await editor(page).getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
	await expect(editor(page)).toBeHidden();
	expect(state.writes).toHaveLength(1);
	expect(state.writes[0].body).toMatchObject({
		requirements: fixture.workspace.gradeLevels.map((grade) => ({
			catalogVersionId: currentSubjectVersionId,
			gradeLevelId: grade.id
		}))
	});
	await expect(page.getByRole('row').filter({ hasText: 'ค21101' })).not.toContainText(/\bv\d+\b/);
	await expect(
		page
			.getByRole('row')
			.filter({ hasText: 'ค21101' })
			.getByRole('button', { name: 'เก่า', exact: true })
	).toHaveCount(0);
	expect(state.errors).toEqual([]);
	expect(state.unexpected).toEqual([]);
});

test('missing old option retains pinned v1 metadata and visibly blocks the same subject v2', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ omitOldOption: true });
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	await expect(rows(page)).toHaveCount(1);
	await expect(rows(page)).toContainText('คณิตศาสตร์พื้นฐาน');
	await expect(rows(page).getByRole('button', { name: 'เก่า', exact: true })).toBeVisible();
	await expect(editor(page)).not.toContainText(/\bv\d+\b/);
	await expect(currentCheckbox(page)).toBeDisabled();
	await expect(picker(page)).toContainText('มีรายวิชานี้แล้ว');
	await rows(page).getByRole('button', { name: 'เก่า', exact: true }).focus();
	await expect(page.getByRole('tooltip')).toContainText(
		'มีรายวิชารุ่นใหม่ที่เริ่มใช้แล้วในทะเบียน'
	);
	await page.keyboard.press('Escape');
	await editor(page).getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
	expect(state.writes[0].body).toMatchObject({
		requirements: [{ catalogVersionId: ids.catalogVersion }]
	});
	expect(state.errors).toEqual([]);
});

test('explicit remove then add replaces only the selected pinned row', async ({ page }) => {
	const fixture = curriculumCatalogFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	await rows(page).getByRole('button', { name: 'นำรายการออก', exact: true }).click();
	await expect(currentCheckbox(page)).toBeEnabled();
	await addCurrent(page);
	await expect(rows(page).getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(0);
	await expect(editor(page)).not.toContainText(/\bv\d+\b/);
	await editor(page).getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
	expect(state.writes[0].body).toMatchObject({
		requirements: [{ catalogVersionId: currentSubjectVersionId }]
	});
	expect(fixture.original.catalogVersionId).toBe(ids.catalogVersion);
	expect(fixture.original.versionNo).toBe(1);
	expect(state.errors).toEqual([]);
});

test('legacy v1 and v2 duplicates remain independently editable and are not silently removed', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ legacyDuplicates: true });
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	await expect(rows(page)).toHaveCount(2);
	await expect(picker(page)).toContainText('มีรายวิชานี้แล้ว');
	await rows(page)
		.filter({ has: page.getByRole('button', { name: 'เก่า', exact: true }) })
		.getByRole('button', { name: 'บังคับ', exact: true })
		.click();
	await page.getByRole('option', { name: 'เลือก', exact: true }).click();
	await expect(
		rows(page)
			.filter({ hasNot: page.getByRole('button', { name: 'เก่า', exact: true }) })
			.getByRole('button', { name: 'บังคับ', exact: true })
	).toBeVisible();
	await editor(page).getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
	expect(state.writes[0].body).toMatchObject({
		requirements: [
			{ catalogVersionId: ids.catalogVersion, requirementKind: 'elective' },
			{ catalogVersionId: currentSubjectVersionId, requirementKind: 'required' }
		]
	});
	expect(state.errors).toEqual([]);
});

test('context changes clear pending selections and other programs, grades and terms allow the same subject', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	await selectContext(page, 'ระดับชั้น', 'มัธยมศึกษาปีที่ 2');
	await currentCheckbox(page).check();
	await selectContext(page, 'ระดับชั้น', 'มัธยมศึกษาปีที่ 1');
	await expect(currentCheckbox(page)).not.toBeChecked();
	await expect(currentCheckbox(page)).toBeDisabled();
	await expect(picker(page).getByRole('button', { name: /เพิ่ม.*รายการ/ })).toBeDisabled();
	await selectContext(page, 'ภาคเรียน', 'ภาคเรียนที่ 2');
	await currentCheckbox(page).check();
	await selectContext(page, 'แผนการเรียน', 'แผนวิทย์คณิต');
	await expect(currentCheckbox(page)).not.toBeChecked();
	await expect(picker(page).getByRole('button', { name: /เพิ่ม.*รายการ/ })).toBeDisabled();
	await addCurrent(page);
	await expect(currentCheckbox(page)).toBeDisabled();
	await selectContext(page, 'แผนการเรียน', 'แผนการเรียนพื้นฐาน');
	await expect(currentCheckbox(page)).toBeEnabled();
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});

test('stale-save 409 preserves chosen v2 and never retries or upgrades it', async ({ page }) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	const state = await installCurriculumCatalog(page, fixture, { failSaveOnce: true });
	await openCurriculumEditor(page);
	await addCurrent(page);
	await editor(page).getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
	await expect(editor(page).getByRole('alert')).toContainText('ไม่ใช่รุ่นปัจจุบันแล้ว');
	await expect(rows(page).getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(0);
	await expect(editor(page)).not.toContainText(/\bv\d+\b/);
	await expect(currentCheckbox(page)).toBeDisabled();
	expect(state.writes).toHaveLength(1);
	expect(state.writes[0].body).toMatchObject({
		requirements: [{ catalogVersionId: currentSubjectVersionId }]
	});
	expect(state.errors).toEqual([]);
});

test('activity revisions remain selectable with the existing exact-version duplicate policy', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	for (const version of [1, 2]) {
		await picker(page)
			.getByRole('checkbox', {
				name:
					version === 1
						? /เลือก กิจกรรมชุมนุม · ตั้งแต่ 1 พ.ค. 2570/
						: /เลือก กิจกรรมชุมนุม · ตั้งแต่ 1 มิ.ย. 2570/
			})
			.check();
		await picker(page)
			.getByRole('button', { name: /เพิ่ม 1 รายการ/ })
			.click();
	}
	const table = editor(page).getByRole('region', { name: 'รายการในหลักสูตร' });
	await expect(table.getByRole('row').filter({ hasText: 'กิจกรรมชุมนุม' })).toHaveCount(2);
	await expect(table.getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(0);
	await expect(table).not.toContainText(/\bv\d+\b/);
	await expect(table).not.toContainText('A01');
	await picker(page)
		.getByRole('checkbox', { name: /เลือก กิจกรรมชุมนุม · ตั้งแต่ 1 พ.ค. 2570/ })
		.check();
	await picker(page)
		.getByRole('button', { name: /เพิ่ม 1 รายการ/ })
		.click();
	await expect(table.getByRole('row').filter({ hasText: 'กิจกรรมชุมนุม' })).toHaveCount(2);
	await expect(table.getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(0);
	await expect(table).not.toContainText(/\bv\d+\b/);
	await expect(table).not.toContainText('A01');
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});

test('published comparison and document show only the old badge without creating a curriculum draft', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ readOnly: true });
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath.split('?')[0]);
	await expect(
		page
			.getByRole('row')
			.filter({ hasText: 'ค21101' })
			.getByRole('button', { name: 'เก่า', exact: true })
	).toBeVisible();
	await expect(page.getByRole('row').filter({ hasText: 'ค21101' })).not.toContainText(/\bv\d+\b/);
	await page.getByRole('tab', { name: 'เอกสารรายแผน' }).click();
	await expect(
		page
			.getByRole('row')
			.filter({ hasText: 'ค21101' })
			.getByRole('button', { name: 'เก่า', exact: true })
	).toBeVisible();
	await expect(page.getByRole('row').filter({ hasText: 'ค21101' })).not.toContainText(/\bv\d+\b/);
	await expect(page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true })).toHaveCount(0);
	await page.goto(
		`${curriculumCatalogPath.split('?')[0]}?publicationId=${fixture.workspace.level.publicationId}`
	);
	await expect(page.getByText('ประวัติการเผยแพร่ · อ่านอย่างเดียว')).toBeVisible();
	for (const tab of ['เทียบทุกแผน', 'เอกสารรายแผน']) {
		await page.getByRole('tab', { name: tab, exact: true }).click();
		const historicalCourse = page.getByRole('row').filter({ hasText: 'ค21101' });
		await expect(historicalCourse.getByRole('button', { name: 'เก่า', exact: true })).toBeVisible();
		await expect(historicalCourse).not.toContainText(/\bv\d+\b/);
	}
	await expect(page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true })).toHaveCount(0);
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});

test('copying a published program keeps its pinned v1 even when v2 is current', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await page.getByRole('button', { name: 'คัดลอกแผนจากฉบับเดิม', exact: true }).click();
	const dialog = page.getByRole('dialog');
	for (const [field, choice] of [
		['ฉบับต้นทาง *', edition().name],
		['ระดับการศึกษาต้นทาง *', 'ระดับมัธยมศึกษาตอนต้น'],
		['แผนต้นทาง *', 'แผนการเรียนพื้นฐาน']
	]) {
		await dialog.getByLabel(field, { exact: true }).click();
		await page.getByRole('option', { name: choice, exact: true }).click();
	}
	await dialog.getByRole('button', { name: 'คัดลอกเป็นแผนร่าง', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect
		.poll(() =>
			fixture.workspace.requirements
				.filter((item) => item.studyProgramId === 'copied-program')
				.map((item) => item.catalogVersionId)
		)
		.toEqual([ids.catalogVersion]);
	await page.getByRole('tab', { name: 'เอกสารรายแผน' }).click();
	await page.getByRole('button', { name: 'แผนการเรียนที่แสดง', exact: true }).click();
	await page.getByRole('option', { name: 'แผนคัดลอก', exact: true }).click();
	await expect(
		page
			.getByRole('row')
			.filter({ hasText: 'ค21101' })
			.getByRole('button', { name: 'เก่า', exact: true })
	).toBeVisible();
	await expect(page.getByRole('row').filter({ hasText: 'ค21101' })).not.toContainText(/\bv\d+\b/);
	expect(state.writes).toHaveLength(1);
	expect(state.writes[0].path).toContain('/copy-program');
	expect(state.errors).toEqual([]);
});

for (const width of [1440, 390])
	for (const mode of ['light', 'dark'] as const) {
		test(`${width}px ${mode}: old badge, picker dates and nested scrolling remain readable`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			const fixture = curriculumCatalogFixture({ longCatalog: true });
			const state = await installCurriculumCatalog(page, fixture, { mode });
			await openCurriculumEditor(page);
			await expect(page.locator('html')).toHaveClass(mode === 'dark' ? /dark/ : /^(?!.*dark)/);
			await expect(rows(page).getByRole('button', { name: 'เก่า', exact: true })).toBeVisible();
			await expect(editor(page)).not.toContainText(/\bv\d+\b/);
			await expect(picker(page)).toContainText('ตั้งแต่ 1 พ.ค. 2569');
			expect(
				await editor(page).evaluate((element) => element.scrollWidth <= element.clientWidth)
			).toBe(true);
			const catalogScroll = picker(page).locator('.overflow-y-auto');
			expect(
				await catalogScroll.evaluate((element) => element.scrollHeight > element.clientHeight)
			).toBe(true);
			await catalogScroll.evaluate((element) => {
				element.scrollTop = element.scrollHeight;
			});
			await expect(picker(page).getByRole('checkbox', { name: /^เลือก ท17 / })).toBeVisible();
			const tableScroll = editor(page)
				.getByRole('region', { name: 'รายการในหลักสูตร' })
				.locator('[data-slot="table-container"]');
			if (width === 390)
				expect(
					await tableScroll.evaluate((element) => element.scrollWidth > element.clientWidth)
				).toBe(true);
			await catalogScroll.evaluate((element) => {
				element.scrollTop = 0;
			});
			await currentCheckbox(page).scrollIntoViewIfNeeded();
			const dir = process.env.E2E_ARTIFACT_DIR ?? testInfo.outputPath('screenshots');
			await mkdir(dir, { recursive: true });
			await page.screenshot({
				path: path.join(dir, `outdated-badge-${width}-${mode}.png`),
				fullPage: true
			});
			if (width === 390) {
				const badge = rows(page).getByRole('button', { name: 'เก่า', exact: true });
				await badge.scrollIntoViewIfNeeded();
				await badge.tap();
				await expect(page.getByRole('tooltip')).toContainText(
					'มีรายวิชารุ่นใหม่ที่เริ่มใช้แล้วในทะเบียน'
				);
				await page.screenshot({
					path: path.join(dir, `outdated-badge-${width}-${mode}-explanation.png`),
					fullPage: true
				});
			}
			expect(state.errors).toEqual([]);
			expect(state.unexpected).toEqual([]);
		});
	}

test('false server status shows no badge in comparison, document or editor even with higher revision metadata', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	fixture.workspace.requirements[0].hasNewerCurrentVersion = false;
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	const course = page.getByRole('row').filter({ hasText: 'ค21101' });
	await expect(course.getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(0);
	await expect(course).not.toContainText(/\bv\d+\b/);
	await page.getByRole('tab', { name: 'เอกสารรายแผน' }).click();
	await expect(course.getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(0);
	await expect(course).not.toContainText(/\bv\d+\b/);
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	await expect(rows(page).getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(0);
	await expect(editor(page)).not.toContainText(/\bv\d+\b/);
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});
