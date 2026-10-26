import { expect, test, type Page } from '@playwright/test';
import type {
	ActivityVersion,
	CatalogActivityOverview,
	CreateActivityVersionRequest,
	GradeLevelOption,
	ReplaceCurriculumStructureRequest
} from '../../src/lib/api/academic-core.js';
import {
	installSubjectCatalog,
	openSubjectDetails,
	replySubject,
	subjectFixture,
	subjectGrade,
	subjectPath
} from './fixtures/subject-catalog';
import {
	curriculumCatalogFixture,
	curriculumCatalogPath,
	installCurriculumCatalog
} from './fixtures/curriculum-catalog';

test.use({ serviceWorkers: 'block', timezoneId: 'Asia/Bangkok' });

function grade(index: number) {
	return {
		...subjectGrade,
		id: index === 1 ? subjectGrade.id : `grade-${index}`,
		code: `M${index}`,
		name: `มัธยมศึกษาปีที่ ${index}`,
		short_name: `ม.${index}`,
		level_order: index
	} satisfies GradeLevelOption;
}

async function chooseScope(page: Page, all: boolean) {
	await page.getByRole('combobox', { name: /เลือกระดับชั้นสำหรับ/ }).click();
	if (all) await page.getByRole('button', { name: 'ทุกระดับชั้น', exact: true }).click();
	else await page.getByRole('option', { name: /มัธยมศึกษาปีที่ 1/ }).click();
	await page.keyboard.press('Escape');
}

for (const all of [true, false]) {
	test(`course ${all ? 'all' : 'partial'} scope snapshots at save and stays fixed through publish and reopen`, async ({
		page
	}) => {
		const fixture = subjectFixture(1);
		fixture.grades.push(grade(2));
		const requests = await installSubjectCatalog(page, fixture);
		await page.goto(subjectPath);
		await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
		await page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true }).fill('ค99999');
		await page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด', exact: true }).click();
		const form = page.getByRole('form', { name: 'สร้างรุ่นใหม่' });
		await expect(form.getByRole('combobox', { name: /เลือกระดับชั้นสำหรับ/ })).toHaveText(
			'ทุกระดับชั้น'
		);
		await expect(form).toContainText('จะไม่เพิ่มชั้นใหม่ในภายหลังโดยอัตโนมัติ');
		await form.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('รายวิชาทุกชั้น ณ วันบันทึก');
		await chooseScope(page, all);
		// A grade added after the browser loaded options must be resolved by the save owner.
		fixture.grades.push(grade(3));
		await form.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
		await expect.poll(() => fixture.histories.get('subject-created')?.length).toBe(1);
		const saved = fixture.histories.get('subject-created')![0];
		const expected = (all ? fixture.grades : [subjectGrade]).map((item) => item.id).sort();
		expect(requests.writes.find((write) => write.path.endsWith('/versions'))!.body).toMatchObject({
			gradeLevelIds: all ? [] : [subjectGrade.id]
		});
		expect(saved.gradeLevelIds).toEqual(expected);
		fixture.grades.push(grade(4));
		await page.getByRole('button', { name: 'เผยแพร่', exact: true }).click();
		await expect.poll(() => saved.status).toBe('published');
		expect(saved.gradeLevelIds).toEqual(expected);
		await page.goto(subjectPath);
		await openSubjectDetails(page, 'ค99999');
		const scope = page
			.getByRole('form', { name: 'สร้างรุ่นใหม่' })
			.getByRole('combobox', { name: /เลือกระดับชั้นสำหรับ/ });
		await expect(scope).toHaveText(all ? 'เลือกแล้ว 3 ระดับ' : 'ม.1');
		await scope.click();
		await expect(
			page.getByRole('checkbox', { name: 'เลือกระดับชั้น มัธยมศึกษาปีที่ 4', exact: true })
		).not.toBeChecked();
		await page.keyboard.press('Escape');
		expect(saved.gradeLevelIds).toEqual(expected);
		expect(requests.pageErrors).toEqual([]);
		expect(requests.unexpected).toEqual([]);
	});

	test(`activity ${all ? 'all' : 'partial'} scope stores explicit save-time grades without expanding on publication`, async ({
		page
	}) => {
		const grades = [grade(1), grade(2)];
		const versions: ActivityVersion[] = [];
		const bodies: CreateActivityVersionRequest[] = [];
		let created = false;
		const activity = {
			id: 'activity-social',
			code: 'INTERNAL-SOCIAL',
			activityType: 'social',
			owningOrganizationUnitId: 'school-owner',
			archivedAt: null,
			rowVersion: 1,
			createdAt: '2026-10-20T05:00:00Z',
			updatedAt: '2026-10-20T05:00:00Z'
		};
		const overview = (): CatalogActivityOverview => ({
			canCreate: true,
			gradeLevelOptions: grades,
			items: created
				? [
						{
							activity,
							displayVersion:
								versions.find((version) => version.status === 'published') ?? versions[0] ?? null,
							displayState: versions.some((version) => version.status === 'published')
								? 'current'
								: 'unpublished',
							draftCount: versions.filter((version) => version.status === 'draft').length,
							gradeLevels: grades.filter((item) => versions[0]?.gradeLevelIds.includes(item.id)),
							canManage: true,
							canDelete: !versions.length
						}
					]
				: []
		});
		const requests = await installSubjectCatalog(page, subjectFixture(0), {
			onRequest: async (route, url) => {
				const path = url.pathname;
				const method = route.request().method();
				if (path === '/api/academic/catalog/activities/overview') {
					await replySubject(route, overview());
					return true;
				}
				if (path === '/api/academic/catalog/activities' && method === 'POST') {
					created = true;
					await replySubject(route, activity);
					return true;
				}
				if (path === `/api/academic/catalog/activities/${activity.id}/versions`) {
					if (method === 'POST') {
						const body: CreateActivityVersionRequest = route.request().postDataJSON();
						bodies.push(body);
						versions.unshift({
							...body,
							activityId: activity.id,
							id: 'social-v1',
							versionNo: 1,
							gradeLevelIds: [
								...new Set(
									body.gradeLevelIds.length ? body.gradeLevelIds : grades.map((item) => item.id)
								)
							].sort(),
							status: 'draft',
							migrated: false,
							rowVersion: 1,
							publishedAt: null,
							createdAt: activity.createdAt,
							updatedAt: activity.updatedAt
						});
						await replySubject(route, versions[0]);
					} else await replySubject(route, versions);
					return true;
				}
				if (path === '/api/academic/catalog/activity-versions/social-v1/publish') {
					versions[0].status = 'published';
					versions[0].publishedAt = activity.createdAt;
					versions[0].rowVersion++;
					await replySubject(route, versions[0]);
					return true;
				}
				return false;
			}
		});
		await page.goto('/staff/academic/catalog/activities');
		await page.getByRole('button', { name: 'เพิ่มกิจกรรม', exact: true }).click();
		await page
			.getByRole('dialog', { name: 'เพิ่มกิจกรรม', exact: true })
			.getByRole('combobox', { name: /^ประเภทกิจกรรม/ })
			.click();
		await page
			.getByRole('option', { name: 'กิจกรรมเพื่อสังคมและสาธารณประโยชน์', exact: true })
			.click();
		await page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด', exact: true }).click();
		const form = page.getByRole('form', { name: 'สร้างรุ่นใหม่' });
		await form
			.getByLabel('ชื่อภาษาไทย', { exact: true })
			.fill('กิจกรรมเพื่อสังคมและสาธารณประโยชน์');
		await chooseScope(page, all);
		grades.push(grade(3));
		await form.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
		await expect.poll(() => versions.length).toBe(1);
		const expected = (all ? grades : [subjectGrade]).map((item) => item.id).sort();
		expect(bodies[0].gradeLevelIds).toEqual(all ? [] : [subjectGrade.id]);
		expect(versions[0].gradeLevelIds).toEqual(expected);
		grades.push(grade(4));
		await page.getByRole('button', { name: 'เผยแพร่', exact: true }).click();
		await expect.poll(() => versions[0].status).toBe('published');
		expect(versions[0].gradeLevelIds).toEqual(expected);
		await page.goto('/staff/academic/catalog/activities');
		const row = page.getByRole('row').filter({ hasText: 'กิจกรรมเพื่อสังคมและสาธารณประโยชน์' });
		await expect(row).toContainText(all ? 'ม.1, ม.2, ม.3' : 'ม.1');
		await expect(row).not.toContainText('ม.4');
		expect(requests.pageErrors).toEqual([]);
		expect(requests.unexpected).toEqual([]);
	});
}

