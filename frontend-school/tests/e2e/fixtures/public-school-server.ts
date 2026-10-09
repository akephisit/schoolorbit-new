import { createServer as createHttpServer } from 'node:http';
import { createServer } from 'vite';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { PublicSchoolStatistics } from '../../../src/lib/api/school.js';

const counts = (male: number, female: number, otherOrUnspecified = 0) => ({
	male,
	female,
	otherOrUnspecified,
	total: male + female + otherOrUnspecified
});
export const publicSchoolStatistics: PublicSchoolStatistics = {
	academicYear: { name: 'ปีการศึกษา 2569', year: 2569 },
	asOf: '2026-10-09T07:00:00Z',
	totalTeachers: 2,
	totalStaff: 8,
	totalHomerooms: 6,
	students: counts(10, 12, 1),
	unassignedStudents: counts(1, 0, 1),
	grades: [
		{
			levelType: 'secondary',
			year: 4,
			students: counts(1, 3),
			unassignedStudents: counts(0, 0),
			homerooms: [{ name: 'ม.4/1', students: counts(1, 3) }]
		},
		{
			levelType: 'primary',
			year: 1,
			students: counts(2, 2),
			unassignedStudents: counts(0, 0),
			homerooms: [{ name: 'ป.1/1', students: counts(2, 2) }]
		},
		{
			levelType: 'secondary',
			year: 1,
			students: counts(5, 6, 1),
			unassignedStudents: counts(1, 0, 1),
			homerooms: [
				{ name: 'ม.1/10', students: counts(1, 1) },
				{ name: 'ม.1/2', students: counts(1, 2) },
				{ name: 'ม.1/1', students: counts(2, 3) }
			]
		},
		{
			levelType: 'kindergarten',
			year: 1,
			students: counts(2, 1),
			unassignedStudents: counts(0, 0),
			homerooms: [{ name: 'อ.1/1', students: counts(2, 1) }]
		},
		{
			levelType: 'secondary',
			year: 2,
			students: counts(0, 0),
			unassignedStudents: counts(0, 0),
			homerooms: []
		}
	]
};

export async function startPublicSchoolServer() {
	const requests: URL[] = [];
	let statistics = publicSchoolStatistics;
	let fail = false;
	const api = createHttpServer((req, res) => {
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
		if (fail && url.pathname.endsWith('/statistics')) {
			res.writeHead(503).end(JSON.stringify({ success: false, error: 'Fixture unavailable' }));
			return;
		}
		const data =
			url.pathname === '/api/school/public/statistics'
				? statistics
				: url.pathname === '/api/school/public/organization'
					? { units: [] }
					: { schoolName: 'โรงเรียนทดสอบ', logoFileId: null };
		res.end(JSON.stringify({ success: true, data }));
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
		cacheDir: path.join(root, 'node_modules/.vite-public-school-test'),
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
		setStatistics(value: PublicSchoolStatistics) {
			statistics = value;
		},
		setFailure(value: boolean) {
			fail = value;
		},
		async close() {
			await vite.close();
			await new Promise<void>((done, reject) =>
				api.close((error) => (error ? reject(error) : done()))
			);
		}
	};
}
