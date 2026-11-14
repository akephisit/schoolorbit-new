import { expect, test, type Page } from '@playwright/test';
import {
	mockCurrentDelivery,
	replaceDeliverySession,
	deferred,
	deliveryReply
} from './fixtures/current-delivery';
import type { AcademicContextOptionsResponse } from '../../src/lib/api/academic-context';
import type { DeliveryDeletionPreview } from '../../src/lib/api/learning-delivery';
import { navigationOfferingIds } from './fixtures/delivery-navigation-route-data';
test.use({ serviceWorkers: 'block' });
const dialog = (page: Page) => page.getByRole('alertdialog');
const confirm = (page: Page) =>
	dialog(page).getByRole('button', { name: 'ยืนยันลบถาวร', exact: true });
async function browserTurn(page: Page) {
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
}

test('a delayed A preview cannot become B confirmation on the same route with management still granted', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page);
	const held = deferred();
	let previews = 0;
	await page.route(
		`**/api/academic/offerings/${fixture.resource().id}/deletion-preview?*`,
		async (route) => {
			previews++;
			if (previews === 1) await held.promise;
			await route.fallback();
		}
	);
	try {
		await page.goto(fixture.detail);
		await page.getByRole('button', { name: 'ลบรายการเปิดสอน', exact: true }).click();
		await expect.poll(() => previews).toBe(1);
		await expect(dialog(page).getByRole('status')).toBeVisible();
		await replaceDeliverySession(page, fixture);
		await expect(dialog(page)).toHaveCount(0);
		held.release();
		await browserTurn(page);
		expect(fixture.writes.filter((write) => write.method === 'DELETE')).toEqual([]);
		await expect(dialog(page)).toHaveCount(0);
		await page.getByRole('button', { name: 'ลบรายการเปิดสอน', exact: true }).click();
		await expect(confirm(page)).toBeEnabled();
		expect(previews).toBe(2);
		await confirm(page).click();
		await page.waitForURL((url) => url.pathname === '/staff/academic/delivery');
		expect(fixture.writes.filter((write) => write.method === 'DELETE')).toHaveLength(1);
	} finally {
		held.release();
	}
});

test('B cannot submit the already accepted A preview and must review a fresh scoped preview', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page);
	await page.goto(fixture.detail);
	await page.getByRole('button', { name: 'ลบรายการเปิดสอน', exact: true }).click();
	await expect(confirm(page)).toBeEnabled();
	expect(fixture.previewReads).toHaveLength(1);
	const original = await confirm(page).elementHandle();
	await replaceDeliverySession(page, fixture);
	await expect(dialog(page)).toHaveCount(0);
	await original!.evaluate((button: HTMLButtonElement) => button.click());
	await browserTurn(page);
	expect(fixture.writes.filter((write) => write.method === 'DELETE')).toEqual([]);
	await page.getByRole('button', { name: 'ลบรายการเปิดสอน', exact: true }).click();
	await expect(confirm(page)).toBeEnabled();
	expect(fixture.previewReads).toHaveLength(2);
	expect(fixture.previewReads[0].epochSessionId).not.toBe(fixture.previewReads[1].epochSessionId);
	await confirm(page).click();
	await page.waitForURL((url) => url.pathname === '/staff/academic/delivery');
	expect(fixture.writes.filter((write) => write.method === 'DELETE')).toHaveLength(1);
});

test('a late accepted A offering deletion cannot run its navigation callback in identity B', async ({
	page
}) => {
	const held = deferred();
	const fixture = await mockCurrentDelivery(page, { deleteGate: held.promise });
	try {
		await page.goto(fixture.detail);
		await page.getByRole('button', { name: 'ลบรายการเปิดสอน', exact: true }).click();
		await expect(confirm(page)).toBeEnabled();
		await confirm(page).click();
		await expect
			.poll(() => fixture.writes.filter((write) => write.method === 'DELETE').length)
			.toBe(1);
		await expect(dialog(page).getByRole('button', { name: 'ยกเลิก', exact: true })).toBeDisabled();
		await replaceDeliverySession(page, fixture);
		await expect(dialog(page)).toHaveCount(0);
		const receipt = page.waitForResponse((response) => response.request().method() === 'DELETE');
		held.release();
		await (await receipt).finished();
		await browserTurn(page);
		expect(new URL(page.url()).pathname).toBe(`/staff/academic/delivery/${fixture.resource().id}`);
		expect(fixture.graph.offerings).toEqual([]);
		expect(fixture.writes.filter((write) => write.method === 'DELETE')).toHaveLength(1);
	} finally {
		held.release();
	}
});

