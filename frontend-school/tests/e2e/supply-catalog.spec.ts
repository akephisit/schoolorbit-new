import { test, expect, type Page, type Route } from '@playwright/test';
import { mockStaffHome, id, actor } from './fixtures/staff-home-route-data';
import type {
	SupplyItem,
	SupplyItemDetail,
	SupplyProposal,
	SupplyCategory
} from '../../src/lib/api/supply-catalog.js';
import { subtotal, sumPrices, matchesItem } from '../../src/lib/supply-catalog/drafts.js';
const categoryId = id(400),
	secondCategoryId = id(401),
	itemId = id(410),
	proposalId = id(420);
const read = 'supply_catalog.read.school',
	manage = 'supply_catalog.manage.school',
	propose = 'supply_catalog.request.own',
	approve = 'supply_catalog.approve.school';
const categories: SupplyCategory[] = [
	{ id: categoryId, name: 'สำนักงาน', revision: 1 },
	{ id: secondCategoryId, name: 'อุปกรณ์เรียน', revision: 1 }
];
function sampleItem(): SupplyItem {
	return {
		id: itemId,
		name: 'กระดาษ A4',
		description: 'กระดาษสำหรับงานเอกสาร',
		categoryId,
		categoryName: 'สำนักงาน',
		unit: 'รีม',
		imageFileId: null,
		favorite: false,
		revision: 3,
		latestVersion: {
			id: id(411),
			version: 2,
			price: '125.50',
			shop: 'ร้านทดสอบ',
			revision: 4,
			createdAt: '2026-10-01T00:00:00Z',
			updatedAt: '2026-10-01T00:00:00Z'
		}
	};
}
function sampleProposal(): SupplyProposal {
	return {
		id: proposalId,
		name: 'ดินสอ',
		description: 'ดินสอสำหรับนักเรียน',
		categoryId,
		unit: 'กล่อง',
		imageFileId: null,
		price: '45.00',
		shop: 'ร้านเดิม',
		sourceUrl: 'https://shop.example.invalid/pencil',
		sourceFileId: null,
		status: 'pending',
		proposedBy: actor,
		reviewedBy: null,
		reviewedAt: null,
		rejectionReason: null,
		itemId: null,
		revision: 2,
		createdAt: '2026-10-01T00:00:00Z'
	};
}
test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
async function catalog(
	page: Page,
	permissions: string[],
	options: {
		failCreate?: boolean;
		failProposal?: boolean;
		holdItems?: boolean;
		longHistory?: boolean;
	} = {}
) {
	await mockStaffHome(page, { permissions });
	const reads: string[] = [],
		writes: { path: string; method: string; payload: Record<string, unknown> }[] = [],
		uploads: string[] = [],
		deleted: string[] = [];
	let item = sampleItem(),
		proposal = sampleProposal(),
		fileNumber = 450,
		createFailed = false,
		proposalFailed = false;
	let releaseItems = () => {};
	const hold = new Promise<void>((resolve) => (releaseItems = resolve));
	let versions = [
		item.latestVersion,
		{ ...item.latestVersion, id: id(412), version: 1, price: '120.00', revision: 1 }
	];
	if (options.longHistory) {
		versions = Array.from({ length: 102 }, (_, index) => ({
			...item.latestVersion,
			id: id(600 + index),
			version: 102 - index
		}));
		item = { ...item, latestVersion: versions[0] };
	}
	const detail = (): SupplyItemDetail => ({
		...item,
		versions: versions.slice(0, 100),
		versionsTotal: versions.length
	});
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			headers: { 'X-CSRF-Token': 'synthetic-csrf' },
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route('**/api/supply-catalog/**', async (route) => {
		const req = route.request(),
			url = new URL(req.url()),
			path = url.pathname,
			method = req.method();
		if (method === 'GET') {
			reads.push(path + url.search);
			if (path.endsWith('/categories')) return reply(route, categories);
			if (path.endsWith('/items')) {
				if (options.holdItems) await hold;
				const matches = matchesItem(item, {
					search: url.searchParams.get('search') ?? '',
					categoryId: url.searchParams.get('categoryId') ?? '',
					favoritesOnly: url.searchParams.get('favoritesOnly') === 'true'
				});
				return reply(route, {
					items: matches ? [item] : [],
					total: matches ? 1 : 0,
					page: Number(url.searchParams.get('page') ?? 1),
					pageSize: Number(url.searchParams.get('pageSize') ?? 24)
				});
			}
			if (path.endsWith('/duplicates')) return reply(route, []);
			if (path === `/api/supply-catalog/items/${itemId}`) return reply(route, detail());
			if (path === `/api/supply-catalog/items/${itemId}/versions`) {
				const pageNumber = Number(url.searchParams.get('page') ?? 1),
					size = Number(url.searchParams.get('pageSize') ?? 100);
				return reply(route, {
					items: versions.slice((pageNumber - 1) * size, pageNumber * size),
					total: versions.length,
					page: pageNumber,
					pageSize: size
				});
			}
			if (path.endsWith('/bundles'))
				return reply(route, { items: [], total: 0, page: 1, pageSize: 10 });
			if (path.endsWith('/proposals'))
				return reply(route, {
					items:
						proposal.status === 'pending' || url.searchParams.get('review') !== 'true'
							? [proposal]
							: [],
					total: 1,
					page: 1,
					pageSize: 20
				});
			if (path === `/api/supply-catalog/proposals/${proposalId}`) return reply(route, proposal);
		}
		const payload = req.postDataJSON();
		writes.push({ path, method, payload });
		if (path.endsWith('/favorite')) {
			item = { ...item, favorite: payload.favorite };
			return reply(route, item);
		}
		if (path.endsWith('/items') && method === 'POST') {
			if (options.failCreate && !createFailed) {
				createFailed = true;
				return reply(route, 'บันทึกไม่สำเร็จ กรุณาตรวจสอบก่อนส่งอีกครั้ง', 503);
			}
			item = { ...item, ...payload };
			return reply(route, item, 201);
		}
		if (path.includes('/versions')) {
			const append = method === 'POST';
			if (append) {
				item = {
					...item,
					revision: item.revision + 1,
					imageFileId: payload.imageFileId,
					latestVersion: {
						...item.latestVersion,
						id: id(413),
						version: item.latestVersion.version + 1,
						price: payload.price,
						shop: payload.shop,
						revision: item.latestVersion.revision + 1
					}
				};
				versions = [item.latestVersion, ...versions];
			} else {
				const targetId = path.split('/').at(-1);
				versions = versions.map((version) =>
					version.id === targetId
						? {
								...version,
								price: payload.price,
								shop: payload.shop,
								revision: version.revision + 1
							}
						: version
				);
				item = {
					...item,
					revision: item.revision + 1,
					latestVersion:
						versions.find((version) => version.id === item.latestVersion.id) ?? item.latestVersion
				};
			}
			return reply(route, detail());
		}
		if (path === `/api/supply-catalog/items/${itemId}`) {
			item = { ...item, ...payload, revision: item.revision + 1 };
			return reply(route, item);
		}
		if (path.endsWith('/approve')) {
			item = {
				...item,
				...payload,
				latestVersion: { ...item.latestVersion, price: payload.price, shop: payload.shop }
			};
			proposal = {
				...proposal,
				status: 'approved',
				sourceFileId: null,
				sourceUrl: null,
				imageFileId: null,
				itemId: item.id
			};
			return reply(route, detail());
		}
		if (path.endsWith('/reject')) {
			proposal = { ...proposal, status: 'rejected', rejectionReason: payload.reason };
			return reply(route, proposal);
		}
		if (path.endsWith('/proposals') && method === 'POST') {
			if (options.failProposal && !proposalFailed) {
				proposalFailed = true;
				return reply(route, 'ส่งข้อเสนอไม่สำเร็จ', 503);
			}
			proposal = { ...proposal, ...payload };
			return reply(route, proposal, 201);
		}
		if (path.endsWith('/bundles')) {
			const entries = payload.entries.map((entry: { itemId: string; quantity: number }) => ({
				item,
				quantity: entry.quantity,
				subtotal: subtotal(item.latestVersion.price, entry.quantity)
			}));
			return reply(
				route,
				{
					id: id(430),
					name: payload.name,
					shared: payload.shared,
					ownerUserId: actor,
					entries,
					revision: 1,
					totalPrice: sumPrices(entries.map((entry: { subtotal: string }) => entry.subtotal))
				},
				201
			);
		}
		return reply(route, {}, 200);
	});
	await page.route('**/api/files**', async (route) => {
		const req = route.request(),
			path = new URL(req.url()).pathname;
		if (req.method() === 'POST' && path === '/api/files') {
			const body = req.postData() ?? '';
			uploads.push(body.includes('supply_proposal_source') ? 'source' : 'image');
			return reply(
				route,
				{
					id: id(fileNumber++),
					filename: 'test.png',
					mimeType: 'image/png',
					size: 68,
					purpose: uploads.at(-1) === 'source' ? 'supply_proposal_source' : 'supply_catalog_image'
				},
				201
			);
		}
		if (req.method() === 'DELETE') {
			deleted.push(path);
			return reply(route, { id: path.split('/').at(-1), deleted: true });
		}
		if (path.endsWith('/download'))
			return reply(route, {
				url: 'https://files.example.invalid/synthetic-image',
				expiresAt: '2026-10-15T00:00:00Z',
				filename: 'test.png',
				mimeType: 'image/png'
			});
		return reply(route, {});
	});
	await page.route('https://files.example.invalid/synthetic-image', (route) =>
		route.fulfill({
			status: 200,
			contentType: 'image/png',
			body: Buffer.from(
				'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+yL9sAAAAASUVORK5CYII=',
				'base64'
			)
		})
	);
	return { reads, writes, uploads, deleted, releaseItems };
}
const png = {
	name: 'test.png',
	mimeType: 'image/png',
	buffer: Buffer.from(
		'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+yL9sAAAAASUVORK5CYII=',
		'base64'
	)
};
for (const width of [1280, 375])
	test(`reader catalog filters favorites and lazy detail at ${width}px`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 900 });
		const api = await catalog(page, [read]);
		await page.goto('/staff/supply-catalog');
		await expect(page.getByTestId('supply-catalog')).toContainText('กระดาษ A4');
		await expect(page.getByRole('button', { name: 'เพิ่มพัสดุ', exact: true })).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'จัดการหมวดหมู่', exact: true })).toHaveCount(0);
		await expect(page.getByRole('link', { name: 'ข้อเสนอพัสดุ', exact: true })).toHaveCount(0);
		expect(
			api.reads.filter(
				(path) =>
					path.includes('/bundles') ||
					path.includes('/proposals') ||
					path.includes(`/items/${itemId}`)
			)
		).toEqual([]);
		await page.getByRole('button', { name: 'เพิ่ม กระดาษ A4 เป็นรายการโปรด', exact: true }).click();
		await expect(
			page.getByRole('button', { name: 'นำ กระดาษ A4 ออกจากรายการโปรด', exact: true })
		).toHaveAttribute('aria-pressed', 'true');
		await page.getByLabel('รายการโปรด', { exact: true }).check();
		await page.getByLabel('ค้นหาพัสดุ', { exact: true }).fill('กระดาษ');
		await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
		await expect
			.poll(() =>
				api.reads.some((path) => path.includes('favoritesOnly=true') && path.includes('search='))
			)
			.toBe(true);
		await page.getByRole('button', { name: 'ดูรายละเอียด', exact: true }).click();
		const dialog = page.getByRole('dialog');
		await expect(dialog).toContainText('ประวัติราคา (2 รุ่น)');
		await expect(dialog.getByRole('button', { name: 'เพิ่มรุ่นราคา', exact: true })).toHaveCount(0);
		await page.keyboard.press('Tab');
		expect(await dialog.evaluate((el) => el.contains(document.activeElement))).toBe(true);
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
		await page.screenshot({ path: testInfo.outputPath(`catalog-${width}-light.png`) });
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.screenshot({ path: testInfo.outputPath(`catalog-${width}-dark.png`) });
		await page.keyboard.press('Escape');
		await expect(dialog).toHaveCount(0);
		expect(api.writes).toHaveLength(1);
	});
