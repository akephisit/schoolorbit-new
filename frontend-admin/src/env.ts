import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
	PUBLIC_API_URL: { public: true, static: true },
	BACKEND_SCHOOL_URL: { schema: (input) => input ?? '' },
	INTERNAL_API_SECRET: { schema: (input) => input ?? '' }
});
