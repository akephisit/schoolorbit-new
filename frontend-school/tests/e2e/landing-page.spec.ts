import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer as createHttpServer, type Server } from 'node:http';
import { createServer, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
let devServer: ViteDevServer;
let apiServer: Server;
let baseUrl: string;
let failStatistics = false;
let failedRegion: string | undefined;
let empty = false;
let logoAvailable = true;
let brokenLogo = false;
let statisticsGate: Promise<void> | undefined;
let releaseStatistics: (() => void) | undefined;
const requests: { path: string; cookie?: string; origin?: string }[] = [];
const counts = (male: number, female: number, other = 0) => ({
	total: male + female + other,
	male,
	female,
	otherOrUnspecified: other
});
const statistics = {
	academicYear: { year: 2569, name: 'ปีการศึกษา 2569' },
	students: counts(61, 62, 1),
	unassignedStudents: counts(0, 1),
	totalTeachers: 12,
	totalStaff: 15,
	totalHomerooms: 4,
	asOf: '2026-10-03T10:00:00Z',
	grades: [
		{
			levelType: 'secondary',
			year: 1,
			students: counts(31, 32, 1),
			unassignedStudents: counts(0, 1),
			homerooms: [
				{ name: 'ม.1/1', students: counts(16, 16, 1) },
				{ name: 'ม.1/2', students: counts(15, 15) }
			]
		},
		{
			levelType: 'secondary',
			year: 2,
			students: counts(30, 30),
			unassignedStudents: counts(0, 0),
			homerooms: [
				{ name: 'ม.2/1', students: counts(30, 30) },
				{ name: 'ม.2/2', students: counts(0, 0) }
			]
		}
	]
};
const organization = {
	units: [
		{
			id: 'root',
			parentId: null,
			name: 'โรงเรียนสาธิตทดสอบ',
			unitType: 'school',
			members: [{ name: 'ผู้บริหาร ทดสอบ', positionCode: 'director', positionTitle: null }]
		},
		{
			id: 'academic',
			parentId: 'root',
			name: 'กลุ่มบริหารวิชาการ',
			unitType: 'management_group',
			members: [
				{ name: 'หัวหน้า ทดสอบ', positionCode: 'head', positionTitle: null },
				{ name: 'รองหัวหน้า ทดสอบ', positionCode: 'deputy_head', positionTitle: null },
				{ name: 'ผู้ประสานงาน ทดสอบ', positionCode: 'coordinator', positionTitle: null },
				{ name: 'สมาชิก หนึ่ง', positionCode: 'member', positionTitle: 'ครูฝ่ายวิชาการ' },
				{ name: 'สมาชิก สอง', positionCode: 'member', positionTitle: null }
			]
		},
		{
			id: 'empty',
			parentId: 'root',
			name: 'กลุ่มบริหารทั่วไป',
			unitType: 'management_group',
			members: []
		}
	]
};

test.describe.configure({ mode: 'serial' });
test.use({ serviceWorkers: 'block' });
test.beforeAll(async () => {
	apiServer = createHttpServer(async (req, res) => {
		const endpoint = new URL(req.url ?? '/', 'http://localhost').pathname;
		requests.push({ path: endpoint, cookie: req.headers.cookie, origin: req.headers.origin });
		res.setHeader('access-control-allow-origin', req.headers.origin ?? '*');
		res.setHeader('access-control-allow-headers', 'X-School-Subdomain,Content-Type');
		res.setHeader('content-type', 'application/json');
		if (req.method === 'OPTIONS') {
			res.writeHead(204).end();
			return;
		}
		if (endpoint.startsWith('/api/public/files/')) {
			if (brokenLogo) {
				res.writeHead(404).end();
				return;
			}
			res.setHeader('content-type', 'image/svg+xml');
			res.end(
				'<svg xmlns="http://www.w3.org/2000/svg" width="80" height="104" viewBox="0 0 80 104"><path fill="#2563eb" d="M40 3 75 23V60Q70 90 40 101 10 90 5 60V23Z"/></svg>'
			);
			return;
		}
		if (endpoint === '/deployment-status') {
			res.end(JSON.stringify({ status: 'ready', releaseId: 'fixture' }));
			return;
		}
		if (endpoint === '/api/school/public/statistics' && statisticsGate) await statisticsGate;
		if (
			(endpoint === '/api/school/public/statistics' && failStatistics) ||
			endpoint === failedRegion
		) {
			res.writeHead(500).end(JSON.stringify({ success: false, error: 'Fixture read failure' }));
			return;
		}
		const data = endpoint.endsWith('/statistics')
			? empty
				? {
						...statistics,
						academicYear: null,
						students: counts(0, 0),
						totalHomerooms: 0,
						grades: []
					}
				: statistics
			: endpoint.endsWith('/organization')
				? empty
					? { units: [] }
					: organization
				: {
						schoolName: 'โรงเรียนสาธิตทดสอบ',
						logoFileId: logoAvailable ? '11111111-1111-4111-8111-111111111111' : null
					};
		res.end(JSON.stringify({ success: true, data }));
	});
	await new Promise<void>((done) => apiServer.listen(0, '127.0.0.1', done));
	const apiAddress = apiServer.address();
	if (!apiAddress || typeof apiAddress === 'string') throw new Error('API fixture did not start');
	process.env.PUBLIC_BACKEND_URL = `http://127.0.0.1:${apiAddress.port}`;
	process.env.PUBLIC_VAPID_KEY = 'test';
	devServer = await createServer({
		root: frontendRoot,
		logLevel: 'silent',
		server: { host: '127.0.0.1', port: 0 }
	});
	await devServer.listen();
	const address = devServer.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('Vite test server did not start');
	baseUrl = `http://127.0.0.1:${address.port}`;
});
test.beforeEach(() => {
	requests.length = 0;
	failStatistics = false;
	failedRegion = undefined;
	empty = false;
	logoAvailable = true;
	brokenLogo = false;
	statisticsGate = undefined;
	releaseStatistics = undefined;
});
test.afterEach(() => releaseStatistics?.());
test.afterAll(async () => {
	await devServer?.close();
	await new Promise<void>((done, reject) =>
		apiServer.close((error) => (error ? reject(error) : done()))
	);
});

test('anonymous visitors see real school regions and existing services', async ({ page }) => {
	await page.goto(baseUrl);
	await expect(
		page.getByRole('heading', { name: 'โรงเรียนสาธิตทดสอบ', exact: true })
	).toBeVisible();
	await expect(page.getByTestId('school-statistics')).toContainText('124');
	await expect(page.getByTestId('school-brand')).toContainText('โรงเรียนสาธิตทดสอบ');
	await expect(page.getByText('ผู้บริหาร ทดสอบ', { exact: true })).toBeVisible();
	await expect(page.getByText('หัวหน้า ทดสอบ', { exact: true })).toBeVisible();
	await page.getByRole('link', { name: 'สำรวจบริการ' }).click();
	await expect(page).toHaveURL(/#services$/);
	for (const [label, href] of [
		['ปฏิทินโรงเรียน.*ดูปฏิทิน', '/calendar'],
		['รับสมัครนักเรียน.*ดูการรับสมัคร', '/apply'],
		['ตรวจสอบเกียรติบัตร.*ตรวจสอบเอกสาร', '/verify/certificate']
	]) {
		await expect(page.getByRole('link', { name: new RegExp(label) })).toHaveAttribute('href', href);
	}
	const reads = requests.filter((r) => r.path.startsWith('/api/school/public'));
	expect(reads).toHaveLength(3);
	expect(reads.every((r) => !r.cookie && r.origin === baseUrl)).toBe(true);
	expect(requests.some((r) => r.path === '/api/auth/me')).toBe(false);
	await page.getByRole('link', { name: 'เข้าสู่ระบบ', exact: true }).click();
	await expect(page).toHaveURL(/\/login$/);
});

test('class details include unassigned students and an empty room', async ({ page }) => {
	await page.goto(baseUrl);
	await page.locator('#statistics summary').filter({ hasText: 'มัธยมศึกษาปีที่ 1' }).click();
	await expect(page.getByRole('rowheader', { name: 'ยังไม่ได้จัดห้อง' })).toBeVisible();
	await page.locator('#statistics summary').filter({ hasText: 'มัธยมศึกษาปีที่ 2' }).click();
	await expect(page.getByRole('row', { name: 'ม.2/2 0 0 0 0', exact: true })).toBeVisible();
});

test('a failed statistics read can retry without refetching identity or organization', async ({
	page
}) => {
	failStatistics = true;
	await page.goto(baseUrl);
	await expect(
		page.getByRole('heading', { name: 'โรงเรียนสาธิตทดสอบ', exact: true })
	).toBeVisible();
	await expect(page.getByText('ผู้บริหาร ทดสอบ', { exact: true })).toBeVisible();
	await expect(page.getByText('โหลดสถิติโรงเรียนไม่สำเร็จ', { exact: true })).toBeVisible();
	const before = requests.filter((r) => r.path.startsWith('/api/school/public')).map((r) => r.path);
	failStatistics = false;
	await page.getByRole('button', { name: 'ลองใหม่: สถิติโรงเรียน' }).click();
	await expect(page.getByTestId('school-statistics')).toContainText('124');
	const after = requests.filter((r) => r.path.startsWith('/api/school/public')).map((r) => r.path);
	expect(after.slice(before.length)).toEqual(['/api/school/public/statistics']);
	await page.screenshot({
		path: '/tmp/schoolorbit-public-retry-desktop.png',
		fullPage: true,
		animations: 'disabled'
	});
});

for (const [endpoint, label] of [
	['/api/school/public', 'ข้อมูลโรงเรียน'],
	['/api/school/public/organization', 'โครงสร้างบริหาร']
]) {
	test(`failed ${label} can recover independently`, async ({ page }) => {
		failedRegion = endpoint;
		await page.goto(baseUrl);
		await expect(page.getByTestId('school-statistics')).toContainText('124');
		await expect(page.getByText(`โหลด${label}ไม่สำเร็จ`, { exact: true })).toBeVisible();
		const before = requests.filter((r) => r.path.startsWith('/api/school/public')).length;
		failedRegion = undefined;
		await page.getByRole('button', { name: `ลองใหม่: ${label}` }).click();
		await expect(
			page.getByRole('heading', { name: 'โรงเรียนสาธิตทดสอบ', exact: true })
		).toBeVisible();
		await expect(page.getByText('ผู้บริหาร ทดสอบ', { exact: true })).toBeVisible();
		await expect(page.getByTestId('school-brand')).toContainText('โรงเรียนสาธิตทดสอบ');
		expect(
			requests
				.filter((r) => r.path.startsWith('/api/school/public'))
				.slice(before)
				.map((r) => r.path)
		).toEqual([endpoint]);
	});
}

test('all current positions are grouped and retain custom position titles', async ({ page }) => {
	await page.goto(baseUrl);
	for (const name of [
		'หัวหน้า ทดสอบ',
		'รองหัวหน้า ทดสอบ',
		'ผู้ประสานงาน ทดสอบ',
		'สมาชิก หนึ่ง',
		'สมาชิก สอง'
	]) {
		await expect(page.getByText(name, { exact: true })).toBeVisible();
	}
	await expect(page.getByRole('region', { name: 'สมาชิก', exact: true })).toContainText('2 คน');
	await expect(page.getByText('ครูฝ่ายวิชาการ', { exact: true })).toBeVisible();
});

test('branding shares one read, and the hero crest follows text height without a frame', async ({
	page
}) => {
	await page.goto(baseUrl);
	await expect(
		page.getByTestId('school-brand').getByRole('img', { name: 'ตราโรงเรียน' })
	).toBeVisible();
	const crest = page.getByTestId('hero-school-crest');
	await expect(crest.getByRole('img', { name: 'ตราโรงเรียน' })).toBeVisible();
	const geometry = await crest.evaluate((node) => ({
		height: node.getBoundingClientRect().height,
		textHeight: node.nextElementSibling?.getBoundingClientRect().height,
		border: getComputedStyle(node).borderTopWidth,
		background: getComputedStyle(node).backgroundColor
	}));
	expect(geometry.height).toBe(geometry.textHeight);
	expect(geometry.border).toBe('0px');
	expect(geometry.background).toBe('rgba(0, 0, 0, 0)');
	expect(requests.filter((r) => r.path === '/api/school/public')).toHaveLength(1);
});

for (const unavailable of ['not configured', 'broken']) {
	test(`school identity remains usable when the crest is ${unavailable}`, async ({ page }) => {
		logoAvailable = unavailable !== 'not configured';
		brokenLogo = unavailable === 'broken';
		await page.goto(baseUrl);
		await expect(page.getByTestId('school-brand')).toContainText('โรงเรียนสาธิตทดสอบ');
		await expect(page.getByTestId('school-brand').locator('img')).toHaveCount(0);
		await expect(page.getByTestId('hero-school-crest').locator('img')).toHaveCount(0);
	});
}

test('ordinary motion enables the subtle decorative animation', async ({ page }) => {
	await page.emulateMedia({ reducedMotion: 'no-preference' });
	await page.goto(baseUrl);
	await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
	expect(
		await page.locator('.hero-orbit').evaluate((node) => getComputedStyle(node).animationName)
	).toMatch(/(?:^|-)orbit-drift$/);
});

test('reduced motion keeps the modern surfaces steady and fully usable', async ({ page }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto(baseUrl);
	await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
	const motion = await page
		.locator('.hero-orbit')
		.evaluate((node) => getComputedStyle(node).animationName);
	expect(motion).toBe('none');
	await page.getByRole('link', { name: /ปฏิทินโรงเรียน.*ดูปฏิทิน/ }).hover();
	expect(
		await page
			.locator('.public-service')
			.first()
			.evaluate((node) => getComputedStyle(node).transform)
	).toBe('none');
});

test('slow statistics do not block successful sibling regions', async ({ page }) => {
	statisticsGate = new Promise<void>((done) => (releaseStatistics = done));
	await page.goto(baseUrl, { waitUntil: 'commit' });
	await expect(
		page.getByRole('heading', { name: 'โรงเรียนสาธิตทดสอบ', exact: true })
	).toBeVisible();
	await expect(page.getByRole('status', { name: 'กำลังโหลดสถิติโรงเรียน' })).toBeVisible();
	await expect(page.getByText('ผู้บริหาร ทดสอบ', { exact: true })).toBeVisible();
	releaseStatistics?.();
	await expect(page.getByTestId('school-statistics')).toContainText('124');
});

test('missing academic context and organization show honest empty states', async ({ page }) => {
	empty = true;
	await page.goto(baseUrl);
	await expect(
		page.getByRole('heading', { name: 'ยังไม่มีปีการศึกษาที่เปิดใช้งาน' })
	).toBeVisible();
	await expect(page.getByRole('heading', { name: 'ยังไม่มีข้อมูลโครงสร้างบริหาร' })).toBeVisible();
	await expect(page.getByTestId('school-statistics')).toContainText('—');
	await page.screenshot({
		path: '/tmp/schoolorbit-public-empty-desktop.png',
		fullPage: true,
		animations: 'disabled'
	});
});

for (const width of [375, 1280]) {
	test(`header, disclosures and layout are usable at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		await page.goto(baseUrl);
		await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
		await expect(page.getByTestId('school-statistics')).toContainText('124');
		await expect(page.locator('header').getByRole('link')).toHaveCount(2);
		await expect(page.getByTestId('school-brand')).toBeVisible();
		await expect(page.locator('header').getByRole('link', { name: 'เข้าสู่ระบบ' })).toBeVisible();
		await page.getByRole('link', { name: 'สำรวจบริการ' }).click();
		await expect(page).toHaveURL(/#services$/);
		await page.locator('#statistics summary').first().click();
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await page.locator('#organization summary').first().click();
		await expect(page.getByText('หัวหน้า ทดสอบ', { exact: true })).not.toBeVisible();
		await page.locator('#organization summary').first().click();
		await expect(page.getByText('หัวหน้า ทดสอบ', { exact: true })).toBeVisible();
		await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'instant' }));
		await page.screenshot({
			path: `/tmp/schoolorbit-public-${width}-light.png`,
			fullPage: true,
			animations: 'disabled'
		});
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.screenshot({
			path: `/tmp/schoolorbit-public-${width}-dark.png`,
			fullPage: true,
			animations: 'disabled'
		});
	});
}
