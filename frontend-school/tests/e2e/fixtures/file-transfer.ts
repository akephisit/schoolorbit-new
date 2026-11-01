import { randomUUID } from 'node:crypto';
import { createServer, type ServerResponse } from 'node:http';
import type { Page } from '@playwright/test';
import type {
	TransferDevice,
	TransferPair,
	TransferSnapshot,
	TransferEvent
} from '../../../src/lib/api/file-transfer.js';
import { mockStaffHome, actor } from './staff-home-route-data';

const prefix = '/api/staff/file-transfer';
const uuidv7 = () => {
	const time = Date.now().toString(16).padStart(12, '0'),
		random = randomUUID();
	return `${time.slice(0, 8)}-${time.slice(8)}-7${random.slice(15, 18)}-${random.slice(19)}`;
};
type DeviceEntry = { device: TransferDevice; code: string; stream?: ServerResponse };
export async function fileTransferFixture() {
	const devices = new Map<string, DeviceEntry>();
	const pairs = new Map<string, TransferPair>();
	let holdAccept = false;
	let holdRequest = false;
	let releaseRequest: (() => void) | null = null;
	let failSenderClose = false;
	let releaseAccept: (() => void) | null = null;
	let blockSignals = false;
	const requests: { path: string; method: string; body: string }[] = [];
	const emit = (deviceId: string, event: TransferEvent) =>
		devices
			.get(deviceId)
			?.stream?.write(`event: file_transfer\ndata: ${JSON.stringify(event)}\n\n`);
	const presence = () => {
		for (const id of devices.keys()) emit(id, { type: 'presence_changed', data: {} });
	};
	const snapshot = (entry: DeviceEntry): TransferSnapshot => ({
		device: entry.device,
		pairingCode: entry.code,
		expiresAt: new Date(Date.now() + 90_000).toISOString(),
		peers: [...devices.values()]
			.filter((other) => other !== entry && other.stream)
			.map((other) => other.device),
		pairs: [...pairs.values()].filter(
			(pair) => pair.initiator.id === entry.device.id || pair.recipient.id === entry.device.id
		)
	});
	const pairEvent = (pair: TransferPair) => {
		for (const device of [pair.initiator, pair.recipient])
			emit(device.id, { type: 'pair_changed', data: pair });
	};
	const closePair = (pair: TransferPair, reason: 'cancelled' | 'disconnected' | 'declined') => {
		pairs.delete(pair.id);
		for (const device of [pair.initiator, pair.recipient])
			emit(device.id, { type: 'pair_closed', data: { pairId: pair.id, reason } });
	};
	const remove = (deviceId: string) => {
		devices.delete(deviceId);
		for (const pair of pairs.values())
			if (pair.initiator.id === deviceId || pair.recipient.id === deviceId)
				closePair(pair, 'disconnected');
		presence();
	};
	const server = createServer(async (request, response) => {
		response.setHeader(
			'Access-Control-Allow-Origin',
			request.headers.origin ?? 'http://127.0.0.1:4173'
		);
		response.setHeader('Access-Control-Allow-Credentials', 'true');
		response.setHeader(
			'Access-Control-Allow-Headers',
			'Content-Type, X-CSRF-Token, X-School-Subdomain'
		);
		response.setHeader('Access-Control-Allow-Methods', 'GET, POST, DELETE, OPTIONS');
		if (request.method === 'OPTIONS') {
			response.writeHead(204).end();
			return;
		}
		const path = new URL(request.url ?? '/', 'http://fixture').pathname;
		let body = '';
		for await (const chunk of request) body += chunk;
		requests.push({ path, method: request.method ?? 'GET', body });
		const send = (data: unknown, status = 200) => {
			response.writeHead(status, { 'Content-Type': 'application/json' });
			response.end(
				JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
			);
		};
		if (path === `${prefix}/devices` && request.method === 'POST') {
			const input: { label: string } = JSON.parse(body);
			const id = uuidv7();
			const entry: DeviceEntry = {
				device: { id, userId: actor, displayName: 'ครู ทดสอบ', label: input.label },
				code: (devices.size + 1).toString(16).toUpperCase().padStart(10, '0')
			};
			devices.set(id, entry);
			send(snapshot(entry));
			return;
		}
		const deviceMatch = path.match(/\/devices\/([^/]+)(\/events)?$/);
		if (deviceMatch) {
			const entry = devices.get(deviceMatch[1]);
			if (!entry) {
				send('ไม่พบอุปกรณ์', 404);
				return;
			}
			if (deviceMatch[2]) {
				response.writeHead(200, {
					'Content-Type': 'text/event-stream',
					'Cache-Control': 'no-cache'
				});
				entry.stream = response;
				emit(entry.device.id, { type: 'ready', data: snapshot(entry) });
				presence();
				response.on('close', () => {
					if (devices.get(entry.device.id)?.stream === response) remove(entry.device.id);
				});
				return;
			}
			if (request.method === 'DELETE') {
				entry.stream?.end();
				remove(entry.device.id);
				send({});
				return;
			}
			send(snapshot(entry));
			return;
		}
		if (path === `${prefix}/pairs`) {
			const input: {
				deviceId: string;
				target: { type: 'device'; deviceId: string } | { type: 'code'; code: string };
				file: TransferPair['file'];
			} = JSON.parse(body);
			const initiator = devices.get(input.deviceId);
			const target = input.target;
			const recipient =
				target.type === 'device'
					? devices.get(target.deviceId)
					: [...devices.values()].find((entry) => entry.code === target.code);
			if (!initiator || !recipient || initiator === recipient) {
				send('รหัสไม่ถูกต้องหรืออุปกรณ์ไม่ออนไลน์', 404);
				return;
			}
			const pair: TransferPair = {
				id: uuidv7(),
				initiator: initiator.device,
				recipient: recipient.device,
				file: input.file,
				status: 'pending',
				expiresAt: new Date(Date.now() + 120_000).toISOString()
			};
			pairs.set(pair.id, pair);
			pairEvent(pair);
			const responsePair = structuredClone(pair);
			if (holdRequest)
				await new Promise<void>((resolve) => {
					releaseRequest = resolve;
				});
			send(responsePair);
			return;
		}
		const match = path.match(/\/pairs\/([^/]+)\/(accept|close|signals)$/);
		if (match) {
			const pair = pairs.get(match[1]);
			if (!pair) {
				send('ไม่พบคำขอ', 404);
				return;
			}
			const input: { deviceId: string; type: 'offer' | 'answer'; sdp: string } = JSON.parse(body);
			if (match[2] === 'accept') {
				pair.status = 'accepted';
				pairEvent(pair);
				if (holdAccept)
					await new Promise<void>((resolve) => {
						releaseAccept = resolve;
					});
				send(pair);
				return;
			}
			if (match[2] === 'close') {
				if (failSenderClose && input.deviceId === pair.initiator.id) {
					failSenderClose = false;
					send('fixture unavailable', 503);
					return;
				}
				closePair(
					pair,
					pair.status === 'pending' && input.deviceId === pair.recipient.id
						? 'declined'
						: 'cancelled'
				);
				send({});
				return;
			}
			const recipient = input.deviceId === pair.initiator.id ? pair.recipient : pair.initiator;
			if (!blockSignals)
				emit(recipient.id, {
					type: 'signal',
					data: { pairId: pair.id, type: input.type, sdp: input.sdp }
				});
			send({});
			return;
		}
		send('not_found', 404);
	});
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	const address = server.address();
	if (!address || typeof address === 'string') throw new Error('No fixture address');
	const origin = `http://127.0.0.1:${address.port}`;
	return {
		requests,
		async setup(page: Page, userType = 'staff') {
			await mockStaffHome(page, { userType, permissions: [], menuGroups: [] });
			await page.route('**/api/notifications', (route) =>
				route.fulfill({ json: { success: true, data: { items: [], unread_count: 0 } } })
			);
			await page.route('https://fonts.googleapis.com/**', (route) =>
				route.fulfill({ contentType: 'text/css', body: '' })
			);
			await page.route(`**${prefix}/**`, (route) => {
				const url = new URL(route.request().url());
				return route.continue({ url: `${origin}${url.pathname}${url.search}` });
			});
		},
		emit,
		failSenderClose() {
			failSenderClose = true;
		},
		holdRequest() {
			holdRequest = true;
		},
		releaseRequest() {
			holdRequest = false;
			releaseRequest?.();
			releaseRequest = null;
		},
		holdAccept() {
			holdAccept = true;
		},
		releaseAccept() {
			holdAccept = false;
			releaseAccept?.();
			releaseAccept = null;
		},
		blockSignals() {
			blockSignals = true;
		},
		get devices() {
			return [...devices.values()].map((entry) => entry.device);
		},
		get pairs() {
			return [...pairs.values()];
		},
		async close() {
			releaseAccept?.();
			releaseRequest?.();
			for (const entry of devices.values()) entry.stream?.end();
			devices.clear();
			await new Promise<void>((resolve, reject) =>
				server.close((error) => (error ? reject(error) : resolve()))
			);
		}
	};
}
