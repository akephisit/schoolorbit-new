import { createServer as createHttpServer } from 'node:http';
import { createServer } from 'vite';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { makeApprovedCalendarEvent } from './calendar-route-data';
import { year } from './staff-home-route-data';

export const publicYears = {
	years: [
		{
			id: year,
			year: 2569,
			name: 'ปีการศึกษา 2569',
			status: 'active',
			startDate: '2026-05-14',
			endDate: '2027-04-09'
		}
	],
	terms: [],
	activeAcademicYearId: year,
	activeAcademicTermId: null
};
export const publicCalendarEvent = makeApprovedCalendarEvent({
	title: 'สอบช่วงเช้า',
	startDate: '2026-10-01',
	endDate: '2026-10-01',
	allDay: false,
	startTime: '08:30',
	endTime: '09:30',
	isPublic: true,
	targets: [{ audienceType: 'all' }],
	tagIds: [],
	reminderOffsetsDays: [],
	notifyAudience: false
});
export async function startPublicCalendarServer() {
	const requests: URL[] = [];
	let fail = false;
	let gate: Promise<void> | undefined;
	let release = () => {};
	const api = createHttpServer(async (req, res) => {
		const url = new URL(req.url ?? '/', 'http://fixture');
		requests.push(url);
		res.setHeader('access-control-allow-origin', req.headers.origin ?? '*');
		res.setHeader('access-control-allow-headers', 'X-School-Subdomain,Content-Type');
		res.setHeader('content-type', 'application/json');
		if (req.method === 'OPTIONS') {
			res.writeHead(204).end();
			return;
		}
		if (url.pathname === '/deployment-status') {
			res.end(JSON.stringify({ status: 'ready', releaseId: 'fixture' }));
			return;
		}
		if (url.pathname === '/api/public/academic-context/options') {
			res.end(JSON.stringify({ success: true, data: publicYears }));
			return;
		}
		if (url.pathname === '/api/public/calendar/events') {
			if (gate) await gate;
			if (fail) {
				res.writeHead(503).end(JSON.stringify({ success: false, error: 'ปฏิทินทดสอบยังไม่พร้อม' }));
				return;
			}
			const search = url.searchParams.get('search') === 'true';
			const event = search
				? {
						...publicCalendarEvent,
						title: 'ทัศนศึกษาพฤศจิกายน',
						startDate: '2026-11-05',
						endDate: '2026-11-05'
					}
				: publicCalendarEvent;
			res.end(
				JSON.stringify({
					success: true,
					data: search || !url.searchParams.get('from')?.startsWith('2026-11') ? [event] : []
				})
			);
			return;
		}
		res.end(
			JSON.stringify({ success: true, data: { schoolName: 'โรงเรียนทดสอบ', logoFileId: null } })
		);
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
		cacheDir: path.join(root, 'node_modules/.vite-public-calendar-test'),
		logLevel: 'silent',
		server: { host: '127.0.0.1', port: 0 }
	});
	await vite.listen();
	const viteAddress = vite.httpServer?.address();
	if (!viteAddress || typeof viteAddress === 'string')
		throw new Error('Fixture frontend unavailable');
	return {
		baseUrl: `http://127.0.0.1:${viteAddress.port}`,
		requests,
		setFailure(value: boolean) {
			fail = value;
		},
		hold() {
			gate = new Promise<void>((done) => (release = done));
		},
		release() {
			release();
			gate = undefined;
		},
		async close() {
			release();
			await vite.close();
			await new Promise<void>((done, reject) =>
				api.close((error) => (error ? reject(error) : done()))
			);
		}
	};
}
