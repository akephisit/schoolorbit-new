import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { createServer, type ServerResponse } from 'node:http';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { createServer as createViteServer, type Plugin, type ViteDevServer } from 'vite';
import { renderProxy } from '../../static/helpers/render-proxy.mjs';

const exec = promisify(execFile);
const nginxImage =
	'public.ecr.aws/docker/library/nginx:stable-alpine@sha256:0985e772fb9f729e6fa0980da05fca5d9c468e870eed43071545afa9d2e27d94';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const entry = 'virtual:notification-recovery-proxy';
export const sessionCookie = 'synthetic-notification-session';

interface RequestRecord {
	path: string;
	method: string;
	cookie: string;
	origin: string;
	csrf: string;
}

function block(source: string, start: string): string {
	const index = source.indexOf(start);
	if (index < 0) throw new Error(`Rendered proxy is missing ${start}`);
	const opening = source.indexOf('{', index);
	let depth = 1;
	let end = opening + 1;
	while (end < source.length && depth) {
		if (source[end] === '{') depth++;
		if (source[end] === '}') depth--;
		end++;
	}
	if (depth) throw new Error(`Rendered proxy has an unterminated ${start}`);
	return source.slice(index, end);
}

function harnessPage(): Plugin {
	return {
		name: 'notification-recovery-proxy-fixture',
		enforce: 'pre',
		resolveId(id) {
			if (id === entry) return `\0${entry}`;
		},
		load(id) {
			if (id !== `\0${entry}`) return;
			return `
import { notificationStore } from '/src/lib/stores/notification.ts';
import { workStore } from '/src/lib/stores/work.ts';
import { authStore } from '/src/lib/stores/auth.ts';
import { authAPI } from '/src/lib/api/auth.ts';
const show = (id, value) => document.querySelector('#' + id).textContent = String(value);
notificationStore.subscribe(state => {
  show('unread', state.unreadCount);
  show('inbox', state.notifications.map(item => item.id).join(','));
});
workStore.subscribe(state => {
  show('work', state.counts.total);
  show('windows', state.windowsRevision);
});
authStore.subscribe(state => {
  show('identity', state.isAuthenticated ? (state.isUnavailable ? 'unavailable' : state.user.id) : 'signed-out');
  if (!state.isAuthenticated) {
    notificationStore.closeSSE();
    workStore.reset();
  }
});
document.querySelector('#start').onclick = async () => {
  try {
    if (await authAPI.refreshCurrentUser() !== 'authenticated') throw new Error('fixture authentication failed');
    await Promise.all([notificationStore.fetchNotifications(), workStore.fetchCounts()]);
    notificationStore.initSSE();
    show('ready', 'connected');
  } catch (error) { show('error', error.message); }
};
document.querySelector('#logout').onclick = async () => {
  try { await authAPI.logout(); show('ready', 'logged-out'); }
  catch (error) { show('error', error.message); }
};
addEventListener('pagehide', () => notificationStore.closeSSE());
`;
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				const pathname = new URL(request.url ?? '/', 'http://fixture').pathname;
				if (pathname === '/login') {
					response.setHeader('Content-Type', 'text/html');
					response.end('<!doctype html><html><body><p>Signed out</p></body></html>');
					return;
				}
				if (pathname !== '/__notification-recovery-proxy') return next();
				response.setHeader('Content-Type', 'text/html');
				response.end(`<!doctype html><html><body>
<button id="start">Start notifications</button><button id="logout">Log out</button>
<output id="ready"></output><output id="identity"></output><output id="unread"></output>
<output id="inbox"></output><output id="work"></output><output id="windows"></output><output id="error"></output>
<script>globalThis.__sveltekit_dev={env:{PUBLIC_SCHOOL_SUBDOMAIN:"fixture"}};</script>
<script type="module" src="/@id/${entry}"></script></body></html>`);
			});
		}
	};
}