test('late A group deletion does not clear B selection and B can review and delete another current group', async ({
	page
}) => {
	const held = deferred();
	const fixture = await mockCurrentDelivery(page, { deleteGate: held.promise });
	try {
		await page.goto(`${fixture.detail}&groupId=${fixture.ids.groupA}`);
		await expect(page.getByRole('heading', { name: 'กลุ่ม ก', exact: true })).toBeVisible();
		await page.getByRole('button', { name: 'ลบกลุ่ม', exact: true }).click();
		await expect(confirm(page)).toBeEnabled();
		await confirm(page).click();
		await expect
			.poll(() => fixture.writes.filter((write) => write.method === 'DELETE').length)
			.toBe(1);
		await replaceDeliverySession(page, fixture);
		await expect(dialog(page)).toHaveCount(0);
		await page.getByRole('button', { name: /^กลุ่ม ข ครู / }).click();
		await expect(page.getByRole('heading', { name: 'กลุ่ม ข', exact: true })).toBeVisible();
		const groupsPath = `/api/academic/offerings/${fixture.resource().id}/groups`;
		const before = fixture.count(groupsPath);
		const receipt = page.waitForResponse((response) => response.request().method() === 'DELETE');
		held.release();
		await (await receipt).finished();
		await browserTurn(page);
		await expect(page.getByRole('heading', { name: 'กลุ่ม ข', exact: true })).toBeVisible();
		expect(fixture.count(groupsPath)).toBe(before);
		expect(fixture.groups().map((group) => group.id)).toEqual([fixture.ids.groupB]);
		await page.getByRole('button', { name: 'ลบกลุ่ม', exact: true }).click();
		await expect(confirm(page)).toBeEnabled();
		await confirm(page).click();
		await expect.poll(() => fixture.groups().length).toBe(0);
		expect(fixture.writes.filter((write) => write.method === 'DELETE')).toHaveLength(2);
	} finally {
		held.release();
	}
});

function ownerContext(
	fixture: Awaited<ReturnType<typeof mockCurrentDelivery>>
): AcademicContextOptionsResponse {
	const resource = fixture.resource();
	return {
		defaultAcademicYearId: fixture.ids.activeYear,
		defaultAcademicTermId: fixture.ids.activeTerm,
		years: [
			{
				id: resource.academicYearId,
				year: 2570,
				name: 'ปีเตรียมการร่วม',
				status: 'planning',
				isVisible: true
			},
			{
				id: fixture.ids.activeYear,
				year: 2569,
				name: 'ปีเตรียมการร่วม',
				status: 'active',
				isVisible: true
			}
		],
		terms: [
			{
				id: resource.academicTermId,
				academicYearId: resource.academicYearId,
				sequence: 2,
				code: '2',
				name: 'ภาคเรียนเจ้าของรายการ',
				termType: 'regular',
				schoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI'],
				status: 'planning',
				isVisible: true,
				includedInYearResult: true,
				blocksYearClosure: true
			},
			{
				id: fixture.ids.activeTerm,
				academicYearId: fixture.ids.activeYear,
				sequence: 1,
				code: '1',
				name: 'ภาคเรียนปัจจุบันอีกปี',
				termType: 'regular',
				schoolDays: ['MON', 'TUE', 'WED', 'THU', 'FRI'],
				status: 'active',
				isVisible: true,
				includedInYearResult: true,
				blocksYearClosure: true
			}
		]
	};
}

for (const scope of ['offering', 'group'] as const)
	test(`${scope} deletion shows the exact owner year, term, offering, optional group and impacts before its one confirmation`, async ({
		page
	}) => {
		const fixture = await mockCurrentDelivery(page),
			resource = fixture.resource();
		let contextReads = 0;
		await page.route('**/api/academic/context/options', (route) => {
			contextReads++;
			return deliveryReply(route, ownerContext(fixture));
		});
		await page.goto(`${fixture.detail}&groupId=${fixture.ids.groupA}`);
		await page
			.getByRole('button', {
				name: scope === 'offering' ? 'ลบรายการเปิดสอน' : 'ลบกลุ่ม',
				exact: true
			})
			.click();
		await expect(confirm(page)).toBeEnabled();
		const context = dialog(page).getByRole('region', { name: 'บริบทการลบ', exact: true });
		await expect(context).toContainText('ปีเตรียมการร่วม (ปีการศึกษา 2570)');
		await expect(context).toContainText('ภาคเรียนเจ้าของรายการ · ลำดับ 2');
		await expect(context).toContainText(resource.nameSnapshot);
		await expect(context).not.toContainText('ปีการศึกษา 2569');
		if (scope === 'group') await expect(context).toContainText('กลุ่ม ก');
		else await expect(context.getByText('กลุ่ม ก', { exact: true })).toHaveCount(0);
		const impact = dialog(page).getByLabel('ผลกระทบการลบ', { exact: true });
		const counts = await impact.evaluate((list) =>
			Object.fromEntries(
				[...list.querySelectorAll('dt')].map((label) => [
					label.textContent?.trim(),
					label.nextElementSibling?.textContent?.trim()
				])
			)
		);
		expect(counts).toMatchObject({
			กลุ่มเรียน: scope === 'group' ? '1' : '2',
			คาบเรียน: '2',
			คะแนน: '3',
			คาบเช็คชื่อ: '4',
			ไฟล์หลักฐาน: '1'
		});
		const revision = scope === 'group' ? fixture.groups()[0].rowVersion : resource.rowVersion;
		const digest = fixture.graph.rowVersion.toString(16).padStart(64, '0');
		await confirm(page).click();
		await expect
			.poll(() => fixture.writes.filter((write) => write.method === 'DELETE').length)
			.toBe(1);
		await expect(dialog(page)).toHaveCount(0);
		expect(fixture.writes.find((write) => write.method === 'DELETE')?.body).toEqual({
			academicYearId: resource.academicYearId,
			academicTermId: resource.academicTermId,
			rowVersion: revision,
			sourceDigest: digest
		});
		expect(contextReads).toBe(1);
	});

