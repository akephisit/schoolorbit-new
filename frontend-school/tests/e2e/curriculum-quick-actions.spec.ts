import { expect, test, type Locator, type Page } from '@playwright/test';
import type {
	CurriculumStructureRequirement,
	ReplaceCurriculumStructureRequest
} from '../../src/lib/api/academic-core.js';
import { ids } from './helpers/curriculum-fixtures';
import {
	curriculumCatalogFixture,
	curriculumCatalogPath,
	currentSubjectVersionId,
	installCurriculumCatalog
} from './fixtures/curriculum-catalog';

test.use({ serviceWorkers: 'block', timezoneId: 'Asia/Bangkok' });
type Fixture = ReturnType<typeof curriculumCatalogFixture>;
const matrix = (page: Page) => page.getByTestId('curriculum-matrix');
const sheet = (page: Page, kind = 'รายวิชา') =>
	page.getByRole('dialog', { name: `เพิ่ม${kind}`, exact: true });
const requirement = (page: Page, id = ids.requirement) =>
	matrix(page).locator(`[data-requirement-id="${id}"]`).first();
function contextTarget(
	page: Page,
	fixture: Fixture,
	options: {
		program?: number;
		term?: number;
		section?: string;
		header?: boolean;
	} = {}
) {
	const scope = `[data-program-id="${fixture.workspace.programs[options.program ?? 0].id}"][data-grade-id="${fixture.workspace.gradeLevels[0].id}"][data-term-id="${fixture.workspace.termSlots[options.term ?? 0].id}"]`;
	return matrix(page)
		.locator(
			`${options.header ? 'th' : 'td'}${scope}${options.section ? `[data-section-id="${options.section}"]` : ''}`
		)
		.first();
}
async function menu(page: Page, target: Locator) {
	await target.click({ button: 'right' });
	await expect(page.getByRole('menu')).toBeVisible();
}
async function add(page: Page, target: Locator, kind = 'รายวิชา') {
	await menu(page, target);
	await page.getByRole('menuitem', { name: `เพิ่ม${kind}…`, exact: true }).click();
	await expect(sheet(page, kind)).toBeVisible();
}
const course = (page: Page) => sheet(page).getByRole('checkbox', { name: /^เลือก ค21101 / });
function input(row: CurriculumStructureRequirement) {
	return {
		resourceKind: row.resourceKind,
		catalogVersionId: row.catalogVersionId,
		gradeLevelId: row.gradeLevel.id,
		termSlotId: row.termSlotId,
		requirementKind: row.requirementKind,
		displayOrder: row.displayOrder
	};
}
function body(state: Awaited<ReturnType<typeof installCurriculumCatalog>>, index = 0) {
	return state.writes[index].body as ReplaceCurriculumStructureRequest;
}
async function bounded(page: Page, locator: Locator) {
	const viewport = page.viewportSize()!;
	await expect
		.poll(async () => {
			const bounds = await locator.boundingBox();
			return (
				!!bounds &&
				bounds.x >= -1 &&
				bounds.y >= -1 &&
				bounds.x + bounds.width <= viewport.width + 1 &&
				bounds.y + bounds.height <= viewport.height + 1
			);
		})
		.toBe(true);
}
function preserveOtherContexts(fixture: Fixture) {
	const original = fixture.original;
	fixture.workspace.requirements.push(
		{ ...original, id: 'other-term', termSlotId: fixture.workspace.termSlots[1].id },
		{ ...original, id: 'other-grade', gradeLevel: fixture.workspace.gradeLevels[1] },
		{ ...original, id: 'other-program', studyProgramId: fixture.workspace.programs[1].id }
	);
}
async function closeClean(page: Page, dialog: Locator) {
	await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
}

