import { expect, test } from '@playwright/test';
import { mockCurrentDelivery, deliveryReply } from './fixtures/current-delivery';
import type { DatedRosterMembership } from '../../src/lib/api/learning-delivery';
test.use({ serviceWorkers: 'block' });
const confirmName = 'ยืนยันลบถาวร';

test('a current weekly-target edit saves once without changing the pinned curriculum source', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page),
		source = structuredClone(fixture.resource().snapshot);
	await page.goto(fixture.detail);
	await page.getByLabel('คาบต่อสัปดาห์', { exact: true }).fill('4');
	await page.getByRole('button', { name: 'บันทึกจำนวนคาบ', exact: true }).click();
	await expect.poll(() => fixture.resource().weeklyPeriodTarget).toBe(4);
	expect(fixture.resource().snapshot).toEqual(source);
	const writes = fixture.writes.filter((row) => row.method === 'PATCH');
	expect(writes).toHaveLength(1);
	expect(writes[0].body).toEqual({ rowVersion: 1, weeklyPeriodTarget: 4 });
	expect(
		fixture.reads.filter((url) => /delivery-versions|term-change-sets/.test(url.pathname))
	).toEqual([]);
});

test('read-only staff inspect the current opening without management options or mutation controls', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page, {
		permissions: ['learning_offering.read.school']
	});
	fixture.setCanManage(false);
	await page.goto(fixture.detail);
	await expect(page.getByRole('heading', { name: 'กลุ่ม ก', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'แก้ไขกลุ่มและครู', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'ลบรายการเปิดสอน', exact: true })).toHaveCount(0);
	expect(fixture.count('/api/academic/delivery/management-options')).toBe(0);
	expect(fixture.writes).toEqual([]);
});

test('read-only staff retain actual dated roster intervals without a publication or management preview', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page, {
		permissions: ['learning_offering.read.school']
	});
	fixture.setCanManage(false);
	const membership: DatedRosterMembership = {
		id: 'd0000000-0000-4000-8000-000000000001',
		learningGroupId: fixture.ids.groupA,
		studentAcademicYearId: 'd0000000-0000-4000-8000-000000000002',
		studentId: 'd0000000-0000-4000-8000-000000000003',
		displayName: 'เด็กชายทดสอบ รายชื่อ',
		studentCode: '101',
		membershipStatus: 'ended',
		rosterSource: 'manual',
		joinedAt: '2026-08-30',
		leftAt: '2026-09-01',
		rowVersion: 2
	};
	await page.route(`**/api/academic/learning-groups/${fixture.ids.groupA}/memberships`, (route) =>
		deliveryReply(route, [membership])
	);
	await page.goto(fixture.detail);
	const history = page.getByTestId('delivery-memberships-ready');
	await expect(history).toContainText(membership.displayName);
	await expect(history).toContainText('30 ส.ค. 2569');
	await expect(history).toContainText('1 ก.ย. 2569');
	await expect(
		history.getByRole('button', { name: 'เพิ่มนักเรียนตามวันที่', exact: true })
	).toHaveCount(0);
	expect(fixture.count('/api/academic/delivery/management-options')).toBe(0);
	expect(fixture.count(`/api/academic/learning-groups/${fixture.ids.groupA}/roster-preview`)).toBe(
		0
	);
	expect(fixture.writes).toEqual([]);
});

test('destructive current removal requires the exact accepted impact preview and deliberate confirmation', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page);
	await page.goto(fixture.detail);
	await page.getByRole('button', { name: 'ลบกลุ่ม', exact: true }).click();
	const dialog = page.getByRole('alertdialog');
	await expect(dialog.getByRole('button', { name: confirmName, exact: true })).toBeEnabled();
	await expect(dialog).toContainText('คะแนนและเช็คชื่อรายคาบ');
	expect(fixture.writes).toEqual([]);
	await page.keyboard.press('Escape');
	expect(fixture.groups()).toHaveLength(2);
	await page.getByRole('button', { name: 'ลบกลุ่ม', exact: true }).click();
	await dialog.getByRole('button', { name: confirmName, exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(fixture.writes.filter((row) => row.method === 'DELETE')).toHaveLength(1);
	expect(fixture.groups().map((row) => row.id)).toEqual([fixture.ids.groupB]);
});

test('a stale destructive preview is invalidated and a new exact revision must be reviewed', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page, { deleteConflict: true });
	await page.goto(fixture.detail);
	await page.getByRole('button', { name: 'ลบกลุ่ม', exact: true }).click();
	const dialog = page.getByRole('alertdialog'),
		confirm = dialog.getByRole('button', { name: confirmName, exact: true });
	await expect(confirm).toBeEnabled();
	await confirm.click();
	await expect(confirm).toBeDisabled();
	await expect(dialog.getByRole('alert')).toContainText('ข้อมูลเปลี่ยนแล้ว');
	expect(fixture.previewReads).toHaveLength(1);
	expect(fixture.groups()).toHaveLength(2);
	await dialog.getByRole('button', { name: 'ตรวจผลกระทบอีกครั้ง', exact: true }).click();
	await expect(confirm).toBeEnabled();
	await confirm.click();
	await expect(dialog).toHaveCount(0);
	expect(
		fixture.writes.filter((row) => row.method === 'DELETE').map((row) => row.body.rowVersion)
	).toEqual([3, 4]);
	expect(fixture.previewReads).toHaveLength(2);
});

test('opening an impact preview captures the current server revision rather than the stale displayed group', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page);
	await page.goto(fixture.detail);
	await expect(page.getByRole('heading', { name: 'กลุ่ม ก', exact: true })).toBeVisible();
	fixture.groups()[0].rowVersion = 4;
	fixture.graph.rowVersion = 2;
	await page.getByRole('button', { name: 'ลบกลุ่ม', exact: true }).click();
	const dialog = page.getByRole('alertdialog');
	await expect(dialog.getByRole('button', { name: confirmName, exact: true })).toBeEnabled();
	await dialog.getByRole('button', { name: confirmName, exact: true }).click();
	await expect(dialog).toHaveCount(0);
	const writes = fixture.writes.filter((row) => row.method === 'DELETE');
	expect(writes).toHaveLength(1);
	expect(writes[0].body).toEqual({
		academicYearId: fixture.ids.year,
		academicTermId: fixture.ids.term,
		rowVersion: 4,
		sourceDigest: '2'.padStart(64, '0')
	});
	expect(fixture.previewReads).toHaveLength(1);
	expect(fixture.groups().map((row) => row.id)).toEqual([fixture.ids.groupB]);
});