export async function startNotificationRecoveryProxy() {
	const directory = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-notification-proxy-'));
	const container = `schoolorbit-notification-proxy-${randomUUID()}`;
	const requests: RequestRecord[] = [];
	const streams = new Set<ServerResponse>();
	const pendingSnapshots: Array<() => void> = [];
	let app: ViteDevServer | undefined;
	let containerStarted = false;
	let authStatus = 200;
	let unread = 1;
	let workTotal = 2;
	let holdSnapshots = false;
	let connections = 0;
	const api = createServer((request, response) => {
		const url = new URL(request.url ?? '/', 'http://upstream');
		requests.push({
			path: url.pathname,
			method: request.method ?? 'GET',
			cookie: request.headers.cookie ?? '',
			origin: request.headers.origin ?? '',
			csrf: String(request.headers['x-csrf-token'] ?? '')
		});
		if (url.pathname === '/api/notifications/stream') {
			connections++;
			response.writeHead(200, {
				'Content-Type': 'text/event-stream',
				'Cache-Control': 'no-cache'
			});
			// A native EventSource reconnect is fast; application auth retries retain their real 3s delay.
			response.write('retry: 100\n: subscribed\n\n');
			streams.add(response);
			response.on('close', () => streams.delete(response));
			return;
		}
		response.setHeader('Content-Type', 'application/json');
		response.setHeader('X-CSRF-Token', 'synthetic-csrf-token');
		const send = (data: object, status = 200) => {
			response.writeHead(status);
			response.end(JSON.stringify(status === 200 ? { success: true, data } : data));
		};
		if (url.pathname === '/api/auth/me') {
			if (authStatus !== 200)
				return send({ success: false, error: 'fixture auth refusal' }, authStatus);
			return send({
				id: 'synthetic-user',
				username: 'fixture',
				firstName: 'Synthetic',
				lastName: 'User',
				displayName: 'Synthetic User',
				userType: 'staff',
				status: 'active',
				permissions: []
			});
		}
		if (url.pathname === '/api/auth/logout') {
			response.setHeader(
				'Set-Cookie',
				'__Host-schoolorbit_session=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0'
			);
			return send({});
		}
		if (url.pathname === '/api/notifications' || url.pathname === '/api/me/work-items/counts') {
			const data =
				url.pathname === '/api/notifications'
					? {
							items: [{ id: `snapshot-${unread}`, title: 'Synthetic snapshot' }],
							unread_count: unread
						}
					: { open: workTotal, total: workTotal, dueSoon: 0, overdue: 0, submitted: 0, closed: 0 };
			const complete = () => send(data);
			if (holdSnapshots) pendingSnapshots.push(complete);
			else complete();
			return;
		}
		if (url.pathname === '/deployment-status') {
			response.end(JSON.stringify({ status: 'ready', releaseId: 'synthetic' }));
			return;
		}
		send({ success: false, error: 'unexpected fixture route' }, 404);
	});

	async function close() {
		for (const stream of streams) stream.destroy();
		api.closeAllConnections();
		const results = await Promise.allSettled([
			containerStarted ? exec('docker', ['rm', '-f', '-v', container]) : Promise.resolve(),
			app?.close(),
			api.listening
				? new Promise<void>((resolve, reject) =>
						api.close((error) => (error ? reject(error) : resolve()))
					)
				: Promise.resolve()
		]);
		containerStarted = false;
		await rm(directory, { recursive: true, force: true });
		const failures = results.filter((result) => result.status === 'rejected');
		if (failures.length)
			throw new AggregateError(
				failures.map((result) => result.reason),
				'Notification fixture cleanup failed'
			);
	}

	try {
		await new Promise<void>((resolve, reject) => {
			api.once('error', reject);
			api.listen(8081, '127.0.0.1', resolve);
		});
		const rendered = await renderProxy('nginx-configs/school-api.conf.template');
		const reservation = createServer();
		await new Promise<void>((resolve) => reservation.listen(0, '127.0.0.1', resolve));
		const reservedAddress = reservation.address();
		if (!reservedAddress || typeof reservedAddress === 'string')
			throw new Error('Proxy port unavailable');
		const proxyPort = reservedAddress.port;
		await new Promise<void>((resolve) => reservation.close(() => resolve()));
		// Only the enclosing loopback listener is fixture-owned. These executable blocks are unchanged.
		const config = `events {}\nhttp {
${block(rendered, 'map $http_origin $allow_origin')}
${block(rendered, 'map $http_x_schoolorbit_diagnostic_id $school_diagnostic_id')}
${block(rendered, 'upstream school_api_http')}
server {
listen 127.0.0.1:${proxyPort};
${block(rendered, 'location = /api/notifications/stream')}
${block(rendered, 'location / {\n        proxy_pass http://school_api_http;')}
}}
`;
		await writeFile(path.join(directory, 'nginx.conf'), config);
		// Container networking maps the production upstream name and port to the synthetic HTTP server.
		await exec('docker', [
			'create',
			'--name',
			container,
			'--network',
			'host',
			'--add-host',
			'schoolorbit-backend-school:127.0.0.1',
			'-v',
			`${directory}:/fixture:ro`,
			nginxImage,
			'nginx',
			'-c',
			'/fixture/nginx.conf',
			'-g',
			'daemon off;'
		]);
		containerStarted = true;
		await exec('docker', ['start', container]);
		await exec('docker', ['exec', container, 'nginx', '-t', '-c', '/fixture/nginx.conf']);
		const apiUrl = `http://127.0.0.1:${proxyPort}`;
		process.env.PUBLIC_BACKEND_URL = apiUrl;
		process.env.PUBLIC_SCHOOL_SUBDOMAIN = 'fixture';
		process.env.PUBLIC_VAPID_KEY = 'test';
		app = await createViteServer({
			root,
			cacheDir: path.join(root, 'node_modules/.vite-notification-recovery-proxy'),
			logLevel: 'silent',
			plugins: [harnessPage()],
			server: {
				host: '127.0.0.1',
				port: 3000,
				strictPort: true
			}
		});
		await app.listen();
		const address = app.httpServer?.address();
		if (!address || typeof address === 'string') throw new Error('Fixture Vite server unavailable');
		return {
			baseUrl: `http://127.0.0.1:${address.port}`,
			apiUrl,
			requests,
			get connections() {
				return connections;
			},
			get openStreams() {
				return streams.size;
			},
			get pendingSnapshots() {
				return pendingSnapshots.length;
			},
			reset() {
				requests.length = 0;
				connections = 0;
				authStatus = 200;
				unread = 1;
				workTotal = 2;
				holdSnapshots = false;
				pendingSnapshots.splice(0);
			},
			setState(nextUnread: number, nextWork: number) {
				unread = nextUnread;
				workTotal = nextWork;
			},
			setAuthStatus(status: number) {
				authStatus = status;
			},
			holdSnapshots() {
				holdSnapshots = true;
			},
			releaseSnapshots() {
				holdSnapshots = false;
				for (const complete of pendingSnapshots.splice(0)) complete();
			},
			send(event: string, data: object | string = '') {
				if (streams.size !== 1) throw new Error('Expected exactly one subscribed browser stream');
				for (const stream of streams) {
					stream.write(
						`${event ? `event: ${event}\n` : ''}data: ${typeof data === 'string' ? data : JSON.stringify(data)}\n\n`
					);
				}
			},
			drop() {
				for (const stream of streams) stream.end();
			},
			close
		};
	} catch (error) {
		await close();
		throw error;
	}
}
