import { expect, test, type Page } from '@playwright/test';
import type {
	ActivityVersion,
	CatalogActivityOverview,
	CreateActivityVersionRequest
} from '../../src/lib/api/academic-core.js';
import {
	failSubject,
	installSubjectCatalog,
	replySubject,
	subjectFixture,
	subjectGate,
	subjectPath
} from './fixtures/subject-catalog';

test.use({ serviceWorkers: 'block' });

const activityPath = '/staff/academic/catalog/activities';
const activityApi = '/api/academic/catalog/activities';
const activityId = 'activity-unpublished';
const activityName = 'ชุมนุม · ยังไม่ได้ตั้งชื่อ';

function activityFixture(canDelete = true): CatalogActivityOverview {
	return {
		canCreate: true,
		gradeLevelOptions: [],
		items: [
			{
				activity: {
					id: activityId,
					code: 'internal-activity-code',
					activityType: 'club',
					rowVersion: 7,
					createdAt: '2026-05-01T00:00:00Z',
					updatedAt: '2026-05-01T00:00:00Z'
				},
				displayState: 'unpublished',
				displayVersion: null,
				draftCount: 0,
				gradeLevels: [],
				canManage: canDelete,
				canDelete
			}
		]
	};
}

async function setup(page: Page, kind: 'subject' | 'activity', canDelete = true) {
	const subjects = subjectFixture(2);
	subjects.items[0].canDelete = canDelete;
	subjects.items[0].canManage = canDelete;
	subjects.items[0].displayState = 'unpublished';
	subjects.items[0].displayVersion = null;
	subjects.histories.set('subject-1', []);
	const activities = activityFixture(canDelete);
	const historyGate = subjectGate();
	const deleteGate = subjectGate();
	let refuse = false;
	const api = kind === 'subject' ? '/api/academic/catalog/subjects' : activityApi;
	const id = kind === 'subject' ? 'subject-1' : activityId;
	const requests = await installSubjectCatalog(page, subjects, {
		onRequest: async (route, url) => {
			if (url.pathname === `${activityApi}/overview`) {
				await replySubject(route, activities);
				return true;
			}
			if (url.pathname === `${api}/${id}/versions`) {
				await historyGate.promise;
				await replySubject(route, []);
				return true;
			}
			if (url.pathname === `${api}/${id}` && route.request().method() === 'DELETE') {
				await deleteGate.promise;
				if (refuse) await failSubject(route, 409, 'มีรุ่นที่เผยแพร่แล้ว ไม่สามารถลบได้');
				else await replySubject(route, {});
				return true;
			}
			return false;
		}
	});
	await page.goto(kind === 'subject' ? subjectPath : activityPath);
	await page
		.getByRole('button', {
			name: kind === 'subject' ? 'เปิด ค00001' : `เปิดรายละเอียด ${activityName}`,
			exact: true
		})
		.click();
	return {
		requests,
		historyGate,
		deleteGate,
		api,
		id,
		refuse: () => (refuse = true)
	};
}