test('all-grade save with no active grades shows the server error and retains the revision draft', async ({
	page
}) => {
	const fixture = subjectFixture(1);
	const original = structuredClone(fixture.histories.get('subject-1'));
	const requests = await installSubjectCatalog(page, fixture);
	await page.goto(subjectPath);
	await openSubjectDetails(page);
	const form = page.getByRole('form', { name: 'สร้างรุ่นใหม่' });
	await form.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('ร่างที่ยังไม่บันทึก');
	await chooseScope(page, true);
	fixture.grades.splice(0);
	await form.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
	await expect(form.getByRole('alert')).toContainText('ไม่มีระดับชั้นที่เปิดใช้งาน');
	await expect(form.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue('ร่างที่ยังไม่บันทึก');
	expect(fixture.histories.get('subject-1')).toEqual(original);
	expect(requests.pageErrors).toEqual([]);
});

test('saved inactive or unresolved grade IDs remain selected and show an accurate count', async ({
	page
}) => {
	const fixture = subjectFixture(1);
	fixture.items[0].displayVersion!.gradeLevelIds = ['inactive-1', 'inactive-2'];
	fixture.items[0].gradeLevels = [];
	await installSubjectCatalog(page, fixture);
	await page.goto(subjectPath);
	await expect(page.getByRole('row').filter({ hasText: 'ค00001' })).toContainText('2 ระดับชั้น');
	await openSubjectDetails(page);
	const form = page.getByRole('form', { name: 'สร้างรุ่นใหม่' });
	await expect(form.getByRole('combobox', { name: /เลือกระดับชั้นสำหรับ/ })).toHaveText(
		'เลือกแล้ว 2 ระดับ'
	);
	await form.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
	await expect.poll(() => fixture.histories.get('subject-1')?.length).toBe(2);
	const saved = fixture.histories.get('subject-1')![0];
	expect(saved.gradeLevelIds).toEqual(['inactive-1', 'inactive-2']);
});

for (const workflow of ['quick', 'full'] as const) {
	test(`${workflow} picker admits corrected course and social activity snapshots for M1 and preserves the old published pin`, async ({
		page
	}) => {
		const fixture = curriculumCatalogFixture();
		const original = structuredClone(fixture.workspace.requirements[0]);
		const correctedCourse = {
			...fixture.current,
			id: 'course-correction',
			name: 'รายวิชาที่แก้ไขขอบเขตชั้น',
			gradeLevelIds: fixture.workspace.gradeLevels.map((item) => item.id)
		};
		const correctedActivity = {
			...correctedCourse,
			id: 'social-correction',
			catalogId: 'social-catalog',
			resourceKind: 'activity' as const,
			name: 'กิจกรรมเพื่อสังคมและสาธารณประโยชน์',
			section: 'student_development' as const,
			creditCategory: null
		};
		fixture.catalog.splice(
			0,
			fixture.catalog.length,
			{
				...correctedCourse,
				id: original.catalogVersionId,
				versionNo: 1,
				gradeLevelIds: [],
				selectableForNewRequirement: false
			},
			correctedCourse,
			correctedActivity,
			{
				...correctedActivity,
				id: 'social-old-empty',
				name: 'กิจกรรมเก่าที่ยังไม่มีขอบเขต',
				gradeLevelIds: []
			},
			{
				...correctedCourse,
				id: 'partial-m2',
				catalogId: 'partial',
				name: 'รายวิชาเฉพาะ ม.2',
				gradeLevelIds: [fixture.workspace.gradeLevels[1].id]
			}
		);
		const state = await installCurriculumCatalog(page, fixture);
		await page.goto(curriculumCatalogPath);
		if (workflow === 'full') {
			await page.getByRole('button', { name: 'จัดโครงสร้าง', exact: true }).click();
			const editor = page.getByRole('dialog', { name: 'จัดโครงสร้างหลักสูตร', exact: true });
			await editor
				.getByRole('combobox', { name: 'แผนการเรียนที่จะจัดโครงสร้าง', exact: true })
				.click();
			await page.getByRole('option', { name: 'แผนวิทย์คณิต', exact: true }).click();
			for (const name of ['รายวิชาที่แก้ไขขอบเขตชั้น', 'กิจกรรมเพื่อสังคมและสาธารณประโยชน์']) {
				await editor.getByRole('checkbox', { name: new RegExp(`^เลือก .*${name}`) }).check();
			}
			await expect(
				editor.getByRole('checkbox', { name: /กิจกรรมเก่าที่ยังไม่มีขอบเขต|รายวิชาเฉพาะ ม.2/ })
			).toHaveCount(0);
			await editor.getByRole('button', { name: /เพิ่ม 2 รายการ/ }).click();
			await editor.getByRole('button', { name: 'บันทึกโครงสร้าง', exact: true }).click();
			await expect(editor).toBeHidden();
		} else {
			for (const [kind, name, section] of [
				['รายวิชา', correctedCourse.name, 'basic_course'],
				['กิจกรรม', correctedActivity.name, 'student_development']
			]) {
				const target = page
					.getByTestId('curriculum-matrix')
					.locator(
						`td[data-program-id="${fixture.workspace.programs[1].id}"][data-grade-id="${fixture.workspace.gradeLevels[0].id}"][data-term-id="${fixture.workspace.termSlots[0].id}"][data-section-id="${section}"]`
					)
					.first();
				await target.click({ button: 'right' });
				await page.getByRole('menuitem', { name: `เพิ่ม${kind}…`, exact: true }).click();
				const panel = page.getByRole('dialog', { name: `เพิ่ม${kind}`, exact: true });
				await panel.getByRole('checkbox', { name: new RegExp(`^เลือก .*${name}`) }).check();
				await expect(
					panel.getByRole('checkbox', { name: /กิจกรรมเก่าที่ยังไม่มีขอบเขต|รายวิชาเฉพาะ ม.2/ })
				).toHaveCount(0);
				await panel.getByRole('button', { name: /เพิ่ม 1 รายการ/ }).click();
				await expect(panel).toBeHidden();
			}
		}
		const body = state.writes.at(-1)!.body as ReplaceCurriculumStructureRequest;
		expect(body.requirements.map((item) => item.catalogVersionId)).toEqual([
			'course-correction',
			'social-correction'
		]);
		expect(
			body.requirements.every((item) => item.gradeLevelId === fixture.workspace.gradeLevels[0].id)
		).toBe(true);
		expect(fixture.workspace.requirements.find((item) => item.id === original.id)).toEqual(
			original
		);
		expect(state.errors).toEqual([]);
		expect(state.unexpected).toEqual([]);
	});
}
