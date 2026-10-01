import type { Page, Route } from '@playwright/test';
import type { SessionDto } from '../../../src/lib/api/auth';
import type { ConsentRecord, UserConsentStatus } from '../../../src/lib/api/consent';
import { mockStaffHome, actor, id } from './staff-home-route-data';
export const currentSession = id(600),
	otherSession = id(601),
	consentId = id(602);
export type Region = 'sessions' | 'consent' | 'mutation';
export async function mockAccountConsent(
	page: Page,
	options: { hold?: Region; holdAt?: number; fail?: Region; failAt?: number } = {}
) {
	const base = await mockStaffHome(page, { permissions: [] });
	const counts = new Map<Region, number>();
	const reads = base.reads,
		writes = base.writes;
	let release = () => {},
		settled = () => {};
	const gate = new Promise<void>((r) => {
		release = r;
	});
	const completed = new Promise<void>((r) => {
		settled = r;
	});
	let loggedOut = false;
	let sessions: SessionDto[] = [currentSession, otherSession].map((id, i) => ({
		id,
		isCurrent: i === 0,
		deviceLabel: i ? 'อุปกรณ์อื่น' : 'อุปกรณ์ปัจจุบัน',
		rememberMe: false,
		createdAt: '2026-10-01T00:00:00Z',
		lastSeenAt: '2026-10-01T00:00:00Z',
		idleExpiresAt: '2026-10-02T00:00:00Z',
		absoluteExpiresAt: '2026-10-30T00:00:00Z'
	}));
	const record = (id: string, required: boolean): ConsentRecord => ({
		id,
		user_id: actor,
		user_type: 'staff',
		consent_type: required ? 'required' : 'optional',
		consent_type_name: required ? 'ความยินยอมจำเป็น' : 'ความยินยอมเสริม',
		purpose: 'วัตถุประสงค์ทดสอบ',
		data_categories: ['test'],
		consent_status: 'granted',
		granted_at: '2026-10-01T00:00:00Z',
		withdrawn_at: null,
		expires_at: null,
		is_expired: false,
		is_required: required,
		consent_method: 'electronic',
		is_minor_consent: false,
		parent_guardian_name: null,
		created_at: '2026-10-01T00:00:00Z'
	});
	let consents = [record(id(603), true), record(consentId, false)];
	const status = (): UserConsentStatus => ({
		user_id: actor,
		user_type: 'staff',
		total_required: 1,
		granted_required: 1,
		is_compliant: true,
		missing_required_consents: [],
		consents
	});
	const reply = (route: Route, data: unknown, code = 200) =>
		route.fulfill({
			status: code,
			json: code < 400 ? { success: true, data } : { success: false, error: data }
		});
	await page.route(
		(url) => url.pathname === '/api/auth/me',
		(route) => (loggedOut ? reply(route, 'เข้าสู่ระบบใหม่', 401) : route.fallback())
	);
	await page.route(
		(url) =>
			url.pathname.startsWith('/api/auth/sessions') ||
			url.pathname === '/api/auth/logout' ||
			url.pathname === '/api/auth/logout-all' ||
			url.pathname === '/api/auth/me/change-password' ||
			url.pathname.startsWith('/api/consent/'),
		async (route) => {
			const path = new URL(route.request().url()).pathname,
				method = route.request().method();
			const kind: Region =
				method !== 'GET' ? 'mutation' : path === '/api/auth/sessions' ? 'sessions' : 'consent';
			const ordinal = (counts.get(kind) ?? 0) + 1;
			counts.set(kind, ordinal);
			if (method === 'GET') reads.push(path);
			else writes.push(`${method} ${path}`);
			const snapshot =
				kind === 'sessions'
					? { sessions: structuredClone(sessions) }
					: kind === 'consent'
						? structuredClone(status())
						: {};
			const held = options.hold === kind && ordinal === (options.holdAt ?? 1);
			if (held) await gate;
			try {
				if (options.fail === kind && ordinal === (options.failAt ?? 1))
					return await reply(route, 'ส่วนนี้ไม่พร้อม', 503);
				if (method !== 'GET') {
					if (path.endsWith('/withdraw'))
						consents = consents.map((c) =>
							c.id === consentId
								? { ...c, consent_status: 'withdrawn', withdrawn_at: '2026-10-01T01:00:00Z' }
								: c
						);
					if (method === 'DELETE') sessions = sessions.filter((s) => !path.endsWith(s.id));
					if (path.endsWith('/change-password')) sessions = sessions.filter((s) => s.isCurrent);
					if (
						path.endsWith('/logout-all') ||
						path.endsWith('/logout') ||
						path.endsWith(currentSession)
					)
						loggedOut = true;
				}
				await reply(route, snapshot);
			} finally {
				if (held) settled();
			}
		}
	);
	return {
		...base,
		reads,
		writes,
		release,
		completed,
		count: (kind: Region) => counts.get(kind) ?? 0
	};
}