for (const kind of ['subject', 'activity'] as const) {
	const label = kind === 'subject' ? 'รายวิชา' : 'กิจกรรม';
	test(`${kind}: unnamed stable record requires confirmation and patches only its list after deletion`, async ({
		page
	}) => {
		const fixture = await setup(page, kind);
		const deleteButton = page.getByRole('button', { name: `ลบ${label}`, exact: true });
		await deleteButton.click();
		const confirm = page.getByRole('dialog', { name: `ยืนยันลบ${label}`, exact: true });
		await expect(confirm).toContainText(kind === 'subject' ? 'ค00001' : activityName);
		await confirm.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
		expect(fixture.requests.writes).toEqual([]);
		await expect(deleteButton).toBeVisible();
		await deleteButton.click();
		await confirm.getByRole('button', { name: `ยืนยันลบ${label}`, exact: true }).click();
		try {
			await expect.poll(() => fixture.requests.writes.length).toBe(1);
			await expect(confirm.getByRole('button', { name: 'กำลังลบ...', exact: true })).toBeDisabled();
			await expect(confirm.getByRole('button', { name: 'ยกเลิก', exact: true })).toBeDisabled();
			await confirm.press('Escape');
			await expect(confirm).toBeVisible();
			expect(fixture.requests.writes).toEqual([
				{
					method: 'DELETE',
					path: `${fixture.api}/${fixture.id}`,
					body: { rowVersion: kind === 'subject' ? 2 : 7 }
				}
			]);
		} finally {
			fixture.deleteGate.release();
		}
		await expect(page.getByRole('dialog')).toHaveCount(0);
		fixture.historyGate.release();
		await expect(
			page.getByTestId(`catalog-${kind === 'subject' ? 'subjects' : 'activities'}-ready`)
		).toBeVisible();
		await expect(
			page.getByRole('button', {
				name: kind === 'subject' ? 'เปิด ค00001' : `เปิดรายละเอียด ${activityName}`,
				exact: true
			})
		).toHaveCount(0);
		if (kind === 'subject') {
			await expect(page.getByRole('button', { name: 'เปิด ค00002', exact: true })).toBeVisible();
			expect(fixture.requests.overviewReads()).toBe(1);
		} else await expect(page.getByText('ยังไม่มีกิจกรรมในทะเบียน', { exact: true })).toBeVisible();
		expect(fixture.requests.pageErrors).toEqual([]);
	});

	test(`${kind}: refused deletion retains the selected record and entered version draft`, async ({
		page
	}) => {
		const fixture = await setup(page, kind);
		fixture.historyGate.release();
		await page.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('ร่างที่ต้องเก็บไว้');
		await page.getByRole('button', { name: `ลบ${label}`, exact: true }).click();
		fixture.refuse();
		fixture.deleteGate.release();
		const confirm = page.getByRole('dialog', { name: `ยืนยันลบ${label}`, exact: true });
		await confirm.getByRole('button', { name: `ยืนยันลบ${label}`, exact: true }).click();
		await expect(confirm.getByRole('alert')).toContainText('มีรุ่นที่เผยแพร่แล้ว');
		await expect(
			confirm.getByRole('button', { name: `ยืนยันลบ${label}`, exact: true })
		).toBeEnabled();
		await confirm.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
		await expect(page.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue('ร่างที่ต้องเก็บไว้');
		expect(fixture.requests.writes).toHaveLength(1);
		expect(fixture.requests.pageErrors).toEqual([]);
	});

	test(`${kind}: unavailable delete capability hides the destructive action`, async ({ page }) => {
		const fixture = await setup(page, kind, false);
		fixture.historyGate.release();
		await expect(page.getByText('ยังไม่มีรุ่นข้อมูล', { exact: true })).toBeVisible();
		await expect(page.getByRole('button', { name: `ลบ${label}`, exact: true })).toHaveCount(0);
		expect(fixture.requests.writes).toEqual([]);
	});
}

test('activity zero weekly and term hours remain decimal strings through draft and publication', async ({
	page
}) => {
	const activities = activityFixture();
	const versions: ActivityVersion[] = [];
	const writes: CreateActivityVersionRequest[] = [];
	const requests = await installSubjectCatalog(page, subjectFixture(), {
		onRequest: async (route, url) => {
			if (url.pathname === `${activityApi}/overview`) {
				await replySubject(route, activities);
				return true;
			}
			if (url.pathname === `${activityApi}/${activityId}/versions`) {
				if (route.request().method() === 'GET') await replySubject(route, versions);
				else {
					const payload: CreateActivityVersionRequest = route.request().postDataJSON();
					writes.push(payload);
					const version: ActivityVersion = {
						...payload,
						id: 'activity-zero-version',
						activityId,
						versionNo: 1,
						status: 'draft',
						rowVersion: 3,
						migrated: false,
						createdAt: '2026-10-20T00:00:00Z',
						updatedAt: '2026-10-20T00:00:00Z'
					};
					versions.push(version);
					activities.items[0].displayVersion = version;
					activities.items[0].draftCount = 1;
					await replySubject(route, version);
				}
				return true;
			}
			if (
				url.pathname === '/api/academic/catalog/activity-versions/activity-zero-version/publish'
			) {
				expect(route.request().postDataJSON()).toEqual({ rowVersion: 3 });
				versions[0].status = 'published';
				versions[0].rowVersion += 1;
				activities.items[0].canDelete = false;
				activities.items[0].displayState = 'current';
				activities.items[0].draftCount = 0;
				await replySubject(route, versions[0]);
				return true;
			}
			return false;
		}
	});
	await page.goto(activityPath);
	await page.getByRole('button', { name: `เปิดรายละเอียด ${activityName}`, exact: true }).click();
	await page.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('กิจกรรมในหลักสูตรไม่จัดการสอน');
	for (const label of ['ชั่วโมงต่อสัปดาห์', 'ชั่วโมงรวมต่อภาคเรียน']) {
		const input = page.getByLabel(label, { exact: true });
		for (const value of ['', '-1', '1.234']) {
			await input.fill(value);
			expect(await input.evaluate((element: HTMLInputElement) => element.checkValidity())).toBe(
				false
			);
			await page.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
			expect(writes).toEqual([]);
		}
		await input.fill('0');
		expect(await input.evaluate((element: HTMLInputElement) => element.checkValidity())).toBe(true);
	}
	await page.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
	await expect.poll(() => writes.length).toBe(1);
	expect(writes[0]).toMatchObject({ hoursPerWeek: '0', hoursPerTerm: '0' });
	await page.getByRole('button', { name: 'เผยแพร่', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ประวัติรุ่น', exact: true })).toContainText(
		'เผยแพร่แล้ว'
	);
	await expect(page.getByRole('button', { name: 'ลบกิจกรรม', exact: true })).toHaveCount(0);
	await expect(page.getByRole('region', { name: 'ประวัติรุ่น', exact: true })).toContainText(
		'ชม./สัปดาห์ 0'
	);
	expect(requests.pageErrors).toEqual([]);
});
