import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';
import {
	RoutePerformanceProbe,
	summarizeFiveWarmRuns,
	type RouteRegionSample
} from './helpers/route-performance';

// Real accounts: never retain a trace, screenshot, response body, URL parameters or credentials.
test.use({ trace: 'off', screenshot: 'off', video: 'off' });
// The installed Playwright otherwise captures a DOM error-context even with tracing off.
process.env.PLAYWRIGHT_NO_COPY_PROMPT = '1';
test.describe.configure({ mode: 'default' });
const enabled = process.env.E2E_ROUTE_LOADING_LIVE === '1';
const base = process.env.E2E_BASE_URL || process.env.SMOKE_TENANT_URL || '';
const api = (
	process.env.E2E_API_URL ||
	process.env.SMOKE_API_URL ||
	'https://school-api.schoolorbit.app'
).replace(/\/$/, '');
type Schemas = components['schemas'];
type Selection = { academicYearId?: string; academicTermId?: string };
type Representative = {
	route: string;
	apiPath: string;
	ready: string;
	context?: 'year' | 'term';
	skeleton?: string;
	path?: string;
};
const staffRoutes: Representative[] = [
	{ route: 'staff', apiPath: '/api/staff/dashboard', ready: 'staff-overview', context: 'year' },
	{
		route: 'staff/academic/catalog/subjects',
		apiPath: '/api/academic/catalog/subjects/overview',
		ready: 'catalog-subjects-ready'
	},
	{
		route: 'staff/academic/core',
		apiPath: '/api/academic/setup/workspace',
		ready: 'academic-setup-ready'
	},
	{
		route: 'staff/academic/delivery',
		apiPath: '/api/academic/delivery/homerooms',
		ready: 'delivery-homerooms-ready',
		context: 'term'
	},
	{
		route: 'staff/academic/admission',
		apiPath: '/api/admission/rounds',
		ready: 'admission-round-list-region',
		context: 'year'
	},
	{
		route: 'staff/academic/supervision/templates',
		apiPath: '/api/supervision/templates/summaries',
		ready: 'supervision-templates-ready'
	},
	{ route: 'staff/manage', apiPath: '/api/staff', ready: 'staff-directory' },
	{ route: 'staff/students', apiPath: '/api/students', ready: 'student-list', context: 'year' },
	{ route: 'staff/roles', apiPath: '/api/roles', ready: 'roles-list' },
	{
		route: 'staff/organization',
		apiPath: '/api/organization/units',
		ready: 'organization-catalog'
	},
	{
		route: 'staff/menu',
		apiPath: '/api/admin/menu/items',
		ready: 'menu-items',
		skeleton: 'กำลังโหลดเมนูบริการ'
	},
	{ route: 'staff/work', apiPath: '/api/me/work-items', ready: 'work-items' },
	{
		route: 'staff/calendar',
		apiPath: '/api/calendar/events',
		ready: 'calendar-events',
		context: 'year'
	},
	{
		route: 'staff/facility/buildings',
		apiPath: '/api/facilities/buildings',
		ready: 'facility-buildings'
	},
	{
		route: 'staff/certificates',
		apiPath: '/api/certificates/campaigns',
		ready: 'certificate-campaigns',
		skeleton: 'กำลังโหลดกิจกรรมเกียรติบัตร'
	},
	{ route: 'staff/school-settings', apiPath: '/api/school/settings', ready: 'settings-school' },
	{ route: 'staff/features', apiPath: '/api/admin/features', ready: 'settings-features' },
	{
		route: 'staff/school-fonts',
		apiPath: '/api/school-fonts',
		ready: 'school-font-library',
		skeleton: 'กำลังโหลดคลังฟอนต์'
	},
	{ route: 'account/security', apiPath: '/api/auth/sessions', ready: 'session-list' },
	{ route: 'settings/consent', apiPath: '/api/consent/my-status', ready: 'consent-region' }
];

