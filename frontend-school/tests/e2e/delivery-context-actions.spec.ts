import { expect, test, type Page, type Route } from '@playwright/test';
import type {
	AcademicTermChangeSet,
	DeliveryManagementOptions,
	LearningGroup,
	LearningOffering
} from '../../src/lib/api/learning-delivery.js';
import {
	mockDeliveryNavigation,
	navigationIds as ids,
	navigationOfferingIds as offerings,
	navigationVersion,
	navigationWorkspace,
	navigationSummary
} from './fixtures/delivery-navigation-route-data';

test.use({ serviceWorkers: 'block' });
const groupIds = [
	'a0000000-0000-4000-8000-000000000001',
	'a0000000-0000-4000-8000-000000000002',
	'a0000000-0000-4000-8000-000000000003'
];
const stamp = '2027-05-01T00:00:00Z';
async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		json: status < 400 ? { success: true, data } : { success: false, error: data }
	});
}

async function mockContextActions(
	page: Page,
	mode: 'course' | 'independent' | 'synchronized',
	options: {
		failDelete?: boolean;
		failRefresh?: boolean;
		saveGate?: Promise<void>;
		reader?: boolean;
		published?: boolean;
	} = {}
) {
	await mockDeliveryNavigation(page, { manager: !options.reader, readiness: 'course' });
	const offeringId = mode === 'course' ? offerings.basic21 : offerings.club;
	let graph = navigationVersion();
	const selected = graph.snapshot.offerings.find((item) => item.id === offeringId)!;
	if (selected.catalog.kind === 'activity' && mode !== 'course')
		selected.catalog.schedulingMode = mode;
	const registry: LearningGroup[] = groupIds.map((id, index) => ({
		id,
		learningOfferingId: offeringId,
		academicYearId: ids.year,
		academicTermId: ids.term,
		name: ['กลุ่มร่วมสองห้อง', 'กลุ่มที่คงไว้', 'กลุ่มที่ไม่อยู่ในรุ่น'][index],
		code: `G${index}`,
		description: null,
		capacity: 40,
		status: 'published',
		rosterStatus: 'draft',
		teachersLocked: true,
		rowVersion: 1,
		teacherAssignments: [],
		homeroomIds: index === 0 ? [ids.roomA, ids.roomB] : [ids.roomB],
		preferredRoomIds: [],
		createdAt: stamp,
		updatedAt: stamp,
		migrated: false
	}));
	selected.groups = registry.slice(0, 2).map((group) => ({
		id: group.id,
		name: group.name,
		capacity: group.capacity ?? null,
		homeroomIds: group.homeroomIds,
		preferredRoomIds: [],
		teachers: []
	}));
	graph = {
		...graph,
		rowVersion: 4,
		status: options.published ? 'published' : 'draft',
		effectiveFrom: options.published ? '2027-05-01' : null
	};
	let changeSet: AcademicTermChangeSet = {
		id: ids.changeSet,
		academicYearId: ids.year,
		academicTermId: ids.term,
		status: options.published ? 'published' : 'draft',
		rowVersion: 7,
		targetDeliveryVersionId: ids.version,
		effectiveFrom: null,
		referenceDate: '2027-05-01',
		reason: 'รุ่นสำหรับจัดกลุ่มภายหลัง',
		items: [],
		changes: [],
		offeringLabels: [],
		createdAt: stamp,
		updatedAt: stamp,
		createdBy: ids.actor
	};
	const writes: { path: string; method: string; body: unknown }[] = [];
	const reads: string[] = [];
	let failDelete = options.failDelete ?? false,
		failRefresh = false;
	let lastMutation = false;
	const management: DeliveryManagementOptions = {
		academicYearId: ids.year,
		academicTermId: ids.term,
		studyPrograms: [navigationWorkspace().homerooms[0].studyProgram],
		gradeLevels: [navigationWorkspace().homerooms[0].gradeLevel],
		homerooms: navigationWorkspace().homerooms.map((room) => room.homeroom),
		learningGroups: registry,
		teachers: [],
		rooms: [],
		catalogVersions: []
	};
	function resource(): LearningOffering {
		return {
			id: selected.id,
			kind: selected.kind,
			codeSnapshot: selected.code,
			nameSnapshot: selected.name,
			academicYearId: ids.year,
			academicTermId: ids.term,
			owningOrganizationUnitId: selected.owningOrganizationUnitId,
			status: 'published',
			rowVersion: 1,
			snapshot: selected.catalog,
			targets: [],
			startsOn: '2027-05-01',
			endsOn: null,
			stopReason: null,
			migrated: false,
			createdAt: stamp,
			updatedAt: stamp
		};
	}
	await page.route(
		(url) => url.pathname.startsWith('/api/academic/'),
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method();
			if (method === 'GET') reads.push(path);
			if (path === '/api/academic/delivery-versions')
				return reply(route, [
					{
						...navigationSummary(),
						status: graph.status,
						effectiveFrom: graph.effectiveFrom,
						rowVersion: graph.rowVersion,
						changeSetId: ids.changeSet,
						offeringCount: graph.snapshot.offerings.length,
						groupCount: graph.snapshot.offerings.reduce((sum, row) => sum + row.groups.length, 0)
					}
				]);
			if (path === `/api/academic/delivery-versions/${ids.version}`) {
				if (lastMutation && options.failRefresh && !failRefresh) {
					failRefresh = true;
					return reply(route, 'แสดงรุ่นล่าสุดไม่สำเร็จ', 503);
				}
				return reply(route, graph);
			}
			if (path === '/api/academic/term-change-sets') return reply(route, [changeSet]);
			if (path === `/api/academic/term-change-sets/${ids.changeSet}`)
				return reply(route, changeSet);
			if (path === '/api/academic/delivery/management-options') return reply(route, management);
			if (path === `/api/academic/offerings/${offeringId}`) return reply(route, resource());
			if (path === `/api/academic/offerings/${offeringId}/groups`) {
				if (method === 'POST') {
					const body = route.request().postDataJSON();
					writes.push({ path, method, body });
					const created: LearningGroup = {
						...registry[0],
						id: groupIds[2],
						name: body.name,
						status: 'draft',
						teachersLocked: false,
						homeroomIds: []
					};
					registry.splice(2, 1, created);
					graph.snapshot.offerings
						.find((item) => item.id === offeringId)!
						.groups.push({
							id: created.id,
							name: created.name,
							capacity: null,
							homeroomIds: [],
							preferredRoomIds: [],
							teachers: []
						});
					graph.rowVersion++;
					lastMutation = true;
					return reply(route, created);
				}
				return reply(route, registry);
			}
			const group = registry.find((item) => path === `/api/academic/learning-groups/${item.id}`);
			if (group) return reply(route, group);
			if (path.endsWith('/roster-tracking'))
				return reply(route, {
					mode: 'manual',
					startsOn: '2027-05-01',
					endsOn: '2028-03-31',
					groupRowVersion: 1
				});
			if (path.endsWith('/memberships')) return reply(route, []);
			if (
				method === 'DELETE' &&
				path.startsWith(`/api/academic/term-change-sets/${ids.changeSet}/`)
			) {
				const body = route.request().postDataJSON();
				writes.push({ path, method, body });
				expect(body).toEqual({
					changeSetRowVersion: changeSet.rowVersion,
					targetDeliveryVersionRowVersion: graph.rowVersion
				});
				if (failDelete) {
					failDelete = false;
					changeSet = { ...changeSet, rowVersion: changeSet.rowVersion + 1 };
					graph = { ...graph, rowVersion: graph.rowVersion + 1 };
					return reply(route, 'แบบร่างถูกแก้ไข กรุณาตรวจขอบเขตล่าสุด', 409);
				}
				await options.saveGate;
				if (path.includes('/groups/'))
					graph = {
						...graph,
						rowVersion: graph.rowVersion + 1,
						snapshot: {
							offerings: graph.snapshot.offerings.map((item) =>
								item.id === offeringId
									? { ...item, groups: item.groups.filter((group) => !path.endsWith(group.id)) }
									: item
							)
						}
					};
				else
					graph = {
						...graph,
						rowVersion: graph.rowVersion + 1,
						snapshot: {
							offerings: graph.snapshot.offerings.filter((item) => item.id !== offeringId)
						}
					};
				changeSet = { ...changeSet, rowVersion: changeSet.rowVersion + 1 };
				lastMutation = true;
				return reply(route, changeSet);
			}
			if (path === '/api/academic/delivery/homerooms') {
				const workspace = navigationWorkspace();
				for (const room of workspace.homerooms)
					for (const item of room.items)
						if (!graph.snapshot.offerings.some((row) => row.id === item.offeringId)) {
							item.offeringId = null;
							item.offeringState = 'missing';
							item.groups = [];
						}
				return reply(route, workspace);
			}
			return route.fallback();
		}
	);
	const origin = `/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}&view=offerings&kind=${mode === 'course' ? 'course' : 'activity'}&offeringSearch=${encodeURIComponent(selected.name)}&expandedRooms=${ids.roomB}`;
	const detail = `/staff/academic/delivery/${offeringId}?academicYearId=${ids.year}&academicTermId=${ids.term}&deliveryVersionId=${ids.version}&groupId=${groupIds[0]}&returnTo=${encodeURIComponent(origin)}`;
	return { origin, detail, offeringId, writes, reads, graph: () => graph };
}

