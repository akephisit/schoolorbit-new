import { expect, test } from '@playwright/test';
import type {
	DatedRosterMembership,
	LearningGroupRosterTracking,
	UpdateRosterTrackingRequest
} from '../../src/lib/api/learning-delivery';
import { mockCurrentDelivery, deliveryReply, deferred } from './fixtures/current-delivery';
test.use({ serviceWorkers: 'block' });
const addName = 'เพิ่มนักเรียนตามวันที่';
function savedMember(group: string): DatedRosterMembership {
	return {
		id: 'c0000000-0000-4000-8000-000000000001',
		learningGroupId: group,
		studentAcademicYearId: 'c0000000-0000-4000-8000-000000000002',
		studentId: 'c0000000-0000-4000-8000-000000000003',
		displayName: 'นักเรียนที่มีประวัติจริง',
		studentCode: '123',
		membershipStatus: 'active',
		rosterSource: 'manual',
		joinedAt: '2026-10-01',
		leftAt: null,
		rowVersion: 1
	};
}

test('manual membership actions wait for accepted tracking while dated history remains readable', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page),
		held = deferred();
	let trackingReads = 0,
		historyReads = 0;
	await page.route(
		`**/api/academic/learning-groups/${fixture.ids.groupA}/roster-tracking`,
		async (route) => {
			trackingReads++;
			await held.promise;
			return deliveryReply(route, fixture.tracking.get(fixture.ids.groupA));
		}
	);
	await page.route(`**/api/academic/learning-groups/${fixture.ids.groupA}/memberships`, (route) => {
		historyReads++;
		return deliveryReply(route, [savedMember(fixture.ids.groupA)]);
	});
	try {
		await page.goto(fixture.detail);
		await expect(page.getByTestId('delivery-memberships-ready')).toContainText(
			'นักเรียนที่มีประวัติจริง'
		);
		await expect(page.getByRole('button', { name: addName, exact: true })).toHaveCount(0);
		expect(trackingReads).toBe(1);
		expect(historyReads).toBe(1);
	} finally {
		held.release();
	}
	await expect(page.getByRole('button', { name: addName, exact: true })).toBeVisible();
	expect(trackingReads).toBe(1);
	expect(historyReads).toBe(1);
	expect(fixture.writes).toEqual([]);
});

test('an unsaved manual mode grants no membership writes until its actual acknowledgment', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page),
		held = deferred();
	fixture.tracking.set(fixture.ids.groupA, {
		mode: 'homeroom',
		effectiveFrom: '2026-10-01',
		groupRowVersion: 3
	});
	let trackingReads = 0,
		historyReads = 0;
	const writes: UpdateRosterTrackingRequest[] = [];
	await page.route(`**/api/academic/learning-groups/${fixture.ids.groupA}/memberships`, (route) => {
		historyReads++;
		return deliveryReply(route, [savedMember(fixture.ids.groupA)]);
	});
	await page.route(
		`**/api/academic/learning-groups/${fixture.ids.groupA}/roster-tracking`,
		async (route) => {
			if (route.request().method() === 'GET') {
				trackingReads++;
				return deliveryReply(route, fixture.tracking.get(fixture.ids.groupA));
			}
			const body: UpdateRosterTrackingRequest = route.request().postDataJSON();
			writes.push(body);
			await held.promise;
			fixture.groups()[0].rowVersion = 4;
			const accepted: LearningGroupRosterTracking = {
				mode: body.mode,
				effectiveFrom: null,
				groupRowVersion: 4
			};
			fixture.tracking.set(fixture.ids.groupA, accepted);
			return deliveryReply(route, accepted);
		}
	);
	try {
		await page.goto(fixture.detail);
		await expect(page.getByTestId('delivery-memberships-ready')).toContainText(
			'นักเรียนที่มีประวัติจริง'
		);
		await expect(page.getByRole('button', { name: addName, exact: true })).toHaveCount(0);
		await page.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true }).click();
		await page.getByRole('option', { name: 'จัดรายชื่อเอง', exact: true }).click();
		await expect(page.getByRole('button', { name: addName, exact: true })).toHaveCount(0);
		await page.getByRole('button', { name: 'บันทึกวิธีจัดรายชื่อ', exact: true }).click();
		await expect.poll(() => writes.length).toBe(1);
		await expect(page.getByRole('button', { name: addName, exact: true })).toHaveCount(0);
	} finally {
		held.release();
	}
	await expect(page.getByRole('button', { name: addName, exact: true })).toBeVisible();
	await expect.poll(() => historyReads).toBe(2);
	expect(writes).toEqual([{ mode: 'manual', effectiveFrom: null, rowVersion: 3 }]);
	expect(trackingReads).toBe(1);
});

test('a teacher-only group revision does not revoke the accepted manual membership capability', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page),
		held = deferred();
	let reads = 0;
	await page.route(
		`**/api/academic/learning-groups/${fixture.ids.groupA}/roster-tracking`,
		async (route) => {
			reads++;
			if (reads > 1) await held.promise;
			return deliveryReply(route, fixture.tracking.get(fixture.ids.groupA));
		}
	);
	try {
		await page.goto(fixture.detail);
		await expect(page.getByRole('button', { name: addName, exact: true })).toBeVisible();
		await page.getByRole('button', { name: 'แก้ไขกลุ่มและครู', exact: true }).click();
		await page.getByRole('button', { name: 'บันทึกครูผู้สอน', exact: true }).click();
		await expect.poll(() => fixture.groups()[0].rowVersion).toBe(4);
		await expect(page.getByRole('button', { name: 'บันทึกครูผู้สอน', exact: true })).toBeEnabled();
		await expect(page.getByRole('button', { name: addName, exact: true })).toBeVisible();
		expect(fixture.tracking.get(fixture.ids.groupA)?.groupRowVersion).toBe(3);
		expect(fixture.writes.filter((row) => row.path.endsWith('/memberships'))).toEqual([]);
	} finally {
		held.release();
	}
});