function headers() {
	return {
		Origin: new URL(base).origin,
		'X-School-Subdomain': process.env.SMOKE_SUBDOMAIN || new URL(base).hostname.split('.')[0]
	};
}
async function read<T>(context: BrowserContext, path: string): Promise<T> {
	try {
		const response = await context.request.get(`${api}${path}`, { headers: headers() });
		if (!response.ok()) throw new Error('LIVE_READ_FAILED');
		const envelope = (await response.json()) as { success: boolean; data?: T };
		if (!envelope.success || envelope.data === undefined) throw new Error('LIVE_READ_FAILED');
		return envelope.data;
	} catch {
		throw new Error('LIVE_READ_FAILED');
	}
}
async function login(page: Page, username: string, password: string, role: string) {
	let status = 0;
	try {
		await page.goto('/login');
		await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill(username);
		await page.getByLabel('รหัสผ่าน', { exact: true }).fill(password);
		const response = page.waitForResponse(
			(result) => new URL(result.url()).pathname === '/api/auth/login',
			{ timeout: 20_000 }
		);
		await page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true }).click();
		status = (await response).status();
		if (status !== 200) throw new Error('Login rejected');
		await page.waitForURL(new RegExp(`/${role}(?:/[^?#]*)?(?:[?#].*)?$`), { timeout: 20_000 });
	} catch {
		throw new Error(`LIVE_LOGIN_FAILED_HTTP_${status}`);
	}
}
async function cleanup(context: BrowserContext) {
	try {
		const current = await context.request.get(`${api}/api/auth/me`, { headers: headers() });
		if (current.status() === 401) return;
		if (!current.ok()) throw new Error('Cleanup unavailable');
		const csrf = current.headers()['x-csrf-token'];
		if (!csrf) throw new Error('Cleanup unavailable');
		const result = await context.request.post(`${api}/api/auth/logout`, {
			headers: { ...headers(), 'X-CSRF-Token': csrf }
		});
		if (!result.ok()) throw new Error('Cleanup unavailable');
	} catch {
		throw new Error('LIVE_CURRENT_SESSION_CLEANUP_FAILED');
	}
}
function selection(options: Schemas['AcademicContextOptions']): Selection {
	const year =
		options.years.find((row) => row.id === options.activeAcademicYearId) ?? options.years[0];
	const terms = options.terms.filter((row) => row.academicYearId === year?.id);
	const term = terms.find((row) => row.id === options.activeAcademicTermId) ?? terms[0];
	return { academicYearId: year?.id, academicTermId: term?.id };
}
async function measure(page: Page, representative: Representative, selected: Selection) {
	if (
		(representative.context && !selected.academicYearId) ||
		(representative.context === 'term' && !selected.academicTermId)
	) {
		test.info().annotations.push({
			type: 'unrun',
			description: `${representative.route}: NO_ACADEMIC_CONTEXT`
		});
		console.log(JSON.stringify({ route: representative.route, unrun: 'NO_ACADEMIC_CONTEXT' }));
		return;
	}
	const destination = new URL(`/${representative.path ?? representative.route}`, base);
	if (representative.context)
		destination.searchParams.set('academicYearId', selected.academicYearId!);
	if (representative.context === 'term')
		destination.searchParams.set('academicTermId', selected.academicTermId!);
	const samples: RouteRegionSample[] = [];
	let phase = 'navigation';
	const probe = new RoutePerformanceProbe(page, representative.route, [
		{ region: 'primary', apiPath: representative.apiPath, readyTestId: representative.ready }
	]);
	try {
		// One discarded warm-up followed by exactly five runs in the same browser/session/context.
		for (let run = 0; run < 6; run++) {
			phase = 'navigation';
			await page.goto('/debug');
			await page.getByRole('button', { name: 'Force Refresh Auth', exact: true }).waitFor();
			await page.evaluate((target) => {
				const link = document.createElement('a');
				link.href = target;
				link.textContent = 'Live route acceptance';
				link.dataset.sveltekitPreloadData = 'off';
				document.querySelector('main')?.append(link);
			}, destination.pathname + destination.search);
			const responsePromise = page.waitForResponse(
				(response) =>
					new URL(response.url()).pathname === representative.apiPath &&
					response.request().method() === 'GET',
				{ timeout: 20_000 }
			);
			probe.beginNavigation();
			await page.getByRole('link', { name: 'Live route acceptance', exact: true }).click();
			phase = 'primary-response';
			const response = await responsePromise;
			if (!response.ok()) throw new Error(`LIVE_PRIMARY_HTTP_${response.status()}`);
			const envelope = (await response.json()) as { success?: boolean; data?: unknown };
			if (!envelope.success || envelope.data === undefined)
				throw new Error('LIVE_PRIMARY_INVALID_ENVELOPE');
			phase = 'useful-region';
			const region = page.getByTestId(representative.ready);
			await expect(region).toBeVisible();
			if ((await region.getAttribute('aria-busy')) !== null)
				await expect(region).toHaveAttribute('aria-busy', 'false');
			if (representative.skeleton)
				await expect(
					page.getByRole('status', { name: representative.skeleton, exact: true })
				).toHaveCount(0);
			await expect(region.getByRole('status')).toHaveCount(0);
			await expect(region.getByRole('button', { name: /ลอง.*อีกครั้ง/ })).toHaveCount(0);
			const sample = await probe.sample('primary');
			if (run) samples.push(sample);
		}
		console.log(
			JSON.stringify({
				profile: 'deployed-chromium-default-network-no-preload',
				warmRuns: 5,
				...summarizeFiveWarmRuns(samples)
			})
		);
	} catch {
		// Raw browser/API errors may contain real route IDs or response text.
		throw new Error(`LIVE_ROUTE_ACCEPTANCE_FAILED: ${representative.route}: ${phase}`);
	} finally {
		probe.dispose();
	}
}