test('desktop context menu is lazy, keyboard accessible and shared by comparison and document', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const state = await installCurriculumCatalog(page, fixture);
	let optionReads = 0;
	page.on('request', (request) => {
		if (request.url().includes('/management-options')) optionReads++;
	});
	await page.goto(curriculumCatalogPath);
	await expect(requirement(page)).toBeVisible();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(optionReads).toBe(0);
	for (const document of [false, true]) {
		if (document) await page.getByRole('tab', { name: 'เอกสารรายแผน' }).click();
		const target = requirement(page);
		await expect(target.getByRole('button', { name: /เมนู|ดำเนินการ/ })).toHaveCount(0);
		await menu(page, target);
		await expect(page.getByRole('menuitem')).toHaveText([
			'เพิ่มรายวิชา…',
			'เพิ่มกิจกรรม…',
			'แก้ไขเงื่อนไข…',
			'นำรายการนี้ออก…'
		]);
		await expect(target).toHaveAttribute('data-selected', 'true');
		await bounded(page, page.getByRole('menu'));
		await page.keyboard.press('Escape');
		await expect(target).toBeFocused();
		await target.press('Shift+F10');
		await expect(page.getByRole('menu')).toBeVisible();
		await page.keyboard.press('ArrowDown');
		await expect(page.getByRole('menuitem', { name: 'เพิ่มกิจกรรม…', exact: true })).toBeFocused();
		await page.keyboard.press('Escape');
		await expect(target).toBeFocused();
		await target.press('ContextMenu');
		await expect(page.getByRole('menu')).toBeVisible();
		await page.keyboard.press('Escape');
		for (const target of [
			contextTarget(page, fixture, { header: true }),
			contextTarget(page, fixture, { section: 'basic_course' }),
			matrix(page).locator('td[aria-label="ไม่มีรายการ"]').first()
		]) {
			await menu(page, target);
			await expect(page.getByRole('menuitem')).toHaveText(['เพิ่มรายวิชา…', 'เพิ่มกิจกรรม…']);
			await page.keyboard.press('Escape');
		}
	}
	expect(optionReads).toBe(0);
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});

test('adding current registry selections replaces the entire correct program and preserves every other pin and context', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ longCatalog: true });
	preserveOtherContexts(fixture);
	const before = structuredClone(fixture.workspace.requirements);
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await add(page, contextTarget(page, fixture, { program: 1, term: 1, section: 'basic_course' }));
	await expect(sheet(page)).toContainText('แผนวิทย์คณิต');
	await expect(sheet(page)).toContainText('มัธยมศึกษาปีที่ 1');
	await expect(sheet(page)).toContainText('ภาคเรียนที่ 2');
	await expect(course(page)).toHaveCount(1);
	await expect(sheet(page).getByRole('combobox', { name: 'หมวดทะเบียน', exact: true })).toHaveText(
		'รายวิชาพื้นฐาน'
	);
	await expect(sheet(page)).not.toContainText('รายวิชารุ่นที่ถูกแทนที่');
	await course(page).check();
	await sheet(page)
		.getByRole('checkbox', { name: /^เลือก ว20001 / })
		.check();
	await sheet(page)
		.getByRole('button', { name: /เพิ่ม 2 รายการ/ })
		.click();
	await expect(sheet(page)).toBeHidden();
	expect(state.writes).toHaveLength(1);
	expect(state.writes[0].path).toBe(
		`/api/academic/study-programs/${fixture.workspace.programs[1].id}/structure`
	);
	const sent = body(state);
	expect(sent.rowVersion).toBe(1);
	expect(sent.requirements).toHaveLength(3);
	expect(sent.requirements).toContainEqual(
		input(before.find((row) => row.id === 'other-program')!)
	);
	for (const version of [currentSubjectVersionId, 'subject-latest-start'])
		expect(sent.requirements).toContainEqual(
			expect.objectContaining({
				catalogVersionId: version,
				gradeLevelId: ids.grade,
				termSlotId: fixture.workspace.termSlots[1].id,
				requirementKind: 'required'
			})
		);
	expect(
		fixture.workspace.requirements.filter((row) => row.studyProgramId === ids.program)
	).toEqual(before.filter((row) => row.studyProgramId === ids.program));
	await add(page, contextTarget(page, fixture, { section: 'basic_course' }));
	await expect(course(page)).toBeDisabled();
	await closeClean(page, sheet(page));
	expect(state.writes).toHaveLength(1);
	expect(state.errors).toEqual([]);
	expect(state.unexpected).toEqual([]);
});

