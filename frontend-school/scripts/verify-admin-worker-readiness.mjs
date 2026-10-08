import { readFile, readdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from 'playwright';

const origin = new URL(process.env.ADMIN_ORIGIN);
const directory = path.resolve('../frontend-admin/build/client');
async function assets(directory, prefix = '') {
	const result = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const relative = `${prefix}${entry.name}`;
		if (entry.isDirectory())
			result.push(...(await assets(path.join(directory, entry.name), `${relative}/`)));
		else if (relative.startsWith('_app/immutable/') && /\.(js|css)$/.test(relative))
			result.push(relative);
	}
	return result;
}
const expected = await assets(directory);
if (expected.length === 0) throw new Error('Admin immutable assets are missing');
for (let attempt = 1; attempt <= 12; attempt++) {
	try {
		const response = await fetch(origin, { signal: AbortSignal.timeout(15000), cache: 'no-store' });
		if (
			response.status !== 503 ||
			!response.headers.get('cache-control')?.includes('no-store') ||
			!(await response.text()).includes('กำลังปรับปรุงระบบ')
		)
			throw new Error('Admin maintenance document is not active');
		for (let start = 0; start < expected.length; start += 8) {
			await Promise.all(
				expected.slice(start, start + 8).map(async (file) => {
					const response = await fetch(new URL(file, origin), {
						signal: AbortSignal.timeout(15000)
					});
					if (!response.ok) throw new Error('Admin asset is unavailable');
					const deployed = createHash('sha256')
						.update(Buffer.from(await response.arrayBuffer()))
						.digest('hex');
					const prepared = createHash('sha256')
						.update(await readFile(path.join(directory, file)))
						.digest('hex');
					if (deployed !== prepared)
						throw new Error('Admin deployed asset differs from prepared bundle');
				})
			);
		}
		break;
	} catch (error) {
		if (attempt === 12) throw error;
		console.error(`Admin Worker propagation is incomplete (attempt ${attempt}/12)`);
		await delay(5000);
	}
}
const browser = await chromium.launch({ headless: true });
try {
	const page = await browser.newPage();
	const errors = [];
	page.on('pageerror', (error) => errors.push(error));
	const response = await page.goto(origin.href, { waitUntil: 'domcontentloaded', timeout: 60000 });
	if (response?.status() !== 503) throw new Error('Admin browser must remain in maintenance');
	await page.getByRole('heading', { name: 'กำลังปรับปรุงระบบ', exact: true }).waitFor();
	if (errors.length) throw new Error('Admin maintenance page has a browser error');
} finally {
	await browser.close();
}
console.log(`Admin maintenance document, ${expected.length} prepared assets and browser passed.`);
