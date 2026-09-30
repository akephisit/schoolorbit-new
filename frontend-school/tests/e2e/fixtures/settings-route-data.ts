import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id } from './staff-home-route-data';
import type { FeatureToggle } from '../../../src/lib/api/feature-toggles';
import type { SchoolFontSummary } from '../../../src/lib/api/school-fonts';
export const featurePath = '/api/admin/features',
	settingsPath = '/api/school/settings',
	fontsPath = '/api/school-fonts';
export const logoId = id(201),
	uploadedId = id(202),
	fontId = id(203);
export async function mockSettings(
	page: Page,
	options: {
		hold?: string;
		holdAt?: number;
		fail?: string;
		failAt?: number;
		permissions?: string[];
	} = {}
) {
	const base = await mockStaffHome(page, {
		permissions: options.permissions ?? [
			'features.read.all',
			'features.update.all',
			'settings.read.all',
			'settings.update.all',
			'font.manage.school'
		]
	});
	const counts = new Map<string, number>(),
		writes: string[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	let feature: FeatureToggle = {
		id: id(200),
		code: 'library',
		name: 'ระบบงานทดสอบ',
		name_en: null,
		module: 'library',
		is_enabled: false
	};
	let fonts: SchoolFontSummary[] = [
		{
			id: fontId,
			displayName: 'ฟอนต์ทดสอบ',
			fontFamily: 'E2E Font',
			fontStyle: 'normal',
			fontWeight: 400,
			referenceCount: 0,
			createdAt: '2026-10-01T00:00:00Z'
		}
	];
	let logo: string | null = logoId;
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			json: status < 400 ? { success: true, data } : { success: false, error: data }
		});
	await page.route(
		(url) =>
			url.pathname.startsWith(featurePath) ||
			url.pathname.startsWith(settingsPath) ||
			url.pathname.startsWith(fontsPath) ||
			url.pathname === '/api/files',
		async (route) => {
			const { pathname: path } = new URL(route.request().url()),
				method = route.request().method();
			const kind = method === 'GET' ? path : path === '/api/files' ? 'upload' : 'mutation';
			counts.set(kind, (counts.get(kind) ?? 0) + 1);
			if (method !== 'GET') writes.push(`${method} ${path}`);
			const ordinal = counts.get(kind);
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, 'ส่วนนี้ไม่พร้อม', 503);
			if (path.startsWith(featurePath)) {
				if (method !== 'GET') feature = { ...feature, is_enabled: !feature.is_enabled };
				return reply(route, method === 'GET' ? [feature] : feature);
			}
			if (path.startsWith(settingsPath)) {
				if (method === 'GET') return reply(route, { logoFileId: logo });
				logo = method === 'DELETE' ? null : route.request().postDataJSON().logoFileId;
				return reply(route, {});
			}
			if (path === '/api/files')
				return reply(route, {
					id: uploadedId,
					byteSize: 10,
					currentVersion: 1,
					detectedMimeType: 'image/png',
					displayFilename: 'logo.png',
					lifecycleStatus: 'ready',
					publicContentUrl: null,
					purpose: 'school_logo'
				});
			if (method === 'DELETE') {
				fonts = [];
				return reply(route, {});
			}
			return reply(route, { items: fonts });
		}
	);
	return { ...base, count: (path: string) => counts.get(path) ?? 0, writes, release };
}