test('edit and confirmed removal affect only the clicked legacy duplicate; full-program writes preserve unrelated grades and terms', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ legacyDuplicates: true });
	preserveOtherContexts(fixture);
	const initial = structuredClone(fixture.workspace.requirements);
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await menu(page, requirement(page, 'legacy-requirement-v2'));
	await page.getByRole('menuitem', { name: 'แก้ไขเงื่อนไข…', exact: true }).click();
	const edit = page.getByRole('dialog', { name: 'แก้ไขเงื่อนไข', exact: true });
	await expect(edit).toContainText('คณิตศาสตร์พื้นฐาน');
	await expect(edit.getByRole('spinbutton')).toHaveCount(0);
	await expect(edit.getByRole('textbox')).toHaveCount(0);
	await edit.getByRole('combobox', { name: 'เงื่อนไข', exact: true }).click();
	await page.getByRole('option', { name: 'เพิ่มเติม', exact: true }).click();
	await edit.getByRole('button', { name: /บันทึก/ }).click();
	await expect(edit).toBeHidden();
	const expected = initial
		.filter((row) => row.studyProgramId === ids.program)
		.map((row) => ({
			...input(row),
			requirementKind: row.id === 'legacy-requirement-v2' ? 'optional' : row.requirementKind
		}));
	expect(body(state).requirements).toEqual(expected);
	const currentId = fixture.workspace.requirements.find(
		(row) => row.studyProgramId === ids.program && row.catalogVersionId === currentSubjectVersionId
	)!.id;
	await menu(page, requirement(page, currentId));
	await page.getByRole('menuitem', { name: 'นำรายการนี้ออก…', exact: true }).click();
	const remove = page.getByRole('alertdialog', { name: 'นำรายการออก', exact: true });
	await expect(remove).toContainText('แผนการเรียนพื้นฐาน');
	await expect(remove).toContainText('ภาคเรียนที่ 1');
	await remove.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	expect(state.writes).toHaveLength(1);
	await menu(page, requirement(page, currentId));
	await page.getByRole('menuitem', { name: 'นำรายการนี้ออก…', exact: true }).click();
	await remove.getByRole('button', { name: 'นำออกจากร่างนี้', exact: true }).click();
	await expect(remove).toBeHidden();
	expect(body(state, 1).rowVersion).toBe(2);
	expect(body(state, 1).requirements).toEqual(
		expected.filter((row) => row.catalogVersionId !== currentSubjectVersionId)
	);
	expect(fixture.workspace.requirements.find((row) => row.id === 'other-program')).toEqual(
		initial.find((row) => row.id === 'other-program')
	);
	expect(state.errors).toEqual([]);
});

test('totals and rows update from the authoritative response only after a successful save', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	// The registry response determines metrics; the submitted requirement contains no metric fields.
	fixture.original.metrics = { ...fixture.original.metrics, credit: '7.25', totalHours: '145' };
	const state = await installCurriculumCatalog(page, fixture);
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let received = false;
	await page.route('**/api/academic/study-programs/*/structure', async (route) => {
		received = true;
		await gate;
		await route.fallback();
	});
	await page.goto(curriculumCatalogPath);
	await add(page, contextTarget(page, fixture, { header: true }));
	await course(page).check();
	await sheet(page)
		.getByRole('button', { name: /เพิ่ม 1 รายการ/ })
		.click();
	await expect.poll(() => received).toBe(true);
	await expect(sheet(page)).toBeVisible();
	await expect(matrix(page).locator('[data-requirement-id]')).toHaveCount(0);
	await expect(matrix(page)).not.toContainText('7.25');
	release();
	await expect(sheet(page)).toBeHidden();
	await expect(matrix(page).locator('[data-requirement-id]')).toHaveCount(1);
	await expect(matrix(page).locator('tfoot')).toContainText('7.25');
	expect(body(state).requirements[0]).not.toHaveProperty('metrics');
	expect(state.errors).toEqual([]);
});

test('conflict retains selection, dirty closing confirms, and latest context requires explicit discard before reload', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	const state = await installCurriculumCatalog(page, fixture, { failSaveOnce: true });
	await page.goto(curriculumCatalogPath);
	await add(page, contextTarget(page, fixture, { header: true }));
	await course(page).check();
	await sheet(page).getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	const discard = page.getByRole('alertdialog', { name: 'ละทิ้งการเปลี่ยนแปลง?', exact: true });
	await expect(discard).toBeVisible();
	await discard.getByRole('button', { name: 'แก้ไขต่อ', exact: true }).click();
	await expect(course(page)).toBeChecked();
	expect(state.writes).toEqual([]);
	await sheet(page)
		.getByRole('button', { name: /เพิ่ม 1 รายการ/ })
		.click();
	await expect(sheet(page)).toContainText('รุ่นรายวิชาที่เลือกไม่ใช่รุ่นปัจจุบันแล้ว');
	await expect(course(page)).toBeChecked();
	await expect(sheet(page).getByRole('button', { name: /เพิ่ม 1 รายการ/ })).toBeDisabled();
	expect(state.writes).toHaveLength(1);
	await sheet(page).getByRole('button', { name: 'กลับไปดูข้อมูลล่าสุด', exact: true }).click();
	await expect(discard).toBeVisible();
	await discard.getByRole('button', { name: 'แก้ไขต่อ', exact: true }).click();
	await expect(course(page)).toBeChecked();
	expect(state.structureReads()).toBe(1);
	await sheet(page).getByRole('button', { name: 'กลับไปดูข้อมูลล่าสุด', exact: true }).click();
	await discard.getByRole('button', { name: 'ละทิ้งการเปลี่ยนแปลง', exact: true }).click();
	await expect.poll(() => state.structureReads()).toBe(2);
	expect(state.writes).toHaveLength(1);
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
	expect(state.errors).toEqual([]);
});