for (const mode of ['independent', 'synchronized'] as const)
	for (const width of [390, 1280])
		for (const dark of [false, true]) {
			test(`${mode} activity groups can be removed individually, added later, and the activity removed at ${width}px ${dark ? 'dark' : 'light'}`, async ({
				page
			}) => {
				await page.setViewportSize({ width, height: 844 });
				const fixture = await mockContextActions(page, mode);
				await page.goto(fixture.detail);
				if (dark) await page.getByRole('button', { name: 'Toggle Dark Mode', exact: true }).click();
				await expect(page.getByText('กลุ่มที่ไม่อยู่ในรุ่น', { exact: true })).toHaveCount(0);
				for (const name of ['กลุ่มร่วมสองห้อง', 'กลุ่มที่คงไว้']) {
					const trigger = page.getByRole('button', {
						name: `ลบกลุ่มจากรุ่นนี้ · ${name}`,
						exact: true
					});
					await trigger.focus();
					await page.keyboard.press('Enter');
					const confirmation = page.getByRole('alertdialog', {
						name: 'ลบกลุ่มจากรุ่นนี้',
						exact: true
					});
					await expect(confirmation).toContainText('กลุ่มอื่นยังอยู่');
					if (name === 'กลุ่มร่วมสองห้อง') {
						await expect(confirmation).toContainText('ม.1/1');
						await expect(confirmation).toContainText('ม.1/2');
					}
					if (mode === 'synchronized')
						await expect(confirmation).toContainText('กิจกรรมคาบพร้อมกัน');
					await confirmation
						.getByRole('button', { name: 'ยืนยันลบกลุ่มจากรุ่นนี้', exact: true })
						.click();
					await expect(confirmation).toHaveCount(0);
					await expect(
						page.getByRole('button', { name: `ลบกลุ่มจากรุ่นนี้ · ${name}`, exact: true })
					).toHaveCount(0);
				}
				await expect(page).not.toHaveURL(/groupId=/);
				await expect(
					page.getByText('ยังไม่มีกลุ่มเรียน เริ่มจากเพิ่มกลุ่มตามห้องเรียนหรือชุดผู้เรียนจริง')
				).toBeVisible();
				await expect(
					page.getByRole('button', { name: 'ลบจากรุ่นนี้ · ชุมนุม', exact: true })
				).toBeVisible();
				await page.getByRole('button', { name: 'เพิ่มกลุ่ม', exact: true }).click();
				const create = page.getByRole('dialog', { name: 'เพิ่มกลุ่มเรียน', exact: true });
				await create.getByLabel('ชื่อกลุ่ม', { exact: true }).fill('กลุ่มที่เพิ่มภายหลัง');
				await create.getByRole('button', { name: 'สร้างกลุ่มเรียน', exact: true }).click();
				await expect(create).toHaveCount(0);
				await expect(
					page.getByRole('button', {
						name: 'ลบกลุ่มจากรุ่นนี้ · กลุ่มที่เพิ่มภายหลัง',
						exact: true
					})
				).toBeVisible();
				await page.getByRole('button', { name: 'ลบจากรุ่นนี้ · ชุมนุม', exact: true }).click();
				const remove = page.getByRole('alertdialog', { name: 'ลบจากรุ่นนี้', exact: true });
				await expect(remove).toContainText('พร้อมทุกกลุ่มเรียน 1 กลุ่ม');
				await remove.getByRole('button', { name: 'ยืนยันลบจากรุ่นนี้', exact: true }).click();
				await expect(page).toHaveURL(new URL(fixture.origin, page.url()).href);
				await expect(page.getByTestId('delivery-offerings-ready')).not.toContainText('ชุมนุม');
				expect(fixture.writes.map((write) => write.method)).toEqual([
					'DELETE',
					'DELETE',
					'POST',
					'DELETE'
				]);
				expect(
					fixture.graph().snapshot.offerings.some((item) => item.id === fixture.offeringId)
				).toBe(false);
				expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
					true
				);
			});
		}

