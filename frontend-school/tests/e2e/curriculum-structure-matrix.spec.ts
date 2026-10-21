import { expect, test, type Page, type Locator } from '@playwright/test';
import type { CurriculumStructureRequirement } from '../../src/lib/api/academic-core.js';
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
			const table = matrix.getByRole('table').first();
			await expect(table.locator('[data-requirement-id="science-1"]')).toBeVisible();
			const scienceRow = table
				.getByRole('row')
				.filter({ has: page.locator('[data-requirement-id="science-1"]') });
			await expect(scienceRow).toContainText('วิทยาศาสตร์ ๒');
			await expect(scienceRow.getByRole('button', { name: 'เก่า', exact: true })).toHaveCount(1);
			await expect(table.locator('thead')).toContainText('ภาคฤดูร้อน');
			await expect(matrix).not.toContainText('HIDDEN-ACTIVITY-CODE');
			await expect(matrix).not.toContainText(/\bv[12]\b|ข้อมูลต่างกันระหว่างแผน|แผนเริ่มต้น/);
			await expect(matrix.locator('abbr').first()).toBeVisible();
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
			await expect(matrix.getByRole('table')).toHaveCount(3);
			await expect(matrix).toContainText('รวมทั้งระดับ · ทุกชั้น ทุกภาคเรียน');
			await page.getByRole('checkbox', { name: 'ภาคเรียนที่ 2', exact: true }).uncheck();
			await expect(matrix.getByRole('table').first().locator('thead')).not.toContainText(
				'ภาคเรียนที่ 2'
			);
			await expect(matrix.getByRole('table').first().locator('thead')).toContainText('ภาคฤดูร้อน');
			await page.getByRole('checkbox', { name: 'ทุกภาคเรียน', exact: true }).check();
			await expect(matrix.getByRole('table').first().locator('thead')).toContainText(
				'ภาคเรียนที่ 2'
			);
			await page.getByRole('checkbox', { name: 'ภาคเรียนที่ 2', exact: true }).uncheck();
			await page.getByRole('checkbox', { name: 'ภาคฤดูร้อน', exact: true }).uncheck();
			await page.getByRole('checkbox', { name: 'ภาคเรียนที่ 1', exact: true }).uncheck();
			await expect(matrix.getByRole('table')).toHaveCount(0);
			await expect(matrix.getByText('เลือกภาคเรียนเพื่อแสดงตาราง')).toHaveCount(3);
			await page.getByRole('checkbox', { name: 'ทุกภาคเรียน', exact: true }).focus();
			await page.keyboard.press('Space');
			await expect(page.getByRole('checkbox', { name: 'ทุกภาคเรียน', exact: true })).toBeChecked();
			await expect(matrix.getByRole('table')).toHaveCount(3);
			await expect(matrix.getByRole('table').first().locator('thead')).toContainText('ภาคฤดูร้อน');
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
	await editor.getByText('ตั้งค่าภาคเรียนในหลักสูตร', { exact: true }).click();
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
	await editor.getByRole('button', { name: 'ตั้งค่าภาคเรียนในหลักสูตร', exact: true }).click();
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
	await expect(page.getByTestId('curriculum-matrix').getByRole('table')).toHaveCount(0);
	expect(state.writes).toEqual([]);
	expect(state.errors).toEqual([]);
});
