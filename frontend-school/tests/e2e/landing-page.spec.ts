import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer as createHttpServer, type Server } from 'node:http';
import { createServer, type ViteDevServer } from 'vite';
import { crc32, deflateSync } from 'node:zlib';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
let devServer: ViteDevServer;
let apiServer: Server;
let baseUrl: string;
let failStatistics = false;
let failedRegion: string | undefined;
let empty = false;
let logoAvailable = true;
let brokenLogo = false;
let faviconDelayMs = 0;
let shareImageMode = 'ready';
let schoolName = 'โรงเรียนสาธิตทดสอบ';
let identityGate: Promise<void> | undefined;
let releaseIdentity: (() => void) | undefined;
let statisticsGate: Promise<void> | undefined;
let releaseStatistics: (() => void) | undefined;
let organizationGate: Promise<void> | undefined;
let releaseOrganization: (() => void) | undefined;
const requests: { path: string; cookie?: string; origin?: string }[] = [];
const shareImageQueries: string[] = [];
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
function fixtureShareImage() {
	const chunk = (type: string, data: Buffer) => {
		const name = Buffer.from(type);
		const size = Buffer.alloc(4);
		size.writeUInt32BE(data.length);
		const checksum = Buffer.alloc(4);
		checksum.writeUInt32BE(crc32(Buffer.concat([name, data])));
		return Buffer.concat([size, name, data, checksum]);
	};
	const header = Buffer.alloc(13);
	header.writeUInt32BE(1200, 0);
	header.writeUInt32BE(630, 4);
	header[8] = 8;
	header[9] = 6;
	return Buffer.concat([
		Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
		chunk('IHDR', header),
		chunk('IDAT', deflateSync(Buffer.alloc((1200 * 4 + 1) * 630))),
		chunk('IEND', Buffer.alloc(0))
	]);
}
const shareImage = fixtureShareImage();
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
const organizationFixture = {
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
				},
				{ name: 'รองผู้อำนวยการ ทดสอบ', positionCode: 'deputy_director', positionTitle: null }
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
				{ name: 'สมาชิก สอง', positionCode: 'member', positionTitle: null },
				...Array.from({ length: 3 }, (_, i) => ({
					name: `สมาชิกวิชาการ ${i + 3}`,
					positionCode: 'member',
					positionTitle: null
				}))
			]
		},
		{
			id: 'subject-group',
			parentId: 'academic',
			name: 'กลุ่มสาระคณิตศาสตร์',
			unitType: 'subject_group',
			members: [
				{ name: 'หัวหน้าสาระ ทดสอบ', positionCode: 'head', positionTitle: null },
				{ name: 'ครูคณิตศาสตร์ ทดสอบ', positionCode: 'member', positionTitle: 'ครู' },
				...Array.from({ length: 6 }, (_, i) => ({
					name: `ครูคณิตศาสตร์ ${i + 2}`,
					positionCode: 'member',
					positionTitle: 'ครู'
				}))
			]
		},
		{
			id: 'subject-child',
			parentId: 'subject-group',
			name: 'งานย่อยกลุ่มสาระ',
			unitType: 'work',
			members: []
		},
		{
			id: 'work',
			parentId: 'academic',
			name: 'งานวัดผล',
			unitType: 'work',
			members: [
				{ name: 'หัวหน้างาน ทดสอบ', positionCode: 'head', positionTitle: null },
				{
					name: 'สมาชิกงาน ชื่อยาวเพื่อทดสอบการตัดบรรทัดบนหน้าจอมือถือ',
					positionCode: 'member',
					positionTitle: 'เจ้าหน้าที่วัดผล'
				}
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

const organization = {
	units: organizationFixture.units.map((unit) => ({
		...unit,
		learningAreaId: unit.unitType === 'subject_group' ? 'linked-learning-area' : null,
		members: unit.members.map((member) => ({ ...member, personKey: member.name }))
	}))
};

test.describe.configure({ mode: 'serial', timeout: 60000 });
test.use({ serviceWorkers: 'block' });
test.beforeAll(async () => {
	// Compile the self-hosted Vite routes during fixture setup, outside user-interaction timing.
	test.setTimeout(120_000);
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
		if (endpoint === '/api/school/public/share-image') {
			shareImageQueries.push(new URL(req.url ?? '/', 'http://localhost').search);
			if (shareImageMode === 'timeout') {
				await new Promise<void>((resolve) => req.on('close', resolve));
				return;
			}
			if (shareImageMode === 'redirect') {
				res.writeHead(302, { location: '/untrusted-share-image' }).end();
				return;
			}
			if (failedRegion === endpoint) {
				res.writeHead(503).end();
				return;
			}
			res.setHeader('content-type', shareImageMode === 'mime' ? 'text/html' : 'image/png');
			if (shareImageMode === 'oversize') {
				res.setHeader('content-length', 2 * 1024 * 1024 + 1);
				res.end(Buffer.alloc(2 * 1024 * 1024 + 1));
			} else if (shareImageMode === 'chunked-oversize') {
				res.write(Buffer.alloc(1024 * 1024));
				res.end(Buffer.alloc(1024 * 1024 + 1));
			} else if (shareImageMode === 'invalid') {
				res.end('not a PNG');
			} else if (shareImageMode === 'square') {
				res.end(readFileSync(new URL('../../static/schoolorbit.png', import.meta.url)));
			} else {
				res.end(shareImage);
			}
			return;
		}
		if (endpoint === '/api/school/public/favicon') {
			if (faviconDelayMs) await new Promise((resolve) => setTimeout(resolve, faviconDelayMs));
			if (failedRegion === endpoint) {
				res.writeHead(503).end(JSON.stringify({ success: false, error: 'Fixture read failure' }));
			} else if (!logoAvailable || brokenLogo) {
				res.writeHead(404).end();
			} else {
				res.setHeader('content-type', 'image/png');
				const size = new URL(req.url ?? '/', 'http://localhost').searchParams.get('size') ?? '256';
				const file = size === '256' ? 'schoolorbit.png' : `schoolorbit-${size}.png`;
				res.end(readFileSync(new URL(`../../static/${file}`, import.meta.url)));
			}
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
		if (endpoint === '/api/school/public/organization' && organizationGate) await organizationGate;
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
	for (const route of ['/', '/login']) {
		const response = await fetch(`${baseUrl}${route}`);
		if (!response.ok) throw new Error(`Public fixture route ${route} failed to start`);
		await response.text();
	}
});
test.beforeEach(() => {
	requests.length = 0;
	shareImageQueries.length = 0;
	failStatistics = false;
	failedRegion = undefined;
	empty = false;
	logoAvailable = true;
	brokenLogo = false;
	faviconDelayMs = 0;
	shareImageMode = 'ready';
	schoolName = 'โรงเรียนสาธิตทดสอบ';
	identityGate = undefined;
	releaseIdentity = undefined;
	statisticsGate = undefined;
	releaseStatistics = undefined;
	organizationGate = undefined;
	releaseOrganization = undefined;
});
test.afterEach(() => {
	releaseOrganization?.();
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
	await expect(page.getByTestId('public-executive-team')).toHaveCount(0);
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
	expect(reads).toHaveLength(2);
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
		await expect(page).toHaveTitle(`${schoolName} - ระบบจัดการโรงเรียนครบวงจร`);
		await expect(page.locator('head title')).toHaveCount(1);
		await expect(page.locator('meta[name="description"]')).toHaveCount(1);
		await expect(page.locator('meta[name="description"]')).toHaveAttribute(
			'content',
			`${schoolName} เชื่อมครู นักเรียน และผู้ปกครอง ผ่านระบบจัดการโรงเรียนครบวงจร`
		);
		await expect(page.locator('meta[property="og:image"]')).toHaveAttribute(
			'content',
			new RegExp(`^${baseUrl}/og-image\\.png\\?v=3-[a-f0-9]+$`)
		);
		await expect(page.locator('meta[property="og:image:width"]')).toHaveAttribute(
			'content',
			'1200'
		);
		await expect(page.locator('meta[property="og:image:height"]')).toHaveAttribute(
			'content',
			'630'
		);
		await expect(page.locator('meta[property="og:image:type"]')).toHaveAttribute(
			'content',
			'image/png'
		);
		await expect(page.locator('meta[property="og:image:alt"]')).toHaveAttribute(
			'content',
			`${schoolName} - ระบบจัดการโรงเรียนครบวงจร`
		);
		await expect(page.locator('meta[name="twitter:card"]')).toHaveAttribute(
			'content',
			'summary_large_image'
		);
		expect(requests.some((r) => r.path === '/api/school/public/share-image')).toBe(false);
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
			logo: `${baseUrl}/favicon.png`
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
		'ซับน้อยเหนือวิทยาคม',
		'โรงเรียนทดสอบแห่งที่สอง',
		'โรงเรียน "ทดสอบ" & </script><script>window.seoInjection=true</script>'
	]) {
		schoolName = name;
		await page.goto(baseUrl);
		await expect(page).toHaveTitle(`${name} - ระบบจัดการโรงเรียนครบวงจร`);
		await expect(page.locator('meta[name="description"]')).toHaveAttribute(
			'content',
			`${name.startsWith('โรงเรียน') ? name : `โรงเรียน${name}`} เชื่อมครู นักเรียน และผู้ปกครอง ผ่านระบบจัดการโรงเรียนครบวงจร`
		);
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
	const missing = await request.get(`${baseUrl}/school-logo`, { maxRedirects: 0 });
	expect(missing.status()).toBe(307);
	expect(missing.headers().location).toBe('/schoolorbit.png');
	logoAvailable = true;
	brokenLogo = true;
	expect(
		(await request.get(`${baseUrl}/school-logo`, { maxRedirects: 0 })).headers().location
	).toBe('/schoolorbit.png');
	brokenLogo = false;
	failedRegion = '/api/school/public';
	expect((await request.get(`${baseUrl}/school-logo`, { maxRedirects: 0 })).status()).toBe(503);
});

test('social crawlers read one current-school wide PNG without session cookies or caller selectors', async ({
	request
}) => {
	const response = await request.get(
		`${baseUrl}/og-image.png?fileId=private-file&url=https://evil.example/image&v=old`,
		{
			maxRedirects: 0,
			headers: { Cookie: 'fixture-private-cookie=not-a-real-session' }
		}
	);
	expect(response.status()).toBe(200);
	expect(response.headers()['content-type']).toBe('image/png');
	expect(response.headers()['cache-control']).toBe('public, max-age=300');
	expect(response.headers()['x-content-type-options']).toBe('nosniff');
	expect(await response.body()).toEqual(shareImage);
	expect(shareImageQueries).toEqual(['']);
	expect(requests).toEqual([
		{ path: '/api/school/public/share-image', origin: baseUrl, cookie: undefined }
	]);
	// Missing/deleted crests are represented by a valid designed card from the backend.
	logoAvailable = false;
	expect((await request.get(`${baseUrl}/og-image.png`)).status()).toBe(200);
	brokenLogo = true;
	expect((await request.get(`${baseUrl}/og-image.png`)).status()).toBe(200);
});

for (const mode of ['mime', 'oversize', 'chunked-oversize', 'redirect', 'invalid', 'square']) {
	test(`social image rejects ${mode} responses without caching a failure`, async ({ request }) => {
		shareImageMode = mode;
		const response = await request.get(`${baseUrl}/og-image.png`, { maxRedirects: 0 });
		expect(response.status()).toBe(503);
		expect(response.headers()['cache-control']).toBe('no-store');
		expect(response.headers()['retry-after']).toBe('60');
		expect(response.headers().location).toBeUndefined();
		expect(requests.map((r) => r.path)).toEqual(['/api/school/public/share-image']);
		shareImageMode = 'ready';
		expect((await request.get(`${baseUrl}/og-image.png`)).status()).toBe(200);
	});
}

test('social image upstream failures and timeout are uncached and retryable', async ({
	request
}) => {
	failedRegion = '/api/school/public/share-image';
	const unavailable = await request.get(`${baseUrl}/og-image.png`);
	expect(unavailable.status()).toBe(503);
	expect(unavailable.headers()['cache-control']).toBe('no-store');
	expect(unavailable.headers()['retry-after']).toBe('60');
	failedRegion = undefined;
	shareImageMode = 'timeout';
	const started = Date.now();
	const timeout = await request.get(`${baseUrl}/og-image.png`);
	expect(Date.now() - started).toBeLessThan(20_000);
	expect(timeout.status()).toBe(503);
	expect(timeout.headers()['cache-control']).toBe('no-store');
	expect(timeout.headers()['retry-after']).toBe('60');
	shareImageMode = 'ready';
	expect((await request.get(`${baseUrl}/og-image.png`)).status()).toBe(200);
});

test('social image proxy forwards the current host origin for each tenant', async ({ request }) => {
	for (const host of ['alpha.localhost', 'beta.localhost']) {
		const response = await request.get(`${baseUrl}/og-image.png`, { headers: { Host: host } });
		expect(response.status()).toBe(200);
		expect(requests.at(-1)).toEqual({
			path: '/api/school/public/share-image',
			origin: `http://${host}`,
			cookie: undefined
		});
	}
});

test('protected route initial HTML keeps menu titles and wide public card without identity reads', async ({
	request
}) => {
	const response = await request.get(`${baseUrl}/staff/academic/assessments`);
	expect(response.status()).toBe(200);
	const html = await response.text();
	expect(html).toContain('<title>โครงสร้างคะแนน - SchoolOrbit</title>');
	expect(html).toContain(`property="og:image" content="${baseUrl}/og-image.png"`);
	expect(html).toContain('property="og:image:width" content="1200"');
	expect(html).toContain('property="og:image:height" content="630"');
	expect(html).toContain('name="twitter:card" content="summary_large_image"');
	expect(requests).toHaveLength(0);
});

test('favicon reads only this school crest and uses the Admin brand when no crest is available', async ({
	request
}) => {
	const icon = await request.get(`${baseUrl}/favicon.png?fileId=private-file`, {
		headers: { Cookie: 'fixture-private-cookie=not-a-session' },
		maxRedirects: 0
	});
	expect(icon.status()).toBe(200);
	expect(icon.headers()['content-type']).toBe('image/png');
	expect(await icon.body()).toEqual(
		readFileSync(new URL('../../static/schoolorbit.png', import.meta.url))
	);
	expect(requests.map((r) => r.path)).toEqual(['/api/school/public/favicon']);
	expect(requests.every((r) => !r.cookie && r.origin === baseUrl)).toBe(true);
	logoAvailable = false;
	const fallback = await request.get(`${baseUrl}/favicon.png`, { maxRedirects: 0 });
	expect(fallback.status()).toBe(307);
	expect(fallback.headers().location).toBe('/schoolorbit.png');
	logoAvailable = true;
	brokenLogo = true;
	expect(
		(await request.get(`${baseUrl}/favicon.png`, { maxRedirects: 0 })).headers().location
	).toBe('/schoolorbit.png');
	brokenLogo = false;
	failedRegion = '/api/school/public/favicon';
	const unavailable = await request.get(`${baseUrl}/favicon.png`, { maxRedirects: 0 });
	expect(unavailable.status()).toBe(503);
	expect(unavailable.headers()['cache-control']).toBe('no-store');
});

test('installation manifest and Apple icon use this school PNGs at their declared sizes', async ({
	request,
	page
}) => {
	await page.goto(`${baseUrl}/login`);
	await expect(page.locator('link[rel="icon"]')).toHaveCount(1);
	await expect(page.locator('link[rel="icon"]')).toHaveAttribute('href', '/favicon.png');
	await expect(page.locator('link[rel="apple-touch-icon"]')).toHaveAttribute(
		'href',
		'/app-icon/192.png'
	);
	const manifestPath = await page.locator('link[rel="manifest"]').getAttribute('href');
	expect(manifestPath).toBe('/manifest.webmanifest');
	const session = await page.context().newCDPSession(page);
	const parsed = await session.send('Page.getAppManifest');
	expect(parsed.errors).toEqual([]);
	const manifest = JSON.parse(parsed.data ?? '{}');
	expect(manifest.icons).toEqual(
		[192, 512].map((size) => ({
			src: `/app-icon/${size}.png`,
			sizes: `${size}x${size}`,
			type: 'image/png',
			purpose: 'any'
		}))
	);
	for (const size of [192, 512]) {
		const icon = await request.get(`${baseUrl}/app-icon/${size}.png?fileId=private-file`, {
			headers: { Cookie: 'fixture-private-cookie=not-a-session' },
			maxRedirects: 0
		});
		expect(icon.status()).toBe(200);
		expect(icon.headers()['content-type']).toBe('image/png');
		const bytes = await icon.body();
		expect(bytes.readUInt32BE(16)).toBe(size);
		expect(bytes.readUInt32BE(20)).toBe(size);
	}
	expect(
		requests
			.filter((r) => r.path === '/api/school/public/favicon')
			.every((r) => !r.cookie && r.origin === baseUrl)
	).toBe(true);
	await session.detach();
});

test('installation icons use the Admin brand at the correct size for absent or deleted crests', async ({
	request
}) => {
	for (const absent of ['not-configured', 'deleted']) {
		logoAvailable = absent !== 'not-configured';
		brokenLogo = absent === 'deleted';
		for (const size of [192, 512]) {
			const icon = await request.get(`${baseUrl}/app-icon/${size}.png`, { maxRedirects: 0 });
			expect(icon.status()).toBe(307);
			expect(icon.headers().location).toBe(`/schoolorbit-${size}.png`);
			const png = await request.get(`${baseUrl}${icon.headers().location}`);
			const bytes = await png.body();
			expect(bytes.readUInt32BE(16)).toBe(size);
			expect(bytes.readUInt32BE(20)).toBe(size);
		}
	}
	logoAvailable = true;
	brokenLogo = false;
	requests.length = 0;
	const invalid = await request.get(`${baseUrl}/app-icon/4096.png`);
	expect(invalid.status()).toBe(404);
	expect(requests).toHaveLength(0);
	failedRegion = '/api/school/public/favicon';
	const unavailable = await request.get(`${baseUrl}/app-icon/192.png`);
	expect(unavailable.status()).toBe(503);
	expect(unavailable.headers()['cache-control']).toBe('no-store');
});

test('a cold favicon response can take ten seconds without becoming a false outage', async ({
	request
}) => {
	faviconDelayMs = 10_000;
	const response = await request.get(`${baseUrl}/favicon.png`, { maxRedirects: 0 });
	expect(response.status()).toBe(200);
	expect(response.headers()['content-type']).toBe('image/png');
	expect(await response.body()).toEqual(
		readFileSync(new URL('../../static/schoolorbit.png', import.meta.url))
	);
	expect(requests.map((r) => r.path)).toEqual(['/api/school/public/favicon']);
});

test('a four-second identity read still provides the school title in the first HTML', async ({
	browser
}) => {
	identityGate = new Promise<void>((done) => (releaseIdentity = done));
	const release = setTimeout(() => releaseIdentity?.(), 4_000);
	const context = await browser.newContext({ javaScriptEnabled: false });
	try {
		const page = await context.newPage();
		await page.goto(baseUrl);
		await expect(page).toHaveTitle(`${schoolName} - ระบบจัดการโรงเรียนครบวงจร`);
		await expect(
			page.getByRole('heading', { name: schoolName, exact: true, level: 1 })
		).toBeVisible();
		expect(requests.filter((r) => isRegionRead(r.path))).toHaveLength(2);
	} finally {
		clearTimeout(release);
		await context.close();
	}
});

test('a slow identity read is bounded while sibling reads start concurrently and retries remain local', async ({
	page
}) => {
	identityGate = new Promise<void>((done) => (releaseIdentity = done));
	const started = Date.now();
	const navigation = page.goto(baseUrl);
	await expect.poll(() => requests.filter((r) => isRegionRead(r.path)).length).toBe(2);
	await navigation;
	expect(Date.now() - started).toBeGreaterThanOrEqual(7_500);
	expect(Date.now() - started).toBeLessThan(11_000);
	await expect(page.getByText('โหลดข้อมูลโรงเรียนไม่สำเร็จ', { exact: true })).toBeVisible();
	await expect(page.getByTestId('school-statistics')).toContainText('124');
	await expect(page.getByTestId('public-executive-team')).toHaveCount(0);
	await expect(page).toHaveTitle('เว็บไซต์โรงเรียน - ระบบจัดการโรงเรียนครบวงจร');
	await expect(page.locator('script[type="application/ld+json"]')).toHaveCount(0);
	releaseIdentity?.();
	identityGate = undefined;
	const before = requests.filter((r) => isRegionRead(r.path)).length;
	await page.getByRole('button', { name: 'ลองใหม่: ข้อมูลโรงเรียน' }).click();
	await expect(
		page.getByRole('heading', { name: schoolName, exact: true, level: 1 })
	).toBeVisible();
	await expect(page).toHaveTitle(`${schoolName} - ระบบจัดการโรงเรียนครบวงจร`);
	expect(
		requests
			.filter((r) => isRegionRead(r.path))
			.slice(before)
			.map((r) => r.path)
	).toEqual(['/api/school/public']);
});

test('grade disclosures preserve all rooms, grade and stage subtotals', async ({ page }) => {
	await page.goto(baseUrl);
	await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
	const table = page.getByTestId('school-student-summary');
	await expect(page.locator('#statistics table')).toHaveCount(1);
	await expect(table.getByRole('button')).toHaveCount(2);
	for (const name of ['มัธยมศึกษาปีที่ 1', 'มัธยมศึกษาปีที่ 2']) {
		const toggle = table.getByRole('button', { name, exact: true });
		await expect(toggle).toHaveAttribute('aria-expanded', 'false');
		await toggle.click();
		await expect(toggle).toHaveAttribute('aria-expanded', 'true');
	}
	await expect(page.getByRole('rowheader', { name: /ยังไม่ได้จัดห้อง/ })).toBeVisible();
	await expect(table.getByRole('row', { name: 'ม.2/2 0 0 0 0', exact: true })).toBeVisible();
	await expect
		.poll(() =>
			table
				.locator('tr')
				.evaluateAll((rows) =>
					rows.map((row) =>
						Array.from(row.querySelectorAll('th, td'), (cell) => cell.textContent?.trim())
					)
				)
		)
		.toEqual([
			['ชั้น / ห้อง', 'ชาย', 'หญิง', 'อื่น ๆ / ไม่ระบุ', 'รวม'],
			['มัธยมศึกษาปีที่ 1', '31', '32', '1', '64'],
			['ม.1/1', '16', '16', '1', '33'],
			['ม.1/2', '15', '15', '0', '30'],
			['มัธยมศึกษาปีที่ 1 · ยังไม่ได้จัดห้อง', '0', '1', '0', '1'],
			['มัธยมศึกษาปีที่ 2', '30', '30', '0', '60'],
			['ม.2/1', '30', '30', '0', '60'],
			['ม.2/2', '0', '0', '0', '0'],
			['รวมมัธยมศึกษาตอนต้น', '61', '62', '1', '124'],
			['รวมทั้งโรงเรียน', '61', '62', '1', '124']
		]);
});

test('a failed statistics read can retry without refetching identity', async ({ page }) => {
	failStatistics = true;
	await page.goto(baseUrl);
	await expect(
		page.getByRole('heading', { name: 'โรงเรียนสาธิตทดสอบ', exact: true, level: 1 })
	).toBeVisible();
	await expect(page.getByTestId('public-executive-team')).toHaveCount(0);
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

for (const [endpoint, label] of [['/api/school/public', 'ข้อมูลโรงเรียน']]) {
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
		await expect(page.getByTestId('public-executive-team')).toHaveCount(0);
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
	await page.goto(`${baseUrl}/management-groups`);
	for (const name of [
		'หัวหน้า ทดสอบ',
		'รองหัวหน้า ทดสอบ',
		'ผู้ประสานงาน ทดสอบ',
		'สมาชิก หนึ่ง',
		'สมาชิก สอง'
	]) {
		await expect(page.getByText(name, { exact: true })).toBeVisible();
	}
	await expect(
		page
			.getByRole('list', { name: 'สมาชิก กลุ่มบริหารวิชาการ', exact: true })
			.getByTestId('public-person')
	).toHaveCount(9);
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
		await expect(
			page.getByTestId('school-brand').getByRole('img', { name: 'SchoolOrbit', exact: true })
		).toHaveAttribute('src', '/schoolorbit.png');
		await expect(
			page.getByTestId('hero-school-crest').getByRole('img', { name: 'SchoolOrbit', exact: true })
		).toBeVisible();
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
	await expect(page.getByTestId('public-executive-team')).toHaveCount(0);
	releaseStatistics?.();
	await expect(page.getByTestId('school-statistics')).toContainText('124');
});

test('missing academic context shows an honest empty state', async ({ page }) => {
	empty = true;
	await page.goto(baseUrl);
	await expect(
		page.getByRole('heading', { name: 'ยังไม่มีปีการศึกษาที่เปิดใช้งาน' })
	).toBeVisible();
	await expect(page.getByTestId('school-statistics')).toContainText('—');
	await page.screenshot({
		path: '/tmp/schoolorbit-public-empty-desktop.png',
		fullPage: true,
		animations: 'disabled'
	});
});

for (const width of [320, 375, 768, 1280]) {
	test(`public personnel navigation and portraits work at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		await page.goto(baseUrl);
		await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
		await expect(page.getByTestId('school-statistics')).toContainText('124');
		await expect(page.getByTestId('public-executive-team')).toHaveCount(0);
		if (width < 1024) await page.getByRole('button', { name: 'เปิดเมนูโรงเรียน' }).click();
		await page
			.getByRole('navigation', { name: 'เมนูโรงเรียน' })
			.getByRole('link', { name: 'คณะผู้บริหาร', exact: true })
			.click();
		await expect(page).toHaveURL(/\/executives$/);
		await expect(
			page.getByRole('heading', { name: 'คณะผู้บริหาร', exact: true, level: 1 })
		).toBeVisible();
		if (width < 1024) await page.getByRole('button', { name: 'เปิดเมนูโรงเรียน' }).click();
		await expect(page.getByRole('link', { name: 'คณะผู้บริหาร', exact: true })).toHaveAttribute(
			'aria-current',
			'page'
		);
		if (width < 1024) await page.getByRole('button', { name: 'ปิดเมนูโรงเรียน' }).click();
		await expect(page.getByRole('link', { name: 'กลับหน้าหลัก' })).toHaveCount(0);
		const team = page.getByTestId('public-executive-team');
		await expect(team.getByTestId('public-person')).toHaveCount(3);
		await expect(team.getByRole('heading')).toHaveCount(0);
		await expect(team.getByText('รองผู้อำนวยการ ทดสอบ', { exact: true })).toBeVisible();
		const photo = page.getByRole('img', { name: 'รูป ผู้บริหาร ทดสอบ' });
		const photoSize = await photo.evaluate((node) => ({
			width: node.getBoundingClientRect().width,
			height: node.getBoundingClientRect().height,
			rem: parseFloat(getComputedStyle(document.documentElement).fontSize)
		}));
		expect(photoSize.width).toBeGreaterThanOrEqual(
			photoSize.rem * (width < 375 ? 7 : width < 640 ? 8 : width < 1024 ? 9 : 10) - 1
		);
		expect(Math.abs(photoSize.width - photoSize.height)).toBeLessThan(1);
		await expect(team.getByText('หัวหน้า ทดสอบ', { exact: true })).toBeVisible();
		await expect(team.getByText('สมาชิก หนึ่ง', { exact: true })).toHaveCount(0);
		await expect(team.getByText('กลุ่มสาระคณิตศาสตร์', { exact: true })).toHaveCount(0);
		await expect(page.getByRole('img', { name: 'รูป ผู้บริหาร ทดสอบ' })).toBeVisible();
		await expect(page.getByRole('button', { name: 'ซูมเข้า' })).toHaveCount(0);
		for (const theme of ['light', 'dark']) {
			await page.evaluate(
				(dark) => document.documentElement.classList.toggle('dark', dark),
				theme === 'dark'
			);
			await page.screenshot({
				path: `/tmp/schoolorbit-executives-${width}-${theme}.png`,
				fullPage: true,
				animations: 'disabled'
			});
		}
		await page.evaluate(() => document.documentElement.classList.remove('dark'));
		if (width < 1024) {
			const toggle = page.getByRole('button', { name: 'เปิดเมนูโรงเรียน' });
			await toggle.focus();
			await page.keyboard.press('Enter');
			await expect(page.getByRole('button', { name: 'ปิดเมนูโรงเรียน' })).toHaveAttribute(
				'aria-expanded',
				'true'
			);
		}
		await page
			.getByRole('navigation', { name: 'เมนูโรงเรียน' })
			.getByRole('link', { name: 'กลุ่มบริหาร', exact: true })
			.click();
		await expect(page).toHaveURL(/\/management-groups$/);
		await expect(
			page.getByRole('heading', { level: 1, name: 'กลุ่มบริหาร', exact: true })
		).toBeVisible();
		await expect(page.getByText('สมาชิก หนึ่ง', { exact: true })).toBeVisible();
		await expect(
			page
				.getByRole('list', { name: 'สมาชิก กลุ่มบริหารวิชาการ', exact: true })
				.getByText('หัวหน้างาน ทดสอบ', { exact: true })
		).toBeVisible();
		await expect(
			page
				.getByRole('list', { name: 'สมาชิก กลุ่มบริหารวิชาการ', exact: true })
				.getByText('เจ้าหน้าที่วัดผล', { exact: true })
		).toBeVisible();
		await expect(
			page.getByRole('heading', { name: 'กลุ่มสาระคณิตศาสตร์', exact: true })
		).toHaveCount(0);
		await page
			.getByRole('navigation', { name: 'เลือกกลุ่มบริหาร' })
			.getByRole('link', { name: 'กลุ่มบริหารทั่วไป' })
			.click();
		await expect(page).toHaveURL(/\/management-groups#unit-empty$/);
		await expect(page.getByText('ยังไม่มีรายชื่อบุคลากรในหน่วยงานนี้')).toBeVisible();
		if (width < 1024) await page.getByRole('button', { name: 'เปิดเมนูโรงเรียน' }).click();
		await page
			.getByRole('navigation', { name: 'เมนูโรงเรียน' })
			.getByRole('link', { name: 'กลุ่มสาระ', exact: true })
			.click();
		await expect(page).toHaveURL(/\/subject-groups$/);
		await expect(
			page.getByRole('heading', { level: 1, name: 'กลุ่มสาระ', exact: true })
		).toBeVisible();
		await expect(page.getByText('หัวหน้าสาระ ทดสอบ', { exact: true })).toBeVisible();
		await expect(page.getByText('ครูคณิตศาสตร์ ทดสอบ', { exact: true })).toBeVisible();
		for (const route of ['management-groups', 'subject-groups']) {
			await page.goto(`${baseUrl}/${route}`);
			await expect(page.getByTestId('public-personnel-directory')).toBeVisible();
			await expect(page.getByRole('link', { name: 'กลับหน้าหลัก' })).toHaveCount(0);
			const memberLists = page.getByRole('list', { name: /^สมาชิก / });
			for (const list of await memberLists.all()) {
				const geometry = await list.evaluate((node) => {
					const bounds = node.getBoundingClientRect();
					const rows = new Map<number, { left: number; right: number }[]>();
					for (const item of node.children) {
						const rect = item.getBoundingClientRect();
						const top = Math.round(rect.top);
						const row = rows.get(top) ?? [];
						row.push({ left: rect.left, right: rect.right });
						rows.set(top, row);
					}
					return { center: (bounds.left + bounds.right) / 2, rows: [...rows.values()] };
				});
				for (const row of geometry.rows) {
					expect(
						Math.abs((row[0].left + row[row.length - 1].right) / 2 - geometry.center)
					).toBeLessThan(1);
				}
			}
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			const portraits = await page.getByTestId('public-person').evaluateAll((nodes) =>
				nodes.map((node) => {
					const rect = node.getBoundingClientRect();
					return { left: rect.left, right: rect.right, width: innerWidth };
				})
			);
			expect(portraits.every((rect) => rect.left >= 0 && rect.right <= rect.width)).toBe(true);
			for (const theme of ['light', 'dark']) {
				await page.evaluate(
					(dark) => document.documentElement.classList.toggle('dark', dark),
					theme === 'dark'
				);
				await page.screenshot({
					path: `/tmp/schoolorbit-${route}-${width}-${theme}.png`,
					fullPage: true,
					animations: 'disabled'
				});
			}
		}
	});
}

for (const route of ['executives', 'management-groups', 'subject-groups']) {
	const rosterTestId =
		route === 'executives' ? 'public-executive-team' : 'public-personnel-directory';
	test(`${route} streams its roster without blocking the page heading`, async ({ page }) => {
		organizationGate = new Promise<void>((done) => (releaseOrganization = done));
		await page.goto(`${baseUrl}/${route}`, { waitUntil: 'commit' });
		await expect(
			page.getByRole('heading', {
				level: 1,
				name:
					route === 'executives'
						? 'คณะผู้บริหาร'
						: route === 'management-groups'
							? 'กลุ่มบริหาร'
							: 'กลุ่มสาระ',
				exact: true
			})
		).toBeVisible();
		await expect(page.getByRole('status', { name: 'กำลังโหลดรายชื่อบุคลากร' })).toBeVisible();
		releaseOrganization?.();
		await expect(page.getByTestId(rosterTestId)).toBeVisible();
	});
	test(`${route} reads only identity and organization and retries its own region`, async ({
		page
	}) => {
		failedRegion = '/api/school/public/organization';
		await page.goto(`${baseUrl}/${route}`);
		await expect(page.getByRole('alert')).toContainText('โหลดรายชื่อบุคลากรไม่สำเร็จ');
		expect(
			requests
				.filter((r) => isRegionRead(r.path))
				.map((r) => r.path)
				.sort()
		).toEqual(['/api/school/public', '/api/school/public/organization']);
		const before = requests.filter((r) => isRegionRead(r.path)).length;
		failedRegion = undefined;
		await page.getByRole('button', { name: 'ลองใหม่: รายชื่อบุคลากร' }).click();
		await expect(page.getByTestId(rosterTestId)).toBeVisible();
		expect(
			requests
				.filter((r) => isRegionRead(r.path))
				.slice(before)
				.map((r) => r.path)
		).toEqual(['/api/school/public/organization']);
		await page.reload();
		await expect(page.getByTestId(rosterTestId)).toBeVisible();
		expect(requests.every((r) => !r.cookie)).toBe(true);
	});
	test(`${route} has an honest empty state`, async ({ page }) => {
		empty = true;
		await page.goto(`${baseUrl}/${route}`);
		await expect(
			page.getByRole('heading', {
				name:
					route === 'executives'
						? 'ยังไม่มีข้อมูลคณะผู้บริหาร'
						: route === 'management-groups'
							? 'ยังไม่มีข้อมูลกลุ่มบริหาร'
							: 'ยังไม่มีข้อมูลกลุ่มสาระ'
			})
		).toBeVisible();
	});
}

test('personnel identity retry updates the shared brand while its roster stays visible', async ({
	page
}) => {
	failedRegion = '/api/school/public';
	await page.goto(`${baseUrl}/executives`);
	await expect(page.getByRole('alert')).toContainText('โหลดข้อมูลโรงเรียนไม่สำเร็จ');
	await expect(page.getByTestId('public-executive-team').getByTestId('public-person')).toHaveCount(
		3
	);
	const before = requests.filter((r) => isRegionRead(r.path)).length;
	failedRegion = undefined;
	await page.getByRole('button', { name: 'ลองใหม่: ข้อมูลโรงเรียน' }).click();
	await expect(page.getByTestId('school-brand')).toContainText(schoolName);
	await expect(page).toHaveTitle(`คณะผู้บริหาร — ${schoolName}`);
	expect(
		requests
			.filter((r) => isRegionRead(r.path))
			.slice(before)
			.map((r) => r.path)
	).toEqual(['/api/school/public']);
	await expect(page.getByRole('alert')).toHaveCount(0);
});
