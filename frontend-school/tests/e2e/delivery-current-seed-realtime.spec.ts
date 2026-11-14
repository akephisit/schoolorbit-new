import { expect, test } from '@playwright/test';
import { mockCurrentDelivery, deliveryReply } from './fixtures/current-delivery';
import type { DatedRosterMembership } from '../../src/lib/api/learning-delivery';
import { startNativeAcademicEvents } from './fixtures/native-academic-events';
test.use({ serviceWorkers: 'block' });

test('default-route seeds belong only to their accepted group even when its revision is higher', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page);
	fixture.groups()[0].rowVersion = 9;
	fixture.tracking.set(fixture.ids.groupA, {
		mode: 'manual',
		effectiveFrom: null,
		groupRowVersion: 9
	});
	const memberships = new Map<string, DatedRosterMembership[]>(
		fixture
			.groups()
			.map((group, index) => [
				group.id,
				[
					{
						id: `b0000000-0000-4000-8000-00000000000${index + 1}`,
						learningGroupId: group.id,
						studentAcademicYearId: `b0000000-0000-4000-8000-00000000001${index + 1}`,
						studentId: `b0000000-0000-4000-8000-00000000002${index + 1}`,
						displayName: index ? 'นักเรียนกลุ่ม ข' : 'นักเรียนกลุ่ม ก',
						studentCode: index ? '002' : '001',
						membershipStatus: 'active',
						rosterSource: 'manual',
						joinedAt: '2026-10-01',
						leftAt: null,
						rowVersion: 1
					}
				]
			])
	);
	const reads: string[] = [];
	await page.route('**/api/academic/learning-groups/*/memberships', (route) => {
		const groupId = new URL(route.request().url()).pathname.split('/')[4];
		reads.push(groupId);
		return deliveryReply(route, memberships.get(groupId));
	});
	await page.goto(fixture.detail);
	await expect(page.getByRole('heading', { name: 'กลุ่ม ก', exact: true })).toBeVisible();
	await expect(
		page
			.getByTestId('roster-tracking')
			.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })
	).toHaveText('จัดรายชื่อเอง');
	await expect(page.getByTestId('delivery-memberships-ready')).toContainText('นักเรียนกลุ่ม ก');
	expect(fixture.count(`/api/academic/learning-groups/${fixture.ids.groupA}`)).toBe(0);
	expect(fixture.count(`/api/academic/learning-groups/${fixture.ids.groupA}/roster-tracking`)).toBe(
		1
	);
	await page.getByRole('button', { name: /^กลุ่ม ข ครู / }).click();
	await expect(page.getByRole('heading', { name: 'กลุ่ม ข', exact: true })).toBeVisible();
	await expect(
		page
			.getByTestId('roster-tracking')
			.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })
	).toHaveText('ติดตามห้องอัตโนมัติ');
	await expect(page.getByTestId('delivery-memberships-ready')).toContainText('นักเรียนกลุ่ม ข');
	await expect(page.getByTestId('delivery-memberships-ready')).not.toContainText('นักเรียนกลุ่ม ก');
	expect(fixture.count(`/api/academic/learning-groups/${fixture.ids.groupB}/roster-tracking`)).toBe(
		1
	);
	expect(reads).toEqual([fixture.ids.groupA, fixture.ids.groupB]);
	fixture.tracking.set(fixture.ids.groupA, {
		mode: 'homeroom',
		effectiveFrom: '2026-11-11',
		groupRowVersion: 9
	});
	memberships.get(fixture.ids.groupA)![0].displayName = 'ข้อมูลกลุ่ม ก ที่อ่านใหม่';
	await page.getByRole('button', { name: /^กลุ่ม ก ครู / }).click();
	await expect(
		page
			.getByTestId('roster-tracking')
			.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })
	).toHaveText('ติดตามห้องอัตโนมัติ');
	await expect(
		page
			.getByTestId('roster-tracking')
			.getByRole('button', { name: 'วันที่เริ่มติดตาม', exact: true })
	).toContainText('11 พ.ย. 2569');
	await expect(page.getByTestId('delivery-memberships-ready')).toContainText(
		'ข้อมูลกลุ่ม ก ที่อ่านใหม่'
	);
	expect(reads).toEqual([fixture.ids.groupA, fixture.ids.groupB, fixture.ids.groupA]);
	expect(fixture.count(`/api/academic/learning-groups/${fixture.ids.groupA}/roster-tracking`)).toBe(
		2
	);
	expect(fixture.writes).toEqual([]);
});