for (const published of [false, true])
	test(`read-only ${published ? 'publication' : 'permission'} has no management reads or mutation controls`, async ({
		page
	}) => {
		const fixture = curriculumCatalogFixture({ readOnly: published });
		const state = await installCurriculumCatalog(page, fixture);
		if (!published)
			await page.route('**/api/auth/me', (route) =>
				route.fulfill({
					contentType: 'application/json',
					body: JSON.stringify({
						success: true,
						data: {
							id: ids.user,
							username: 'synthetic-reader',
							firstName: 'ทดสอบ',
							lastName: 'อ่าน',
							displayName: 'ทดสอบ อ่าน',
							userType: 'staff',
							status: 'ACTIVE',
							permissions: ['academic_curriculum.read.school']
						}
					})
				})
			);
		let optionReads = 0;
		page.on('request', (request) => {
			if (request.url().includes('/management-options')) optionReads++;
		});
		await page.goto(curriculumCatalogPath);
		await expect(requirement(page)).toBeVisible();
		await requirement(page).click({ button: 'right' });
		await expect(page.getByRole('menu')).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true })).toHaveCount(0);
		await page.getByRole('tab', { name: 'เอกสารรายแผน' }).click();
		await requirement(page).click({ button: 'right' });
		await expect(page.getByRole('menu')).toHaveCount(0);
		expect(optionReads).toBe(0);
		expect(state.writes).toEqual([]);
		expect(state.errors).toEqual([]);
	});

test.describe('touch and panel layouts', () => {
	test.use({ hasTouch: true });
	for (const width of [390, 1543])
		for (const mode of ['light', 'dark'] as const)
			test(`bounded ${width}px ${mode} add panel fixes filters and footer while its catalog scrolls`, async ({
				page
			}, testInfo) => {
				await page.setViewportSize({ width, height: 900 });
				const fixture = curriculumCatalogFixture({ longCatalog: true });
				const state = await installCurriculumCatalog(page, fixture, { mode });
				await page.goto(curriculumCatalogPath);
				const target = requirement(page);
				await target.getByRole('button', { name: /เมนู|ดำเนินการ/ }).click();
				await expect(page.getByRole('menu')).toBeVisible();
				await bounded(page, page.getByRole('menu'));
				await page.getByRole('menuitem', { name: 'เพิ่มรายวิชา…', exact: true }).click();
				const panel = sheet(page);
				await expect(panel).toBeVisible();
				await bounded(page, panel);
				const bounds = (await panel.boundingBox())!;
				expect(bounds.width).toBeLessThanOrEqual(601);
				if (width === 390) expect(bounds.width).toBeGreaterThanOrEqual(388);
				else expect(bounds.width).toBeGreaterThanOrEqual(599);
				await expect(course(page)).toBeDisabled();
				const search = panel.getByRole('textbox', { name: /ค้นหา/ });
				const footer = panel.getByRole('button', { name: /เพิ่ม.*รายการ/ });
				const before = { search: await search.boundingBox(), footer: await footer.boundingBox() };
				const last = panel.getByRole('checkbox', { name: /^เลือก ท17 / });
				await last.scrollIntoViewIfNeeded();
				await expect(last).toBeVisible();
				await expect(search).toBeInViewport();
				await expect(footer).toBeInViewport();
				expect(Math.abs((await search.boundingBox())!.y - before.search!.y)).toBeLessThan(2);
				expect(Math.abs((await footer.boundingBox())!.y - before.footer!.y)).toBeLessThan(2);
				await page.screenshot({
					path: testInfo.outputPath(`quick-actions-${width}-${mode}.png`),
					fullPage: true
				});
				await closeClean(page, panel);
				await add(
					page,
					contextTarget(page, fixture, { section: 'student_development' }),
					'กิจกรรม'
				);
				await expect(
					sheet(page, 'กิจกรรม').getByRole('checkbox', { name: /^เลือก กิจกรรมชุมนุม / })
				).toHaveCount(2);
				await expect(sheet(page, 'กิจกรรม')).not.toContainText('ค21101');
				await closeClean(page, sheet(page, 'กิจกรรม'));
				await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
				await expect(
					page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร', exact: true })
				).toBeVisible();
				expect(state.writes).toEqual([]);
				expect(state.errors).toEqual([]);
			});
});

