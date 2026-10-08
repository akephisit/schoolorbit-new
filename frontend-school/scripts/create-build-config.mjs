import { writeFileSync } from 'node:fs';

// A generic Worker configuration selects the same adapter output as each tenant.
// Tenant names/routes/IDs are runtime bindings applied during upload, not build inputs.
for (const key of ['PUBLIC_BACKEND_URL', 'PUBLIC_VAPID_KEY']) {
	if (!process.env[key]) throw new Error(`Missing compiled public configuration: ${key}`);
}
writeFileSync(
	new URL('../wrangler.json', import.meta.url),
	JSON.stringify({
		name: 'schoolorbit-school-shared',
		main: 'build/index.js',
		compatibility_date: '2025-09-15',
		compatibility_flags: ['nodejs_compat'],
		assets: { directory: 'build/client', binding: 'ASSETS' },
		vars: {
			PUBLIC_BACKEND_URL: process.env.PUBLIC_BACKEND_URL,
			PUBLIC_VAPID_KEY: process.env.PUBLIC_VAPID_KEY,
			SUBDOMAIN: ''
		}
	})
);