test('a nonzero realtime counter is the new route baseline rather than a duplicate primary load', async ({
	page
}) => {
	const fixture = await mockCurrentDelivery(page),
		relay = await startNativeAcademicEvents();
	await page.addInitScript(
		({ relayUrl, term }) => {
			const Native = window.WebSocket;
			let serial = 0;
			window.WebSocket = class extends Native {
				constructor(address: string | URL, protocols?: string | string[]) {
					const original = new URL(String(address), location.href);
					if (original.pathname !== '/ws/timetable')
						throw new Error('unexpected realtime resource in academic fixture');
					const target = new URL(relayUrl),
						connectionId = String(++serial);
					target.search = original.search;
					target.searchParams.set('fixtureConnectionId', connectionId);
					super(target, protocols);
					if (original.searchParams.get('academicTermId') === term) {
						delete document.documentElement.dataset.generalTimetableSync;
						this.addEventListener('message', (event) => {
							const value: unknown = JSON.parse(String(event.data));
							if (
								typeof value === 'object' &&
								value !== null &&
								'type' in value &&
								value.type === 'StateSync'
							)
								requestAnimationFrame(() => {
									if (this.readyState === Native.OPEN && this.onmessage !== null)
										document.documentElement.dataset.generalTimetableSync = connectionId;
								});
						});
						this.addEventListener('close', () => {
							if (document.documentElement.dataset.generalTimetableSync === connectionId)
								delete document.documentElement.dataset.generalTimetableSync;
						});
					}
				}
			};
		},
		{ relayUrl: relay.url, term: fixture.ids.term }
	);
	const signal = (seq: number, term = fixture.ids.term) =>
		relay.send(fixture.ids.term, {
			seq,
			type: 'LearningDeliveryChanged',
			payload: {
				user_id: fixture.ids.teacherB,
				academic_term_id: term,
				learning_offering_id: fixture.resource().id,
				learning_group_id: null,
				revision: seq
			}
		});
	const currentBaseline = async () => {
		await expect
			.poll(async () => {
				const witness = await page.locator('html').getAttribute('data-general-timetable-sync');
				return witness !== null && relay.liveConnectionIds(fixture.ids.term).includes(witness);
			})
			.toBe(true);
	};
	try {
		await page.goto(fixture.overview);
		await expect(page.getByTestId('delivery-offerings-ready')).toBeVisible();
		await currentBaseline();
		signal(1);
		await expect.poll(() => fixture.count('/api/academic/delivery/workspace')).toBe(2);
		const connected = relay.connections();
		await page
			.getByTestId('delivery-offerings-ready')
			.getByRole('link', { name: 'รายละเอียด', exact: true })
			.click();
		await expect(page.getByRole('heading', { name: 'กลุ่ม ก', exact: true })).toBeVisible();
		await expect(
			page
				.getByTestId('roster-tracking')
				.getByRole('combobox', { name: 'วิธีจัดรายชื่อ', exact: true })
		).toHaveText('จัดรายชื่อเอง');
		const offeringPath = `/api/academic/offerings/${fixture.resource().id}`,
			groupsPath = `${offeringPath}/groups`;
		expect(fixture.count(offeringPath)).toBe(1);
		expect(fixture.count(groupsPath)).toBe(1);
		expect(
			fixture.count(`/api/academic/learning-groups/${fixture.ids.groupA}/roster-tracking`)
		).toBe(1);
		await expect.poll(() => relay.connections()).toBeGreaterThan(connected);
		await currentBaseline();
		signal(2);
		await expect.poll(() => fixture.count(offeringPath)).toBe(2);
		await expect.poll(() => fixture.count(groupsPath)).toBe(2);
		signal(3, fixture.ids.activeTerm);
		await page.evaluate(
			() =>
				new Promise<void>((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
				)
		);
		expect(fixture.count(offeringPath)).toBe(2);
		expect(fixture.count(groupsPath)).toBe(2);
		expect(fixture.writes).toEqual([]);
	} finally {
		await relay.close();
	}
});
