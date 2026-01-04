/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />

// This service worker uses a Network-Only strategy (no caching)
// Perfect for online-only PWA that always needs fresh data

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
            .catch((error) => {
                console.error('[ServiceWorker] Fetch failed:', error);

                // Return a custom offline response
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