for (const role of ['staff', 'student', 'parent'] as const) {
	test(`deployed ${role}: readonly route acceptance and five warm samples`, async ({
		page,
		context
	}) => {
		test.setTimeout(720_000);
		test.skip(!enabled, 'Set E2E_ROUTE_LOADING_LIVE=1 explicitly for credentialed deployed reads.');
		const username =
			process.env[`E2E_${role.toUpperCase()}_USERNAME`] ||
			(role === 'staff' ? process.env.SMOKE_USERNAME : undefined);
		const password =
			process.env[`E2E_${role.toUpperCase()}_PASSWORD`] ||
			(role === 'staff' ? process.env.SMOKE_PASSWORD : undefined);
		test.skip(
			!username || !password,
			`Missing dedicated E2E_${role.toUpperCase()}_USERNAME/PASSWORD.`
		);
		if (!base || new URL(base).protocol !== 'https:') throw new Error('LIVE_HTTPS_TENANT_REQUIRED');
		try {
			await login(page, username!, password!, role);
			const actor = await read<Schemas['CurrentUserResponse']>(context, '/api/auth/me');
			const identity = (value: Schemas['CurrentUserResponse']) =>
				JSON.stringify([value.id, value.userType, value.permissions.slice().sort()]);
			if (actor.userType !== role) throw new Error('LIVE_ROLE_MISMATCH');
			const options = await read<Schemas['AcademicContextOptions']>(
				context,
				role === 'staff'
					? '/api/academic/context/options'
					: role === 'student'
						? '/api/me/academic-context/options'
						: '/api/parent/academic-context/options'
			);
			const selected = selection(options);
			if (role === 'staff')
				for (const representative of staffRoutes) await measure(page, representative, selected);
			if (role === 'student')
				for (const representative of [
					{
						route: 'student/profile',
						apiPath: '/api/student/profile',
						ready: 'student-profile-region',
						context: 'year'
					},
					{
						route: 'student/timetable',
						apiPath: '/api/me/timetable',
						ready: 'student-timetable-region',
						context: 'term'
					},
					{
						route: 'student/exams',
						apiPath: '/api/me/exam-schedules',
						ready: 'student-exams-region',
						context: 'term'
					},
					{
						route: 'student/calendar',
						apiPath: '/api/me/calendar/events',
						ready: 'student-calendar-region',
						context: 'year'
					}
				] satisfies Representative[])
					await measure(page, representative, selected);
			if (role === 'parent') {
				await measure(
					page,
					{
						route: 'parent',
						apiPath: '/api/parent/profile',
						ready: 'parent-profile-region',
						context: 'year'
					},
					selected
				);
				if (selected.academicYearId) {
					const profile = await read<Schemas['ParentProfile']>(
						context,
						`/api/parent/profile?academicYearId=${encodeURIComponent(selected.academicYearId)}`
					);
					const child = profile.children[0];
					if (!child) throw new Error('LIVE_LINKED_CHILD_REQUIRED');
					const childSelected = selection(
						await read<Schemas['AcademicContextOptions']>(
							context,
							`/api/parent/students/${encodeURIComponent(child.id)}/academic-context/options`
						)
					);
					await measure(
						page,
						{
							route: 'parent/student/[id]/timetable',
							path: `parent/student/${child.id}/timetable`,
							apiPath: `/api/parent/students/${child.id}/timetable`,
							ready: 'parent-timetable-region',
							context: 'term'
						},
						childSelected
					);
				}
			}
			if (
				identity(await read<Schemas['CurrentUserResponse']>(context, '/api/auth/me')) !==
				identity(actor)
			)
				throw new Error('LIVE_IDENTITY_OR_PERMISSIONS_CHANGED');
		} finally {
			await cleanup(context);
		}
	});
}
