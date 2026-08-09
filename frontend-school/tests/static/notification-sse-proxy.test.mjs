import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { renderProxy } from './helpers/render-proxy.mjs';

function extractLocationBlocks(source) {
	const blocks = [];
	const locationPattern = /^\s*location\s+(?:(=|\^~|~\*?)\s+)?([^\s{]+)\s*\{/gm;

	for (const match of source.matchAll(locationPattern)) {
		let depth = 1;
		let cursor = match.index + match[0].length;
		while (cursor < source.length && depth > 0) {
			if (source[cursor] === '{') depth += 1;
			if (source[cursor] === '}') depth -= 1;
			cursor += 1;
		}
		assert.equal(depth, 0, `unterminated nginx location ${match[2]}`);
		blocks.push({
			modifier: match[1] ?? '',
			pattern: match[2],
			body: source.slice(match.index + match[0].length, cursor - 1)
		});
	}

	return blocks;
}

function resolveLocation(blocks, requestPath) {
	const exact = blocks.find((block) => block.modifier === '=' && block.pattern === requestPath);
	if (exact) return exact;

	const prefixes = blocks
		.filter(
			(block) =>
				block.modifier !== '~' && block.modifier !== '~*' && requestPath.startsWith(block.pattern)
		)
		.sort((left, right) => right.pattern.length - left.pattern.length);
	const longestPrefix = prefixes[0];
	if (longestPrefix?.modifier === '^~') return longestPrefix;

	const regex = blocks.find(
		(block) =>
			(block.modifier === '~' || block.modifier === '~*') &&
			new RegExp(block.pattern, block.modifier === '~*' ? 'i' : '').test(requestPath)
	);
	return regex ?? longestPrefix;
}

test('notification stream resolves to the unbuffered credentialed SSE proxy', async () => {
	const source = await renderProxy('nginx-configs/school-api.conf.template');
	const location = resolveLocation(extractLocationBlocks(source), '/api/notifications/stream');

	assert.ok(location, 'notification stream must resolve to an nginx location');
	assert.equal(location.modifier, '=');
	assert.equal(location.pattern, '/api/notifications/stream');
	assert.match(location.body, /proxy_pass\s+http:\/\/schoolorbit-backend-school:8081;/);
	assert.match(location.body, /proxy_buffering\s+off;/);
	assert.match(location.body, /proxy_cache\s+off;/);
	assert.match(location.body, /proxy_http_version\s+1\.1;/);
	assert.match(location.body, /proxy_set_header\s+Connection\s+"";/);
	assert.match(location.body, /chunked_transfer_encoding\s+on;/);
	assert.match(location.body, /proxy_read_timeout\s+24h;/);
	assert.match(location.body, /proxy_send_timeout\s+24h;/);
	assert.match(
		location.body,
		/add_header\s+'Access-Control-Allow-Origin'\s+\$allow_origin\s+always;/
	);
	assert.match(location.body, /add_header\s+'Access-Control-Allow-Credentials'\s+'true'\s+always;/);
});

test('notification stream recovers through authoritative auth before bounded manual reconnects', async () => {
	const store = await readFile(
		new URL('../../src/lib/stores/notification.ts', import.meta.url),
		'utf8'
	);
	const bell = await readFile(
		new URL('../../src/lib/components/layout/NotificationBell.svelte', import.meta.url),
		'utf8'
	);

	assert.match(store, /getSchoolSubdomainHint/);
	assert.match(store, /new URL\(['"]\/api\/notifications\/stream['"],\s*BACKEND_URL\)/);
	assert.match(store, /searchParams\.set\(['"]school_subdomain['"],\s*schoolSubdomain\)/);
	assert.equal(
		[...store.matchAll(/['"]school_subdomain['"]/g)].length,
		1,
		'the SSE URL must append the sanitized tenant hint at most once'
	);

	for (const eventName of ['session_invalid', 'session_unavailable']) {
		assert.match(store, new RegExp(`addEventListener\\(['"]${eventName}['"]`));
	}
	assert.match(store, /async function recoverAfterSessionSignal/);
	assert.match(store, /recoveryInFlight\?\.generation\s*===\s*generation/);
	assert.match(store, /ownsEventSource\(source,\s*generation\)/);
	assert.match(store, /function closeSSE\(\)[\s\S]*sseGeneration\s*\+=\s*1/);
	assert.match(
		store,
		/realtimeAuthRecovery\(\(\)\s*=>\s*authAPI\.refreshCurrentUser\(\{\s*silent:\s*true\s*\}\)\s*\)/
	);
	assert.match(store, /readyState\s*!==\s*EventSource\.CLOSED[\s\S]*recoverAfterSessionSignal/);
	assert.match(store, /recoveryAction\s*===\s*['"]reconnect['"][\s\S]*scheduleSseReconnect/);
	assert.match(store, /recoveryAction\s*===\s*['"]retry['"][\s\S]*scheduleAuthRecovery/);
	assert.match(store, /recoveryAction\s*===\s*['"]stop['"][\s\S]*clearReconnectTimer/);
	assert.doesNotMatch(store, /setTimeout\([\s\S]{0,180}this\.initSSE\(\)/);

	assert.match(bell, /const isAuthenticated = \$derived\(\$authStore\.isAuthenticated\)/);
	assert.match(bell, /if \(isAuthenticated\)\s*\{[\s\S]*notificationStore\.initSSE\(\)/);
});
