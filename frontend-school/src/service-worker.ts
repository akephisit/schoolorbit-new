/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />

import {
	bindingMatches,
	claimPresentation,
	readBrowserState
} from './lib/notifications/browser-state.js';

// Push-only worker. Requests use the browser network directly; no application cache.
const sw = self as unknown as ServiceWorkerGlobalScope;
sw.addEventListener('install', (event) => event.waitUntil(sw.skipWaiting()));
sw.addEventListener('activate', (event) => event.waitUntil(sw.clients.claim()));

interface PushPayload {
	id: string;
	device_id: string;
	device_version: string;
	binding_token: string;
	expires_at: string;
	link: string;
}
function validPayload(value: unknown): value is PushPayload {
	if (!value || typeof value !== 'object') return false;
	return ['id', 'device_id', 'device_version', 'binding_token', 'expires_at', 'link'].every(
		(key) => typeof Reflect.get(value, key) === 'string'
	);
}
function appLink(link: string) {
	const url = new URL(link, sw.location.origin);
	return url.origin === sw.location.origin && link.startsWith('/') && !link.startsWith('//')
		? url.href
		: new URL('/', sw.location.origin).href;
}
function handoff(client: WindowClient, payload: PushPayload): Promise<boolean> {
	return new Promise((resolve) => {
		const channel = new MessageChannel();
		const timer = setTimeout(() => {
			channel.port1.close();
			resolve(false);
		}, 500);
		channel.port1.onmessage = (event) => {
			clearTimeout(timer);
			channel.port1.close();
			resolve(event.data?.handled === true);
		};
		client.postMessage({ type: 'schoolorbit:push', payload }, [channel.port2]);
	});
}
async function showPush(payload: PushPayload) {
	const state = await readBrowserState();
	if (!bindingMatches(state.binding, payload)) return;
	const clients = await sw.clients.matchAll({ type: 'window', includeUncontrolled: false });
	for (const client of clients) {
		if (client.visibilityState === 'visible' && client.focused && (await handoff(client, payload)))
			return;
	}
	if (
		!(await claimPresentation(
			`${state.binding!.identity}:${payload.id}`,
			Date.parse(payload.expires_at)
		))
	)
		return;
	// Re-read after asynchronous coordination so logout/account changes invalidate queued pushes.
	if (!bindingMatches((await readBrowserState()).binding, payload)) return;
	await sw.registration.showNotification('SchoolOrbit', {
		body: 'มีการแจ้งเตือนใหม่ กรุณาเปิดดูในกล่องแจ้งเตือน',
		icon: '/app-icon/192.png',
		badge: '/notification-badge.png',
		tag: `schoolorbit:${payload.id}`,
		renotify: false,
		silent: false,
		actions: [{ action: 'open', title: 'เปิดดู' }],
		data: {
			link: appLink(payload.link),
			device_id: payload.device_id,
			device_version: payload.device_version
		}
	});
}
sw.addEventListener('push', (event) => {
	if (!event.data) return;
	try {
		const payload: unknown = event.data.json();
		if (validPayload(payload))
			event.waitUntil(
				showPush(payload).catch((error) =>
					console.error('Failed to present push notification', error)
				)
			);
	} catch (error) {
		console.error('Invalid push payload', error);
	}
});
sw.addEventListener('notificationclick', (event) => {
	event.notification.close();
	event.waitUntil(
		(async () => {
			const binding = (await readBrowserState()).binding;
			const data = event.notification.data;
			if (!binding || binding.id !== data?.device_id || binding.version !== data?.device_version)
				return;
			const link = appLink(typeof data.link === 'string' ? data.link : '/');
			for (const client of await sw.clients.matchAll({
				type: 'window',
				includeUncontrolled: true
			})) {
				if (client.url === link) {
					await client.focus();
					return;
				}
			}
			await sw.clients.openWindow(link);
		})()
	);
});
