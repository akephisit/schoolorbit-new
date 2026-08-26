import assert from 'node:assert/strict';
import test from 'node:test';

import { createSchoolSSE } from '../src/lib/utils/sse.ts';

test('an SSE stream that closes without a terminal event reports a connection error', async () => {
	const originalFetch = globalThis.fetch;
	globalThis.fetch = async () => new Response('', { status: 200 });
	let reportedError: string | undefined;

	try {
		await createSchoolSSE(
			'https://admin-api.example.test',
			{},
			{
				onError: (error) => {
					reportedError = error;
				}
			}
		);
	} finally {
		globalThis.fetch = originalFetch;
	}

	assert.equal(reportedError, 'Connection lost');
});
