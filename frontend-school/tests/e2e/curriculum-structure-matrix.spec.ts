import { expect, test, type Page, type Locator } from '@playwright/test';
import type { CurriculumStructureRequirement } from '../../src/lib/api/academic-core.js';
import { edition } from './helpers/curriculum-fixtures';
import {
	curriculumCatalogFixture,
	curriculumCatalogPath,
	installCurriculumCatalog,
	openCurriculumEditor
} from './fixtures/curriculum-catalog';

test.use({ serviceWorkers: 'block', timezoneId: 'Asia/Bangkok' });
function matrixFixture() {
	const fixture = curriculumCatalogFixture();
	const workspace = fixture.workspace;
	workspace.termSlots.push({
		...workspace.termSlots[0],
		id: 'summer',
		name: 'ภาคฤดูร้อน',
		sequence: 3,
		termType: 'summer',
		typeOccurrence: 1
	});
	const first = {
		...fixture.original,
		id: 'science-1',
		name: 'วิทยาศาสตร์ 1',
		code: 'ว21101',
		subjectGroupId: 'science',
		subjectGroupOrder: 2
	};
	workspace.requirements = [
		first,
		{
			...first,
			id: 'science-2',
			catalogId: 'science-2',
			catalogVersionId: 'science-version-2',
			name: 'วิทยาศาสตร์ ๒',
			code: 'ว21102',
			termSlotId: workspace.termSlots[1].id,
			hasNewerCurrentVersion: false
		},
		{ ...first, id: 'grade-2', gradeLevel: workspace.gradeLevels[1] },
		{
			...first,
			id: 'activity',
			catalogId: 'activity',
			catalogVersionId: 'activity',
			name: 'กิจกรรมชุมนุม',
			code: 'HIDDEN-ACTIVITY-CODE',
			resourceKind: 'activity',
			creditCategory: null,
			hasNewerCurrentVersion: false,
			section: 'student_development',
			subjectGroupId: null,
			subjectGroupOrder: null,
			metrics: { credit: null, weeklyUnit: 'hour', weeklyValue: '1.50', totalHours: '30' }
		},
		...Array.from({ length: 24 }, (_, index): CurriculumStructureRequirement => ({
			...first,
			id: `long-${index}`,
			catalogId: `long-${index}`,
			catalogVersionId: `long-${index}`,
			name: `รายวิชาทดสอบ ${String(index).padStart(2, '0')} เนื้อหาสำหรับอ่านและตรวจการตัดบรรทัด`,
			code: `ท${index}`,
			hasNewerCurrentVersion: false,
			metrics: { ...first.metrics, credit: index === 0 ? null : '0.10' },
			subjectGroupId: index < 12 ? 'thai' : 'other',
			subjectGroupOrder: index < 12 ? 1 : 3
		}))
	];
	return fixture;
}
async function choose(page: Page, label: string, option: string) {
	await page.getByRole('button', { name: label, exact: true }).click();
	await page.getByRole('option', { name: option, exact: true }).click();
}

async function showSciencePair(viewport: Locator) {
	await viewport.evaluate((node) => {
		const target = node.querySelector('[data-requirement-id="science-1"]')!.getBoundingClientRect();
		const header = node.querySelector('thead')!.getBoundingClientRect();
		const section = node.querySelector('tbody td')!.getBoundingClientRect();
		node.scrollTop +=
			target.top - node.getBoundingClientRect().top - header.height - section.height - 8;
		node.scrollLeft = 0;
	});
}

