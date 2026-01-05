/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />

// This service worker uses a Network-Only strategy (no caching)
// Perfect for online-only PWA that always needs fresh data

// Version - increment this to force SW update
const VERSION = '2.0.0';

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
        caches.keys().then((cacheNames) => {
            return Promise.all(
                cacheNames.map((cacheName) => {
                    console.log('[ServiceWorker] Deleting cache:', cacheName);
                    return caches.delete(cacheName);
                })
            );
        }).then(() => {
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
                        // Fallback if offline.html can't be fetched
                        return new Response(
                            `<!DOCTYPE html>
<html lang="th">
<head>
	<meta charset="UTF-8">
	<meta name="viewport" content="width=device-width, initial-scale=1.0">
	<title>ไม่มีการเชื่อมต่อ</title>
	<style>
		body { font-family: system-ui; display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; text-align: center; padding: 20px; }
		.container { background: white; color: #333; padding: 40px; border-radius: 12px; max-width: 400px; }
		h1 { margin: 0 0 16px; }
		button { background: #667eea; color: white; border: none; padding: 12px 24px; border-radius: 8px; cursor: pointer; margin-top: 16px; }
	</style>
</head>
<body>
	<div class="container">
		<h1>🌐 ไม่มีการเชื่อมต่อ</h1>
		<p>กรุณาตรวจสอบอินเทอร์เน็ตของคุณ</p>
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
sw.addEventListener('push', (event) => {
    console.log('[ServiceWorker] Push notification received:', event);

    // Handle push notifications here if needed in the future
    // Example:
    // const data = event.data?.json();
    // event.waitUntil(
    //   sw.registration.showNotification(data.title, {
    //     body: data.body,
    //     icon: '/icon-192.png'
    //   })
    // );
});

console.log('[ServiceWorker] Script loaded - Network-Only mode');
