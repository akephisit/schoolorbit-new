import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id } from './staff-home-route-data';
import type { Building, Room } from '../../../src/lib/api/facility';
export const buildingsPath = '/api/facilities/buildings',
	roomsPath = '/api/facilities/rooms';
export const buildingId = id(210),
	secondBuilding = id(211),
	roomId = id(212);
export async function mockFacility(
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
			'facility.read.all',
			'facility.create.all',
			'facility.update.all',
			'facility.delete.all'
		]
	});
	const timestamp = '2026-10-01T00:00:00Z';
	let buildings: Building[] = [buildingId, secondBuilding].map((value, index) => ({
		id: value,
		name_th: index ? 'อาคารสอง' : 'อาคารแรก',
		name_en: null,
		code: String(index + 1),
		description: null,
		created_at: timestamp,
		updated_at: timestamp
	}));
	let rooms: Room[] = buildings.map((building, index) => ({
		id: id(212 + index),
		building_id: building.id,
		name_th: index ? 'ห้องสอง' : 'ห้องแรก',
		name_en: null,
		code: String(101 + index),
		room_type: 'GENERAL',
		capacity: 40,
		floor: 1,
		status: 'active',
		description: null,
		created_at: timestamp,
		updated_at: timestamp,
		building_name: building.name_th
	}));
	const counts = new Map<string, number>(),
		reads: URL[] = [],
		writes: { path: string; method: string }[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			json: status < 400 ? { success: true, data } : { success: false, error: data }
		});
	await page.route(
		(url) => url.pathname.startsWith('/api/facilities/'),
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method();
			const kind = method !== 'GET' ? 'mutation' : path;
			counts.set(kind, (counts.get(kind) ?? 0) + 1);
			if (method === 'GET') reads.push(url);
			else writes.push({ path, method });
			const roomSnapshot = rooms
				.filter(
					(room) =>
						(!url.searchParams.get('building_id') ||
							room.building_id === url.searchParams.get('building_id')) &&
						(!url.searchParams.get('search') ||
							[room.name_th, room.code].join(' ').includes(url.searchParams.get('search')!))
				)
				.map((room) => ({ ...room }));
			if (options.hold === kind && counts.get(kind) === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && counts.get(kind) === (options.failAt ?? 1))
				return reply(route, 'ส่วนนี้ไม่พร้อม', 503);
			if (method === 'GET') return reply(route, path === buildingsPath ? buildings : roomSnapshot);
			const resourceId = path.split('/').at(-1);
			if (path.startsWith(buildingsPath)) {
				if (method === 'DELETE') {
					buildings = buildings.filter((building) => building.id !== resourceId);
					rooms = rooms.map((room) =>
						room.building_id === resourceId
							? { ...room, building_id: null, building_name: null }
							: room
					);
					return reply(route, {});
				}
				const payload = route.request().postDataJSON();
				const saved: Building = {
					...buildings.find((building) => building.id === resourceId),
					id: method === 'POST' ? id(214) : resourceId!,
					name_th: payload.name_th,
					name_en: payload.name_en || null,
					code: payload.code || null,
					description: payload.description || null,
					created_at: timestamp,
					updated_at: timestamp
				};
				buildings = [...buildings.filter((building) => building.id !== saved.id), saved];
				rooms = rooms.map((room) =>
					room.building_id === saved.id ? { ...room, building_name: saved.name_th } : room
				);
				return reply(route, saved);
			}
			if (method === 'DELETE') {
				rooms = rooms.filter((room) => room.id !== resourceId);
				return reply(route, {});
			}
			const payload = route.request().postDataJSON();
			const saved: Room = {
				...rooms.find((room) => room.id === resourceId),
				...payload,
				id: method === 'POST' ? id(215) : resourceId!,
				building_id: payload.building_id ?? null,
				building_name:
					buildings.find((building) => building.id === payload.building_id)?.name_th ?? null,
				status: 'active',
				name_en: payload.name_en || null,
				code: payload.code || null,
				floor: payload.floor ?? null,
				description: payload.description || null,
				created_at: timestamp,
				updated_at: timestamp
			};
			rooms = [...rooms.filter((room) => room.id !== saved.id), saved];
			return reply(route, saved);
		}
	);
	return { ...base, reads, writes, release, count: (path: string) => counts.get(path) ?? 0 };
}