for (const width of [390, 1543])
	for (const mode of ['light', 'dark'] as const) {
		test(`aligned matrix and bounded sticky scrolling at ${width}px ${mode}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 950 });
			const fixture = matrixFixture();
			const state = await installCurriculumCatalog(page, fixture, { mode });
			await page.goto(curriculumCatalogPath);
			const matrix = page.getByTestId('curriculum-matrix');
			const table = matrix.locator('.matrix-table').first();
			await expect(table.locator('[data-requirement-id="science-1"]')).toBeVisible();
			const scienceRow = table
				.getByRole('row')
				.filter({ has: page.locator('[data-requirement-id="science-1"]') });
			await expect(scienceRow).toContainText('วิทยาศาสตร์ ๒');
			await expect(scienceRow.getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(1);
			await expect(table.locator('thead')).toContainText('ภาคฤดูร้อน');
			await expect(table).not.toContainText('คาบ');
			await expect(
				table.getByRole('columnheader', { name: 'รายวิชา / กิจกรรม', exact: true })
			).toHaveCount(0);
			await expect(matrix).not.toContainText('HIDDEN-ACTIVITY-CODE');
			await expect(matrix).not.toContainText(/\bv[12]\b|ข้อมูลต่างกันระหว่างแผน|แผนเริ่มต้น/);
			await expect(matrix.locator('abbr').first()).toBeVisible();
			const columnAlignment = await table.evaluate((node) => {
				const headers = [...node.querySelectorAll('thead tr:last-child th')].map((cell) =>
					cell.getBoundingClientRect()
				);
				return [...node.querySelectorAll('tbody tr:first-child, tfoot tr')].every(
					(row) =>
						[...row.children].length === headers.length &&
						[...row.children].every((cell, index) => {
							const bounds = cell.getBoundingClientRect();
							return (
								cell.getAttribute('colspan') === null &&
								Math.abs(
									(bounds.left + bounds.right) / 2 -
										(headers[index].left + headers[index].right) / 2
								) < 2
							);
						})
				);
			});
			expect(columnAlignment).toBe(true);
			const viewport = matrix.locator('[data-slot="table-container"]').first();
			await expect
				.poll(() => viewport.evaluate((node) => node.scrollWidth > node.clientWidth))
				.toBe(true);
			await viewport.evaluate((node) => {
				node.scrollTop = 420;
				node.scrollLeft = 120;
			});
			await expect.poll(() => viewport.evaluate((node) => node.scrollTop)).toBeGreaterThan(300);
			const geometry = await viewport.evaluate((node) => {
				const header = node.querySelector('thead')!.getBoundingClientRect();
				const section = node.querySelector('tbody td')!.getBoundingClientRect();
				return {
					headerBottom: header.bottom,
					sectionTop: section.top,
					headerTop: header.top,
					viewportTop: node.getBoundingClientRect().top
				};
			});
			expect(Math.abs(geometry.headerTop - geometry.viewportTop)).toBeLessThan(3);
			expect(Math.abs(geometry.sectionTop - geometry.headerBottom)).toBeLessThan(3);
			await viewport.evaluate((node) => {
				node.scrollTop = 0;
				node.scrollLeft = 0;
			});
			await matrix.locator(':scope > section').first().scrollIntoViewIfNeeded();
			await page.screenshot({
				path: testInfo.outputPath(`comparison-${width}-${mode}.png`),
				fullPage: true
			});
			await showSciencePair(viewport);
			await page.screenshot({
				path: testInfo.outputPath(`comparison-scrolled-${width}-${mode}.png`),
				fullPage: true
			});
			await page.getByRole('tab', { name: 'เอกสารรายแผน' }).click();
			await expect(
				matrix.getByRole('columnheader', { name: 'รายวิชา / กิจกรรม', exact: true })
			).toHaveCount(0);
			await expect(matrix.locator('[data-requirement-id="science-1"]')).toBeVisible();
			await matrix.locator(':scope > section').first().scrollIntoViewIfNeeded();
			await page.screenshot({
				path: testInfo.outputPath(`document-${width}-${mode}.png`),
				fullPage: true
			});
			await showSciencePair(viewport);
			await page.screenshot({
				path: testInfo.outputPath(`document-scrolled-${width}-${mode}.png`),
				fullPage: true
			});
			await choose(page, 'ระดับชั้นที่แสดง', 'ทุกระดับชั้น');
			await expect(matrix.locator('.matrix-table')).toHaveCount(3);
			await expect(matrix).toContainText('รวมทั้งระดับ · ทุกชั้น ทุกภาคเรียน');
			await page.getByRole('checkbox', { name: 'ภาคเรียนที่ 2', exact: true }).uncheck();
			await expect(matrix.locator('.matrix-table').first().locator('thead')).not.toContainText(
				'ภาคเรียนที่ 2'
			);
			await expect(matrix.locator('.matrix-table').first().locator('thead')).toContainText(
				'ภาคฤดูร้อน'
			);
			await page.getByRole('checkbox', { name: 'ทุกภาคเรียน', exact: true }).check();
			await expect(matrix.locator('.matrix-table').first().locator('thead')).toContainText(
				'ภาคเรียนที่ 2'
			);
			await page.getByRole('checkbox', { name: 'ภาคเรียนที่ 2', exact: true }).uncheck();
			await page.getByRole('checkbox', { name: 'ภาคฤดูร้อน', exact: true }).uncheck();
			await page.getByRole('checkbox', { name: 'ภาคเรียนที่ 1', exact: true }).uncheck();
			await expect(matrix.locator('.matrix-table')).toHaveCount(0);
			await expect(matrix.getByText('เลือกภาคเรียนเพื่อแสดงตาราง')).toHaveCount(3);
			await page.getByRole('checkbox', { name: 'ทุกภาคเรียน', exact: true }).focus();
			await page.keyboard.press('Space');
			await expect(page.getByRole('checkbox', { name: 'ทุกภาคเรียน', exact: true })).toBeChecked();
			await expect(matrix.locator('.matrix-table')).toHaveCount(3);
			await expect(matrix.locator('.matrix-table').first().locator('thead')).toContainText(
				'ภาคฤดูร้อน'
			);
			expect(state.writes).toEqual([]);
			expect(state.errors).toEqual([]);
		});
	}

test('editor starts from displayed concrete grade, program and single term; grade and category filters prevent hidden selections', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	fixture.catalog.push({
		...fixture.current,
		id: 'grade-one-only',
		catalogId: 'grade-one-only',
		code: 'ทเฉพาะ1',
		name: 'เฉพาะชั้นหนึ่ง',
		gradeLevelIds: [fixture.workspace.gradeLevels[0].id]
	});
	fixture.catalog.push({
		...fixture.current,
		id: 'additional-grade-two',
		catalogId: 'additional-grade-two',
		code: 'ทเพิ่มเติม',
		name: 'วิชาเพิ่มเติมชั้นสอง',
		section: 'additional_course',
		creditCategory: 'additional',
		gradeLevelIds: [fixture.workspace.gradeLevels[1].id]
	});
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await choose(page, 'ระดับชั้นที่แสดง', 'มัธยมศึกษาปีที่ 2');
	await page.getByRole('tab', { name: 'เอกสารรายแผน' }).click();
	await choose(page, 'แผนการเรียนที่แสดง', 'แผนวิทย์คณิต');
	await page.getByRole('checkbox', { name: 'ภาคเรียนที่ 1', exact: true }).uncheck();
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	await expect(editor.getByRole('button', { name: 'ระดับชั้นที่จะจัดโครงสร้าง' })).toHaveText(
		'มัธยมศึกษาปีที่ 2'
	);
	await expect(editor.getByRole('button', { name: 'แผนการเรียนที่จะจัดโครงสร้าง' })).toHaveText(
		'แผนวิทย์คณิต'
	);
	await expect(editor.getByRole('button', { name: 'ภาคเรียนที่จะจัดโครงสร้าง' })).toHaveText(
		'ภาคเรียนที่ 2'
	);
	await expect(editor.getByLabel('ชื่อแผนการเรียนใหม่')).toBeHidden();
	await expect(editor.getByLabel('ลำดับแสดง').first()).toBeHidden();
	const picker = editor.getByRole('region', { name: 'เลือกจากทะเบียน' });
	await expect(picker).not.toContainText('เฉพาะชั้นหนึ่ง');
	await choose(page, 'หมวดทะเบียน', 'รายวิชาเพิ่มเติม');
	await expect(picker.getByRole('checkbox')).toHaveCount(1);
	await picker.getByRole('checkbox').check();
	await choose(page, 'ระดับชั้นที่จะจัดโครงสร้าง', 'มัธยมศึกษาปีที่ 1');
	await expect(picker.getByRole('checkbox')).toHaveCount(0);
	await expect(picker.getByRole('button', { name: /เพิ่ม.*รายการ/ })).toBeDisabled();
	await choose(page, 'หมวดทะเบียน', 'กิจกรรมพัฒนาผู้เรียน');
	await expect(picker.getByRole('checkbox')).toHaveCount(2);
	await expect(picker).not.toContainText('A01');
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});

test('term display order editing and removal preserve name, type and matching occurrence independently', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByLabel('ลำดับแสดง', { exact: true }).first().fill('7');
	await expect(editor.getByLabel('ชื่อภาคเรียน', { exact: true }).first()).toHaveValue(
		'ภาคเรียนที่ 1'
	);
	await expect(editor.getByLabel('ครั้งที่ภายในประเภท', { exact: true }).first()).toHaveValue('1');
	await editor.getByRole('button', { name: 'ลบภาคเรียน', exact: true }).last().click();
	await expect(editor.getByLabel('ลำดับแสดง', { exact: true }).first()).toHaveValue('7');
	await editor.getByRole('button', { name: 'บันทึกภาคเรียน', exact: true }).click();
	await expect(editor).toBeHidden();
	expect(state.writes).toHaveLength(1);
	expect(state.writes[0].body).toMatchObject({
		slots: [
			{
				id: fixture.workspace.termSlots[0].id,
				sequence: 7,
				name: 'ภาคเรียนที่ 1',
				termType: 'regular',
				typeOccurrence: 1
			}
		]
	});
	expect(state.errors).toEqual([]);
});

test('saving term settings retains an explicit subset filter when a new slot is added', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await page.getByRole('checkbox', { name: 'ภาคเรียนที่ 2', exact: true }).uncheck();
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByRole('button', { name: 'ภาคฤดูร้อน', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกภาคเรียน', exact: true }).click();
	await expect(editor).toBeHidden();
	await expect(page.getByRole('checkbox', { name: 'ภาคเรียนที่ 1', exact: true })).toBeChecked();
	await expect(
		page.getByRole('checkbox', { name: 'ภาคเรียนที่ 2', exact: true })
	).not.toBeChecked();
	await expect(page.getByRole('checkbox', { name: 'ภาคฤดูร้อน 1', exact: true })).not.toBeChecked();
	expect(state.writes).toHaveLength(1);
	expect(state.errors).toEqual([]);
});

test('all-terms control is disabled when the level has no configured term slots', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ empty: true });
	fixture.workspace.termSlots = [];
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	await expect(page.getByRole('checkbox', { name: 'ทุกภาคเรียน', exact: true })).toBeDisabled();
	await expect(page.getByTestId('curriculum-matrix').locator('.matrix-table')).toHaveCount(0);
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});

test('numeric section, term and annual summaries align school hours while annual weekly loads remain unaggregated', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const course = {
		...fixture.original,
		metrics: { ...fixture.original.metrics, weeklyValue: '32', totalHours: '64' }
	};
	fixture.workspace.requirements = [
		course,
		{
			...course,
			id: 'activity-hours',
			catalogId: 'activity-hours',
			catalogVersionId: 'activity-hours',
			name: 'กิจกรรม',
			code: 'HIDDEN-ACTIVITY',
			resourceKind: 'activity',
			creditCategory: null,
			hasNewerCurrentVersion: false,
			section: 'student_development',
			subjectGroupId: null,
			subjectGroupOrder: null,
			metrics: { credit: null, weeklyUnit: 'hour', weeklyValue: '3', totalHours: '6' }
		}
	];
	const state = await installCurriculumCatalog(page, fixture);
	await page.goto(curriculumCatalogPath);
	const table = page.getByTestId('curriculum-matrix').locator('.matrix-table').first();
	await expect(table.locator('tfoot tr td').nth(1)).toHaveText('0.50');
	await expect(table.locator('tfoot tr td').nth(2)).toHaveText('35.00');
	await expect(table.locator('tfoot tr td').nth(3)).toHaveText('70.00');
	const activities = table.locator('tbody').nth(2).getByRole('row').first().getByRole('cell');
	await expect(activities.nth(1)).toHaveText('—');
	await expect(activities.nth(2)).toHaveText('3.00');
	const annual = page.getByRole('table', {
		name: 'สรุปทั้งปี มัธยมศึกษาปีที่ 1 · ทุกภาคเรียน',
		exact: true
	});
	await expect(annual.locator('tbody td').nth(4)).toHaveText('—');
	await expect(annual.locator('tbody td').nth(5)).toHaveText('70.00');
	const range = page.getByRole('table', { name: 'สรุปช่วงที่เลือกและทั้งระดับ', exact: true });
	await expect(range.locator('tbody tr').first().locator('td').nth(4)).toHaveText('—');
	await expect(table).not.toContainText('คาบ');
	await expect(table).not.toContainText('ชม.');
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});

test('settings and atomic program order save preserve selections, requirement kinds and unrelated name/term drafts', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ legacyDuplicates: true });
	const initialLevelVersion = fixture.workspace.level.rowVersion;
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	const staged = editor.getByRole('region', { name: 'รายการในหลักสูตร' });
	const oldRow = staged
		.getByRole('row')
		.filter({ has: page.getByRole('button', { name: 'เก่า', exact: true }) });
	await oldRow.getByRole('button', { name: 'บังคับ', exact: true }).click();
	await page.getByRole('option', { name: 'เลือก', exact: true }).click();
	const pendingActivity = editor.getByRole('checkbox', {
		name: /เลือก กิจกรรมชุมนุม · ตั้งแต่ 1 พ.ค. 2570/
	});
	await pendingActivity.check();
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByLabel('ชื่อแผนการเรียนใหม่', { exact: true }).fill('ชื่อแผนฉบับร่าง');
	await editor.getByLabel('ชื่อภาคเรียน', { exact: true }).first().fill('ชื่อภาคฉบับร่าง');
	await editor.getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor).toBeVisible();
	await expect(editor.getByLabel('ชื่อแผนการเรียนใหม่', { exact: true })).toHaveValue(
		'ชื่อแผนฉบับร่าง'
	);
	await expect(editor.getByLabel('ชื่อภาคเรียน', { exact: true }).first()).toHaveValue(
		'ชื่อภาคฉบับร่าง'
	);
	expect(state.writes).toHaveLength(1);
	expect(state.writes[0].path).toContain('/program-order');
	expect(state.writes[0].body).toMatchObject({
		studyProgramIds: [fixture.workspace.programs[0].id, fixture.workspace.programs[1].id],
		rowVersion: initialLevelVersion
	});
	expect(state.structureReads()).toBe(1);
	await editor.getByRole('button', { name: 'กลับไปจัดรายวิชา', exact: true }).click();
	await expect(editor.getByRole('button', { name: 'แผนการเรียนที่จะจัดโครงสร้าง' })).toHaveText(
		'แผนการเรียนพื้นฐาน'
	);
	await expect(pendingActivity).toBeChecked();
	await expect(oldRow.getByRole('button', { name: 'เลือก', exact: true })).toBeVisible();
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByLabel('ชื่อภาคเรียน', { exact: true }).first().fill('ภาคเรียนที่ 1');
	await editor.getByRole('button', { name: 'กลับไปจัดรายวิชา', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
	await expect(editor).toBeHidden();
	expect(state.writes).toHaveLength(2);
	expect(state.writes[1].body).toMatchObject({
		rowVersion: 2,
		requirements: [
			{ catalogVersionId: fixture.original.catalogVersionId, requirementKind: 'elective' },
			{ catalogVersionId: fixture.current.id, requirementKind: 'required' }
		]
	});
	await expect(
		page
			.getByTestId('curriculum-matrix')
			.locator('.matrix-table')
			.first()
			.locator('thead tr')
			.first()
			.getByRole('columnheader')
			.first()
	).toHaveText('แผนวิทย์คณิต');
	await page.reload();
	await expect(
		page
			.getByTestId('curriculum-matrix')
			.locator('.matrix-table')
			.first()
			.locator('thead tr')
			.first()
			.getByRole('columnheader')
			.first()
	).toHaveText('แผนวิทย์คณิต');
	await page.getByRole('tab', { name: 'เอกสารรายแผน', exact: true }).click();
	await page.getByRole('button', { name: 'แผนการเรียนที่แสดง', exact: true }).click();
	await expect(page.getByRole('option')).toHaveText(['แผนวิทย์คณิต', 'แผนการเรียนพื้นฐาน']);
	await page.keyboard.press('Escape');
	expect(state.errors).toEqual([]);
});

test('order conflict retains drafts and requires an explicit latest-order review before retry', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const state = await installCurriculumCatalog(page, fixture, { failOrderOnce: true });
	await openCurriculumEditor(page);
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByLabel('ชื่อแผนการเรียนใหม่', { exact: true }).fill('ชื่อที่ยังไม่บันทึก');
	await editor.getByLabel('ชื่อภาคเรียน', { exact: true }).first().fill('ภาคที่ยังไม่บันทึก');
	await editor.getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	const desired = await editor
		.locator('[data-program-order-id]')
		.evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-program-order-id')));
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor.getByRole('alert')).toContainText('เปลี่ยนไปแล้ว');
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeDisabled();
	expect(state.writes).toHaveLength(1);
	expect(
		await editor
			.locator('[data-program-order-id]')
			.evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-program-order-id')))
	).toEqual(desired);
	await editor.getByRole('button', { name: 'กลับไปจัดรายวิชา', exact: true }).click();
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await expect(editor.getByLabel('ชื่อแผนการเรียนใหม่', { exact: true })).toHaveValue(
		'ชื่อที่ยังไม่บันทึก'
	);
	await expect(editor.getByLabel('ชื่อภาคเรียน', { exact: true }).first()).toHaveValue(
		'ภาคที่ยังไม่บันทึก'
	);
	await editor.getByRole('button', { name: 'โหลดลำดับล่าสุดเพื่อตรวจสอบ', exact: true }).click();
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeEnabled();
	expect(state.writes).toHaveLength(1);
	expect(state.structureReads()).toBe(2);
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor).toBeVisible();
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeDisabled();
	expect(state.writes).toHaveLength(2);
	expect(state.errors).toEqual([]);
});

test('membership changes after an order conflict do not silently replace the working order', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const state = await installCurriculumCatalog(page, fixture, { failOrderOnce: true });
	await openCurriculumEditor(page);
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor.getByRole('alert')).toBeVisible();
	fixture.workspace.programs.push({
		...fixture.workspace.programs[0],
		id: 'new-member',
		code: 'NEW',
		nameTh: 'แผนใหม่จากผู้ใช้อื่น',
		displayOrder: 3
	});
	await editor.getByRole('button', { name: 'โหลดลำดับล่าสุดเพื่อตรวจสอบ', exact: true }).click();
	await expect(editor.locator('[data-program-order-id]')).toHaveCount(2);
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeDisabled();
	await editor.getByRole('button', { name: 'ใช้ลำดับปัจจุบัน', exact: true }).click();
	await expect(editor.locator('[data-program-order-id]')).toHaveCount(3);
	await editor.getByRole('button', { name: 'เลื่อนแผน 3 ขึ้น', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeDisabled();
	expect(state.writes).toHaveLength(2);
	expect(state.writes[1].body).toMatchObject({
		studyProgramIds: fixture.workspace.programs.map((program) => program.id)
	});
	expect(state.errors).toEqual([]);
});

test('successful ordering never rebases a staged structure over an unseen concurrent program edit', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const originalProgramId = fixture.workspace.programs[0].id;
	const state = await installCurriculumCatalog(page, fixture, { concurrentStructureOnOrder: true });
	await openCurriculumEditor(page);
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	const staged = editor.getByRole('region', { name: 'รายการในหลักสูตร' });
	const course = staged.getByRole('row').filter({ hasText: 'ค21101' });
	await course.getByRole('button', { name: 'บังคับ', exact: true }).click();
	await page.getByRole('option', { name: 'เลือก', exact: true }).click();
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeDisabled();
	await editor.getByRole('button', { name: 'กลับไปจัดรายวิชา', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
	await expect(editor.getByRole('alert')).toContainText('โครงสร้างแผนเปลี่ยนไปแล้ว');
	await expect(course.getByRole('button', { name: 'เลือก', exact: true })).toBeVisible();
	expect(state.writes).toHaveLength(2);
	expect(state.writes[1].body).toMatchObject({ rowVersion: 1 });
	expect(
		fixture.workspace.programs.find((program) => program.id === originalProgramId)?.rowVersion
	).toBe(3);
	expect(fixture.workspace.requirements[0].requirementKind).toBe('optional');
	expect(state.errors).toEqual([]);
});

test('explicit order review and later order save never rebase stale staged term settings', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const initialSlotVersion = fixture.workspace.level.rowVersion;
	const state = await installCurriculumCatalog(page, fixture, {
		failOrderOnce: true,
		concurrentSlotsOnOrderConflict: true
	});
	await openCurriculumEditor(page);
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByLabel('ชื่อภาคเรียน', { exact: true }).first().fill('ชื่อภาคฉบับร่าง');
	await editor.getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor.getByRole('alert')).toBeVisible();
	await editor.getByRole('button', { name: 'โหลดลำดับล่าสุดเพื่อตรวจสอบ', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeDisabled();
	await editor.getByRole('button', { name: 'บันทึกภาคเรียน', exact: true }).click();
	await expect(editor.getByRole('alert')).toContainText('ภาคเรียนเปลี่ยนไปแล้ว');
	await expect(editor.getByLabel('ชื่อภาคเรียน', { exact: true }).first()).toHaveValue(
		'ชื่อภาคฉบับร่าง'
	);
	expect(state.writes).toHaveLength(3);
	expect(state.writes[2].body).toMatchObject({ rowVersion: initialSlotVersion });
	expect(fixture.workspace.level.rowVersion).toBeGreaterThan(initialSlotVersion);
	expect(fixture.workspace.termSlots[0].name).toBe('ภาคจากผู้ใช้อื่น');
	expect(state.errors).toEqual([]);
});

test('staged sections omit row ordering controls and saving preserves duplicate rows and persisted order', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture({ legacyDuplicates: true });
	const original = fixture.original;
	fixture.workspace.requirements[0].displayOrder = 7;
	fixture.workspace.requirements[1].displayOrder = 19;
	fixture.workspace.requirements.push({
		...original,
		id: 'additional-row',
		catalogId: 'additional',
		catalogVersionId: 'additional',
		code: 'วเพิ่มเติม',
		name: 'รายวิชาเพิ่ม',
		section: 'additional_course',
		creditCategory: 'additional',
		displayOrder: 2
	});
	fixture.workspace.requirements.push({
		...original,
		id: 'activity-row',
		catalogId: 'activity-row',
		catalogVersionId: 'activity-row',
		code: 'HIDDEN-ACTIVITY',
		name: 'กิจกรรม',
		resourceKind: 'activity',
		creditCategory: null,
		hasNewerCurrentVersion: false,
		section: 'student_development',
		subjectGroupId: null,
		subjectGroupOrder: null,
		displayOrder: 3,
		metrics: { credit: null, weeklyUnit: 'hour', weeklyValue: '1.50', totalHours: '30' }
	});
	const state = await installCurriculumCatalog(page, fixture);
	await openCurriculumEditor(page);
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	const staged = editor.getByRole('region', { name: 'รายการในหลักสูตร' });
	await expect(staged.getByRole('columnheader')).toHaveCount(4);
	await expect(staged.getByRole('columnheader', { name: 'ลำดับ', exact: true })).toHaveCount(0);
	await expect(staged.getByRole('button', { name: /^เลื่อนรายการ(ขึ้น|ลง)$/ })).toHaveCount(0);
	for (const label of ['รายวิชาพื้นฐาน', 'รายวิชาเพิ่มเติม', 'กิจกรรมพัฒนาผู้เรียน']) {
		const heading = staged.getByRole('cell', { name: label, exact: true });
		await expect(heading).toBeVisible();
		await expect(heading).toHaveAttribute('colspan', '4');
	}
	const duplicates = staged.getByRole('row').filter({ hasText: 'ค21101' });
	await expect(duplicates).toHaveCount(2);
	await expect(duplicates.first().getByRole('button', { name: 'เก่า', exact: true })).toBeVisible();
	await expect(staged).not.toContainText('HIDDEN-ACTIVITY');
	await duplicates.first().getByRole('button', { name: 'นำรายการออก', exact: true }).click();
	await expect(duplicates).toHaveCount(1);
	await editor.getByRole('button', { name: 'ย้อนกลับ', exact: true }).click();
	await expect(duplicates).toHaveCount(2);
	await expect(duplicates.first().getByRole('button', { name: 'เก่า', exact: true })).toBeVisible();
	await editor.getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
	await expect(editor).toBeHidden();
	expect(state.writes[0].body).toMatchObject({
		requirements: [
			{ catalogVersionId: original.catalogVersionId, displayOrder: 7 },
			{ catalogVersionId: fixture.current.id, displayOrder: 19 },
			{ catalogVersionId: 'additional', displayOrder: 2 },
			{ catalogVersionId: 'activity-row', displayOrder: 3 }
		]
	});
	expect(fixture.workspace.requirements).toHaveLength(4);
	expect(state.errors).toEqual([]);
});

for (const width of [390, 1543])
	for (const mode of ['light', 'dark'] as const) {
		test(`editor default and settings remain compact at ${width}px ${mode}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 1000 });
			const fixture = curriculumCatalogFixture({ legacyDuplicates: true });
			fixture.workspace.requirements.push({
				...fixture.original,
				id: 'extra-course',
				catalogId: 'extra-course',
				catalogVersionId: 'extra-course',
				name: 'รายวิชาเพิ่มเติมสำหรับตรวจการจัดกลุ่มในตัวแก้ไข',
				section: 'additional_course',
				creditCategory: 'additional',
				displayOrder: 3
			});
			fixture.workspace.requirements.push({
				...fixture.original,
				id: 'extra-activity',
				catalogId: 'extra-activity',
				catalogVersionId: 'extra-activity',
				code: 'HIDDEN-ACTIVITY',
				name: 'กิจกรรมพัฒนาผู้เรียน',
				resourceKind: 'activity',
				creditCategory: null,
				hasNewerCurrentVersion: false,
				section: 'student_development',
				subjectGroupId: null,
				subjectGroupOrder: null,
				displayOrder: 4,
				metrics: { credit: null, weeklyUnit: 'hour', weeklyValue: '1.50', totalHours: '30' }
			});
			const state = await installCurriculumCatalog(page, fixture, { mode });
			await openCurriculumEditor(page);
			const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
			await expect
				.poll(() => editor.evaluate((node) => Math.round(node.getBoundingClientRect().right)))
				.toBe(width);
			await expect
				.poll(() => editor.evaluate((node) => node.getBoundingClientRect().left))
				.toBeGreaterThanOrEqual(0);
			await expect(editor.getByRole('region', { name: 'รายการในหลักสูตร' })).toBeVisible();
			await expect(
				editor.getByRole('region', { name: 'เพิ่มแผนการเรียน', exact: true })
			).toHaveCount(0);
			const headerGeometry = await editor.evaluate((node) => {
				const title = node.querySelector('[data-slot="sheet-title"]')!.getBoundingClientRect();
				const settings = [...node.querySelectorAll('button')]
					.find((button) => button.textContent?.trim() === 'ตั้งค่า')!
					.getBoundingClientRect();
				return Math.abs((title.top + title.bottom) / 2 - (settings.top + settings.bottom) / 2);
			});
			expect(headerGeometry).toBeLessThan(3);
			const activityRow = editor
				.getByRole('region', { name: 'รายการในหลักสูตร' })
				.getByRole('row')
				.filter({ hasText: 'กิจกรรมพัฒนาผู้เรียน' })
				.filter({ has: page.getByRole('button', { name: 'นำรายการออก', exact: true }) });
			await expect(activityRow).toHaveCount(1);
			await expect(activityRow).not.toContainText('หน่วยกิต');
			await expect(activityRow).toContainText('30 ชม.');
			await expect(activityRow.getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(0);
			await page.screenshot({
				animations: 'disabled',
				path: testInfo.outputPath(`editor-default-${width}-${mode}.png`),
				fullPage: true
			});
			await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
			await expect(
				editor.getByRole('region', { name: 'ลำดับแผนการเรียน', exact: true })
			).toBeVisible();
			await expect(editor.getByRole('region', { name: 'รายการในหลักสูตร' })).toHaveCount(0);
			await page.screenshot({
				animations: 'disabled',
				path: testInfo.outputPath(`editor-settings-${width}-${mode}.png`),
				fullPage: true
			});
			expect(state.writes).toEqual([]);
			expect(state.errors).toEqual([]);
		});
	}