test('legacy pinned revisions with identical labels and order are edited and removed by clicked identity', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	fixture.workspace.requirements.push({
		...fixture.original,
		id: 'same-presentation-second-row',
		catalogVersionId: currentSubjectVersionId,
		versionNo: 2,
		hasNewerCurrentVersion: false
	});
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await menu(page, requirement(page, 'same-presentation-second-row'));
	await page.getByRole('menuitem', { name: 'แก้ไขเงื่อนไข…', exact: true }).click();
	const edit = page.getByRole('dialog', { name: 'แก้ไขเงื่อนไข', exact: true });
	await edit.getByRole('combobox', { name: 'เงื่อนไข', exact: true }).click();
	await page.getByRole('option', { name: 'เพิ่มเติม', exact: true }).click();
	await edit.getByRole('button', { name: /บันทึก/ }).click();
	await expect(edit).toBeHidden();
	expect(body(state).requirements).toEqual([
		input(fixture.original),
		{
			...input(fixture.original),
			catalogVersionId: currentSubjectVersionId,
			requirementKind: 'optional'
		}
	]);
	const second = fixture.workspace.requirements.find(
		(row) => row.catalogVersionId === currentSubjectVersionId
	)!;
	await menu(page, requirement(page, second.id));
	await page.getByRole('menuitem', { name: 'นำรายการนี้ออก…', exact: true }).click();
	await page
		.getByRole('alertdialog', { name: 'นำรายการออก', exact: true })
		.getByRole('button', { name: 'นำออกจากร่างนี้', exact: true })
		.click();
	await expect.poll(() => state.writes.length).toBe(2);
	expect(body(state, 1).requirements).toEqual([input(fixture.original)]);
	expect(state.errors).toEqual([]);
});

test('ordinary save errors keep selections and allow an explicit retry', async ({ page }) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	const state = await installCurriculumCatalog(page, fixture);
	let attempts = 0;
	await page.route('**/api/academic/study-programs/*/structure', async (route) => {
		if (++attempts === 1)
			return route.fulfill({
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({ success: false, error: 'บันทึกไม่ได้ชั่วคราว' })
			});
		await route.fallback();
	});
	await page.goto(curriculumCatalogPath);
	await add(page, contextTarget(page, fixture, { header: true }));
	await course(page).check();
	await sheet(page)
		.getByRole('button', { name: /เพิ่ม 1 รายการ/ })
		.click();
	await expect(sheet(page)).toContainText('บันทึกไม่ได้ชั่วคราว');
	await expect(course(page)).toBeChecked();
	await expect(sheet(page).getByRole('button', { name: /เพิ่ม 1 รายการ/ })).toBeEnabled();
	expect(attempts).toBe(1);
	await sheet(page)
		.getByRole('button', { name: /เพิ่ม 1 รายการ/ })
		.click();
	await expect(sheet(page)).toBeHidden();
	expect(attempts).toBe(2);
	expect(body(state).requirements).toHaveLength(1);
	expect(state.errors).toEqual([]);
});

test('late registry options cannot reopen a cancelled panel or restore a selection in a different context', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	const state = await installCurriculumCatalog(page, fixture);
	let release!: () => void;
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let requested = false;
	await page.route('**/api/academic/curriculum-levels/*/management-options', async (route) => {
		requested = true;
		await gate;
		await route.fallback();
	});
	await page.goto(curriculumCatalogPath);
	await add(page, contextTarget(page, fixture, { header: true }));
	await expect.poll(() => requested).toBe(true);
	await closeClean(page, sheet(page));
	await page.getByRole('tab', { name: 'เอกสารรายแผน' }).click();
	release();
	await expect(sheet(page)).toHaveCount(0);
	await add(page, contextTarget(page, fixture, { term: 1, header: true }));
	await expect(course(page)).toBeEnabled();
	await expect(course(page)).not.toBeChecked();
	await expect(sheet(page)).toContainText('ภาคเรียนที่ 2');
	await closeClean(page, sheet(page));
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});
