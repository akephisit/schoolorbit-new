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
let schoolName = 'โรงเรียนสาธิตทดสอบ';
let identityGate: Promise<void> | undefined;
let releaseIdentity: (() => void) | undefined;
let statisticsGate: Promise<void> | undefined;
let releaseStatistics: (() => void) | undefined;
const requests: { path: string; cookie?: string; origin?: string }[] = [];
const isRegionRead = (resource: string) =>
	[
		'/api/school/public',
		'/api/school/public/statistics',
		'/api/school/public/organization'
	].includes(resource);
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
			members: [
				{
					name: 'ผู้บริหาร ทดสอบ',
					positionCode: 'director',
					positionTitle: null,
					avatarUrl: '/api/school/public/organization-members/fixture/avatar'
				}
			]
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
			id: 'subject-group',
			parentId: 'academic',
			name: 'กลุ่มสาระคณิตศาสตร์',
			unitType: 'subject_group',
			members: []
		},
		{
			id: 'subject-child',
			parentId: 'subject-group',
			name: 'งานย่อยกลุ่มสาระ',
			unitType: 'work',
			members: []
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

test.describe.configure({ mode: 'serial', timeout: 60000 });
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
		if (endpoint.startsWith('/api/public/files/') || endpoint.endsWith('/avatar')) {
			if (brokenLogo) {
				res.writeHead(404).end();
				return;
			}
			if (endpoint.endsWith('/delivery')) {
				res.end(
					JSON.stringify({
						success: true,
						data: { url: 'https://public-files.example/fixture-school-logo.svg' }
					})
				);
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
		if (endpoint === '/api/school/public' && identityGate) await identityGate;
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
						schoolName,
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
		configFile: path.join(frontendRoot, 'vite.config.ts'),
		cacheDir: path.resolve(frontendRoot, 'node_modules/.vite-public-school-layout-test'),
		logLevel: 'error',
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
	schoolName = 'โรงเรียนสาธิตทดสอบ';
	identityGate = undefined;
	releaseIdentity = undefined;
	statisticsGate = undefined;
	releaseStatistics = undefined;
});
test.afterEach(() => {
	releaseStatistics?.();
	releaseIdentity?.();
});
test.afterAll(async () => {
	await devServer?.close();
	await new Promise<void>((done, reject) =>
		apiServer.close((error) => (error ? reject(error) : done()))
	);
});

test('anonymous visitors see real school regions and existing services', async ({ page }) => {
	await page.goto(baseUrl);
	await expect(
		page.getByRole('heading', { name: 'โรงเรียนสาธิตทดสอบ', exact: true, level: 1 })
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
	const reads = requests.filter((r) => isRegionRead(r.path));
	expect(reads).toHaveLength(3);
	expect(reads.every((r) => !r.cookie && r.origin === baseUrl)).toBe(true);
	expect(requests.some((r) => r.path === '/api/auth/me')).toBe(false);
	await page.getByRole('link', { name: 'เข้าสู่ระบบ', exact: true }).click();
	await expect(page).toHaveURL(/\/login$/);
});

test('school identity and SEO are readable without JavaScript in the first HTML response', async ({
	browser
}) => {
	const context = await browser.newContext({ javaScriptEnabled: false, serviceWorkers: 'block' });
	try {
		const page = await context.newPage();
		const response = await page.goto(`${baseUrl}/?utm_source=fixture`);
		expect(response?.headers()['x-robots-tag']).toBe('noindex');
		await expect(
			page.getByRole('heading', { name: schoolName, exact: true, level: 1 })
		).toBeVisible();
		await expect(page).toHaveTitle(`${schoolName} — ข้อมูลและบริการสาธารณะ`);
		await expect(page.locator('head title')).toHaveCount(1);
		await expect(page.locator('meta[name="description"]')).toHaveCount(1);
		await expect(page.locator('meta[name="description"]')).toHaveAttribute(
			'content',
			new RegExp(schoolName)
		);
		await expect(page.locator('meta[property="og:title"]')).toHaveCount(1);
		await expect(page.locator('meta[property="og:site_name"]')).toHaveAttribute(
			'content',
			schoolName
		);
		await expect(page.locator('meta[property="og:url"]')).toHaveAttribute('content', `${baseUrl}/`);
		await expect(page.locator('link[rel="canonical"]')).toHaveAttribute('href', `${baseUrl}/`);
		const schema = JSON.parse(
			(await page.locator('script[type="application/ld+json"]').textContent()) || 'null'
		);
		expect(schema).toEqual({
			'@context': 'https://schema.org',
			'@type': 'School',
			name: schoolName,
			url: `${baseUrl}/`,
			logo: `${baseUrl}/school-logo`
		});
		expect(requests.filter((r) => r.path === '/api/school/public')).toHaveLength(1);
	} finally {
		await context.close();
	}
});

test('fresh HTML metadata follows each school name and escapes hostile text safely', async ({
	page
}) => {
	for (const name of [
		'โรงเรียนทดสอบแห่งที่สอง',
		'โรงเรียน "ทดสอบ" & </script><script>window.seoInjection=true</script>'
	]) {
		schoolName = name;
		await page.goto(baseUrl);
		await expect(page).toHaveTitle(`${name} — ข้อมูลและบริการสาธารณะ`);
		await expect(page.getByRole('heading', { name, exact: true, level: 1 })).toBeVisible();
		const schema = await page.locator('script[type="application/ld+json"]').textContent();
		expect(JSON.parse(schema || 'null').name).toBe(name);
		expect(schema).not.toContain('</script>');
		expect(await page.evaluate(() => Reflect.has(window, 'seoInjection'))).toBe(false);
		await expect(page.locator('head title')).toHaveCount(1);
		await expect(page.locator('meta[name="description"]')).toHaveCount(1);
	}
});

test('crawler endpoints and non-home HTML apply the local noindex policy without API reads', async ({
	request
}) => {
	const robots = await request.get(`${baseUrl}/robots.txt`);
	expect(robots.status()).toBe(200);
	expect(robots.headers()['content-type']).toContain('text/plain');
	expect(await robots.text()).toBe('User-agent: *\nDisallow:\n');
	const sitemap = await request.get(`${baseUrl}/sitemap.xml`);
	expect(sitemap.status()).toBe(200);
	expect(sitemap.headers()['content-type']).toContain('application/xml');
	expect(await sitemap.text()).not.toContain('<url>');
	expect(requests).toHaveLength(0);
	for (const route of ['/login', '/staff/academic/assessments', '/not-an-existing-page']) {
		const response = await request.get(`${baseUrl}${route}`);
		expect(response.headers()['x-robots-tag']).toBe('noindex');
	}
});

test('logo crawlers resolve only the current public crest without cookies or caller-selected files', async ({
	request
}) => {
	const response = await request.get(`${baseUrl}/school-logo?fileId=private-file`, {
		maxRedirects: 0,
		headers: { Cookie: 'fixture-private-cookie=not-a-real-session' }
	});
	expect(response.status()).toBe(307);
	expect(response.headers().location).toBe('https://public-files.example/fixture-school-logo.svg');
	expect(response.headers()['cache-control']).toBe('no-store');
	expect(requests.map((r) => r.path)).toEqual([
		'/api/school/public',
		'/api/public/files/11111111-1111-4111-8111-111111111111/delivery'
	]);
	expect(requests.every((r) => !r.cookie && r.origin === baseUrl)).toBe(true);
	logoAvailable = false;
	expect((await request.get(`${baseUrl}/school-logo`, { maxRedirects: 0 })).status()).toBe(404);
	logoAvailable = true;
	brokenLogo = true;
	expect((await request.get(`${baseUrl}/school-logo`, { maxRedirects: 0 })).status()).toBe(404);
	brokenLogo = false;
	failedRegion = '/api/school/public';
	expect((await request.get(`${baseUrl}/school-logo`, { maxRedirects: 0 })).status()).toBe(503);
});

test('a slow identity read is bounded while sibling reads start concurrently and retries remain local', async ({
	page
}) => {
	identityGate = new Promise<void>((done) => (releaseIdentity = done));
	const started = Date.now();
	const navigation = page.goto(baseUrl);
	await expect.poll(() => requests.filter((r) => isRegionRead(r.path)).length).toBe(3);
	await navigation;
	expect(Date.now() - started).toBeLessThan(6_000);
	await expect(page.getByText('โหลดข้อมูลโรงเรียนไม่สำเร็จ', { exact: true })).toBeVisible();
	await expect(page.getByTestId('school-statistics')).toContainText('124');
	await expect(page.getByText('ผู้บริหาร ทดสอบ', { exact: true })).toBeVisible();
	await expect(page).toHaveTitle('เว็บไซต์โรงเรียน — ข้อมูลและบริการสาธารณะ');
	await expect(page.locator('script[type="application/ld+json"]')).toHaveCount(0);
	releaseIdentity?.();
	identityGate = undefined;
	const before = requests.filter((r) => isRegionRead(r.path)).length;
	await page.getByRole('button', { name: 'ลองใหม่: ข้อมูลโรงเรียน' }).click();
	await expect(
		page.getByRole('heading', { name: schoolName, exact: true, level: 1 })
	).toBeVisible();
	await expect(page).toHaveTitle(`${schoolName} — ข้อมูลและบริการสาธารณะ`);
	expect(
		requests
			.filter((r) => isRegionRead(r.path))
			.slice(before)
			.map((r) => r.path)
	).toEqual(['/api/school/public']);
});

test('class details include unassigned students and an empty room', async ({ page }) => {
	await page.goto(baseUrl);
	await page.getByRole('button', { name: 'มัธยมศึกษาปีที่ 1', exact: true }).click();
	await expect(page.getByRole('rowheader', { name: 'ยังไม่ได้จัดห้อง' })).toBeVisible();
	await page.getByRole('button', { name: 'มัธยมศึกษาปีที่ 2', exact: true }).click();
	await expect(page.getByRole('row', { name: 'ม.2/2 0 0 0 0', exact: true })).toBeVisible();
});

test('a failed statistics read can retry without refetching identity or organization', async ({
	page
}) => {
	failStatistics = true;
	await page.goto(baseUrl);
	await expect(
		page.getByRole('heading', { name: 'โรงเรียนสาธิตทดสอบ', exact: true, level: 1 })
	).toBeVisible();
	await expect(page.getByText('ผู้บริหาร ทดสอบ', { exact: true })).toBeVisible();
	await expect(page.getByText('โหลดสถิติโรงเรียนไม่สำเร็จ', { exact: true })).toBeVisible();
	const before = requests.filter((r) => isRegionRead(r.path)).map((r) => r.path);
	failStatistics = false;
	await page.getByRole('button', { name: 'ลองใหม่: สถิติโรงเรียน' }).click();
	await expect(page.getByTestId('school-statistics')).toContainText('124');
	const after = requests.filter((r) => isRegionRead(r.path)).map((r) => r.path);
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
		const before = requests.filter((r) => isRegionRead(r.path)).length;
		failedRegion = undefined;
		await page.getByRole('button', { name: `ลองใหม่: ${label}` }).click();
		await expect(
			page.getByRole('heading', { name: 'โรงเรียนสาธิตทดสอบ', exact: true, level: 1 })
		).toBeVisible();
		await expect(page.getByText('ผู้บริหาร ทดสอบ', { exact: true })).toBeVisible();
		await expect(page.getByTestId('school-brand')).toContainText('โรงเรียนสาธิตทดสอบ');
		expect(
			requests
				.filter((r) => isRegionRead(r.path))
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
		page.getByRole('heading', { name: 'โรงเรียนสาธิตทดสอบ', exact: true, level: 1 })
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
		await page.getByTestId('school-grade-summary').getByRole('button').first().click();
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
		).toBe(true);
		await expect(page.getByRole('img', { name: 'รูป ผู้บริหาร ทดสอบ' })).toBeVisible();
		await expect(
			page.getByRole('rowheader', { name: 'มัธยมศึกษาตอนต้น', exact: true })
		).toBeVisible();
		const chartBounds = await page.getByTestId('public-organization-chart').boundingBox();
		const rootBounds = await page.locator('#organization summary').first().boundingBox();
		expect(chartBounds).not.toBeNull();
		expect(rootBounds).not.toBeNull();
		if (!chartBounds || !rootBounds) throw new Error('Organization chart must have visible bounds');
		expect(rootBounds.x).toBeGreaterThanOrEqual(chartBounds.x);
		expect(rootBounds.x + rootBounds.width).toBeLessThanOrEqual(chartBounds.x + chartBounds.width);
		await page.locator('#organization summary').first().click();
		await expect(page.getByText('หัวหน้า ทดสอบ', { exact: true })).not.toBeVisible();
		await page.locator('#organization summary').first().click();
		await expect(page.getByText('หัวหน้า ทดสอบ', { exact: true })).toBeVisible();
		await expect(
			page
				.getByTestId('school-grade-summary')
				.getByRole('row', { name: 'มัธยมศึกษาปีที่ 1 2 31 32 1 64', exact: true })
		).toBeVisible();
		await expect(
			page.locator('#organization').getByText('กลุ่มสาระคณิตศาสตร์', { exact: true })
		).toHaveCount(0);
		await expect(
			page.locator('#organization').getByText('งานย่อยกลุ่มสาระ', { exact: true })
		).toHaveCount(0);
		const canvas = page.getByTestId('public-organization-chart');
		const content = page.getByTestId('organization-chart-content');
		const original = await content.getAttribute('style');
		await page.getByRole('button', { name: 'ซูมเข้า', exact: true }).click();
		await expect.poll(() => content.getAttribute('style')).not.toBe(original);
		await page.getByRole('button', { name: 'พอดีจอ', exact: true }).click();
		await expect.poll(() => content.getAttribute('style')).toBe(original);
		const bounds = await canvas.boundingBox();
		if (!bounds) throw new Error('Canvas bounds must exist');
		await page.mouse.move(bounds.x + 12, bounds.y + 12);
		await page.mouse.down();
		await page.mouse.move(bounds.x + 72, bounds.y + 42);
		await page.mouse.up();
		await expect.poll(() => content.getAttribute('style')).not.toBe(original);
		await page.getByRole('button', { name: 'พอดีจอ', exact: true }).click();
		await page.getByRole('button', { name: 'เลื่อนและซูมผังบริหาร', exact: true }).focus();
		await page.keyboard.press('ArrowRight');
		await expect.poll(() => content.getAttribute('style')).not.toBe(original);
		await page.keyboard.press('0');
		await expect.poll(() => content.getAttribute('style')).toBe(original);
		await page.mouse.move(bounds.x + 12, bounds.y + 12);
		await page.mouse.wheel(0, -100);
		await expect.poll(() => content.getAttribute('style')).not.toBe(original);
		await page.getByRole('button', { name: 'พอดีจอ', exact: true }).click();
		await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'instant' }));
		await page.screenshot({
			path: `/tmp/schoolorbit-student-chart-${width}-light.png`,
			fullPage: true,
			animations: 'disabled'
		});
		await page.evaluate(() => document.documentElement.classList.add('dark'));
		await page.screenshot({
			path: `/tmp/schoolorbit-student-chart-${width}-dark.png`,
			fullPage: true,
			animations: 'disabled'
		});
	});
}