// Hold a completed transport response so cancellation alone cannot hide a stale-result bug.
// The page still receives real provider JSON; no application state is exposed to the test.
async function delayFirstOrderReviewResponse(page: Page) {
	await page.addInitScript(() => {
		const requestFetch = window.fetch.bind(window);
		let structureReads = 0;
		window.fetch = async (input, init) => {
			const response = await requestFetch(input, init);
			const url = new URL(input instanceof Request ? input.url : String(input), location.href);
			if (
				!url.pathname.endsWith('/structure') ||
				(init?.method ?? 'GET') !== 'GET' ||
				++structureReads !== 2
			)
				return response;
			const body = await response.text();
			const headers = new Headers(response.headers);
			const status = response.status;
			document.documentElement.dataset.orderReviewWaiting = 'true';
			await new Promise<void>((resolve) =>
				window.addEventListener('release-test-order-review', () => resolve(), { once: true })
			);
			const buffered = new Response(body, { status, headers });
			const read = buffered.text.bind(buffered);
			buffered.text = async () => {
				const text = await read();
				document.documentElement.dataset.orderReviewConsumed = 'true';
				return text;
			};
			return buffered;
		};
	});
}
async function releaseOrderReviewResponse(page: Page) {
	await page.evaluate(() => window.dispatchEvent(new Event('release-test-order-review')));
	await page.waitForFunction(() => document.documentElement.dataset.orderReviewConsumed === 'true');
	await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
}
async function startDelayedOrderReview(page: Page) {
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor.getByRole('alert')).toContainText('เปลี่ยนไปแล้ว');
	await editor.getByRole('button', { name: 'โหลดลำดับล่าสุดเพื่อตรวจสอบ', exact: true }).click();
	await page.waitForFunction(() => document.documentElement.dataset.orderReviewWaiting === 'true');
	await editor.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(editor).toBeHidden();
}

