/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />

// This service worker uses a Network-Only strategy (no caching)
// Perfect for online-only PWA that always needs fresh data

// Version - increment this to force SW update

const sw = self as unknown as ServiceWorkerGlobalScope;

// Listen for install event
sw.addEventListener('install', (event) => {
	console.log('[ServiceWorker] Installing...');
	// Skip waiting to activate immediately
	event.waitUntil(sw.skipWaiting());
});

// Listen for activate event
sw.addEventListener('activate', (event) => {
	console.log('[ServiceWorker] Activating...');

	// Clean up any old caches (we don't use any, but clean up just in case)
	event.waitUntil(
		caches
			.keys()
			.then((cacheNames) => {
				return Promise.all(
					cacheNames.map((cacheName) => {
						console.log('[ServiceWorker] Deleting cache:', cacheName);
						return caches.delete(cacheName);
					})
				);
			})
			.then(() => {
				// Take control of all clients immediately
				return sw.clients.claim();
			})
	);
});

// Listen for fetch event - Network-Only strategy
sw.addEventListener('fetch', (event) => {
	// Network-Only: Always fetch from network, never cache
	event.respondWith(
		fetch(event.request)
			.then((response) => {
				// Clone response for logging if needed
				return response;
			})
			.catch(async (error) => {
				console.error('[ServiceWorker] Fetch failed:', error);

				// For navigation requests (HTML pages), show offline page
				if (event.request.mode === 'navigate') {
					try {
						const offlinePage = await fetch('/offline.html');
						return offlinePage;
					} catch {
						// Fallback if offline.html can't be fetched (matches new design)
						return new Response(
							`<!DOCTYPE html>
<html lang="th">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>ไม่มีการเชื่อมต่อ - SchoolOrbit</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Kanit:wght@300;400;500;600;700&display=swap" rel="stylesheet">
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: 'Kanit', sans-serif; background: hsl(0 0% 100%); color: hsl(240 10% 3.9%); min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 20px; }
.container { max-width: 600px; text-align: center; }
.logo { font-size: 48px; font-weight: 700; margin-bottom: 16px; }
.icon { width: 120px; height: 120px; margin: 32px auto; background: hsl(221.2 83.2% 53.3%); border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 48px; }
h1 { font-size: 32px; font-weight: 600; margin-bottom: 12px; }
p { font-size: 18px; color: hsl(240 3.8% 46.1%); margin-bottom: 32px; }
button { background: hsl(221.2 83.2% 53.3%); color: white; border: none; padding: 14px 32px; border-radius: 8px; font-size: 16px; font-weight: 500; font-family: 'Kanit', sans-serif; cursor: pointer; }
</style>
</head>
<body>
<div class="container">
<div class="logo">SchoolOrbit</div>
<div class="icon">📶</div>
<h1>ไม่มีการเชื่อมต่อ</h1>
<p>กรุณาตรวจสอบอินเทอร์เน็ต</p>
<button onclick="window.location.reload()">ลองอีกครั้ง</button>
</div>
</body>
</html>`,
							{
								status: 503,
								statusText: 'Service Unavailable',
								headers: { 'Content-Type': 'text/html; charset=utf-8' }
							}
						);
					}
				}

				// For other requests (API, assets), return error
				return new Response(
					JSON.stringify({
						error: 'Network unavailable',
						message: 'กรุณาตรวจสอบการเชื่อมต่ออินเทอร์เน็ต'
					}),
					{
						status: 503,
						statusText: 'Service Unavailable',
						headers: {
							'Content-Type': 'application/json'
						}
					}
				);
			})
	);
});

// Handle push notifications (optional - for future use)
// Handle push notifications
sw.addEventListener('push', (event) => {
	if (event.data) {
		try {
			const data = event.data.json();

			const options: NotificationOptions = {
				body: data.body,
				icon: '/icon-192.png',
				// @ts-ignore
				vibrate: [200, 100, 200, 100, 200],
				tag: 'push-notification-v1',
				renotify: true,
				requireInteraction: true,
				timestamp: Date.now(),
				data: {
					link: data.link || '/'
				}
			};

			event.waitUntil(
				sw.registration.showNotification(data.title, options)
			);
		} catch (e) {
			console.error('Error parsing push data', e);
		}
	}
});

sw.addEventListener('notificationclick', (event) => {
	event.notification.close();

	if (event.action === 'open' || !event.action) {
		const link = event.notification.data.link;
		event.waitUntil(
			sw.clients.matchAll({ type: 'window', includeUncontrolled: true }).then(windowClients => {
				for (let i = 0; i < windowClients.length; i++) {
					const client = windowClients[i];
					if (client.url === link && 'focus' in client) {
						return (client as WindowClient).focus();
					}
				}
				if (sw.clients.openWindow) {
					return sw.clients.openWindow(link);
				}
			})
		);
	}
});

console.log('[ServiceWorker] Script loaded - Network-Only mode');