test('removal conflicts require a fresh scope, cancellation restores focus, and a saved mutation only retries its read', async ({
	page
}) => {
	const fixture = await mockContextActions(page, 'course', { failDelete: true, failRefresh: true });
	await page.goto(fixture.detail);
	const trigger = page.getByRole('button', {
		name: 'ลบกลุ่มจากรุ่นนี้ · กลุ่มร่วมสองห้อง',
		exact: true
	});
	await trigger.click();
	const dialog = page.getByRole('alertdialog', { name: 'ลบกลุ่มจากรุ่นนี้', exact: true });
	await expect(
		dialog.getByRole('button', { name: 'ยืนยันลบกลุ่มจากรุ่นนี้', exact: true })
	).toBeEnabled();
	await page.keyboard.press('Escape');
	await expect(dialog).toHaveCount(0);
	await expect(trigger).toBeFocused();
	await trigger.click();
	await dialog.getByRole('button', { name: 'ยืนยันลบกลุ่มจากรุ่นนี้', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('แบบร่างถูกแก้ไข');
	await expect(
		dialog.getByRole('button', { name: 'ยืนยันลบกลุ่มจากรุ่นนี้', exact: true })
	).toHaveCount(0);
	await dialog.getByRole('button', { name: 'ตรวจขอบเขตล่าสุดอีกครั้ง', exact: true }).click();
	await dialog.getByRole('button', { name: 'ยืนยันลบกลุ่มจากรุ่นนี้', exact: true }).click();
	await expect(dialog.getByRole('status')).toContainText('นำออกจากแบบร่างแล้ว');
	await expect(dialog.getByRole('alert')).toContainText('แสดงรุ่นล่าสุดไม่สำเร็จ');
	expect(fixture.writes).toHaveLength(2);
	await dialog.getByRole('button', { name: 'แสดงข้อมูลล่าสุด', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(fixture.writes).toHaveLength(2);
	await expect(page).toHaveURL(new RegExp(`groupId=${groupIds[1]}`));
});

test('pending deletion prevents dismissal and duplicate writes', async ({ page }) => {
	let release!: () => void;
	const gate = new Promise<void>((resolve) => (release = resolve));
	const fixture = await mockContextActions(page, 'independent', { saveGate: gate });
	try {
		await page.goto(fixture.detail);
		await page.getByRole('button', { name: 'ลบจากรุ่นนี้ · ชุมนุม', exact: true }).click();
		const dialog = page.getByRole('alertdialog', { name: 'ลบจากรุ่นนี้', exact: true });
		await dialog.getByRole('button', { name: 'ยืนยันลบจากรุ่นนี้', exact: true }).click();
		await expect(dialog.getByRole('button', { name: 'กำลังนำออก', exact: true })).toBeDisabled();
		await page.keyboard.press('Escape');
		await expect(dialog).toBeVisible();
		await expect(dialog.getByRole('button', { name: 'ยกเลิก', exact: true })).toBeDisabled();
		expect(fixture.writes).toHaveLength(1);
	} finally {
		release();
	}
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
});

for (const published of [false, true])
	test(`removal stays hidden for ${published ? 'published versions' : 'read-only staff'}`, async ({
		page
	}) => {
		const fixture = await mockContextActions(page, 'independent', {
			published,
			reader: !published
		});
		await page.goto(fixture.detail);
		await expect(
			page.getByTestId(published ? 'delivery-version-offering' : 'delivery-offering-ready')
		).toBeVisible();
		await expect(page.getByRole('button', { name: /^ลบ(กลุ่ม)?จากรุ่นนี้/ })).toHaveCount(0);
		expect(fixture.writes).toEqual([]);
	});

test('a successful group creation with a failed projection read retries only the read', async ({
	page
}) => {
	const fixture = await mockContextActions(page, 'independent', { failRefresh: true });
	await page.goto(fixture.detail);
	await page.getByRole('button', { name: 'เพิ่มกลุ่ม', exact: true }).click();
	const create = page.getByRole('dialog', { name: 'เพิ่มกลุ่มเรียน', exact: true });
	await create.getByLabel('ชื่อกลุ่ม', { exact: true }).fill('กลุ่มที่สร้างสำเร็จ');
	await create.getByRole('button', { name: 'สร้างกลุ่มเรียน', exact: true }).click();
	await expect(create).toHaveCount(0);
	await expect(page.getByRole('status')).toContainText('สร้างกลุ่ม “กลุ่มที่สร้างสำเร็จ” แล้ว');
	await expect(page.getByRole('button', { name: 'เพิ่มกลุ่ม', exact: true })).toBeDisabled();
	expect(fixture.writes.filter((write) => write.method === 'POST')).toHaveLength(1);
	await page.getByRole('button', { name: 'แสดงกลุ่มที่สร้างแล้ว', exact: true }).click();
	await expect(
		page.getByRole('button', { name: 'ลบกลุ่มจากรุ่นนี้ · กลุ่มที่สร้างสำเร็จ', exact: true })
	).toBeVisible();
	expect(fixture.writes.filter((write) => write.method === 'POST')).toHaveLength(1);
});

test('nearby add actions filter pinned basic and additional types and load options only after opening', async ({
	page
}) => {
	const fixture = await mockContextActions(page, 'course');
	const catalogs: DeliveryManagementOptions['catalogVersions'] = [
		{
			id: offerings.basic21,
			kind: 'course',
			subjectType: 'BASIC',
			code: 'ค21101',
			name: 'วิชาพื้นฐานทดสอบ',
			label: 'ค21101 — วิชาพื้นฐานทดสอบ',
			versionNo: 1,
			standardPeriodsPerWeek: 2,
			schedulingMode: null
		},
		{
			id: offerings.additional,
			kind: 'course',
			subjectType: 'ADDITIONAL',
			code: 'ค20101',
			name: 'วิชาเพิ่มเติมทดสอบ',
			label: 'ค20101 — วิชาเพิ่มเติมทดสอบ',
			versionNo: 1,
			standardPeriodsPerWeek: 2,
			schedulingMode: null
		},
		{
			id: offerings.unknown,
			kind: 'course',
			subjectType: null,
			code: 'ค99999',
			name: 'วิชาไม่ระบุประเภท',
			label: 'ค99999 — วิชาไม่ระบุประเภท',
			versionNo: 1,
			standardPeriodsPerWeek: 2,
			schedulingMode: null
		}
	];
	let optionReads = 0;
	await page.route('**/api/academic/delivery/management-options?*', (route) => {
		optionReads++;
		return reply(route, {
			academicYearId: ids.year,
			academicTermId: ids.term,
			catalogVersions: catalogs,
			homerooms: [],
			gradeLevels: [],
			studyPrograms: [],
			learningGroups: [],
			teachers: [],
			rooms: []
		} satisfies DeliveryManagementOptions);
	});
	await page.goto(fixture.origin);
	await expect(page.getByTestId('delivery-offerings-ready')).toBeVisible();
	expect(optionReads).toBe(0);
	await expect(page.getByRole('button', { name: 'เพิ่ม/ปรับรายการสอน', exact: true })).toHaveCount(
		0
	);
	await expect(page.getByRole('button', { name: 'เปลี่ยนครูผู้สอน', exact: true })).toHaveCount(0);
	for (const [label, expected] of [
		['เพิ่มวิชาพื้นฐาน', 'วิชาพื้นฐานทดสอบ'],
		['เพิ่มวิชาเพิ่มเติม', 'วิชาเพิ่มเติมทดสอบ']
	]) {
		const trigger = page.getByRole('button', { name: label, exact: true });
		await trigger.click();
		const dialog = page.getByRole('dialog', { name: label, exact: true });
		await dialog.getByRole('combobox', { name: 'เลือกรายวิชา', exact: true }).click();
		await expect(page.getByRole('option')).toHaveCount(1);
		await expect(page.getByRole('option')).toContainText(expected);
		await page.keyboard.press('Escape');
		await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
		await expect(trigger).toBeFocused();
	}
	expect(optionReads).toBe(1);
});

test('touch removal cancellation restores the underlying detail and scroll controls', async ({
	browser,
	baseURL
}) => {
	const context = await browser.newContext({
		baseURL,
		hasTouch: true,
		isMobile: true,
		viewport: { width: 390, height: 844 },
		serviceWorkers: 'block'
	});
	try {
		const page = await context.newPage();
		const fixture = await mockContextActions(page, 'independent');
		await page.goto(fixture.detail);
		const trigger = page.getByRole('button', {
			name: 'ลบกลุ่มจากรุ่นนี้ · กลุ่มร่วมสองห้อง',
			exact: true
		});
		await trigger.tap();
		const confirmation = page.getByRole('alertdialog', { name: 'ลบกลุ่มจากรุ่นนี้', exact: true });
		await expect(
			confirmation.getByRole('button', { name: 'ยืนยันลบกลุ่มจากรุ่นนี้', exact: true })
		).toBeEnabled();
		await confirmation.getByRole('button', { name: 'ยกเลิก', exact: true }).tap();
		await expect(confirmation).toHaveCount(0);
		await expect(trigger).toBeFocused();
		await page.getByRole('button', { name: 'เพิ่มกลุ่ม', exact: true }).tap();
		const create = page.getByRole('dialog', { name: 'เพิ่มกลุ่มเรียน', exact: true });
		await expect(create).toBeVisible();
		await create.getByRole('button', { name: 'ยกเลิก', exact: true }).tap();
		const main = page.getByRole('main');
		await main.evaluate((element) => {
			element.scrollTop = 0;
		});
		await page.mouse.move(370, 600);
		await page.mouse.wheel(0, 500);
		await expect.poll(() => main.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
		expect(fixture.writes).toHaveLength(0);
	} finally {
		await context.close();
	}
});