test('an accepted offering identity cannot expose deletion before its exact human-readable context arrives', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page),
		held = deferred();
	let contextReads = 0;
	await page.route('**/api/academic/context/options', async (route) => {
		contextReads++;
		await held.promise;
		return deliveryReply(route, ownerContext(fixture));
	});
	try {
		const navigation = page.goto(fixture.detail);
		await expect.poll(() => contextReads).toBe(1);
		await expect(
			page.getByRole('heading', { name: fixture.resource().nameSnapshot, level: 1, exact: true })
		).toBeVisible();
		await expect(page.getByRole('button', { name: 'ลบรายการเปิดสอน', exact: true })).toHaveCount(0);
		expect(fixture.previewReads).toEqual([]);
		expect(fixture.writes).toEqual([]);
		held.release();
		await navigation;
		await page.getByRole('button', { name: 'ลบรายการเปิดสอน', exact: true }).click();
		await expect(confirm(page)).toBeEnabled();
		await expect(
			dialog(page).getByRole('region', { name: 'บริบทการลบ', exact: true })
		).toContainText('ปีการศึกษา 2570');
		expect(contextReads).toBe(1);
	} finally {
		held.release();
	}
});

for (const [scope, field] of [
	['offering', 'academicYearId'],
	['offering', 'academicTermId'],
	['group', 'learningOfferingId'],
	['group', 'learningGroupId']
] as const)
	test(`${scope} deletion refuses preview ${field} from another context without confirming it under the visible owner`, async ({
		page
	}) => {
		const fixture = await mockCurrentDelivery(page),
			resource = fixture.resource();
		const group = scope === 'group' ? fixture.groups()[0] : null;
		const preview: DeliveryDeletionPreview = {
			academicYearId: resource.academicYearId,
			academicTermId: resource.academicTermId,
			learningOfferingId: resource.id,
			learningGroupId: group?.id ?? null,
			rowVersion: group?.rowVersion ?? resource.rowVersion,
			sourceDigest: 'a'.repeat(64),
			impact: {
				learningGroups: group ? 1 : 2,
				teacherAssignments: 1,
				memberships: 2,
				timetableBlocks: 2,
				examScheduleItems: 1,
				assessmentScores: 3,
				attendanceSessions: 4,
				attendanceSummaries: 0,
				officialResults: 1,
				resultCorrections: 1,
				termAggregates: 1,
				annualAggregates: 1,
				supervisionLinks: 1,
				derivedBehaviorEvents: 1,
				evidenceFiles: 1
			}
		};
		preview[field] =
			field === 'academicYearId'
				? fixture.ids.activeYear
				: field === 'academicTermId'
					? fixture.ids.activeTerm
					: field === 'learningOfferingId'
						? navigationOfferingIds.club
						: fixture.ids.groupB;
		const path = group
			? `/api/academic/learning-groups/${group.id}/deletion-preview`
			: `/api/academic/offerings/${resource.id}/deletion-preview`;
		await page.route(`**${path}?*`, (route) => deliveryReply(route, preview));
		await page.goto(`${fixture.detail}&groupId=${fixture.ids.groupA}`);
		await page
			.getByRole('button', { name: scope === 'group' ? 'ลบกลุ่ม' : 'ลบรายการเปิดสอน', exact: true })
			.click();
		await expect(dialog(page).getByRole('alert')).toContainText(
			'ผลตรวจไม่ตรงกับปี ภาคเรียน หรือรายการที่ขอ'
		);
		await expect(confirm(page)).toBeDisabled();
		await expect(dialog(page).getByLabel('ผลกระทบการลบ', { exact: true })).toHaveCount(0);
		expect(fixture.writes.filter((write) => write.method === 'DELETE')).toEqual([]);
	});
