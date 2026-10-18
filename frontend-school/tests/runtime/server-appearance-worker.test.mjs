import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

// Reuse the locked production Worker toolchain, including its bundler and workerd.
const wranglerRequire = createRequire(import.meta.resolve('wrangler/package.json'));
const { build } = wranglerRequire('esbuild');
const { Miniflare, convertV4MiniflareOptions } = wranglerRequire('miniflare');

const appRoot = fileURLToPath(new URL('../../', import.meta.url));

async function workerFixture(t) {
	// Bundle the production route, API transport, validation and CSS serializer. Only
	// Kit's generated environment modules are supplied by this isolated harness.
	const bundle = await build({
		absWorkingDir: appRoot,
		stdin: {
			resolveDir: appRoot,
			contents: `
import { GET } from './src/routes/school-appearance.css/+server.ts';
export default {
  fetch(request) {
    return GET({
      request,
      url: new URL(request.url),
      // Kit normalizes event.fetch inputs into native Requests before transport.
      fetch: (input, init) => fetch(new Request(input, init))
    });
  }
};`
		},
		bundle: true,
		write: false,
		format: 'esm',
		platform: 'neutral',
		conditions: ['worker', 'default'],
		plugins: [
			{
				name: 'kit-generated-environment',
				setup(builder) {
					builder.onResolve({ filter: /^\$app\// }, ({ path }) => ({
						path,
						namespace: 'kit-environment'
					}));
					builder.onLoad({ filter: /.*/, namespace: 'kit-environment' }, ({ path }) => {
						const modules = {
							'$app/env': 'export const browser = false;',
							'$app/paths': 'export const resolve = (path) => path;',
							'$app/env/public': `export const PUBLIC_SCHOOL_SUBDOMAIN = '';
export const PUBLIC_BACKEND_URL = 'https://school-api.schoolorbit.test';`
						};
						assert.ok(path in modules, `Unexpected Kit environment import: ${path}`);
						return { contents: modules[path], loader: 'js' };
					});
				}
			}
		]
	});
	const saved = new Map([
		[
			'https://first.schoolorbit.test',
			{
				appearance: { palette: 'blue', font: 'noto', density: 'compact', primary: null },
				revision: 1
			}
		],
		[
			'https://second.schoolorbit.test',
			{
				appearance: {
					palette: 'purple',
					font: 'sarabun',
					density: 'comfortable',
					primary: '#23b2ee'
				},
				revision: 5
			}
		]
	]);
	const requests = [];
	let status = 200;
	const worker = new Miniflare(
		convertV4MiniflareOptions({
			modules: true,
			script: bundle.outputFiles[0].text,
			// Match the School production release configuration.
			compatibilityDate: '2025-09-15',
			compatibilityFlags: ['nodejs_compat'],
			outboundService: async (request) => {
				if (request.url === 'https://school-api.schoolorbit.test/deployment-status')
					return Response.json({ status: 'ready', releaseId: null });
				requests.push(request);
				assert.equal(
					request.url,
					'https://school-api.schoolorbit.test/api/school/public/appearance'
				);
				const data = saved.get(request.headers.get('origin'));
				assert.ok(data, 'The public request must identify its document tenant');
				return Response.json(
					{ success: true, data },
					{
						status,
						headers: status >= 300 ? { Location: 'https://untrusted.schoolorbit.test/' } : {}
					}
				);
			}
		})
	);
	t.after(() => worker.dispose());
	return { worker, saved, requests, setStatus: (value) => (status = value) };
}

test(
	'production appearance route serves fresh tenant CSS in the Cloudflare runtime',
	{ timeout: 15_000 },
	async (t) => {
		const { worker, saved, requests } = await workerFixture(t);
		const url = 'https://first.schoolorbit.test/school-appearance.css';
		const first = await worker.dispatchFetch(url, {
			headers: { Cookie: 'private-session=fixture', Authorization: 'Bearer fixture' }
		});
		assert.equal(first.status, 200);
		assert.equal(first.headers.get('cache-control'), 'private, no-cache, must-revalidate');
		const etag = first.headers.get('etag');
		assert.ok(etag);
		const css = await first.text();
		assert.match(css, /--school-appearance-palette:blue;/);
		assert.match(css, /--school-appearance-font:noto;/);
		assert.match(css, /--school-appearance-density:compact;/);
		assert.match(css, /--school-appearance-revision:1;/);
		const unchanged = await worker.dispatchFetch(url, { headers: { 'If-None-Match': etag } });
		assert.equal(unchanged.status, 304);
		assert.equal(await unchanged.text(), '');
		const other = await worker.dispatchFetch(
			'https://second.schoolorbit.test/school-appearance.css',
			{
				headers: { 'If-None-Match': etag }
			}
		);
		assert.equal(other.status, 200);
		assert.match(await other.text(), /--school-appearance-font:sarabun;/);
		assert.notEqual(other.headers.get('etag'), etag);
		saved.set('https://first.schoolorbit.test', {
			appearance: { palette: 'purple', font: 'sarabun', density: 'compact', primary: null },
			revision: 2
		});
		const updated = await worker.dispatchFetch(url, { headers: { 'If-None-Match': etag } });
		assert.equal(updated.status, 200);
		assert.match(await updated.text(), /--school-appearance-revision:2;/);
		assert.notEqual(updated.headers.get('etag'), etag);
		assert.equal(requests.length, 4, 'Every document revalidates against the backend');
		for (const request of requests) {
			for (const header of ['cookie', 'authorization', 'x-csrf-token'])
				assert.equal(request.headers.get(header), null, `Public reads must omit ${header}`);
		}
	}
);

test(
	'Cloudflare appearance reads reject redirects and leave failures uncached',
	{ timeout: 15_000 },
	async (t) => {
		const { worker, requests, setStatus } = await workerFixture(t);
		for (const status of [301, 302, 303, 307, 308, 503]) {
			setStatus(status);
			const response = await worker.dispatchFetch(
				'https://first.schoolorbit.test/school-appearance.css'
			);
			assert.equal(response.status, 200);
			assert.equal(response.headers.get('cache-control'), 'no-store');
			assert.equal(response.headers.get('etag'), null);
			assert.match(await response.text(), /--school-appearance-revision:0;/);
		}
		assert.equal(requests.length, 6, 'No redirect destination may be followed');
		setStatus(200);
		const recovered = await worker.dispatchFetch(
			'https://first.schoolorbit.test/school-appearance.css'
		);
		assert.match(await recovered.text(), /--school-appearance-revision:1;/);
		assert.equal(recovered.headers.get('cache-control'), 'private, no-cache, must-revalidate');
	}
);
