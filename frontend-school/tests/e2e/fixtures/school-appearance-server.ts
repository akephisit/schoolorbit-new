import { createServer as createHttpServer } from 'node:http';
import { createServer } from 'vite';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	DEFAULT_APPEARANCE,
	type SchoolAppearanceResponse
} from '../../../src/lib/appearance/model.js';

export async function startSchoolAppearanceServer() {
	let saved: SchoolAppearanceResponse = {
		appearance: { ...DEFAULT_APPEARANCE, palette: 'purple', font: 'noto', density: 'compact' },
		revision: 8
	};
	let delay = 0;
	let failed = false;
	const requests: { path: string; origin: string | undefined; cookie: string | undefined }[] = [];
	const api = createHttpServer((req, res) => {
		const pathname = new URL(req.url ?? '/', 'http://fixture').pathname;
		requests.push({ path: pathname, origin: req.headers.origin, cookie: req.headers.cookie });
		res.setHeader('access-control-allow-origin', req.headers.origin ?? '*');
		res.setHeader('access-control-allow-credentials', 'true');
		res.setHeader(
			'access-control-allow-headers',
			req.headers['access-control-request-headers'] ??
				'Content-Type,X-School-Subdomain,X-CSRF-Token'
		);
		res.setHeader('access-control-allow-methods', 'GET,PATCH,OPTIONS');
		res.setHeader('content-type', 'application/json');
		if (req.method === 'OPTIONS') {
			res.writeHead(204).end();
			return;
		}
		const respond = () => {
			if (res.destroyed) return;
			if (pathname === '/deployment-status') {
				res.end(JSON.stringify({ status: 'ready', releaseId: 'fixture' }));
				return;
			}
			if (pathname === '/api/school/public/appearance') {
				res
					.writeHead(failed ? 503 : 200)
					.end(
						JSON.stringify(
							failed ? { success: false, error: 'offline' } : { success: true, data: saved }
						)
					);
				return;
			}
			const data = pathname.endsWith('/organization')
				? { units: [] }
				: { schoolName: 'โรงเรียนรูปแบบทดสอบ', logoFileId: null };
			res.end(JSON.stringify({ success: true, data }));
		};
		if (pathname.endsWith('/appearance') && delay) setTimeout(respond, delay);
		else respond();
	});
	await new Promise<void>((done) => api.listen(0, '127.0.0.1', done));
	const address = api.address();
	if (!address || typeof address === 'string') throw new Error('Fixture API unavailable');
	process.env.PUBLIC_BACKEND_URL = `http://127.0.0.1:${address.port}`;
	process.env.PUBLIC_VAPID_KEY = 'test';
	const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
	const vite = await createServer({
		root,
		configFile: path.join(root, 'vite.config.ts'),
		cacheDir: path.join(root, 'node_modules/.vite-school-appearance-test'),
		logLevel: 'error',
		server: { host: '127.0.0.1', port: 0 }
	});
	await vite.listen();
	const frontend = vite.httpServer?.address();
	if (!frontend || typeof frontend === 'string') throw new Error('Fixture frontend unavailable');
	return {
		baseUrl: `http://127.0.0.1:${frontend.port}`,
		requests,
		get saved() {
			return structuredClone(saved);
		},
		set(value: SchoolAppearanceResponse) {
			saved = structuredClone(value);
		},
		reset() {
			saved = {
				appearance: { ...DEFAULT_APPEARANCE, palette: 'purple', font: 'noto', density: 'compact' },
				revision: 8
			};
			delay = 0;
			failed = false;
			requests.length = 0;
		},
		delay(value: number) {
			delay = value;
		},
		failure(value: boolean) {
			failed = value;
		},
		async close() {
			await vite.close();
			await new Promise<void>((done, reject) =>
				api.close((error) => (error ? reject(error) : done()))
			);
		}
	};
}