test('a delayed order review cannot regress a reopened editor after a newer order save', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const initialRevision = fixture.workspace.level.rowVersion;
	const state = await installCurriculumCatalog(page, fixture, { failOrderOnce: true });
	await delayFirstOrderReviewResponse(page);
	await openCurriculumEditor(page);
	await startDelayedOrderReview(page);
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeDisabled();
	await releaseOrderReviewResponse(page);
	const firstProgram = page
		.getByTestId('curriculum-matrix')
		.locator('.matrix-table')
		.first()
		.locator('thead tr')
		.first()
		.getByRole('columnheader')
		.first();
	await expect.soft(firstProgram).toHaveText('แผนวิทย์คณิต');
	await editor.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(editor).toBeHidden();
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await editor.getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect.poll(() => state.writes.length).toBe(3);
	expect(state.writes[2].body).toMatchObject({ rowVersion: initialRevision + 1 });
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeDisabled();
	expect(state.structureReads()).toBe(2);
	expect(state.errors).toEqual([]);
});

test('a delayed order review cannot replace a newer same-context workspace refresh after copying a program', async ({
	page
}) => {
	const fixture = curriculumCatalogFixture();
	const initialRevision = fixture.workspace.level.rowVersion;
	const state = await installCurriculumCatalog(page, fixture, { failOrderOnce: true });
	await delayFirstOrderReviewResponse(page);
	await openCurriculumEditor(page);
	await startDelayedOrderReview(page);
	await page.getByRole('button', { name: 'คัดลอกแผนจากฉบับเดิม', exact: true }).click();
	const copy = page.getByRole('dialog', { name: 'คัดลอกแผนจากฉบับเดิม', exact: true });
	for (const [field, choice] of [
		['ฉบับต้นทาง *', edition().name],
		['ระดับการศึกษาต้นทาง *', 'ระดับมัธยมศึกษาตอนต้น'],
		['แผนต้นทาง *', 'แผนการเรียนพื้นฐาน']
	]) {
		await copy.getByLabel(field, { exact: true }).click();
		await page.getByRole('option', { name: choice, exact: true }).click();
	}
	await copy.getByRole('button', { name: 'คัดลอกเป็นแผนร่าง', exact: true }).click();
	await expect(copy).toBeHidden();
	await expect(page.getByTestId('curriculum-level-ready')).toContainText('3 แผนการเรียน');
	const completeProgramIds = fixture.workspace.programs.map((program) => program.id);
	await releaseOrderReviewResponse(page);
	await expect.soft(page.getByTestId('curriculum-level-ready')).toContainText('3 แผนการเรียน');
	await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
	const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร' });
	await editor.getByRole('button', { name: 'ตั้งค่า', exact: true }).click();
	await expect.soft(editor.locator('[data-program-order-id]')).toHaveCount(3);
	await editor.getByRole('button', { name: 'เลื่อนแผน 2 ขึ้น', exact: true }).click();
	await editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true }).click();
	await expect.poll(() => state.writes.length).toBe(3);
	expect(state.writes[2].body).toMatchObject({
		rowVersion: initialRevision,
		studyProgramIds: [completeProgramIds[1], completeProgramIds[0], completeProgramIds[2]]
	});
	await expect(editor.getByRole('button', { name: 'บันทึกลำดับแผน', exact: true })).toBeDisabled();
	expect(state.structureReads()).toBe(3);
	expect(state.errors).toEqual([]);
});
