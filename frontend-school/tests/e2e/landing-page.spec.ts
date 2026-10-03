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
			leaders: [{ name: 'ผู้บริหาร ทดสอบ', positionCode: 'director', positionTitle: null }]
		},
		{
			id: 'academic',
			parentId: 'root',
			name: 'กลุ่มบริหารวิชาการ',
			unitType: 'management_group',
			leaders: [{ name: 'หัวหน้า ทดสอบ', positionCode: 'head', positionTitle: null }]
		},
		{
			id: 'empty',
			parentId: 'root',
			name: 'กลุ่มบริหารทั่วไป',
			unitType: 'management_group',
			leaders: []
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
				: { schoolName: 'โรงเรียนสาธิตทดสอบ', logoFileId: null };
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
	await page.screenshot({ path: '/tmp/schoolorbit-public-retry-desktop.png', fullPage: true });
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
		expect(
			requests
				.filter((r) => r.path.startsWith('/api/school/public'))
				.slice(before)
				.map((r) => r.path)
		).toEqual([endpoint]);
	});
}

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
	await page.screenshot({ path: '/tmp/schoolorbit-public-empty-desktop.png', fullPage: true });
});

for (const width of [375, 1280]) {
	test(`navigation, disclosures and layout are usable at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		await page.goto(baseUrl);
		await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
		await expect(page.getByTestId('school-statistics')).toContainText('124');
		if (width < 768) {
			const menu = page.getByRole('button', { name: 'เปิดเมนู' });
			await menu.click();
			await expect(page.getByRole('button', { name: 'ปิดเมนู' })).toHaveAttribute(
				'aria-expanded',
				'true'
			);
			await page
				.getByRole('navigation', { name: 'เมนูหลัก' })
				.getByRole('link', { name: 'บริการ' })
				.click();
			await expect(menu).toHaveAttribute('aria-expanded', 'false');
		}
		await page.locator('#statistics summary').first().click();
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await page.locator('#organization summary').first().click();
		await expect(page.getByText('หัวหน้า ทดสอบ', { exact: true })).not.toBeVisible();
		await page.locator('#organization summary').first().click();
		await expect(page.getByText('หัวหน้า ทดสอบ', { exact: true })).toBeVisible();
		await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'instant' }));
		await page.screenshot({ path: `/tmp/schoolorbit-public-${width}-light.png`, fullPage: true });
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.screenshot({ path: `/tmp/schoolorbit-public-${width}-dark.png`, fullPage: true });
	});
}
