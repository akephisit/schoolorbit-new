import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
	PUBLIC_BACKEND_URL: { public: true, static: true },
	PUBLIC_VAPID_KEY: { public: true, static: true },
	PUBLIC_SCHOOL_SUBDOMAIN: { public: true, schema: (input) => input ?? '' }
});