test('manager preserves failed creation draft and distinguishes editing current version from appending with photo', async ({
	page
}) => {
	const api = await catalog(page, [read, manage], { failCreate: true });
	await page.goto('/staff/supply-catalog');
	await page.getByRole('button', { name: 'เพิ่มพัสดุ', exact: true }).click();
	let dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อพัสดุ *', { exact: true }).fill('ปากกา');
	await dialog.getByLabel('หน่วยนับ *', { exact: true }).fill('ด้าม');
	await dialog.getByLabel('ราคาต่อหน่วย (บาท) *', { exact: true }).fill('0.10');
	await dialog.getByLabel('ร้านค้า *', { exact: true }).fill('ร้านใหม่');
	await dialog.getByRole('button', { name: 'บันทึกพัสดุ', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('บันทึกไม่สำเร็จ');
	await expect(dialog.getByLabel('ชื่อพัสดุ *', { exact: true })).toHaveValue('ปากกา');
	await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await page.getByRole('button', { name: 'ดูรายละเอียด', exact: true }).click();
	dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'แก้ไขรุ่นปัจจุบัน', exact: true }).click();
	await dialog.getByLabel('ราคาต่อหน่วย (บาท) *', { exact: true }).fill('130.25');
	await dialog.getByRole('button', { name: 'บันทึกรุ่นปัจจุบัน', exact: true }).click();
	await expect(dialog).toContainText('ราคาล่าสุด · รุ่น 2');
	const edit = api.writes.find(
		(write) => write.path.includes('/versions/') && write.method === 'PUT'
	);
	expect(edit?.payload).toEqual({ price: '130.25', shop: 'ร้านทดสอบ', revision: 4 });
	await dialog.getByRole('button', { name: 'เพิ่มรุ่นราคา', exact: true }).click();
	await dialog.getByLabel('ราคาต่อหน่วย (บาท) *', { exact: true }).fill('140.00');
	await dialog.getByLabel('รูปสินค้า', { exact: true }).setInputFiles(png);
	await expect(dialog.getByRole('status')).toHaveCount(0);
	await dialog.getByRole('button', { name: 'เพิ่มรุ่นราคา', exact: true }).click();
	await expect(dialog).toContainText('ราคาล่าสุด · รุ่น 3');
	const append = api.writes.find(
		(write) => write.path.endsWith('/versions') && write.method === 'POST'
	);
	expect(append?.payload).toEqual({
		price: '140.00',
		shop: 'ร้านทดสอบ',
		revision: 4,
		imageFileId: id(450)
	});
	expect(api.uploads).toEqual(['image']);
});
test('proposal product and temporary source uploads stay separate and survive failed save', async ({
	page
}) => {
	const api = await catalog(page, [read, propose], { failProposal: true });
	await page.goto('/staff/supply-catalog/proposals');
	await page.getByRole('button', { name: 'เสนอพัสดุใหม่', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ชื่อพัสดุ *', { exact: true }).fill('สมุด');
	await dialog.getByLabel('หน่วยนับ *', { exact: true }).fill('เล่ม');
	await dialog.getByLabel('ราคาต่อหน่วย (บาท) *', { exact: true }).fill('20.00');
	await dialog.getByLabel('ร้านค้า *', { exact: true }).fill('ร้านสมุด');
	await dialog.getByLabel('รูปสินค้า', { exact: true }).setInputFiles(png);
	await expect(dialog.getByRole('status')).toHaveCount(0);
	await dialog.getByLabel('รูปแหล่งที่มา', { exact: true }).setInputFiles(png);
	await expect(dialog.getByRole('status')).toHaveCount(0);
	await dialog.getByRole('button', { name: 'ส่งข้อเสนอ', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('ส่งข้อเสนอไม่สำเร็จ');
	await expect(dialog.getByLabel('ชื่อพัสดุ *', { exact: true })).toHaveValue('สมุด');
	expect(api.uploads).toEqual(['image', 'source']);
	expect(api.deleted).toEqual([]);
	expect(api.writes[0].payload).toMatchObject({
		imageFileId: id(450),
		sourceFileId: id(451),
		sourceUrl: null
	});
	await dialog.getByRole('button', { name: 'ส่งข้อเสนอ', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(api.uploads).toHaveLength(2);
	expect(api.deleted).toHaveLength(0);
});
test('reviewer edits every approved field while source stays out of approved payload', async ({
	page
}) => {
	const api = await catalog(page, [read, approve]);
	await page.goto('/staff/supply-catalog/proposals?review=true');
	await page.getByRole('button', { name: 'ตรวจข้อเสนอ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(
		dialog.getByRole('link', { name: 'เปิดลิงก์แหล่งที่มา', exact: true })
	).toHaveAttribute('href', 'https://shop.example.invalid/pencil');
	await dialog.getByRole('button', { name: 'แก้ไขและอนุมัติ', exact: true }).click();
	await dialog.getByLabel('ชื่อพัสดุ *', { exact: true }).fill('ดินสอใหม่');
	await dialog.getByLabel('รายละเอียด', { exact: true }).fill('ตรวจข้อมูลแล้ว');
	await dialog.getByLabel('หน่วยนับ *', { exact: true }).fill('แพ็ก');
	await dialog.getByLabel('ราคาต่อหน่วย (บาท) *', { exact: true }).fill('50.25');
	await dialog.getByLabel('ร้านค้า *', { exact: true }).fill('ร้านตรวจแล้ว');
	await dialog.getByRole('combobox', { name: 'หมวดหมู่ *', exact: true }).click();
	await page.getByRole('option', { name: 'อุปกรณ์เรียน', exact: true }).click();
	await dialog.getByLabel('รูปสินค้า', { exact: true }).setInputFiles(png);
	await expect(dialog.getByRole('status')).toHaveCount(0);
	await dialog.getByRole('button', { name: 'อนุมัติและรับเข้าคลัง', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	await expect(page.getByTestId('supply-proposals')).toContainText('ไม่มีข้อเสนอรออนุมัติ');
	const write = api.writes.find((entry) => entry.path.endsWith('/approve'));
	expect(write?.payload).toEqual({
		name: 'ดินสอใหม่',
		description: 'ตรวจข้อมูลแล้ว',
		categoryId: secondCategoryId,
		unit: 'แพ็ก',
		price: '50.25',
		shop: 'ร้านตรวจแล้ว',
		imageFileId: id(450),
		revision: 2,
		existingItemId: null,
		existingItemRevision: null
	});
});
test('reader can create personal bundles with quantities and latest prices without manager reads', async ({
	page
}) => {
	const api = await catalog(page, [read]);
	await page.goto('/staff/supply-catalog');
	expect(api.reads.some((entry) => entry.includes('/bundles'))).toBe(false);
	await page.getByRole('button', { name: 'ชุดพัสดุ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'สร้างชุดพัสดุ', exact: true }).click();
	await dialog.getByLabel('ชื่อชุดพัสดุ *', { exact: true }).fill('ชุดเอกสาร');
	await dialog.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await dialog.getByRole('button', { name: /กระดาษ A4.*125.50/ }).click();
	await dialog.getByRole('spinbutton', { name: 'จำนวน กระดาษ A4', exact: true }).fill('3');
	await expect(dialog).toContainText('รวม 376.50 ฿');
	await expect(
		dialog.getByRole('checkbox', { name: 'ใช้ร่วมกันทั้งโรงเรียน', exact: true })
	).toHaveCount(0);
	await dialog.getByRole('button', { name: 'บันทึกชุดพัสดุ', exact: true }).click();
	await expect(dialog).toContainText('ชุดเอกสาร');
	const write = api.writes.find((entry) => entry.path.endsWith('/bundles'));
	expect(write?.payload).toEqual({
		name: 'ชุดเอกสาร',
		shared: false,
		revision: null,
		entries: [{ itemId, quantity: 3 }]
	});
});
test('cancelled proposal deletes both unattached upload stages', async ({ page }) => {
	const api = await catalog(page, [read, propose]);
	await page.goto('/staff/supply-catalog/proposals');
	await page.getByRole('button', { name: 'เสนอพัสดุใหม่', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('รูปสินค้า', { exact: true }).setInputFiles(png);
	await expect(dialog.getByRole('status')).toHaveCount(0);
	await dialog.getByLabel('รูปแหล่งที่มา', { exact: true }).setInputFiles(png);
	await expect(dialog.getByRole('status')).toHaveCount(0);
	await dialog.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(api.deleted).toEqual([`/api/files/${id(450)}`, `/api/files/${id(451)}`]);
	expect(api.writes).toHaveLength(0);
});

test('old version history is bounded, loaded lazily and edited without changing the latest price', async ({
	page
}) => {
	const api = await catalog(page, [read, manage], { longHistory: true });
	await page.goto('/staff/supply-catalog');
	await page.getByRole('button', { name: 'ดูรายละเอียด', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toContainText('ประวัติราคา (102 รุ่น)');
	expect(api.reads.some((path) => path.includes('/versions?'))).toBe(false);
	await dialog.getByRole('button', { name: 'รุ่นเก่ากว่า', exact: true }).click();
	await expect(dialog).toContainText('หน้า 2 / 2');
	await expect(dialog.getByRole('listitem')).toHaveCount(2);
	await dialog.getByRole('button', { name: 'แก้ไขรุ่น 1', exact: true }).click();
	await dialog.getByLabel('ราคาต่อหน่วย (บาท) *', { exact: true }).fill('100.00');
	await dialog.getByRole('button', { name: 'บันทึกรุ่นที่เลือก', exact: true }).click();
	await expect(dialog).toContainText('ราคาล่าสุด · รุ่น 102');
	await expect(dialog).toContainText('125.50');
	await expect(dialog).toContainText('100.00');
	await expect(dialog.getByRole('listitem')).toHaveCount(2);
	expect(api.writes[0].path).toBe(`/api/supply-catalog/items/${itemId}/versions/${id(701)}`);
});
test('metadata edits remove an item from the active search and correct the filtered count locally', async ({
	page
}) => {
	const api = await catalog(page, [read, manage]);
	await page.goto('/staff/supply-catalog?search=กระดาษ');
	await page.getByRole('button', { name: 'ดูรายละเอียด', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'แก้ไขข้อมูล / รูป', exact: true }).click();
	await dialog.getByLabel('ชื่อพัสดุ *', { exact: true }).fill('ปากกา');
	await dialog.getByLabel('รายละเอียด', { exact: true }).fill('สำหรับเขียน');
	await dialog.getByRole('button', { name: 'บันทึกพัสดุ', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	await expect(page.getByTestId('supply-catalog')).toContainText('0 รายการ');
	await expect(page.getByTestId('supply-catalog')).toContainText('ไม่พบพัสดุที่ตรงกับตัวกรอง');
	expect(api.reads.filter((path) => path.startsWith('/api/supply-catalog/items?'))).toHaveLength(1);
});
test('choosing existing approval target uses its canonical metadata while keeping proposed price and shop', async ({
	page
}) => {
	const api = await catalog(page, [read, approve]);
	await page.goto('/staff/supply-catalog/proposals?review=true');
	await page.getByRole('button', { name: 'ตรวจข้อเสนอ', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'แก้ไขและอนุมัติ', exact: true }).click();
	await dialog.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await dialog.getByRole('button', { name: /กระดาษ A4.*125.50/ }).click();
	await expect(dialog.getByLabel('ชื่อพัสดุ *', { exact: true })).toHaveValue('กระดาษ A4');
	await expect(dialog.getByLabel('รายละเอียด', { exact: true })).toHaveValue(
		'กระดาษสำหรับงานเอกสาร'
	);
	await expect(dialog.getByLabel('หน่วยนับ *', { exact: true })).toHaveValue('รีม');
	await expect(dialog.getByLabel('ราคาต่อหน่วย (บาท) *', { exact: true })).toHaveValue('45.00');
	await expect(dialog.getByLabel('ร้านค้า *', { exact: true })).toHaveValue('ร้านเดิม');
	await dialog.getByRole('button', { name: 'อนุมัติและรับเข้าคลัง', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(api.writes[0].payload).toMatchObject({
		name: 'กระดาษ A4',
		description: 'กระดาษสำหรับงานเอกสาร',
		unit: 'รีม',
		price: '45.00',
		shop: 'ร้านเดิม',
		existingItemId: itemId,
		existingItemRevision: 3
	});
});
test('permission revocation disposes a delayed catalog response before it can be displayed', async ({
	page
}) => {
	const api = await catalog(page, [read], { holdItems: true });
	await page.goto('/staff/supply-catalog');
	await expect
		.poll(() => api.reads.some((path) => path.startsWith('/api/supply-catalog/items?')))
		.toBe(true);
	await page.evaluate(async () => {
		const modulePath = '/src/lib/stores/auth.ts';
		const { authStore } = await import(modulePath);
		authStore.setUser(
			{
				id: '55000000-0000-4000-8000-000000000010',
				firstName: 'เจ้าของ',
				lastName: 'โปรไฟล์',
				role: 'staff',
				user_type: 'staff',
				status: 'active'
			},
			[]
		);
	});
	api.releaseItems();
	await expect(page.getByTestId('supply-catalog')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'กระดาษ A4', exact: true })).toHaveCount(0);
});
